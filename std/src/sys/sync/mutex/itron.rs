//! 由 μITRON mutex 支撑的 Mutex 实现。假定 `acre_mtx` 和 `TA_INHERIT` 可用。
#![forbid(unsafe_op_in_unsafe_fn)]

use crate::sys::pal::itron::abi;
use crate::sys::pal::itron::error::{ItronError, expect_success, expect_success_aborting, fail};
use crate::sys::pal::itron::spin::SpinIdOnceCell;

pub struct Mutex {
    /// 底层 mutex 对象的 ID
    mtx: SpinIdOnceCell<()>,
}

/// 创建一个 mutex 对象。本函数永不 panic。
fn new_mtx() -> Result<abi::ID, ItronError> {
    ItronError::err_if_negative(unsafe {
        abi::acre_mtx(&abi::T_CMTX {
            // 优先级继承（priority inheritance）mutex
            mtxatr: abi::TA_INHERIT,
            // 未使用
            ceilpri: 0,
        })
    })
}

impl Mutex {
    #[inline]
    pub const fn new() -> Mutex {
        Mutex { mtx: SpinIdOnceCell::new() }
    }

    /// 获取内部 mutex 的 ID，该 ID 是惰性创建的。
    fn raw(&self) -> abi::ID {
        match self.mtx.get_or_try_init(|| new_mtx().map(|id| (id, ()))) {
            Ok((id, ())) => id,
            Err(e) => fail(e, &"acre_mtx"),
        }
    }

    pub fn lock(&self) {
        let mtx = self.raw();
        expect_success(unsafe { abi::loc_mtx(mtx) }, &"loc_mtx");
    }

    pub unsafe fn unlock(&self) {
        let mtx = unsafe { self.mtx.get_unchecked().0 };
        expect_success_aborting(unsafe { abi::unl_mtx(mtx) }, &"unl_mtx");
    }

    pub fn try_lock(&self) -> bool {
        let mtx = self.raw();
        match unsafe { abi::ploc_mtx(mtx) } {
            abi::E_TMOUT => false,
            er => {
                expect_success(er, &"ploc_mtx");
                true
            }
        }
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        if let Some(mtx) = self.mtx.get().map(|x| x.0) {
            expect_success_aborting(unsafe { abi::del_mtx(mtx) }, &"del_mtx");
        }
    }
}
