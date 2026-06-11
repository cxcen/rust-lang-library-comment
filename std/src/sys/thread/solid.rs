//! 基于 μITRON 任务实现的线程。假定 `acre_tsk` 和 `exd_tsk` 可用。

use crate::cell::UnsafeCell;
use crate::mem::ManuallyDrop;
use crate::ptr::NonNull;
use crate::sync::atomic::{Atomic, AtomicUsize, Ordering};
use crate::sys::pal::itron::error::{ItronError, expect_success, expect_success_aborting};
use crate::sys::pal::itron::time::dur2reltims;
use crate::sys::pal::itron::{abi, task};
use crate::thread::ThreadInit;
use crate::time::Duration;
use crate::{hint, io};

pub struct Thread {
    p_inner: NonNull<ThreadInner>,

    /// 底层任务的 ID。
    task: abi::ID,
}

// Safety：`Thread` 中没有任何东西把它与最初的创建者绑定。它可以被任意线程 drop。
unsafe impl Send for Thread {}
// Safety：`Thread` 不提供任何接收 `&self` 的方法。
unsafe impl Sync for Thread {}

/// 在父线程与子线程之间共享的状态数据。当状态转移到某个终态时它会被 drop。
struct ThreadInner {
    /// 此字段在线程创建时使用，用于把初始化数据从 `Thread::new`
    /// 传递给被创建的任务。
    init: UnsafeCell<ManuallyDrop<Box<ThreadInit>>>,

    /// 一个状态机。每一次状态转移在源代码中都用 `[...]` 标注。
    ///
    /// ```text
    ///
    ///    <P>: 父线程, <C>: 子线程, (?): 不关心
    ///
    ///       DETACHED (-1)  -------------------->  EXITED (?)
    ///                        <C>finish/exd_tsk
    ///          ^
    ///          |
    ///          | <P>detach
    ///          |
    ///
    ///       INIT (0)  ----------------------->  FINISHED (-1)
    ///                        <C>finish
    ///          |                                    |
    ///          | <P>join/slp_tsk                    | <P>join/del_tsk
    ///          |                                    | <P>detach/del_tsk
    ///          v                                    v
    ///
    ///       JOINING                              JOINED (?)
    ///     (parent_tid)
    ///                                            ^
    ///             \                             /
    ///              \  <C>finish/wup_tsk        / <P>slp_tsk-complete/ter_tsk
    ///               \                         /                      & del_tsk
    ///                \                       /
    ///                 '--> JOIN_FINALIZE ---'
    ///                          (-1)
    ///
    lifecycle: Atomic<usize>,
}

// Safety：唯一的 `!Sync` 字段 `ThreadInner::init` 只会被
//         由 `ThreadInner` 表示的那个任务触碰。
unsafe impl Sync for ThreadInner {}

const LIFECYCLE_INIT: usize = 0;
const LIFECYCLE_FINISHED: usize = usize::MAX;
const LIFECYCLE_DETACHED: usize = usize::MAX;
const LIFECYCLE_JOIN_FINALIZE: usize = usize::MAX;
const LIFECYCLE_DETACHED_OR_JOINED: usize = usize::MAX;
const LIFECYCLE_EXITED_OR_FINISHED_OR_JOIN_FINALIZE: usize = usize::MAX;
// `JOINING` 没有单一的取值

// 32 位 ISA 用 64KiB，64 位 ISA 用 128KiB。
pub const DEFAULT_MIN_STACK_SIZE: usize = 0x4000 * size_of::<usize>();

impl Thread {
    /// # 安全性(Safety）
    ///
    /// 安全性要求参见 `thread::Builder::spawn_unchecked`。
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        let inner = Box::new(ThreadInner {
            init: UnsafeCell::new(ManuallyDrop::new(init)),
            lifecycle: AtomicUsize::new(LIFECYCLE_INIT),
        });

