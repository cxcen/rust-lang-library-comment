pub use crate::sync::mpsc::{RecvError, RecvTimeoutError, SendError, TryRecvError, TrySendError};
use crate::{error, fmt};

/// 从 [`send_timeout`] 方法返回的错误。
///
/// 该错误携带了正在发送的消息，以便调用方可以将其取回。
///
/// [`send_timeout`]: super::Sender::send_timeout
#[derive(PartialEq, Eq, Clone, Copy)]
#[unstable(feature = "mpmc_channel", issue = "126840")]
pub enum SendTimeoutError<T> {
    /// 消息未能发送：通道已满，且操作超时。
    ///
    /// 如果这是一个容量为 0 的通道（zero-capacity channel），则该错误表示在超时之前一直没有
    /// 可用的接收者来接收这条消息。
    Timeout(T),

    /// 消息未能发送：通道已断连（disconnected）。
    Disconnected(T),
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> fmt::Debug for SendTimeoutError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "SendTimeoutError(..)".fmt(f)
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> fmt::Display for SendTimeoutError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            SendTimeoutError::Timeout(..) => "timed out waiting on send operation".fmt(f),
            SendTimeoutError::Disconnected(..) => "sending on a disconnected channel".fmt(f),
        }
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> error::Error for SendTimeoutError<T> {}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> From<SendError<T>> for SendTimeoutError<T> {
    fn from(err: SendError<T>) -> SendTimeoutError<T> {
        match err {
            SendError(e) => SendTimeoutError::Disconnected(e),
        }
    }
}
