//! 自由函数（Free functions）。

use super::builder::Builder;
use super::current::current;
use super::join_handle::JoinHandle;
use crate::mem::forget;
use crate::num::NonZero;
use crate::sys::thread as imp;
use crate::time::{Duration, Instant};
use crate::{io, panicking};

/// 派生一个新线程，并返回它的 [`JoinHandle`]。
///
/// 这个 join 句柄提供了一个 [`join`] 方法，可用于 join 派生出来的线程。如果派生
/// 线程发生 panic，[`join`] 会返回一个 [`Err`]，其中包含传给 [`panic!`] 的参数。
///
/// 如果这个 join 句柄被丢弃，派生线程就会被隐式地*分离*（detach）。在这种情况下，
/// 派生线程就无法再被 join 了。
/// （程序有责任最终要么 join 它创建的线程，要么将其分离；否则会造成资源泄漏。）
///
/// 本函数使用 [`Builder`] 的默认参数来创建线程。要指定新线程的栈大小或名字，
/// 请使用 [`Builder::spawn`]。
///
/// 正如你在 `spawn` 的签名中所见，对传给 `spawn` 的闭包及其返回值都有两个约束，
/// 让我们解释一下：
///
/// - `'static` 约束意味着闭包及其返回值的生命周期必须覆盖整个程序的执行过程。
///   原因在于线程可以比它被创建时所处的生命周期活得更久。
///
///   的确，如果线程（以及由此延伸的它的返回值）可能比它的调用者活得更久，我们就
///   需要确保它们在那之后仍然有效；而既然我们*无法*知道它何时会返回，就需要让
///   它们尽可能长时间地保持有效，也就是直到程序结束，因此才有了 `'static`
///   生命周期。
/// - [`Send`] 约束的原因是闭包将需要*按值*从派生它的线程传递到新线程。它的返回值
///   也将需要从新线程传递到对它进行 `join` 的线程。
///   提醒一下，[`Send`] 标记 trait 表达的是“可以安全地从一个线程传递到另一个
///   线程”，而 [`Sync`] 表达的是“可以安全地把一个引用从一个线程传递到另一个
///   线程”。
///
/// # Panics
///
/// 如果操作系统创建线程失败，则会 panic；可以使用 [`Builder::spawn`] 从此类
/// 错误中恢复。
///
/// # Examples
///
/// 创建一个线程。
///
/// ```
/// use std::thread;
///
/// let handler = thread::spawn(|| {
///     // 线程代码
/// });
///
/// handler.join().unwrap();
/// ```
///
/// 正如模块文档中所述，线程通常被用来通过 [`channels`] 进行通信，下面展示了它
/// 通常的样子。
///
/// 这个例子还展示了如何使用 `move`，以便把值的所有权交给线程。
///
/// ```
/// use std::thread;
/// use std::sync::mpsc::channel;
///
/// let (tx, rx) = channel();
///
/// let sender = thread::spawn(move || {
///     tx.send("Hello, thread".to_owned())
///         .expect("Unable to send on channel");
/// });
///
/// let receiver = thread::spawn(move || {
///     let value = rx.recv().expect("Unable to receive from channel");
///     println!("{value}");
/// });
///
/// sender.join().expect("The sender thread has panicked");
/// receiver.join().expect("The receiver thread has panicked");
/// ```
///
/// 线程还可以通过它的 [`JoinHandle`] 返回一个值，你可以借此进行异步计算
/// （不过 future 可能更适合）。
///
/// ```
/// use std::thread;
///
/// let computation = thread::spawn(|| {
///     // 某项昂贵的计算。
///     42
/// });
///
/// let result = computation.join().unwrap();
/// println!("{result}");
/// ```
///
/// # Notes
///
/// 关于“外来（foreign）”的展开操作（例如从 C++ 代码抛出的异常，或来自以不同
/// 运行时编译/链接的 Rust 代码中的 `panic!`），本函数提供的最低保证与
/// [`catch_unwind`] 相同；也就是说，如果由 `thread::spawn` 创建的线程带着这样
/// 的异常一路展开到根部，可能出现以下两种行为之一，且不指定具体会发生哪一种：
///
/// * 进程中止（abort）。
/// * 进程不中止，并且 [`join`] 会返回一个 `Result::Err`，其中包含一个不透明
///   类型。
///
/// [`catch_unwind`]: ../../std/panic/fn.catch_unwind.html
/// [`channels`]: crate::sync::mpsc
/// [`join`]: JoinHandle::join
/// [`Err`]: crate::result::Result::Err
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(miri, track_caller)] // 即便没有 panic，这也有助于 Miri 的回溯（backtrace）
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(f).expect("failed to spawn thread")
}