        unsafe extern "C" fn trampoline(exinf: isize) {
            let p_inner: *mut ThreadInner = crate::ptr::with_exposed_provenance_mut(exinf as usize);
            // Safety：此刻 `ThreadInner` 仍存活
            let inner = unsafe { &*p_inner };

            // Safety：由于 `trampoline` 对每个 `ThreadInner` 只会被调用一次，
            //         且只有 `trampoline` 会触碰 `init`，因此 `init` 中含有内容，
            //         可以安全地进行可变借用。
            let init = unsafe { ManuallyDrop::take(&mut *inner.init.get()) };
            let rust_start = init.init();
            rust_start();

            // 以防万一，修正当前线程的状态，使得析构函数不会 abort
            // Safety：其实并不是真的 unsafe
            let _ = unsafe { abi::unl_cpu() };
            let _ = unsafe { abi::ena_dsp() };

            // 现在就运行 TLS 析构函数，因为对于已终止的任务它们不会被自动调用。
            unsafe { crate::sys::thread_local::destructors::run() };

            let old_lifecycle = inner
                .lifecycle
                .swap(LIFECYCLE_EXITED_OR_FINISHED_OR_JOIN_FINALIZE, Ordering::AcqRel);

            match old_lifecycle {
                LIFECYCLE_DETACHED => {
                    // [DETACHED → EXITED]
                    // 永远不会有人来 join，所以我们让回收（collector）任务删除该任务。

                    // 在这种情况下，`*p_inner` 的所有权已经转移给我们，
                    // 我们负责将其 drop。acquire 内存序确保写入
                    // `LIFECYCLE_DETACHED` 的那次 swap 操作 happens-before
                    // `Box::from_raw(p_inner)`。
                    // Safety：见上文。
                    let _ = unsafe { Box::from_raw(p_inner) };

                    // Safety：没有指向栈的 pinned 引用
                    unsafe { terminate_and_delete_current_task() };
                }
                LIFECYCLE_INIT => {
                    // [INIT → FINISHED]
                    // 父线程尚未决定要 join 还是 detach 此线程。无论父线程选择
                    // 哪个选项，它都必须删除此任务。
                    // 由于父线程一旦看到 `FINISHED` 就可能立即 drop `*inner`，
                    // 因此上面的 `swap` 调用必须使用 release 内存序。
                }
                parent_tid => {
                    // 由于父线程一旦看到 `JOIN_FINALIZE` 就可能立即 drop `*inner`
                    // 并终止我们，因此上面的 `swap` 调用必须使用 release 内存序。
                    //
                    // 为了让 `parent_tid` 所指向的任务可见，我们必须在上面的
                    // `swap` 调用中使用 acquire 内存序。

                    // [JOINING → JOIN_FINALIZE]
                    // 唤醒父任务。
                    expect_success(
                        unsafe {
                            let mut er = abi::wup_tsk(parent_tid as _);
                            if er == abi::E_QOVR {
                                // `E_QOVR` 表示已经存在一个 parking token
                                er = abi::E_OK;
                            }
                            er
                        },
                        &"wup_tsk",
                    );
                }
            }
        }

        // Safety：`Box::into_raw` 返回一个非空指针
        let p_inner = unsafe { NonNull::new_unchecked(Box::into_raw(inner)) };

        let new_task = ItronError::err_if_negative(unsafe {
            abi::acre_tsk(&abi::T_CTSK {
                // 立即激活此任务
                tskatr: abi::TA_ACT,
                exinf: p_inner.as_ptr().expose_provenance() as abi::EXINF,
                // 入口点
                task: Some(trampoline),
                // 继承调用任务的基础优先级
                itskpri: abi::TPRI_SELF,
                stksz: stack,
                // 让内核分配栈，
                stk: crate::ptr::null_mut(),
            })
        })
        .map_err(|e| e.as_io_error())?;

