//! 线程派生与 join 的内部逻辑。

use super::current::set_current;
use super::id::ThreadId;
use super::scoped::ScopeData;
use super::thread::Thread;
use super::{Result, spawnhook};
use crate::cell::UnsafeCell;
use crate::marker::PhantomData;
use crate::mem::{ManuallyDrop, MaybeUninit};
use crate::sync::Arc;
use crate::sync::atomic::{Atomic, AtomicUsize, Ordering};
use crate::sys::{AsInner, IntoInner, thread as imp};
use crate::{env, io, panic};

#[cfg_attr(miri, track_caller)] // 即便没有 panic，这也有助于 Miri 的回溯（backtrace）
pub(super) unsafe fn spawn_unchecked<'scope, F, T>(
    name: Option<String>,
    stack_size: Option<usize>,
    no_hooks: bool,
    scope_data: Option<Arc<ScopeData>>,
    f: F,
) -> io::Result<JoinInner<'scope, T>>
where
    F: FnOnce() -> T,
    F: Send,
    T: Send,
{
    let stack_size = stack_size.unwrap_or_else(|| {
        static MIN: Atomic<usize> = AtomicUsize::new(0);

        match MIN.load(Ordering::Relaxed) {
            0 => {}
            n => return n - 1,
        }

        let amt = env::var_os("RUST_MIN_STACK")
            .and_then(|s| s.to_str().and_then(|s| s.parse().ok()))
            .unwrap_or(imp::DEFAULT_MIN_STACK_SIZE);

        // 0 是我们的哨兵值，因此要确保初始化运行之后我们绝不会再看到 0
        MIN.store(amt + 1, Ordering::Relaxed);
        amt
    });

    let id = ThreadId::new();
    let thread = Thread::new(id, name);

    let hooks = if no_hooks {
        spawnhook::ChildSpawnHooks::default()
    } else {
        spawnhook::run_spawn_hooks(&thread)
    };

    let my_packet: Arc<Packet<'scope, T>> =
        Arc::new(Packet { scope: scope_data, result: UnsafeCell::new(None), _marker: PhantomData });
    let their_packet = my_packet.clone();

    // 把 `f` 包在 `MaybeUninit` 中传递，因为实际上那个闭包可能*运行得比 `F` 的
    // 生命周期更久*。
    // 更多细节请参见 <https://github.com/rust-lang/rust/issues/101983>。
    // 为了防止泄漏，我们使用一个会丢弃其内容的包装器。
    #[repr(transparent)]
    struct MaybeDangling<T>(MaybeUninit<T>);
    impl<T> MaybeDangling<T> {
        fn new(x: T) -> Self {
            MaybeDangling(MaybeUninit::new(x))
        }
        fn into_inner(self) -> T {
            // 确保我们不会 drop。
            let this = ManuallyDrop::new(self);
            // SAFETY: 我们始终是已初始化的。
            unsafe { this.0.assume_init_read() }
        }
    }
    impl<T> Drop for MaybeDangling<T> {
        fn drop(&mut self) {
            // SAFETY: 我们始终是已初始化的。
            unsafe { self.0.assume_init_drop() };
        }
    }

    let f = MaybeDangling::new(f);

    // Rust 线程的入口点，在平台特定的线程初始化完成之后执行。
    let rust_start = move || {
        let f = f.into_inner();
        let try_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            crate::sys::backtrace::__rust_begin_short_backtrace(|| hooks.run());
            crate::sys::backtrace::__rust_begin_short_backtrace(f)
        }));
        // SAFETY: `their_packet` 刚刚在上面构建出来，并已被该闭包移动进来
        // （它是一个 Arc<...>），而 `my_packet` 将被存放在与本闭包相同的
        // `JoinInner` 中，这意味着这次修改是安全的（不会修改它并影响到某个
        // 远处的值）。
        unsafe { *their_packet.result.get() = Some(try_result) };
        // 在这里 `their_packet` 被丢弃；如果这是该 packet 的最后一个 `Arc`，
        // 那么这次丢弃将调用 `decrement_num_running_threads`，从而发出信号
        // 表示本线程已经完成。
        drop(their_packet);
        // 到这里，生命周期 `'scope` 可以结束了。`main` 在那之后还会再运行
        // 一小段时间，然后自己返回。
    };

    if let Some(scope_data) = &my_packet.scope {
        scope_data.increment_num_running_threads();
    }

    // SAFETY: Box 的动态大小和对齐保持不变。至于为何对生命周期的更改是合理的，
    // 见下文。
    let rust_start = unsafe {
        let ptr = Box::into_raw(Box::new(rust_start));
        let ptr = crate::mem::transmute::<
            *mut (dyn FnOnce() + Send + '_),
            *mut (dyn FnOnce() + Send + 'static),
        >(ptr);
        Box::from_raw(ptr)
    };

    let init = Box::new(ThreadInit { handle: thread.clone(), rust_start });

    Ok(JoinInner {
        // SAFETY:
        //
        // `imp::Thread::new` 接受一个带 `'static` 生命周期的闭包，因为它会经由
        // FFI 传递，或以其他方式与那些没有生命周期概念、也无法强制生命周期的
        // 底层线程原语一起使用。
        //
        // 如本函数文档的 `Safety` 一节所述，本函数的调用者需要保证所传入的生命
        // 周期足够长，覆盖该线程的生命周期。
        //
        // 类似地，`sys` 实现必须保证：在线程终止之后（其信号是 `Thread::join`
        // 返回），不存在任何指向该闭包的引用。
        native: unsafe { imp::Thread::new(stack_size, init)? },
        thread,
        packet: my_packet,
    })
}

/// 传递给派生线程用于线程初始化的数据。任何线程实现在启动一个新线程时，都应当先
/// 在它上面调用 .init()，然后再做任何其他事情，以确保当前线程被正确初始化、并且
/// 全局分配器能够正常工作。
pub(crate) struct ThreadInit {
    pub handle: Thread,
    pub rust_start: Box<dyn FnOnce() + Send>,
}

impl ThreadInit {
    /// 在本线程上初始化“当前线程（current thread）”机制，并返回 Rust 入口点。
    pub fn init(self: Box<Self>) -> Box<dyn FnOnce() + Send> {
        // 在全局分配器上发生任何（解）分配之前先设置好当前线程，以便其实现可以
        // 调用 std::thread::current()。这也是我们取 Box<Self> 的原因——确保在此
        // 之前该 Box 不会被销毁。克隆该句柄不会触发全局分配器，因为它是一个 Arc。
        if let Err(_thread) = set_current(self.handle.clone()) {
            // 此时当前线程的句柄不应已被设置。这里用 abort 以节省二进制体积
            // （见 #123356）。
            rtabort!("current thread handle already set during thread spawn");
        }

        if let Some(name) = self.handle.cname() {
            imp::set_name(name);
        }

        self.rust_start
    }
}

// 这个 packet 用于在派生线程与程序其余部分之间传递返回值。它通过一个 `Arc`
// 共享，这里不需要 mutex，因为同步是借助 `join()` 完成的（在线程退出之前，
// 调用者绝不会读取这个 packet）。
//
// 一个指向该 packet 的 Arc 被存放在 `JoinInner` 中，而后者又被放进
// `JoinHandle`。
struct Packet<'scope, T> {
    scope: Option<Arc<ScopeData>>,
    result: UnsafeCell<Option<Result<T>>>,
    _marker: PhantomData<Option<&'scope ScopeData>>,
}

