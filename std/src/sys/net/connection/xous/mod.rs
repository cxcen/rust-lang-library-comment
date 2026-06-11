mod dns;

mod tcpstream;
pub use tcpstream::*;

mod tcplistener;
pub use tcplistener::*;

mod udp;
pub use udp::*;

// 这个结构体需要与 net/src/api.rs 中的内容保持同步
#[repr(C)]
#[derive(Debug)]
enum NetError {
    // Ok = 0,
    Unaddressable = 1,
    SocketInUse = 2,
    // AccessDenied = 3,
    Invalid = 4,
    // Finished = 5,
    LibraryError = 6,
    // AlreadyUsed = 7,
    TimedOut = 8,
    WouldBlock = 9,
}

#[repr(C, align(4096))]
struct ConnectRequest {
    raw: [u8; 4096],
}

#[repr(C, align(4096))]
struct SendData {
    raw: [u8; 4096],
}

#[repr(C, align(4096))]
pub struct ReceiveData {
    raw: [u8; 4096],
}

#[repr(C, align(4096))]
pub struct GetAddress {
    raw: [u8; 4096],
}

pub use dns::lookup_host;