/// 主动让出一个时间片给操作系统调度器。
///
/// 它会调用底层操作系统调度器的让出（yield）原语，发出信号表示调用线程愿意放弃
/// 它剩余的时间片，以便操作系统可以在该 CPU 上调度其他线程。
///
/// 在循环中让出的一个缺点是：如果操作系统在当前 CPU 上没有任何其他就绪的线程
/// 可运行，该线程实际上就会忙等（busy-wait），这会浪费 CPU 时间和能源。
///
/// 因此，在等待感兴趣的事件时，程序员的首选应该是使用诸如 [`channel`]、
/// [`Condvar`]、[`Mutex`] 或 [`join`] 这样的同步设施，因为这些原语是以阻塞方式
/// 实现的，会让出 CPU 直到感兴趣的事件发生，从而避免反复让出。
///
/// 因此，`yield_now` 只应在极少数情况下使用，主要用于因没有其他合适方式得知
/// 感兴趣的事件何时发生、而必须反复轮询的场景。
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// thread::yield_now();
/// ```
///
/// [`channel`]: crate::sync::mpsc
/// [`join`]: JoinHandle::join
/// [`Condvar`]: crate::sync::Condvar
/// [`Mutex`]: crate::sync::Mutex
#[stable(feature = "rust1", since = "1.0.0")]
pub fn yield_now() {
    imp::yield_now()
}

/// 判断当前线程是否正因 panic 而处于展开（unwinding）状态。
///
/// 这一特性的常见用途是：在编写不安全代码时，于 `drop` 被调用时检查 `panicking`，
/// 以便对共享资源进行投毒（poison）。
///
/// 在编写安全代码时通常不需要它，因为 [`Mutex`][Mutex] 在线程持有锁期间发生
/// panic 时已经会自我投毒。
///
/// 它也可以用于多线程应用程序，以便向其他线程发送消息，警告某个线程已经 panic
/// （例如用于监控目的）。
///
/// # Examples
///
/// ```should_panic
/// use std::thread;
///
/// struct SomeStruct;
///
/// impl Drop for SomeStruct {
///     fn drop(&mut self) {
///         if thread::panicking() {
///             println!("dropped while unwinding");
///         } else {
///             println!("dropped while not unwinding");
///         }
///     }
/// }
///
/// {
///     print!("a: ");
///     let a = SomeStruct;
/// }
///
/// {
///     print!("b: ");
///     let b = SomeStruct;
///     panic!()
/// }
/// ```
///
/// [Mutex]: crate::sync::Mutex
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn panicking() -> bool {
    panicking::panicking()
}

/// 使用 [`sleep`]。
///
/// 让当前线程至少睡眠指定的时长。
///
/// 由于调度方面的具体情况或平台相关的功能，线程睡眠的时间可能比指定的时长更久。
/// 它绝不会睡得更短。
///
/// 本函数是阻塞的，不应在 `async` 函数中使用。
///
/// # Platform-specific behavior
///
/// 在 Unix 平台上，底层系统调用可能被一次虚假唤醒（spurious wakeup）或信号
/// 处理器打断。为了确保睡眠至少持续指定的时长，本函数可能会多次调用该系统调用。
///
/// # Examples
///
/// ```no_run
/// use std::thread;
///
/// // 让我们睡眠 2 秒：
/// thread::sleep_ms(2000);
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "1.6.0", note = "replaced by `std::thread::sleep`")]
pub fn sleep_ms(ms: u32) {
    sleep(Duration::from_millis(ms as u64))
}

