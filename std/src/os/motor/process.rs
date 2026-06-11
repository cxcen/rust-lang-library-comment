#![unstable(feature = "motor_ext", issue = "147456")]

use crate::sealed::Sealed;
use crate::sys::AsInner;

pub trait ChildExt: Sealed {
    /// 提取主线程的原始句柄（raw handle），不获取其所有权
    fn sys_handle(&self) -> u64;
}

impl ChildExt for crate::process::Child {
    fn sys_handle(&self) -> u64 {
        self.as_inner().handle()
    }
}
