//! 针对 [`std::thread`] 模块中各基础类型的 Unix 特有扩展。
//!
//! [`std::thread`]: crate::thread

#![stable(feature = "thread_extensions", since = "1.9.0")]

#[allow(deprecated)]
use crate::os::unix::raw::pthread_t;
use crate::sys::{AsInner, IntoInner};
use crate::thread::JoinHandle;

#[stable(feature = "thread_extensions", since = "1.9.0")]
#[allow(deprecated)]
pub type RawPthread = pthread_t;

/// 针对 [`JoinHandle`] 的 Unix 特有扩展。
#[stable(feature = "thread_extensions", since = "1.9.0")]
pub trait JoinHandleExt {
    /// 提取裸 pthread_t，但不取得其所有权
    #[stable(feature = "thread_extensions", since = "1.9.0")]
    fn as_pthread_t(&self) -> RawPthread;

    /// 消耗该线程，返回其裸 pthread_t
    ///
    /// 该函数把底层 pthread_t 的所有权**转移（transfers ownership）**给调用方。
    /// 调用方随即成为该 pthread_t 的唯一所有者，并必须在不再需要它时对其进行
    /// detach 或 join。
    #[stable(feature = "thread_extensions", since = "1.9.0")]
    fn into_pthread_t(self) -> RawPthread;
}

#[stable(feature = "thread_extensions", since = "1.9.0")]
impl<T> JoinHandleExt for JoinHandle<T> {
    fn as_pthread_t(&self) -> RawPthread {
        self.as_inner().id() as RawPthread
    }

    fn into_pthread_t(self) -> RawPthread {
        self.into_inner().into_id() as RawPthread
    }
}