/// 让当前线程至少睡眠指定的时长。
///
/// 由于调度方面的具体情况或平台相关的功能，线程睡眠的时间可能比指定的时长更久。
/// 它绝不会睡得更短。
///
/// 本函数是阻塞的，不应在 `async` 函数中使用。
///
/// # Platform-specific behavior
///
/// 在 Unix 平台上，底层系统调用可能被一次虚假唤醒（spurious wakeup）或信号
/// 处理器打断。为了确保睡眠至少持续指定的时长，本函数可能会多次调用该系统调用。
/// 对于不支持纳秒级睡眠精度的平台，`dur` 会被向上取整到它们所能睡眠的最近的
/// 时间粒度。
///
/// 目前，在 Unix 平台上指定零时长会立即返回而不调用底层的 [`nanosleep`] 系统
/// 调用，而在 Windows 平台上则总是会调用底层的 [`Sleep`] 系统调用。
/// 如果意图是让出当前时间片，你可能会想改用 [`yield_now`]。
///
/// [`nanosleep`]: https://linux.die.net/man/2/nanosleep
/// [`Sleep`]: https://docs.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-sleep
///
/// # Examples
///
/// ```no_run
/// use std::{thread, time};
///
/// let ten_millis = time::Duration::from_millis(10);
/// let now = time::Instant::now();
///
/// thread::sleep(ten_millis);
///
/// assert!(now.elapsed() >= ten_millis);
/// ```
#[stable(feature = "thread_sleep", since = "1.4.0")]
pub fn sleep(dur: Duration) {
    imp::sleep(dur)
}

