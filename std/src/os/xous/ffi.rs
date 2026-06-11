#![allow(dead_code)]
#![allow(unused_variables)]
#![stable(feature = "rust1", since = "1.0.0")]

#[path = "../unix/ffi/os_str.rs"]
mod os_str;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::os_str::{OsStrExt, OsStringExt};

mod definitions;
#[stable(feature = "rust1", since = "1.0.0")]
pub use definitions::*;

fn lend_mut_impl(
    connection: Connection,
    opcode: usize,
    data: &mut [u8],
    arg1: usize,
    arg2: usize,
    blocking: bool,
) -> Result<(usize, usize), Error> {
    let mut a0 = if blocking { Syscall::SendMessage } else { Syscall::TrySendMessage } as usize;
    let mut a1: usize = connection.try_into().unwrap();
    let mut a2 = InvokeType::LendMut as usize;
    let a3 = opcode;
    let a4 = data.as_mut_ptr() as usize;
    let a5 = data.len();
    let a6 = arg1;
    let a7 = arg2;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::MemoryReturned as usize {
        Ok((a1, a2))
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

pub(crate) fn lend_mut(
    connection: Connection,
    opcode: usize,
    data: &mut [u8],
    arg1: usize,
    arg2: usize,
) -> Result<(usize, usize), Error> {
    lend_mut_impl(connection, opcode, data, arg1, arg2, true)
}

pub(crate) fn try_lend_mut(
    connection: Connection,
    opcode: usize,
    data: &mut [u8],
    arg1: usize,
    arg2: usize,
) -> Result<(usize, usize), Error> {
    lend_mut_impl(connection, opcode, data, arg1, arg2, false)
}

fn lend_impl(
    connection: Connection,
    opcode: usize,
    data: &[u8],
    arg1: usize,
    arg2: usize,
    blocking: bool,
) -> Result<(usize, usize), Error> {
    let mut a0 = if blocking { Syscall::SendMessage } else { Syscall::TrySendMessage } as usize;
    let a1: usize = connection.try_into().unwrap();
    let a2 = InvokeType::Lend as usize;
    let a3 = opcode;
    let a4 = data.as_ptr() as usize;
    let a5 = data.len();
    let a6 = arg1;
    let a7 = arg2;
    let mut ret1;
    let mut ret2;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1 => ret1,
            inlateout("a2") a2 => ret2,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::MemoryReturned as usize {
        Ok((ret1, ret2))
    } else if result == SyscallResult::Error as usize {
        Err(ret1.into())
    } else {
        Err(Error::InternalError)
    }
}

pub(crate) fn lend(
    connection: Connection,
    opcode: usize,
    data: &[u8],
    arg1: usize,
    arg2: usize,
) -> Result<(usize, usize), Error> {
    lend_impl(connection, opcode, data, arg1, arg2, true)
}

pub(crate) fn try_lend(
    connection: Connection,
    opcode: usize,
    data: &[u8],
    arg1: usize,
    arg2: usize,
) -> Result<(usize, usize), Error> {
    lend_impl(connection, opcode, data, arg1, arg2, false)
}

fn scalar_impl(connection: Connection, args: [usize; 5], blocking: bool) -> Result<(), Error> {
    let mut a0 = if blocking { Syscall::SendMessage } else { Syscall::TrySendMessage } as usize;
    let mut a1: usize = connection.try_into().unwrap();
    let a2 = InvokeType::Scalar as usize;
    let a3 = args[0];
    let a4 = args[1];
    let a5 = args[2];
    let a6 = args[3];
    let a7 = args[4];

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::Ok as usize {
        Ok(())
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

pub(crate) fn scalar(connection: Connection, args: [usize; 5]) -> Result<(), Error> {
    scalar_impl(connection, args, true)
}

pub(crate) fn try_scalar(connection: Connection, args: [usize; 5]) -> Result<(), Error> {
    scalar_impl(connection, args, false)
}

fn blocking_scalar_impl(
    connection: Connection,
    args: [usize; 5],
    blocking: bool,
) -> Result<[usize; 5], Error> {
    let mut a0 = if blocking { Syscall::SendMessage } else { Syscall::TrySendMessage } as usize;
    let mut a1: usize = connection.try_into().unwrap();
    let mut a2 = InvokeType::BlockingScalar as usize;
    let mut a3 = args[0];
    let mut a4 = args[1];
    let mut a5 = args[2];
    let a6 = args[3];
    let a7 = args[4];

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2,
            inlateout("a3") a3,
            inlateout("a4") a4,
            inlateout("a5") a5,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::Scalar1 as usize {
        Ok([a1, 0, 0, 0, 0])
    } else if result == SyscallResult::Scalar2 as usize {
        Ok([a1, a2, 0, 0, 0])
    } else if result == SyscallResult::Scalar5 as usize {
        Ok([a1, a2, a3, a4, a5])
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

pub(crate) fn blocking_scalar(
    connection: Connection,
    args: [usize; 5],
) -> Result<[usize; 5], Error> {
    blocking_scalar_impl(connection, args, true)
}

pub(crate) fn try_blocking_scalar(
    connection: Connection,
    args: [usize; 5],
) -> Result<[usize; 5], Error> {
    blocking_scalar_impl(connection, args, false)
}

fn connect_impl(address: ServerAddress, blocking: bool) -> Result<Connection, Error> {
    let a0 = if blocking { Syscall::Connect } else { Syscall::TryConnect } as usize;
    let address: [u32; 4] = address.into();
    let a1: usize = address[0].try_into().unwrap();
    let a2: usize = address[1].try_into().unwrap();
    let a3: usize = address[2].try_into().unwrap();
    let a4: usize = address[3].try_into().unwrap();
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    let mut result: usize;
    let mut value: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0 => result,
            inlateout("a1") a1 => value,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };
    if result == SyscallResult::ConnectionId as usize {
        Ok(value.try_into().unwrap())
    } else if result == SyscallResult::Error as usize {
        Err(value.into())
    } else {
        Err(Error::InternalError)
    }
}

/// 连接到由所指定 `address` 表示的 Xous 服务器。
///
/// 当前线程将一直阻塞，直到该服务器可用为止。如果该服务器无法再接受更多连接，则返回错误。
pub(crate) fn connect(address: ServerAddress) -> Result<Connection, Error> {
    connect_impl(address, true)
}

/// 尝试连接到由所指定 `address` 表示的 Xous 服务器。
///
/// 如果该服务器不存在，则返回 None。
pub(crate) fn try_connect(address: ServerAddress) -> Result<Option<Connection>, Error> {
    match connect_impl(address, false) {
        Ok(conn) => Ok(Some(conn)),
        Err(Error::ServerNotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 终止当前进程，并把所指定的 code 返回给父进程。
pub(crate) fn exit(return_code: u32) -> ! {
    let a0 = Syscall::TerminateProcess as usize;
    let a1 = return_code as usize;
    let a2 = 0;
    let a3 = 0;
    let a4 = 0;
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") a0,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
            in("a4") a4,
            in("a5") a5,
            in("a6") a6,
            in("a7") a7,
        )
    };
    unreachable!();
}

/// 挂起当前线程，让另一个线程得以运行。如果系统上没有其他可运行的线程，本线程可能会立即
/// 继续执行下去。
pub(crate) fn do_yield() {
    let a0 = Syscall::Yield as usize;
    let a1 = 0;
    let a2 = 0;
    let a3 = 0;
    let a4 = 0;
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0 => _,
            inlateout("a1") a1 => _,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };
}

/// 从系统分配内存。
///
/// 可以选择性地指定一个物理地址和/或虚拟地址，以确保内存被分配在特定的偏移处；否则内核将
/// 自行选择一个地址。
///
/// # 安全性(Safety）
///
/// 本函数是安全的，除非指定了虚拟地址。在那种情况下，内核会返回一个指向已有区间的别名
/// （alias）。这违反了 Rust 的指针唯一性保证。
pub(crate) unsafe fn map_memory<T>(
    phys: Option<core::ptr::NonNull<T>>,
    virt: Option<core::ptr::NonNull<T>>,
    count: usize,
    flags: MemoryFlags,
) -> Result<&'static mut [T], Error> {
    let mut a0 = Syscall::MapMemory as usize;
    let mut a1 = phys.map(|p| p.as_ptr() as usize).unwrap_or_default();
    let mut a2 = virt.map(|p| p.as_ptr() as usize).unwrap_or_default();
    let a3 = count * size_of::<T>();
    let a4 = flags.bits();
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::MemoryRange as usize {
        let start = core::ptr::with_exposed_provenance_mut::<T>(a1);
        let len = a2 / size_of::<T>();
        let end = unsafe { start.add(len) };
        Ok(unsafe { core::slice::from_raw_parts_mut(start, len) })
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

/// 销毁给定的内存，把它归还给编译器。
///
/// Safety: 在本函数返回后，`range` 所指向的内存不应再被使用，即便本函数返回的是 Err()
/// 也是如此。
pub(crate) unsafe fn unmap_memory<T>(range: *mut [T]) -> Result<(), Error> {
    let mut a0 = Syscall::UnmapMemory as usize;
    let mut a1 = range.as_mut_ptr() as usize;
    let a2 = range.len() * size_of::<T>();
    let a3 = 0;
    let a4 = 0;
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::Ok as usize {
        Ok(())
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

/// 调整给定区间的内存标志（memory flags）。
///
/// 这可用于从给定区域中移除标志，以加固（harden）内存访问。注意：标志只能被移除，
/// 而绝不能被添加。
///
/// Safety: `range` 所指向的内存可能变得不可访问，或被移除其可变性（mutability）。
/// 由调用者负责确保 `new_flags` 所指定的标志得到遵守，否则程序将崩溃。
pub(crate) unsafe fn update_memory_flags<T>(
    range: *mut [T],
    new_flags: MemoryFlags,
) -> Result<(), Error> {
    let mut a0 = Syscall::UpdateMemoryFlags as usize;
    let mut a1 = range.as_mut_ptr() as usize;
    let a2 = range.len() * size_of::<T>();
    let a3 = new_flags.bits();
    let a4 = 0; // 进程 ID 当前为 None
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::Ok as usize {
        Ok(())
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

/// 用给定的栈（stack）以及最多四个参数创建一个线程。
pub(crate) fn create_thread(
    start: *mut usize,
    stack: *mut [u8],
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> Result<ThreadId, Error> {
    let mut a0 = Syscall::CreateThread as usize;
    let mut a1 = start as usize;
    let a2 = stack.as_mut_ptr() as usize;
    let a3 = stack.len();
    let a4 = arg0;
    let a5 = arg1;
    let a6 = arg2;
    let a7 = arg3;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::ThreadId as usize {
        Ok(a1.into())
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

/// 等待给定线程终止，并返回该线程的退出码（exit code）。
pub(crate) fn join_thread(thread_id: ThreadId) -> Result<usize, Error> {
    let mut a0 = Syscall::JoinThread as usize;
    let mut a1 = thread_id.into();
    let a2 = 0;
    let a3 = 0;
    let a4 = 0;
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::Scalar1 as usize {
        Ok(a1)
    } else if result == SyscallResult::Scalar2 as usize {
        Ok(a1)
    } else if result == SyscallResult::Scalar5 as usize {
        Ok(a1)
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

/// 获取当前线程的 ID。
pub(crate) fn thread_id() -> Result<ThreadId, Error> {
    let mut a0 = Syscall::GetThreadId as usize;
    let mut a1 = 0;
    let a2 = 0;
    let a3 = 0;
    let a4 = 0;
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::ThreadId as usize {
        Ok(a1.into())
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}

/// 把给定的 `knob` 限制调整为新值 `new`。当前值必须与 `current` 相匹配，本调整才会生效。
///
/// 本调用将以新值作为结果返回。如果调用失败，则返回旧值。无论哪种情况，本函数都会成功返回。
///
/// 如果 `knob` 不是一个有效的限制，或者本调用无法成功，则会产生一个错误。
pub(crate) fn adjust_limit(knob: Limits, current: usize, new: usize) -> Result<usize, Error> {
    let mut a0 = Syscall::AdjustProcessLimit as usize;
    let mut a1 = knob as usize;
    let a2 = current;
    let a3 = new;
    let a4 = 0;
    let a5 = 0;
    let a6 = 0;
    let a7 = 0;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") a0,
            inlateout("a1") a1,
            inlateout("a2") a2 => _,
            inlateout("a3") a3 => _,
            inlateout("a4") a4 => _,
            inlateout("a5") a5 => _,
            inlateout("a6") a6 => _,
            inlateout("a7") a7 => _,
        )
    };

    let result = a0;

    if result == SyscallResult::Scalar2 as usize && a1 == knob as usize {
        Ok(a2)
    } else if result == SyscallResult::Scalar5 as usize && a1 == knob as usize {
        Ok(a1)
    } else if result == SyscallResult::Error as usize {
        Err(a1.into())
    } else {
        Err(Error::InternalError)
    }
}
