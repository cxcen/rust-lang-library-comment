use super::abi;
use crate::{fmt, io};

/// 包装一个 μITRON 错误码。
#[derive(Debug, Copy, Clone)]
pub struct ItronError {
    er: abi::ER,
}

impl ItronError {
    /// 从指定的错误码构造 `ItronError`。如果该错误码并不表示失败或警告，
    /// 则返回 `None`。
    #[inline]
    pub fn new(er: abi::ER) -> Option<Self> {
        if er < 0 { Some(Self { er }) } else { None }
    }

    /// 如果 `er` 表示成功则返回 `Ok(er)`，否则返回 `Err(_)`。
    #[inline]
    pub fn err_if_negative(er: abi::ER) -> Result<abi::ER, Self> {
        if let Some(error) = Self::new(er) { Err(error) } else { Ok(er) }
    }

    /// 获取原始错误码。
    #[inline]
    pub fn as_raw(&self) -> abi::ER {
        self.er
    }
}

impl fmt::Display for ItronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 允许各平台扩展 `error_name`
        if let Some(name) = crate::sys::error::error_name(self.er) {
            write!(f, "{} ({})", name, self.er)
        } else {
            write!(f, "{}", self.er)
        }
    }
}

/// 描述指定的 μITRON 错误码。如果它是未定义的错误码，则返回 `None`。
pub fn error_name(er: abi::ER) -> Option<&'static str> {
    match er {
        // 成功
        er if er >= 0 => None,

        // μITRON 4.0
        abi::E_SYS => Some("system error"),
        abi::E_NOSPT => Some("unsupported function"),
        abi::E_RSFN => Some("reserved function code"),
        abi::E_RSATR => Some("reserved attribute"),
        abi::E_PAR => Some("parameter error"),
        abi::E_ID => Some("invalid ID number"),
        abi::E_CTX => Some("context error"),
        abi::E_MACV => Some("memory access violation"),
        abi::E_OACV => Some("object access violation"),
        abi::E_ILUSE => Some("illegal service call use"),
        abi::E_NOMEM => Some("insufficient memory"),
        abi::E_NOID => Some("no ID number available"),
        abi::E_OBJ => Some("object state error"),
        abi::E_NOEXS => Some("non-existent object"),
        abi::E_QOVR => Some("queue overflow"),
        abi::E_RLWAI => Some("forced release from waiting"),
        abi::E_TMOUT => Some("polling failure or timeout"),
        abi::E_DLT => Some("waiting object deleted"),
        abi::E_CLS => Some("waiting object state changed"),
        abi::E_WBLK => Some("non-blocking code accepted"),
        abi::E_BOVR => Some("buffer overflow"),

        // TOPPERS 第三代内核
        abi::E_NORES => Some("insufficient system resources"),
        abi::E_RASTER => Some("termination request raised"),
        abi::E_COMM => Some("communication failure"),

        _ => None,
    }
}

#[inline]
pub fn is_interrupted(er: abi::ER) -> bool {
    er == abi::E_RLWAI
}

pub fn decode_error_kind(er: abi::ER) -> io::ErrorKind {
    match er {
        // 成功
        er if er >= 0 => io::ErrorKind::Uncategorized,

        // μITRON 4.0
        // abi::E_SYS
        abi::E_NOSPT => io::ErrorKind::Unsupported, // Some("unsupported function"),
        abi::E_RSFN => io::ErrorKind::InvalidInput, // Some("reserved function code"),
        abi::E_RSATR => io::ErrorKind::InvalidInput, // Some("reserved attribute"),
        abi::E_PAR => io::ErrorKind::InvalidInput,  // Some("parameter error"),
        abi::E_ID => io::ErrorKind::NotFound,       // Some("invalid ID number"),
        // abi::E_CTX
        abi::E_MACV => io::ErrorKind::PermissionDenied, // Some("memory access violation"),
        abi::E_OACV => io::ErrorKind::PermissionDenied, // Some("object access violation"),
        // abi::E_ILUSE
        abi::E_NOMEM => io::ErrorKind::OutOfMemory, // Some("insufficient memory"),
        abi::E_NOID => io::ErrorKind::OutOfMemory,  // Some("no ID number available"),
        // abi::E_OBJ
        abi::E_NOEXS => io::ErrorKind::NotFound, // Some("non-existent object"),
        // abi::E_QOVR
        abi::E_RLWAI => io::ErrorKind::Interrupted, // Some("forced release from waiting"),
        abi::E_TMOUT => io::ErrorKind::TimedOut,    // Some("polling failure or timeout"),
        // abi::E_DLT
        // abi::E_CLS
        // abi::E_WBLK
        // abi::E_BOVR

        // TOPPERS 第三代内核
        abi::E_NORES => io::ErrorKind::OutOfMemory, // Some("insufficient system resources"),
        // abi::E_RASTER
        // abi::E_COMM
        _ => io::ErrorKind::Uncategorized,
    }
}

/// 类似于 `ItronError::err_if_negative(er).expect()`，区别在于：在 panic 的同时，
/// 它会把消息打印到 `panic_output` 并改为中止（abort）程序。这样可确保错误消息
/// 不会被二次 panic 所掩盖。
///
/// 这对于诊断 `std` 内部机制所用的同步原语的创建失败很有用。当系统被错误配置为
/// 给内核对象提供过小的池（pool）时，这类失败很常见。
#[inline]
pub fn expect_success(er: abi::ER, msg: &&str) -> abi::ER {
    match ItronError::err_if_negative(er) {
        Ok(x) => x,
        Err(e) => fail(e, msg),
    }
}

/// 类似于 `ItronError::err_if_negative(er).expect()`，但改为中止（abort）。
///
/// 在不允许 panic、或失败的影响会是持久性的场合使用它。
#[inline]
pub fn expect_success_aborting(er: abi::ER, msg: &&str) -> abi::ER {
    match ItronError::err_if_negative(er) {
        Ok(x) => x,
        Err(e) => fail_aborting(e, msg),
    }
}

#[cold]
pub fn fail(e: impl fmt::Display, msg: &&str) -> ! {
    if crate::thread::panicking() {
        fail_aborting(e, msg)
    } else {
        panic!("{} failed: {}", *msg, e)
    }
}

#[cold]
pub fn fail_aborting(e: impl fmt::Display, msg: &&str) -> ! {
    rtabort!("{} failed: {}", *msg, e)
}