/// 让当前线程睡眠，直到指定的截止时刻（deadline）过去为止。
///
/// 由于调度方面的具体情况或平台相关的功能，线程在指定截止时刻之后可能仍处于
/// 睡眠状态。它绝不会在截止时刻之前醒来。
///
/// 本函数是阻塞的，不应在 `async` 函数中使用。
///
/// # Platform-specific behavior
///
/// 大多数情况下，本函数会调用一个操作系统特定的函数。在不支持该函数的平台上，
/// 则使用 [`sleep`]。在下表中，这些平台被统称为 other（其他）。
///
/// # 底层系统调用
///
/// 当前使用[如下][currently]系统调用：
///
/// |  Platform |               System call                                            |
/// |-----------|----------------------------------------------------------------------|
/// | Linux     | [clock_nanosleep] (Monotonic clock)                                  |
/// | BSD except OpenBSD | [clock_nanosleep] (Monotonic Clock)]                        |
/// | Android   | [clock_nanosleep] (Monotonic Clock)]                                 |
/// | Solaris   | [clock_nanosleep] (Monotonic Clock)]                                 |
/// | Illumos   | [clock_nanosleep] (Monotonic Clock)]                                 |
/// | Dragonfly | [clock_nanosleep] (Monotonic Clock)]                                 |
/// | Hurd      | [clock_nanosleep] (Monotonic Clock)]                                 |
/// | Fuchsia   | [clock_nanosleep] (Monotonic Clock)]                                 |
/// | Vxworks   | [clock_nanosleep] (Monotonic Clock)]                                 |
/// | Other     | `sleep_until` uses [`sleep`] and does not issue a syscall itself     |
///
/// [currently]: crate::io#platform-specific-behavior
/// [clock_nanosleep]: https://linux.die.net/man/3/clock_nanosleep
///
/// **免责声明（Disclaimer）：**这些系统调用可能随时间变化。
///
/// # Examples
///
/// 一个把游戏限制在每秒 60 帧的简单游戏循环。
///
/// ```no_run
/// #![feature(thread_sleep_until)]
/// # use std::time::{Duration, Instant};
/// # use std::thread;
/// #
/// # fn update() {}
/// # fn render() {}
/// #
/// let max_fps = 60.0;
/// let frame_time = Duration::from_secs_f32(1.0/max_fps);
/// let mut next_frame = Instant::now();
/// loop {
///     thread::sleep_until(next_frame);
///     next_frame += frame_time;
///     update();
///     render();
/// }
/// ```
///
/// 一个我们不能调用得太频繁、且在成功之前需要尝试几次的慢速 API。通过使用
/// `sleep_until`，该 API 调用所花费的时间不会影响我们何时重试或何时放弃。
///
/// ```no_run
/// #![feature(thread_sleep_until)]
/// # use std::time::{Duration, Instant};
/// # use std::thread;
/// #
/// # enum Status {
/// #     Ready(usize),
/// #     Waiting,
/// # }
/// # fn slow_web_api_call() -> Status { Status::Ready(42) }
/// #
/// # const MAX_DURATION: Duration = Duration::from_secs(10);
/// #
/// # fn try_api_call() -> Result<usize, ()> {
/// let deadline = Instant::now() + MAX_DURATION;
/// let delay = Duration::from_millis(250);
/// let mut next_attempt = Instant::now();
/// loop {
///     if Instant::now() > deadline {
///         break Err(());
///     }
///     if let Status::Ready(data) = slow_web_api_call() {
///         break Ok(data);
///     }
///
///     next_attempt = deadline.min(next_attempt + delay);
///     thread::sleep_until(next_attempt);
/// }
/// # }
/// # let _data = try_api_call();
/// ```
#[unstable(feature = "thread_sleep_until", issue = "113752")]
pub fn sleep_until(deadline: Instant) {
    imp::sleep_until(deadline)
}

/// 用于确保 `park` 和 `park_timeout` 不会展开（unwind），因为如果处理不当，
/// 展开会导致未定义行为（背景请参见 #102398）。
struct PanicGuard;

impl Drop for PanicGuard {
    fn drop(&mut self) {
        rtabort!("an irrecoverable error occurred while synchronizing threads")
    }
}

