use super::id::ThreadId;
use super::main_thread;
use super::thread::Thread;
use crate::mem::ManuallyDrop;
use crate::ptr;
use crate::sys::thread as imp;
use crate::sys::thread_local::local_pointer;

const NONE: *mut () = ptr::null_mut();
const BUSY: *mut () = ptr::without_provenance_mut(1);
const DESTROYED: *mut () = ptr::without_provenance_mut(2);

local_pointer! {
    static CURRENT;
}

/// 用于持久化保存线程 ID 的存储。
///
/// 我们保存线程 ID，使得它在一个线程的整个生命周期内永远不会被销毁，做法是
/// 使用 `#[thread_local]` 或者多个 `local_pointer!`。
pub(super) mod id {
    use super::*;

    cfg_select! {
        target_thread_local => {
            use crate::cell::Cell;

            #[thread_local]
            static ID: Cell<Option<ThreadId>> = Cell::new(None);

            pub(super) const CHEAP: bool = true;

            pub(crate) fn get() -> Option<ThreadId> {
                ID.get()
            }

            pub(super) fn set(id: ThreadId) {
                ID.set(Some(id))
            }
        }
        target_pointer_width = "16" => {
            local_pointer! {
                static ID0;
                static ID16;
                static ID32;
                static ID48;
            }

            pub(super) const CHEAP: bool = false;

            pub(crate) fn get() -> Option<ThreadId> {
                let id0 = ID0.get().addr() as u64;
                let id16 = ID16.get().addr() as u64;
                let id32 = ID32.get().addr() as u64;
                let id48 = ID48.get().addr() as u64;
                ThreadId::from_u64((id48 << 48) + (id32 << 32) + (id16 << 16) + id0)
            }

            pub(super) fn set(id: ThreadId) {
                let val = id.as_u64().get();
                ID0.set(ptr::without_provenance_mut(val as usize));
                ID16.set(ptr::without_provenance_mut((val >> 16) as usize));
                ID32.set(ptr::without_provenance_mut((val >> 32) as usize));
                ID48.set(ptr::without_provenance_mut((val >> 48) as usize));
            }
        }
        target_pointer_width = "32" => {
            local_pointer! {
                static ID0;
                static ID32;
            }

            pub(super) const CHEAP: bool = false;

            pub(crate) fn get() -> Option<ThreadId> {
                let id0 = ID0.get().addr() as u64;
                let id32 = ID32.get().addr() as u64;
                ThreadId::from_u64((id32 << 32) + id0)
            }

            pub(super) fn set(id: ThreadId) {
                let val = id.as_u64().get();
                ID0.set(ptr::without_provenance_mut(val as usize));
                ID32.set(ptr::without_provenance_mut((val >> 32) as usize));
            }
        }
        _ => {
            local_pointer! {
                static ID;
            }

            pub(super) const CHEAP: bool = true;

            pub(crate) fn get() -> Option<ThreadId> {
                let id = ID.get().addr() as u64;
                ThreadId::from_u64(id)
            }

            pub(super) fn set(id: ThreadId) {
                let val = id.as_u64().get();
                ID.set(ptr::without_provenance_mut(val as usize));
            }
        }
    }

    #[inline]
    pub(super) fn get_or_init() -> ThreadId {
        get().unwrap_or_else(
            #[cold]
            || {
                let id = ThreadId::new();
                id::set(id);
                id
            },
        )
    }
}

/// 尝试为当前线程设置线程句柄。如果已经设置过句柄，或者 `thread` 的线程 ID
/// 会改变一个已经设置好的 ID，则失败。
pub(super) fn set_current(thread: Thread) -> Result<(), Thread> {
    if CURRENT.get() != NONE {
        return Err(thread);
    }

    match id::get() {
        Some(id) if id == thread.id() => {}
        None => id::set(thread.id()),
        _ => return Err(thread),
    }

    // 确保 `crate::rt::thread_cleanup` 会被运行，它将调用 `drop_current`。
    crate::sys::thread_local::guard::enable();
    CURRENT.set(thread.into_raw().cast_mut());
    Ok(())
}

/// 获取调用它的线程的唯一标识符。
///
/// 调用本函数可能比通过当前线程句柄访问当前线程 ID（即
/// `thread::current().id()`）更高效。
///
/// 本函数总是会成功，对同一个线程总是返回相同的值，并且保证不会调用全局
/// 分配器。
///
/// # Examples
///
/// ```
/// #![feature(current_thread_id)]
///
/// use std::thread;
///
/// let other_thread = thread::spawn(|| {
///     thread::current_id()
/// });
///
/// let other_thread_id = other_thread.join().unwrap();
/// assert_ne!(thread::current_id(), other_thread_id);
/// ```
#[inline]
#[must_use]
#[unstable(feature = "current_thread_id", issue = "147194")]
pub fn current_id() -> ThreadId {
    // 如果访问持久化的线程 ID 需要多次 TLS 访问，就尝试从当前线程句柄中获取它，
    // 那样只需一次 TLS 访问。
    if !id::CHEAP {
        if let Some(id) = try_with_current(|t| t.map(|t| t.id())) {
            return id;
        }
    }

    id::get_or_init()
}