        Ok(Self { p_inner, task: new_task })
    }

    pub fn join(self) {
        // Safety：此刻 `ThreadInner` 仍存活
        let inner = unsafe { self.p_inner.as_ref() };
        // 获取当前任务 ID。在这里 panic 会导致资源泄漏，所以失败时直接 abort。
        let current_task = task::current_task_id_aborting();
        debug_assert!(usize::try_from(current_task).is_ok());
        debug_assert_ne!(current_task as usize, LIFECYCLE_INIT);
        debug_assert_ne!(current_task as usize, LIFECYCLE_DETACHED);

        let current_task = current_task as usize;

        match inner.lifecycle.swap(current_task, Ordering::AcqRel) {
            LIFECYCLE_INIT => {
                // [INIT → JOINING]
                // 子任务会把状态转移到 `JOIN_FINALIZE` 并唤醒我们。
                //
                // 为了让 `current_task` 所指向的任务从子任务的视角可见，
                // 我们必须在上面的 `swap` 调用中使用 release 内存序。
                loop {
                    expect_success_aborting(unsafe { abi::slp_tsk() }, &"slp_tsk");
                    // 为了与子任务在赋值 `JOIN_FINALIZE` 之前对 `inner` 的内存访问
                    // 同步，`load` 必须使用 `Ordering::Acquire`。
                    if inner.lifecycle.load(Ordering::Acquire) == LIFECYCLE_JOIN_FINALIZE {
                        break;
                    }
                }

                // [JOIN_FINALIZE → JOINED]
            }
            LIFECYCLE_FINISHED => {
                // [FINISHED → JOINED]
                // 为了与子任务在赋值 `FINISHED` 之前对 `inner` 的内存访问同步，
                // 上面的 `swap` 调用必须使用 `Ordering::Acquire`。
            }
            _ => unsafe { hint::unreachable_unchecked() },
        }

        // 终止并删除该任务
        // Safety：`self.task` 仍然表示一个我们拥有的任务（因为对每个 `Thread`，
        //         此方法或 `detach_inner` 只会被调用一次）。该任务通过进入
        //         `FINISHED` 或 `JOIN_FINALIZE` 状态表明它已可被安全删除。
        unsafe { terminate_and_delete_task(self.task) };

        // 无论哪种情况，我们都负责 drop `inner`。
        // Safety：此后不会再访问 `*p_inner` 的内容
        let _inner = unsafe { Box::from_raw(self.p_inner.as_ptr()) };

        // 跳过析构函数（因为它会尝试 detach 该线程）
        crate::mem::forget(self);
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        // Safety：此刻 `ThreadInner` 仍存活
        let inner = unsafe { self.p_inner.as_ref() };

        // detach 该线程。
        match inner.lifecycle.swap(LIFECYCLE_DETACHED_OR_JOINED, Ordering::AcqRel) {
            LIFECYCLE_INIT => {
                // [INIT → DETACHED]
                // 时机到来时，子线程会发现永远不会有人来 join 它。
                // `*p_inner` 的所有权转移给子线程。
                // release 内存序确保上面对 `lifecycle` 的 swap 操作
                // happens-before 子线程的 `Box::from_raw(p_inner)`。
            }
            LIFECYCLE_FINISHED => {
                // [FINISHED → JOINED]
                // 该任务已经决定应由我们删除此任务。
                // 为了与子任务在赋值 `FINISHED` 之前对 `inner` 的内存访问同步，
                // 上面的 `swap` 调用需要 acquire 内存序。

                // 终止并删除该任务
                // Safety：`self.task` 仍然表示一个我们拥有的任务（因为对每个
                //         `Thread`，此方法或 `join_inner` 只会被调用一次）。
                //         该任务通过进入 `FINISHED` 状态表明它已可被安全删除。
                unsafe { terminate_and_delete_task(self.task) };

                // 我们负责 drop `*p_inner`。
                // Safety：此后不会再访问 `*p_inner` 的内容
                let _ = unsafe { Box::from_raw(self.p_inner.as_ptr()) };
            }
            _ => unsafe { hint::unreachable_unchecked() },
        }
    }
}

/// 终止并删除指定的任务。
///
/// 如果 `deleted_task` 指向的是调用方任务自身，此函数将 abort。
///
/// 假定指定的任务完全由调用方管理——即，当此函数仍在执行期间，其他线程
/// 绝不能“复活”该指定任务，也不能提前删除它。指定任务可以自行退出。
///
/// # 安全性(Safety）
///
/// 该任务必须可被安全地终止。这一般并不成立，因为可能存在指向该任务栈的
/// pinned 引用。
unsafe fn terminate_and_delete_task(deleted_task: abi::ID) {
    // 终止该任务
    // Safety：由调用方保证
    match unsafe { abi::ter_tsk(deleted_task) } {
        // 表示该任务已经处于休眠（dormant）状态，忽略它
        abi::E_OBJ => {}
        er => {
            expect_success_aborting(er, &"ter_tsk");
        }
    }

    // 删除该任务
    // Safety：由调用方保证
    expect_success_aborting(unsafe { abi::del_tsk(deleted_task) }, &"del_tsk");
}

/// 终止并删除调用方任务自身。
///
/// 不要求原子性——即，可以假定当此函数仍在执行期间，其他线程不会对调用方任务
/// 执行 `ter_tsk`。（这一性质使得在不支持 `exd_tsk` 的 μITRON 衍生内核上
/// 也易于实现此操作。）
///
/// # 安全性(Safety）
///
/// 该任务必须可被安全地终止。这一般并不成立，因为可能存在指向该任务栈的
/// pinned 引用。
unsafe fn terminate_and_delete_current_task() -> ! {
    expect_success_aborting(unsafe { abi::exd_tsk() }, &"exd_tsk");
    // Safety：`exd_tsk` 成功时永不返回
    unsafe { crate::hint::unreachable_unchecked() };
}

pub fn yield_now() {
    expect_success(unsafe { abi::rot_rdq(abi::TPRI_SELF) }, &"rot_rdq");
}

pub fn sleep(dur: Duration) {
    for timeout in dur2reltims(dur) {
        expect_success(unsafe { abi::dly_tsk(timeout) }, &"dly_tsk");
    }
}
