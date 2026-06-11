/// 在阻塞操作期间被初始化、并由 `read` 或 `write` 消费的临时数据。
///
/// 每个字段都承载与某种特定通道 flavor（array/list/zero）相关的数据。
#[derive(Debug, Default)]
pub struct Token {
    pub(crate) array: super::array::ArrayToken,
    pub(crate) list: super::list::ListToken,
    #[allow(dead_code)]
    pub(crate) zero: super::zero::ZeroToken,
}

/// 与“某线程在某通道上的某次操作”相关联的标识符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation(usize);

impl Operation {
    /// 从一个可变引用创建操作标识符。
    ///
    /// 此函数本质上就是把该引用的地址转成一个数字。该引用应指向一个“专属于此线程、此操作”
    /// 的变量，且在整个阻塞操作期间保持存活。
    #[inline]
    pub fn hook<T>(r: &mut T) -> Operation {
        let val = r as *mut T as usize;
        // 确保该指针地址不会等于 `Selected::{Waiting, Aborted, Disconnected}` 的数值表示
        // （即 0、1、2）。
        assert!(val > 2);
        Operation(val)
    }
}

/// 阻塞操作的当前状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selected {
    /// 仍在等待某个操作。
    Waiting,

    /// 阻塞当前线程的尝试已被中止（abort）。
    Aborted,

    /// 因为通道断连（disconnected），某个操作变为就绪。
    Disconnected,

    /// 因为可以发送或接收一条消息，某个操作变为就绪。
    Operation(Operation),
}

impl From<usize> for Selected {
    #[inline]
    fn from(val: usize) -> Selected {
        match val {
            0 => Selected::Waiting,
            1 => Selected::Aborted,
            2 => Selected::Disconnected,
            oper => Selected::Operation(Operation(oper)),
        }
    }
}

impl Into<usize> for Selected {
    #[inline]
    fn into(self) -> usize {
        match self {
            Selected::Waiting => 0,
            Selected::Aborted => 1,
            Selected::Disconnected => 2,
            Selected::Operation(Operation(val)) => val,
        }
    }
}