/// 阻塞，除非或直到当前线程的 token 变为可用为止。
///
/// 调用 `park` 并不保证线程会永远保持 park 状态，调用者应当为这种可能性做好
/// 准备。不过，可以保证本函数不会 panic（如果实现遇到某些罕见错误，它可能会
/// 中止进程）。
///
/// # `park` 和 `unpark`
///
/// 每个线程都配备了一些基本的底层阻塞支持，通过 [`thread::park`][`park`] 函数和
/// [`thread::Thread::unpark`][`unpark`] 方法实现。[`park`] 会阻塞当前线程，随后
/// 它可以由另一个线程通过在被阻塞线程的句柄上调用 [`unpark`] 方法来恢复。
///
/// 从概念上讲，每个 [`Thread`] 句柄都关联着一个 token，它最初是不存在的：
///
/// * [`thread::park`][`park`] 函数会阻塞当前线程，除非或直到其线程句柄的 token
///   变为可用为止，届时它会原子地消耗掉这个 token。它也可能*虚假地（spuriously）*
///   返回，而不消耗 token。[`thread::park_timeout`] 做的事情相同，但允许指定阻塞
///   线程的最长时间。
///
/// * [`Thread`] 上的 [`unpark`] 方法会原子地令 token 变为可用（如果它尚不可用）。
///   由于即便线程当前没有处于 park 状态，token 也可以被该线程持有，因此
///   [`unpark`] 后接 [`park`] 会导致第二次调用立即返回。
///   然而，请注意：要依赖这一保证，你必须确保你的 `unpark` 发生在其他数据结构
///   可能进行的所有 `park` 之后！
///
/// 这套 API 的典型用法是：获取当前线程的句柄，把该句柄放入一个共享数据结构中，
/// 以便其他线程能够找到它，然后在一个循环中进行 `park`。当某个期望的条件被满足
/// 时，另一个线程在该句柄上调用 [`unpark`]。上面的最后一点保证了：即便 `unpark`
/// 发生在该线程完成 `park` 之前，它也会被正确地唤醒。
///
/// 请注意，通过共享数据结构来进行协调至关重要：如果你在没有先确认某个线程即将
/// 在你的代码中进行 `park` 的情况下就对它 `unpark`，那么这次 `unpark` 可能会被
/// 同一线程中*另一处不同的* `park` 消耗掉，从而导致死锁。这也意味着，你绝不能在
/// 准备进入 park 与真正调用 `park` 之间调用未知代码；例如，如果你调用了
/// `println!`，它自身可能会调用 `park`，从而消耗掉你的 `unpark` 并导致死锁。
///
/// 这一设计的动机有两点：
///
/// * 它避免了在构建新的同步原语时分配 mutex 和 condvar 的需要；线程本身已经
///   提供了基本的阻塞/信号通知能力。
///
/// * 它在许多平台上都可以被非常高效地实现。
///
/// # Memory Ordering
///
/// 对 `unpark` 的调用与对 `park` 的调用之间存在 _synchronize-with_（与之同步）
/// 关系，意味着在调用 `unpark` 之前所执行的内存操作，对那个消耗 token 并从
/// `park` 返回的线程是可见的。请注意，对于给定线程的所有 `park` 和 `unpark`
/// 操作构成一个全序（total order），并且*所有*在先的 `unpark` 操作都与 `park`
/// 同步（synchronize-with）。
///
/// 用原子内存序的术语来说，`unpark` 执行一个 `Release` 操作，而 `park` 执行
/// 与之对应的 `Acquire` 操作。针对同一线程的多次 `unpark` 调用构成一个
/// [release sequence]（释放序列）。
///
/// 请注意，被解除阻塞并不意味着一定有过 `unpark` 调用，因为唤醒也可能是虚假的
/// （spurious）。例如，一个有效但低效的实现可以让 `park` 和 `unpark` 什么都不做
/// 就立即返回，从而使得*所有*唤醒都是虚假的。
///
/// # Examples
///
/// ```
/// use std::thread;
/// use std::sync::atomic::{Ordering, AtomicBool};
/// use std::time::Duration;
///
/// static QUEUED: AtomicBool = AtomicBool::new(false);
/// static FLAG: AtomicBool = AtomicBool::new(false);
///
/// let parked_thread = thread::spawn(move || {
///     println!("Thread spawned");
///     // 发出信号表示我们即将 `park`。在这次 store 和我们的 `park` 之间，
///     // 不能有任何其他 `park`，否则那个 `park` 可能消耗掉我们的 `unpark` token！
///     QUEUED.store(true, Ordering::Release);
///     // 我们想要等到标志被设置。我们*本可以*只是自旋，但使用 park/unpark
///     // 更高效。
///     while !FLAG.load(Ordering::Acquire) {
///         // 我们在这里*不能*使用 `println!`，因为它内部可能会用到线程 park。
///         thread::park();
///         // 我们*可能*会虚假地走到这里，即远在下面那 10ms 结束之前！
///         // 但这没有问题，反正我们会一直循环到标志被设置为止。
///     }
///     println!("Flag received");
/// });
///
/// // 留出一些时间让线程被派生出来。
/// thread::sleep(Duration::from_millis(10));
///
/// // 确保线程即将进入 park。
/// // 这一点至关重要！它保证了下面的 `unpark` 不会被被 park 的线程中的
/// // 其他代码（例如 `println!` 内部）消耗掉。
/// while !QUEUED.load(Ordering::Acquire) {
///     // 自旋当然是低效的；在实践中，这里更可能是一个出队操作，
///     // 当没有人排队时我们就无事可做。
///     std::hint::spin_loop();
/// }
///
/// // 设置标志，并让线程醒来。
/// // 这里没有竞态条件：如果 `unpark` 先发生，`park` 会立即返回。
/// // 也不存在其他可能消耗这个 token 的 `park`，
/// // 因为我们一直等到另一个线程排好队为止。
/// // 因此不存在死锁的风险。
/// FLAG.store(true, Ordering::Release);
/// println!("Unpark the thread");
/// parked_thread.thread().unpark();
///
/// parked_thread.join().unwrap();
/// ```
///
/// [`Thread`]: super::Thread
/// [`unpark`]: super::Thread::unpark
/// [`thread::park_timeout`]: park_timeout
/// [release sequence]: https://en.cppreference.com/w/cpp/atomic/memory_order#Release_sequence
#[stable(feature = "rust1", since = "1.0.0")]
pub fn park() {
    let guard = PanicGuard;
    // SAFETY: park_timeout 是在本线程所拥有的 parker 上调用的。
    unsafe {
        current().park();
    }
    // 没有发生 panic，因此不要中止。
    forget(guard);
}

