use core::sync::atomic::{Atomic, AtomicU32, Ordering};

use crate::os::xous::ffi::Connection;

/// 把 `usize` 大小的若干字节组合（group）成一个 `usize` 并返回，起点为距数据起始处
/// `offset` * sizeof(usize) 个字节。例如，在 32 位系统上对
/// `group_or_null([1,2,3,4,5,6,7,8], 1)` 调用，将返回一个其中打包了 5678 的 `usize`。
fn group_or_null(data: &[u8], offset: usize) -> usize {
    let start = offset * size_of::<usize>();
    let mut out_array = [0u8; size_of::<usize>()];
    if start < data.len() {
        for (dest, src) in out_array.iter_mut().zip(&data[start..]) {
            *dest = *src;
        }
    }
    usize::from_le_bytes(out_array)
}

pub(crate) enum LogScalar<'a> {
    /// 发生了一次 panic，随后将有一条 panic 日志到来
    BeginPanic,

    /// 会有若干字节被追加到日志消息中
    AppendPanicMessage(&'a [u8]),
}

impl<'a> Into<[usize; 5]> for LogScalar<'a> {
    fn into(self) -> [usize; 5] {
        match self {
            LogScalar::BeginPanic => [1000, 0, 0, 0, 0],
            LogScalar::AppendPanicMessage(c) =>
            // 文本被分组为 4 个 `usize` 字（word）。id 是 1100 加上本条消息中的字符数。
            // 由于我们已经在 panic 之中了，忽略各种错误。
            {
                [
                    1100 + c.len(),
                    group_or_null(&c, 0),
                    group_or_null(&c, 1),
                    group_or_null(&c, 2),
                    group_or_null(&c, 3),
                ]
            }
        }
    }
}

pub(crate) enum LogLend {
    StandardOutput = 1,
    StandardError = 2,
}

impl Into<usize> for LogLend {
    fn into(self) -> usize {
        self as usize
    }
}

/// 返回一个到日志服务器（log server）的 `Connection`，该服务器用于向控制台打印消息以及
/// 报告 panic。
///
/// 如果日志服务器尚未启动，本调用将一直阻塞，直到该服务器运行起来。多次调用本函数是安全的，
/// 因为该地址会在一个进程内的所有线程之间共享。
pub(crate) fn log_server() -> Connection {
    static LOG_SERVER_CONNECTION: Atomic<u32> = AtomicU32::new(0);

    let cid = LOG_SERVER_CONNECTION.load(Ordering::Relaxed);
    if cid != 0 {
        return cid.into();
    }

    let cid = crate::os::xous::ffi::connect("xous-log-server ".try_into().unwrap()).unwrap();
    LOG_SERVER_CONNECTION.store(cid.into(), Ordering::Relaxed);
    cid
}
