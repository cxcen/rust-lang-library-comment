use super::id::ThreadId;
use super::main_thread;
use crate::alloc::System;
use crate::ffi::CStr;
use crate::fmt;
use crate::pin::Pin;
use crate::sync::Arc;
use crate::sys::sync::Parker;
use crate::time::Duration;

// 这个模块确保私有字段保持私有，这对于强制满足安全要求是必需的。
mod thread_name_string {
    use crate::ffi::{CStr, CString};
    use crate::str;

    /// 像 `String` 一样保证是 UTF-8，又像 `CString` 一样以空字符结尾。
    pub(crate) struct ThreadNameString {
        inner: CString,
    }

    impl From<String> for ThreadNameString {
        fn from(s: String) -> Self {
            Self {
                inner: CString::new(s).expect("thread name may not contain interior null bytes"),
            }
        }
    }

    impl ThreadNameString {
        pub fn as_cstr(&self) -> &CStr {
            &self.inner
        }

        pub fn as_str(&self) -> &str {
            // SAFETY: `ThreadNameString` 保证是 UTF-8。
            unsafe { str::from_utf8_unchecked(self.inner.to_bytes()) }
        }
    }
}

use thread_name_string::ThreadNameString;

/// `Thread` 句柄的内部表示
///
/// 我们显式地设置对齐，以满足 Thread::into_raw 中的保证。这使得应用程序能够把
/// 额外的元数据位塞进对齐空隙中，这在使用原子操作时相当有用。
#[repr(align(8))]
struct Inner {
    name: Option<ThreadNameString>,
    id: ThreadId,
    parker: Parker,
}

impl Inner {
    fn parker(self: Pin<&Self>) -> Pin<&Parker> {
        unsafe { Pin::map_unchecked(self, |inner| &inner.parker) }
    }
}

#[derive(Clone)]
#[stable(feature = "rust1", since = "1.0.0")]
/// 指向一个线程的句柄。
///
/// 线程通过 `Thread` 类型来表示，你可以通过以下两种方式之一获取它：
///
/// * 通过派生一个新线程，例如使用 [`thread::spawn`] 函数，并在返回的
///   [`JoinHandle`] 上调用 [`thread`]。
/// * 通过 [`thread::current`] 函数请求当前线程。
///
/// [`thread::current`] 函数即便对于并非由本模块 API 派生的线程也是可用的。
///
/// 通常没有必要自己创建一个 `Thread` 结构体，而应当使用诸如 `spawn` 这样的函数
/// 来创建新线程，详见 [`Builder`] 和 [`spawn`] 的文档。
///
/// [`thread::spawn`]: super::spawn
/// [`thread`]: super::JoinHandle::thread
/// [`JoinHandle`]: super::JoinHandle
/// [`thread::current`]: super::current::current
/// [`Builder`]: super::Builder
/// [`spawn`]: super::spawn
pub struct Thread {
    // 我们使用 System 分配器，这样创建或丢弃这个句柄就不会干扰到某个可能使用
    // 线程局部存储的 Global 分配器。
    inner: Pin<Arc<Inner, System>>,
}

impl Thread {
    pub(crate) fn new(id: ThreadId, name: Option<String>) -> Thread {
        let name = name.map(ThreadNameString::from);

        // 我们这里不得不使用 `unsafe` 来就地构造 `Parker`，这是 UNIX 实现所要求的。
        //
        // SAFETY: 我们在创建后立即 pin 住这个 Arc，所以它的地址永不改变。
        let inner = unsafe {
            let mut arc = Arc::<Inner, _>::new_uninit_in(System);
            let ptr = Arc::get_mut_unchecked(&mut arc).as_mut_ptr();
            (&raw mut (*ptr).name).write(name);
            (&raw mut (*ptr).id).write(id);
            Parker::new_in_place(&raw mut (*ptr).parker);
            Pin::new_unchecked(arc.assume_init())
        };

        Thread { inner }
    }

    /// 类似于公开的 [`park`]，但可以在任何句柄上调用。它用于允许在 TLS 析构函数
    /// 中进行 park。
    ///
    /// # Safety
    /// 只能从这个句柄所属的那个线程上调用。
    ///
    /// [`park`]: super::park
    pub(crate) unsafe fn park(&self) {
        unsafe { self.inner.as_ref().parker().park() }
    }

    /// 类似于公开的 [`park_timeout`]，但可以在任何句柄上调用。它用于允许在 TLS
    /// 析构函数中进行 park。
    ///
    /// # Safety
    /// 只能从这个句柄所属的那个线程上调用。
    ///
    /// [`park_timeout`]: super::park_timeout
    pub(crate) unsafe fn park_timeout(&self, dur: Duration) {
        unsafe { self.inner.as_ref().parker().park_timeout(dur) }
    }