/// 使用 [`park_timeout`]。
///
/// 阻塞，除非或直到当前线程的 token 变为可用，或者指定的时长已到为止
/// （可能虚假唤醒）。
///
/// 本函数的语义等同于 [`park`]，区别在于线程被阻塞的时间大致不会超过 `dur`。
/// 由于抢占（preemption）或平台差异等异常情况，可能导致实际等待的最长时间并非
/// 恰好为 `ms` 那么长，因此本方法不应被用于精确计时。
///
/// 更多细节请参阅 [park 文档][`park`]。
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "1.6.0", note = "replaced by `std::thread::park_timeout`")]
pub fn park_timeout_ms(ms: u32) {
    park_timeout(Duration::from_millis(ms as u64))
}

/// 阻塞，除非或直到当前线程的 token 变为可用，或者指定的时长已到为止
/// （可能虚假唤醒）。
///
/// 本函数的语义等同于 [`park`][park]，区别在于线程被阻塞的时间大致不会超过
/// `dur`。由于抢占（preemption）或平台差异等异常情况，可能导致实际等待的最长
/// 时间并非恰好为 `dur` 那么长，因此本方法不应被用于精确计时。
///
/// 更多细节请参阅 [park 文档][park]。
///
/// # Platform-specific behavior
///
/// 对于不支持纳秒级睡眠精度的平台，`dur` 会被向上取整到它们所能睡眠的最近的
/// 时间粒度。
///
/// # Examples
///
/// 等待超时完整地过去：
///
/// ```rust,no_run
/// use std::thread::park_timeout;
/// use std::time::{Instant, Duration};
///
/// let timeout = Duration::from_secs(2);
/// let beginning_park = Instant::now();
///
/// let mut timeout_remaining = timeout;
/// loop {
///     park_timeout(timeout_remaining);
///     let elapsed = beginning_park.elapsed();
///     if elapsed >= timeout {
///         break;
///     }
///     println!("restarting park_timeout after {elapsed:?}");
///     timeout_remaining = timeout - elapsed;
/// }
/// ```
#[stable(feature = "park_timeout", since = "1.4.0")]
pub fn park_timeout(dur: Duration) {
    let guard = PanicGuard;
    // SAFETY: park_timeout 是在本线程所拥有的句柄上调用的。
    unsafe {
        current().park_timeout(dur);
    }
    // 没有发生 panic，因此不要中止。
    forget(guard);
}

