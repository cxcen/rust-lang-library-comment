use crate::io;
use crate::sys::io::RawOsError;

pub fn errno() -> RawOsError {
    // 在 Motor OS 中不使用，因为它含义模糊：Motor OS
    // 是基于微内核(micro-kernel）的，I/O 通过共享内存
    // 环形缓冲区进行，因此一个在 unix 上属于系统调用的 I/O 操作
    // 在 Motor OS 上可能根本不涉及任何系统调用，或者是一个
    // 例如等待 I/O 驱动(sys-io）通知的系统调用；而该等待
    // 系统调用可能成功，但驱动可能报告一个 I/O 错误；或者是
    // 针对若干个 I/O 操作的一批结果，其中一些成功一些
    // 不成功。
    //
    // 另外，Motor OS 进程中的 I/O 操作由一个
    // 独立的运行时后台/I/O 线程处理，所以实在很难
    // 定义「当前线程中的最后一个系统错误」
    // 究竟意味着什么。
    let error_code: moto_rt::ErrorCode = moto_rt::Error::Unknown.into();
    error_code.into()
}

pub fn is_interrupted(_code: io::RawOsError) -> bool {
    false // Motor OS 没有信号(signal）。
}

pub fn decode_error_kind(code: io::RawOsError) -> io::ErrorKind {
    if code < 0 || code > u16::MAX.into() {
        return io::ErrorKind::Uncategorized;
    }

    let error = moto_rt::Error::from(code as moto_rt::ErrorCode);

    match error {
        moto_rt::Error::Unspecified => io::ErrorKind::Uncategorized,
        moto_rt::Error::Unknown => io::ErrorKind::Uncategorized,
        moto_rt::Error::NotReady => io::ErrorKind::WouldBlock,
        moto_rt::Error::NotImplemented => io::ErrorKind::Unsupported,
        moto_rt::Error::VersionTooHigh => io::ErrorKind::Unsupported,
        moto_rt::Error::VersionTooLow => io::ErrorKind::Unsupported,
        moto_rt::Error::InvalidArgument => io::ErrorKind::InvalidInput,
        moto_rt::Error::OutOfMemory => io::ErrorKind::OutOfMemory,
        moto_rt::Error::NotAllowed => io::ErrorKind::PermissionDenied,
        moto_rt::Error::NotFound => io::ErrorKind::NotFound,
        moto_rt::Error::InternalError => io::ErrorKind::Other,
        moto_rt::Error::TimedOut => io::ErrorKind::TimedOut,
        moto_rt::Error::AlreadyInUse => io::ErrorKind::AlreadyExists,
        moto_rt::Error::UnexpectedEof => io::ErrorKind::UnexpectedEof,
        moto_rt::Error::InvalidFilename => io::ErrorKind::InvalidFilename,
        moto_rt::Error::NotADirectory => io::ErrorKind::NotADirectory,
        moto_rt::Error::BadHandle => io::ErrorKind::InvalidInput,
        moto_rt::Error::FileTooLarge => io::ErrorKind::FileTooLarge,
        moto_rt::Error::NotConnected => io::ErrorKind::NotConnected,
        moto_rt::Error::StorageFull => io::ErrorKind::StorageFull,
        moto_rt::Error::InvalidData => io::ErrorKind::InvalidData,
        _ => io::ErrorKind::Uncategorized,
    }
}

pub fn error_string(errno: RawOsError) -> String {
    let error: moto_rt::Error = match errno {
        x if x < 0 => moto_rt::Error::Unknown,
        x if x > u16::MAX.into() => moto_rt::Error::Unknown,
        x => (x as moto_rt::ErrorCode).into(), /* u16 */
    };
    format!("{}", error)
}
