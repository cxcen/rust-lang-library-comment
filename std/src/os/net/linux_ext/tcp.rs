//! 针对 [`std::net`] 模块中各基础类型的 Linux 与 Android 特有 tcp 扩展。
//!
//! [`std::net`]: crate::net

use crate::sealed::Sealed;
use crate::sys::AsInner;
#[cfg(target_os = "linux")]
use crate::time::Duration;
use crate::{io, net};

/// 针对 [`TcpStream`] 的操作系统特有扩展
///
/// [`TcpStream`]: net::TcpStream
#[stable(feature = "tcp_quickack", since = "1.89.0")]
pub trait TcpStreamExt: Sealed {
    /// 启用或禁用 `TCP_QUICKACK`。
    ///
    /// 该标志使 Linux 积极地发送 ACK，而不是延迟发送。
    /// 在对该套接字进行后续操作之后，Linux 可能会重置此标志。
    ///
    /// 更多信息参见 [`man 7 tcp`](https://man7.org/linux/man-pages/man7/tcp.7.html) 与
    /// [TCP delayed acknowledgement](https://en.wikipedia.org/wiki/TCP_delayed_acknowledgment)。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    /// #[cfg(target_os = "linux")]
    /// use std::os::linux::net::TcpStreamExt;
    /// #[cfg(target_os = "android")]
    /// use std::os::android::net::TcpStreamExt;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///         .expect("Couldn't connect to the server...");
    /// stream.set_quickack(true).expect("set_quickack call failed");
    /// ```
    #[stable(feature = "tcp_quickack", since = "1.89.0")]
    fn set_quickack(&self, quickack: bool) -> io::Result<()>;

    /// 获取此套接字上 `TCP_QUICKACK` 选项的值。
    ///
    /// 关于该选项的更多信息，参见 [`TcpStreamExt::set_quickack`]。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    /// #[cfg(target_os = "linux")]
    /// use std::os::linux::net::TcpStreamExt;
    /// #[cfg(target_os = "android")]
    /// use std::os::android::net::TcpStreamExt;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///         .expect("Couldn't connect to the server...");
    /// stream.set_quickack(true).expect("set_quickack call failed");
    /// assert_eq!(stream.quickack().unwrap_or(false), true);
    /// ```
    #[stable(feature = "tcp_quickack", since = "1.89.0")]
    fn quickack(&self) -> io::Result<bool>;

    /// 套接字监听器将仅在数据到达时被唤醒。
    ///
    /// `accept` 参数设置在数据可读之前的最大延迟，从而减少那些没有数据需要处理的
    /// 短生命周期连接的数量。
    /// 与其他平台上等价的 `SO_ACCEPTFILTER` 特性不同，这里无需在 `listen` 调用之后再设置它。
    /// 注意，从用户视角看该延迟以 Duration 表示，调用会将其向下取整到能用 `c_int`
    /// 表示的最近的整秒数。
    ///
    /// 参见 [`man 7 tcp`](https://man7.org/linux/man-pages/man7/tcp.7.html)
    ///
    /// # 示例
    ///
    /// ```no run
    /// #![feature(tcp_deferaccept)]
    /// use std::net::TcpStream;
    /// use std::os::linux::net::TcpStreamExt;
    /// use std::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///         .expect("Couldn't connect to the server...");
    /// stream.set_deferaccept(Duration::from_secs(1u64)).expect("set_deferaccept call failed");
    /// ```
    #[unstable(feature = "tcp_deferaccept", issue = "119639")]
    #[cfg(target_os = "linux")]
    fn set_deferaccept(&self, accept: Duration) -> io::Result<()>;

    /// 获取 `TCP_DEFER_ACCEPT` 选项的 accept 延迟值。
    ///
    /// 关于该选项的更多信息，参见 [`TcpStreamExt::set_deferaccept`]。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(tcp_deferaccept)]
    /// use std::net::TcpStream;
    /// use std::os::linux::net::TcpStreamExt;
    /// use std::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///         .expect("Couldn't connect to the server...");
    /// stream.set_deferaccept(Duration::from_secs(1u64)).expect("set_deferaccept call failed");
    /// assert_eq!(stream.deferaccept().unwrap(), Duration::from_secs(1u64));
    /// ```
    #[unstable(feature = "tcp_deferaccept", issue = "119639")]
    #[cfg(target_os = "linux")]
    fn deferaccept(&self) -> io::Result<Duration>;
}

#[stable(feature = "tcp_quickack", since = "1.89.0")]
impl Sealed for net::TcpStream {}

#[stable(feature = "tcp_quickack", since = "1.89.0")]
impl TcpStreamExt for net::TcpStream {
    fn set_quickack(&self, quickack: bool) -> io::Result<()> {
        self.as_inner().as_inner().set_quickack(quickack)
    }

    fn quickack(&self) -> io::Result<bool> {
        self.as_inner().as_inner().quickack()
    }

    #[cfg(target_os = "linux")]
    fn set_deferaccept(&self, accept: Duration) -> io::Result<()> {
        self.as_inner().as_inner().set_deferaccept(accept)
    }

    #[cfg(target_os = "linux")]
    fn deferaccept(&self) -> io::Result<Duration> {
        self.as_inner().as_inner().deferaccept()
    }
}
