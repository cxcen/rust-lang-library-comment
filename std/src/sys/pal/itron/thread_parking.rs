use super::abi;
use super::error::expect_success_aborting;
use super::time::with_tmos;
use crate::time::Duration;

pub type ThreadId = abi::ID;

pub use super::task::current_task_id_aborting as current;

pub fn park(_hint: usize) {
    match unsafe { abi::slp_tsk() } {
        abi::E_OK | abi::E_RLWAI => {}
        err => {
            expect_success_aborting(err, &"slp_tsk");
        }
    }
}

pub fn park_timeout(dur: Duration, _hint: usize) {
    match with_tmos(dur, |tmo| unsafe { abi::tslp_tsk(tmo) }) {
        abi::E_OK | abi::E_RLWAI | abi::E_TMOUT => {}
        err => {
            expect_success_aborting(err, &"tslp_tsk");
        }
    }
}

pub fn unpark(id: ThreadId, _hint: usize) {
    match unsafe { abi::wup_tsk(id) } {
        // 允许尝试唤醒一个已被销毁或无关的任务，因此我们忽略所有可能由此情形
        // 产生的错误。
        abi::E_OK | abi::E_NOEXS | abi::E_OBJ | abi::E_QOVR => {}
        err => {
            expect_success_aborting(err, &"wup_tsk");
        }
    }
}
