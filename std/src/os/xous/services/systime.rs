use core::sync::atomic::{Atomic, AtomicU32, Ordering};

use crate::os::xous::ffi::{Connection, connect};

pub(crate) enum SystimeScalar {
    GetUtcTimeMs,
}

impl Into<[usize; 5]> for SystimeScalar {
    fn into(self) -> [usize; 5] {
        match self {
            SystimeScalar::GetUtcTimeMs => [3, 0, 0, 0, 0],
        }
    }
}

/// 返回一个到 systime 服务器的 `Connection`。该服务器用于报告实时时钟（realtime clock）。
pub(crate) fn systime_server() -> Connection {
    static SYSTIME_SERVER_CONNECTION: Atomic<u32> = AtomicU32::new(0);
    let cid = SYSTIME_SERVER_CONNECTION.load(Ordering::Relaxed);
    if cid != 0 {
        return cid.into();
    }

    let cid = connect("timeserverpublic".try_into().unwrap()).unwrap();
    SYSTIME_SERVER_CONNECTION.store(cid.into(), Ordering::Relaxed);
    cid
}
