//! 由 SOLID 内核扩展支撑的读写锁实现。
#![forbid(unsafe_op_in_unsafe_fn)]

use crate::sys::pal::abi;
use crate::sys::pal::itron::error::{ItronError, expect_success, expect_success_aborting, fail};
use crate::sys::pal::itron::spin::SpinIdOnceCell;

pub struct RwLock {
    /// 底层 mutex 对象的 ID
    rwl: SpinIdOnceCell<()>,
}

// Safety: `num_readers` 受 `mtx_num_readers` 保护
unsafe impl Send for RwLock {}
unsafe impl Sync for RwLock {}

fn new_rwl() -> Result<abi::ID, ItronError> {
    ItronError::err_if_negative(unsafe { abi::rwl_acre_rwl() })
}

impl RwLock {
    #[inline]
    pub const fn new() -> RwLock {
        RwLock { rwl: SpinIdOnceCell::new() }
    }

    /// 获取内部 mutex 的 ID，该 ID 是惰性创建的。
    fn raw(&self) -> abi::ID {
        match self.rwl.get_or_try_init(|| new_rwl().map(|id| (id, ()))) {
            Ok((id, ())) => id,
            Err(e) => fail(e, &"rwl_acre_rwl"),
        }
    }

    #[inline]
    pub fn read(&self) {
        let rwl = self.raw();
        expect_success(unsafe { abi::rwl_loc_rdl(rwl) }, &"rwl_loc_rdl");
    }

    #[inline]
    pub fn try_read(&self) -> bool {
        let rwl = self.raw();
        match unsafe { abi::rwl_ploc_rdl(rwl) } {
            abi::E_TMOUT => false,
            er => {
                expect_success(er, &"rwl_ploc_rdl");
                true
            }
        }
    }

    #[inline]
    pub fn write(&self) {
        let rwl = self.raw();
        expect_success(unsafe { abi::rwl_loc_wrl(rwl) }, &"rwl_loc_wrl");
    }

    #[inline]
    pub fn try_write(&self) -> bool {
        let rwl = self.raw();
        match unsafe { abi::rwl_ploc_wrl(rwl) } {
            abi::E_TMOUT => false,
            er => {
                expect_success(er, &"rwl_ploc_wrl");
                true
            }
        }
    }

    #[inline]
    pub unsafe fn read_unlock(&self) {
        let rwl = self.raw();
        expect_success_aborting(unsafe { abi::rwl_unl_rwl(rwl) }, &"rwl_unl_rwl");
    }

    #[inline]
    pub unsafe fn write_unlock(&self) {
        let rwl = self.raw();
        expect_success_aborting(unsafe { abi::rwl_unl_rwl(rwl) }, &"rwl_unl_rwl");
    }

    #[inline]
    pub unsafe fn downgrade(&self) {
        // SOLID 平台不支持读写锁的 `downgrade` 操作，所以本函数只是一个空操作
        //（no-op），因为只有 1 个读者可以读：即原来的那个写者。
    }
}

impl Drop for RwLock {
    #[inline]
    fn drop(&mut self) {
        if let Some(rwl) = self.rwl.get().map(|x| x.0) {
            expect_success_aborting(unsafe { abi::rwl_del_rwl(rwl) }, &"rwl_del_rwl");
        }
    }
}