// 由于使用了 `UnsafeCell`，我们需要手动实现 Sync。
// 类型 `T` 本来就应当总是 Send（否则该线程根本无法被创建），而 Packet 之所以
// 是 Sync，是因为对 `UnsafeCell` 的所有访问都已（借助 `join()` 这一边界）同步，
// 并且 `ScopeData` 也是 Sync 的。
unsafe impl<'scope, T: Send> Sync for Packet<'scope, T> {}

impl<'scope, T> Drop for Packet<'scope, T> {
    fn drop(&mut self) {
        // 如果这个 packet 属于一个在某 scope 中运行的线程、该线程发生了 panic、
        // 并且没有人消费这个 panic 载荷，我们就确保 scope 函数会 panic。
        let unhandled_panic = matches!(self.result.get_mut(), Some(Err(_)));
        // 丢弃这个结果，且不引发展开。
        // 这只对那些没有被 join() 的线程有意义，因为 join() 会取走 `result`
        // 并将其设为 None，从而这里就没有任何东西需要丢弃了。
        // 如果这一句 panic，我们应当处理它，因为我们身处本线程最外层
        // `catch_unwind` 之外。
        // 在这种情况下我们直接中止（abort），因为我们没有别的办法。
        //（而且即便我们试图以某种方式处理它，还需要处理“从中取出的 panic 载荷
        // 在丢弃时又 panic”的情况，如此层层递进。见 issue #86027。）
        if let Err(_) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            *self.result.get_mut() = None;
        })) {
            rtabort!("thread result panicked on drop");
        }
        // 记账（book-keeping），以便 scope 知道它何时完成。
        if let Some(scope) = &self.scope {
            // 既然此后这个线程上不会再有能使用 'scope 的用户代码运行，就把该线程
            // 标记为“已完成（finished）”。
            // 重要的是我们只在 `result` 被丢弃之后才这样做，因为丢弃它时可能仍会
            // 使用它从 'scope 借来的东西。
            scope.decrement_num_running_threads(unhandled_panic);
        }
    }
}

/// JoinHandle 的内部表示
pub(super) struct JoinInner<'scope, T> {
    native: imp::Thread,
    thread: Thread,
    packet: Arc<Packet<'scope, T>>,
}

impl<'scope, T> JoinInner<'scope, T> {
    pub(super) fn is_finished(&self) -> bool {
        Arc::strong_count(&self.packet) == 1
    }

    pub(super) fn thread(&self) -> &Thread {
        &self.thread
    }

    pub(super) fn join(mut self) -> Result<T> {
        self.native.join();
        Arc::get_mut(&mut self.packet)
            // FIXME(fuzzypixelz): 在这里返回错误而不是 panic，将需要更新
            // `std::thread::Result` 的文档；目前我们当且仅当线程发生过 panic
            // 时才能返回 `Err`。
            .expect("threads should not terminate unexpectedly")
            .result
            .get_mut()
            .take()
            .unwrap()
    }
}

impl<T> AsInner<imp::Thread> for JoinInner<'static, T> {
    fn as_inner(&self) -> &imp::Thread {
        &self.native
    }
}

impl<T> IntoInner<imp::Thread> for JoinInner<'static, T> {
    fn into_inner(self) -> imp::Thread {
        self.native
    }
}