/// 获取调用它的线程的操作系统线程 ID（如果可用）。如果不可用，则返回 Rust
/// 线程 ID。
///
/// 我们使用 `u64` 来容纳所有可能的平台 ID，从而避免过多的 `cfg`；大多数平台使用
/// `int`，有些使用指针，而 Apple 使用 `uint64_t`。这是一种用于诊断的“尽力而为”
/// 的做法，允许回退到非操作系统 ID（例如 Rust 线程 ID），或非唯一的 ID（例如
/// PID），前提是无法取得线程 ID。
pub(crate) fn current_os_id() -> u64 {
    imp::current_os_id().unwrap_or_else(|| current_id().as_u64().get())
}

/// 获取调用它的线程的句柄的引用（如果该句柄已被初始化）。
fn try_with_current<F, R>(f: F) -> R
where
    F: FnOnce(Option<&Thread>) -> R,
{
    let current = CURRENT.get();
    if current > DESTROYED {
        // SAFETY: `Arc` 不含内部可变性，所以无论它在何处被调用、句柄的地址可能
        // 不同，都没有关系。
        unsafe {
            let current = ManuallyDrop::new(Thread::from_raw(current));
            f(Some(&current))
        }
    } else {
        f(None)
    }
}

/// 以当前线程的名字为参数运行一个函数。
///
/// 除了线程局部访问之外，本函数可以安全地从信号处理器中调用，以及在其他无法
/// 进行内存分配的类似场景下调用。
pub(crate) fn with_current_name<F, R>(f: F) -> R
where
    F: FnOnce(Option<&str>) -> R,
{
    try_with_current(|thread| {
        let name = if let Some(thread) = thread {
            // 如果存在当前线程句柄，就尝试使用其中保存的名字。
            thread.name()
        } else if let Some(main) = main_thread::get()
            && let Some(id) = id::get()
            && id == main
        {
            // 主线程并不总是拥有线程句柄，我们必须通过它的 ID 来识别它。这些检查
            // 经过排序，使得只有在确实需要时才会加载当前 ID，因为从 TLS 加载它
            // 可能需要多次开销较大的访问。
            Some("main")
        } else {
            None
        };

        f(name)
    })
}

/// 获取调用它的线程的句柄。如果保存在线程局部存储中的句柄已经被销毁，本函数会
/// 创建一个新的、未命名的临时句柄，以便在几乎所有情况下都能进行线程 park。
pub(crate) fn current_or_unnamed() -> Thread {
    let current = CURRENT.get();
    if current > DESTROYED {
        unsafe {
            let current = ManuallyDrop::new(Thread::from_raw(current));
            (*current).clone()
        }
    } else if current == DESTROYED {
        Thread::new(id::get_or_init(), None)
    } else {
        init_current(current)
    }
}

/// 获取调用它的线程的句柄。
///
/// # Examples
///
/// 用 `thread::current()` 获取当前线程的句柄：
///
/// ```
/// use std::thread;
///
/// let handler = thread::Builder::new()
///     .name("named thread".into())
///     .spawn(|| {
///         let handle = thread::current();
///         assert_eq!(handle.name(), Some("named thread"));
///     })
///     .unwrap();
///
/// handler.join().unwrap();
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn current() -> Thread {
    let current = CURRENT.get();
    if current > DESTROYED {
        unsafe {
            let current = ManuallyDrop::new(Thread::from_raw(current));
            (*current).clone()
        }
    } else {
        init_current(current)
    }
}

#[cold]
fn init_current(current: *mut ()) -> Thread {
    if current == NONE {
        CURRENT.set(BUSY);
        // 如果线程 ID 已经初始化过，就使用它。
        let id = id::get_or_init();
        let thread = Thread::new(id, None);

        // 确保 `crate::rt::thread_cleanup` 会被运行，它将调用 `drop_current`。
        crate::sys::thread_local::guard::enable();
        CURRENT.set(thread.clone().into_raw().cast_mut());
        thread
    } else if current == BUSY {
        // BUSY 的存在完全是为了这一检查，但由于它处于慢路径，上面那次额外的 TLS
        // 写入应该无关紧要。否则的话，结果几乎总是栈溢出。
        //
        // 如果我们走到这里，意味着我们的初始化例程最终调用了 current()——
        // 要么是直接调用，要么是经由全局分配器间接调用——无论哪种都是 bug，
        // 因为我们不能在 current() 中调用全局分配器。
        rtabort!(
            "init_current() was re-entrant, which indicates a bug in the Rust threading implementation"
        )
    } else {
        debug_assert_eq!(current, DESTROYED);
        panic!(
            "use of std::thread::current() is not possible after the thread's \
            local data has been destroyed"
        )
    }
}

/// 应在 [`crate::rt::thread_cleanup`] 中运行它来重置线程句柄。
pub(crate) fn drop_current() {
    let current = CURRENT.get();
    if current > DESTROYED {
        unsafe {
            CURRENT.set(DESTROYED);
            drop(Thread::from_raw(current));
        }
    }
}
