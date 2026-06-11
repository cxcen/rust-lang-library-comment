use core::sync::atomic::{Atomic, AtomicU32, Ordering};

use crate::os::xous::ffi::Connection;

mod dns;
pub(crate) use dns::*;

mod log;
pub(crate) use log::*;

mod net;
pub(crate) use net::*;

mod systime;
pub(crate) use systime::*;

mod ticktimer;
pub(crate) use ticktimer::*;

mod ns {
    const NAME_MAX_LENGTH: usize = 64;
    use crate::os::xous::ffi::{Connection, lend_mut};
    // 通过把它标为 repr(C)，本 struct 的内存布局变得有明确定义，不会再随意变动。
    // 通过把它标为 `align(4096)`，我们规定它将按页对齐（page-aligned），
    // 这意味着它可以在进程之间发送。我们确保把整个 struct 填充满，
    // 以免有内存被泄漏给名字服务器（name server）。
    #[repr(C, align(4096))]
    struct ConnectRequest {
        data: [u8; 4096],
    }

    impl ConnectRequest {
        pub fn new(name: &str) -> Self {
            let mut cr = ConnectRequest { data: [0u8; 4096] };
            let name_bytes = name.as_bytes();

            // 把该字符串复制到我们的后备存储（backing store）中。
            for (&src_byte, dest_byte) in name_bytes.iter().zip(&mut cr.data[0..NAME_MAX_LENGTH]) {
                *dest_byte = src_byte;
            }

            // 把字符串长度设置为所传入 String 的长度与最大可能长度二者中较小的那个。
            for (&src_byte, dest_byte) in (name.len().min(NAME_MAX_LENGTH) as u32)
                .to_le_bytes()
                .iter()
                .zip(&mut cr.data[NAME_MAX_LENGTH..])
            {
                *dest_byte = src_byte;
            }
            cr
        }
    }

    pub fn connect_with_name_impl(name: &str, blocking: bool) -> Option<Connection> {
        let mut request = ConnectRequest::new(name);
        let opcode = if blocking {
            6 /* BlockingConnect */
        } else {
            7 /* TryConnect */
        };
        let cid = if blocking { super::name_server() } else { super::try_name_server()? };

        lend_mut(cid, opcode, &mut request.data, 0, name.len().min(NAME_MAX_LENGTH))
            .expect("unable to perform lookup");

        // 从名字服务器读回结果码（result code）
        let result = u32::from_le_bytes(request.data[0..4].try_into().unwrap());
        if result == 0 {
            // 如果结果成功，那么 CID 存放在接下来的 4 个字节中
            Some(u32::from_le_bytes(request.data[4..8].try_into().unwrap()).into())
        } else {
            None
        }
    }

    pub fn connect_with_name(name: &str) -> Option<Connection> {
        connect_with_name_impl(name, true)
    }

    pub fn try_connect_with_name(name: &str) -> Option<Connection> {
        connect_with_name_impl(name, false)
    }
}

/// 尝试按名字（name）连接到一个服务器。如果该服务器不存在，本调用将一直阻塞，直到该服务器
/// 被创建出来为止。
///
/// 注意这与按地址（address）连接到服务器不同。服务器地址总是 16 字节长，而服务器名字则是
/// 长度任意、最长 64 字节的字符串。
#[stable(feature = "rust1", since = "1.0.0")]
pub fn connect(name: &str) -> Option<Connection> {
    ns::connect_with_name(name)
}

/// 尝试按名字（name）连接到一个服务器。如果该服务器不存在，本调用将立即返回 `None`。
///
/// 注意这与按地址（address）连接到服务器不同。服务器地址总是 16 字节长，而服务器名字则是
/// 长度任意的字符串。
#[stable(feature = "rust1", since = "1.0.0")]
pub fn try_connect(name: &str) -> Option<Connection> {
    ns::try_connect_with_name(name)
}

static NAME_SERVER_CONNECTION: Atomic<u32> = AtomicU32::new(0);

/// 返回一个到名字服务器（name server）的 `Connection`。如果名字服务器尚未启动，则本调用将
/// 一直阻塞，直到名字服务器启动为止。该 `Connection` 会在一个进程内的所有连接之间共享，
/// 因此多次调用本函数是安全的。
pub(crate) fn name_server() -> Connection {
    let cid = NAME_SERVER_CONNECTION.load(Ordering::Relaxed);
    if cid != 0 {
        return cid.into();
    }

    let cid = crate::os::xous::ffi::connect("xous-name-server".try_into().unwrap()).unwrap();
    NAME_SERVER_CONNECTION.store(cid.into(), Ordering::Relaxed);
    cid
}

fn try_name_server() -> Option<Connection> {
    let cid = NAME_SERVER_CONNECTION.load(Ordering::Relaxed);
    if cid != 0 {
        return Some(cid.into());
    }

    if let Ok(Some(cid)) = crate::os::xous::ffi::try_connect("xous-name-server".try_into().unwrap())
    {
        NAME_SERVER_CONNECTION.store(cid.into(), Ordering::Relaxed);
        Some(cid)
    } else {
        None
    }
}