    /// 原子地令该句柄的 token 变为可用（如果它尚不可用）。
    ///
    /// 每个线程都配备了一些基本的底层阻塞支持，通过 [`park`] 函数和 `unpark()`
    /// 方法实现。它们可以被用作一种更省 CPU 的自旋锁实现。
    ///
    /// 更多细节请参阅 [park 文档][park documentation]。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// static QUEUED: AtomicBool = AtomicBool::new(false);
    ///
    /// let parked_thread = thread::Builder::new()
    ///     .spawn(|| {
    ///         println!("Parking thread");
    ///         QUEUED.store(true, Ordering::Release);
    ///         thread::park();
    ///         println!("Thread unparked");
    ///     })
    ///     .unwrap();
    ///
    /// // 留出一些时间让线程被派生出来。
    /// thread::sleep(Duration::from_millis(10));
    ///
    /// // 等待直到另一个线程排好队。
    /// // 这一点至关重要！它保证了下面的 `unpark` 不会被被 park 的线程中的
    /// // 其他代码（例如 `println!` 内部）消耗掉。
    /// while !QUEUED.load(Ordering::Acquire) {
    ///     // 自旋当然是低效的；在实践中，这里更可能是一个出队操作，
    ///     // 当没有人排队时我们就无事可做。
    ///     std::hint::spin_loop();
    /// }
    ///
    /// println!("Unpark the thread");
    /// parked_thread.thread().unpark();
    ///
    /// parked_thread.join().unwrap();
    /// ```
    ///
    /// [`park`]: super::park
    /// [park documentation]: super::park
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn unpark(&self) {
        self.inner.as_ref().parker().unpark();
    }

    /// 获取该线程的唯一标识符。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let other_thread = thread::spawn(|| {
    ///     thread::current().id()
    /// });
    ///
    /// let other_thread_id = other_thread.join().unwrap();
    /// assert!(thread::current().id() != other_thread_id);
    /// ```
    #[stable(feature = "thread_id", since = "1.19.0")]
    #[must_use]
    pub fn id(&self) -> ThreadId {
        self.inner.id
    }

    /// 获取该线程的名字。
    ///
    /// 关于具名线程的更多信息，请参阅
    /// [此模块级文档][naming-threads]。
    ///
    /// # Examples
    ///
    /// 线程默认没有指定名字：
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let handler = builder.spawn(|| {
    ///     assert!(thread::current().name().is_none());
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// 指定了名字的线程：
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .name("foo".into());
    ///
    /// let handler = builder.spawn(|| {
    ///     assert_eq!(thread::current().name(), Some("foo"))
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// [naming-threads]: ./index.html#naming-threads
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        if let Some(name) = &self.inner.name {
            Some(name.as_str())
        } else if main_thread::get() == Some(self.inner.id) {
            Some("main")
        } else {
            None
        }
    }

    /// 消耗这个 `Thread`，返回一个裸指针。
    ///
    /// 为了避免内存泄漏，必须使用 [`Thread::from_raw`] 把该指针转换回 `Thread`。
    /// 保证该指针至少对齐到 8 字节。
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(thread_raw)]
    ///
    /// use std::thread::{self, Thread};
    ///
    /// let thread = thread::current();
    /// let id = thread.id();
    /// let ptr = Thread::into_raw(thread);
    /// unsafe {
    ///     assert_eq!(Thread::from_raw(ptr).id(), id);
    /// }
    /// ```
    #[unstable(feature = "thread_raw", issue = "97523")]
    pub fn into_raw(self) -> *const () {
        // Safety: 我们只暴露一个不透明指针，它维持了 `Pin` 不变式。
        let inner = unsafe { Pin::into_inner_unchecked(self.inner) };
        Arc::into_raw_with_allocator(inner).0 as *const ()
    }

    /// 从一个裸指针构造出一个 `Thread`。
    ///
    /// 该裸指针必须是此前由一次 [`Thread::into_raw`] 调用返回的。
    ///
    /// # Safety
    ///
    /// 本函数是不安全的，因为不当使用可能导致内存不安全，即便返回的 `Thread`
    /// 从未被访问也是如此。
    ///
    /// 从一个并非由 [`Thread::into_raw`] 返回的指针构造 `Thread` 是**未定义
    /// 行为**。
    ///
    /// 对同一个裸指针调用本函数两次，如果两个 `Thread` 实例都被丢弃，则可能
    /// 导致 double-free。
    #[unstable(feature = "thread_raw", issue = "97523")]
    pub unsafe fn from_raw(ptr: *const ()) -> Thread {
        // Safety: 由调用者保证。
        unsafe {
            Thread { inner: Pin::new_unchecked(Arc::from_raw_in(ptr as *const Inner, System)) }
        }
    }

    pub(crate) fn cname(&self) -> Option<&CStr> {
        if let Some(name) = &self.inner.name {
            Some(name.as_cstr())
        } else if main_thread::get() == Some(self.inner.id) {
            Some(c"main")
        } else {
            None
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Thread")
            .field("id", &self.id())
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}