/// 返回一个估计值，表示程序默认应使用的并行度（parallelism）。
///
/// 并行度是一种资源。给定的机器提供了一定的并行能力，即它能够同时执行的计算
/// 数量的上限。这个数字通常对应于计算机拥有的 CPU 数量，但在各种情况下也可能
/// 与之不同。
///
/// 像虚拟机或容器编排器这样的宿主环境，可能想要限制其中程序可用的并行度。这样做
/// 通常是为了限制（无意中）资源密集型程序对运行在同一台机器上的其他程序的潜在
/// 影响。
///
/// # Limitations
///
/// 本 API 的目的是提供一种简单且可移植的方式，用来查询程序默认应使用的并行度。
/// 除其他事项外，它不暴露关于 NUMA 区域的信息，不考虑（协）处理器能力的差异，
/// 也不考虑当前系统负载，并且不会为了更精确地查询可用并行度而修改程序的全局
/// 状态。
///
/// 在同时提供固定的稳态（steady-state）限制和突发（burst）限制的情况下，会使用
/// 稳态容量，以确保更可预测的延迟。
///
/// 资源限制可能在程序运行期间被更改，因此该值不会被缓存，而是每次调用本函数时
/// 重新计算。不应在热点代码（hot code）中调用它。
///
/// 本函数返回的值应被视为对任意给定时刻实际可用并行度的一种简化近似。要获得对
/// 程序可用并行度更详细或更精确的概览，你可能希望同时使用平台特定的 API。目前
/// `available_parallelism` 适用如下平台限制：
///
/// 在 Windows 上：
/// - 在逻辑 CPU 多于 64 个的系统上，它可能会少算可用并行度。然而，程序通常需要
///   特定支持才能利用超过 64 个逻辑 CPU；在缺乏此类支持的情况下，本函数返回的
///   数字准确地反映了程序默认能够使用的逻辑 CPU 数量。
/// - 在受进程范围亲和性掩码（affinity mask）或作业对象（job object）限制的系统
///   上，它可能会多算可用并行度。
///
/// 在 Linux 上：
/// - 当受进程范围的亲和性掩码或 cgroup 配额限制，而又无法查询
///   `sched_getaffinity()` 或 cgroup fs（例如由于沙箱限制）时，它可能会多算
///   可用并行度。
/// - 如果当前线程的亲和性掩码不能反映进程的 cpuset（例如由于线程被固定 pin），
///   它可能会少算可用并行度。
/// - 如果进程处于 cgroup v1 cpu 控制器中，这可能需要扫描挂载点来找到对应的
///   cgroup v1 控制器，在挂载点数量很多的系统上这可能耗时。
///   （这不适用于 cgroup v2，也不适用于不处于 cgroup 中的进程。）
/// - 它不会试图把 `ulimit` 纳入考虑。如果对线程数量设置了限制，
///   `available_parallelism` 无法得知一个 Rust 程序应该占用该限制中的多少，
///   也无法以可靠且无竞态的方式得知该限制中已经被占用了多少。
///
/// 在所有目标平台上：
/// - 当运行在带有 CPU 使用限制的虚拟机中（例如一台超额配置 overcommitted 的
/// 宿主机）时，它可能会多算可用并行度。
///
/// # Errors
///
/// 本函数会在如下情况（但不限于这些情况）返回错误：
///
/// - 如果该目标平台的并行度未知。
/// - 如果程序缺乏权限来查询提供给它的并行度。
///
/// # Examples
///
/// ```
/// # #![allow(dead_code)]
/// use std::{io, thread};
///
/// fn main() -> io::Result<()> {
///     let count = thread::available_parallelism()?.get();
///     assert!(count >= 1_usize);
///     Ok(())
/// }
/// ```
#[doc(alias = "available_concurrency")] // 我们曾在 unstable 阶段为该 API 取的旧名的别名。
#[doc(alias = "hardware_concurrency")] // C++ `std::thread::hardware_concurrency` 的别名。
#[doc(alias = "num_cpus")] // 一个提供类似功能的流行生态系统 crate 的别名。
#[stable(feature = "available_parallelism", since = "1.59.0")]
pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    imp::available_parallelism()
}
