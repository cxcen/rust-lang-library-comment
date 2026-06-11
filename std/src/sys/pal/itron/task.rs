use super::abi;
use super::error::{ItronError, fail, fail_aborting};
use crate::mem::MaybeUninit;

/// 获取处于 Running 状态的任务的 ID。失败时 panic。
#[inline]
pub fn current_task_id() -> abi::ID {
    try_current_task_id().unwrap_or_else(|e| fail(e, &"get_tid"))
}

/// 获取处于 Running 状态的任务的 ID。失败时 abort。
#[inline]
pub fn current_task_id_aborting() -> abi::ID {
    try_current_task_id().unwrap_or_else(|e| fail_aborting(e, &"get_tid"))
}

/// 获取处于 Running 状态的任务的 ID。
#[inline]
pub fn try_current_task_id() -> Result<abi::ID, ItronError> {
    unsafe {
        let mut out = MaybeUninit::uninit();
        ItronError::err_if_negative(abi::get_tid(out.as_mut_ptr()))?;
        Ok(out.assume_init())
    }
}

/// 获取指定任务的优先级。失败时 panic。
#[inline]
pub fn task_priority(task: abi::ID) -> abi::PRI {
    try_task_priority(task).unwrap_or_else(|e| fail(e, &"get_pri"))
}

/// 获取指定任务的优先级。
#[inline]
pub fn try_task_priority(task: abi::ID) -> Result<abi::PRI, ItronError> {
    unsafe {
        let mut out = MaybeUninit::uninit();
        ItronError::err_if_negative(abi::get_pri(task, out.as_mut_ptr()))?;
        Ok(out.assume_init())
    }
}
