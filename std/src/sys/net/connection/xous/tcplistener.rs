use core::convert::TryInto;
use core::sync::atomic::{Atomic, AtomicBool, AtomicU16, AtomicUsize, Ordering};

use super::*;
use crate::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use crate::os::xous::services;
use crate::sync::Arc;
use crate::sys::net::connection::each_addr;
use crate::{fmt, io};

macro_rules! unimpl {
    () => {
        return Err(io::const_error!(
            io::ErrorKind::Unsupported,
            "this function is not yet implemented",
        ));
    };
}

#[derive(Clone)]
pub struct TcpListener {
    fd: Arc<Atomic<u16>>,
    local: SocketAddr,
    handle_count: Arc<Atomic<usize>>,
    nonblocking: Arc<Atomic<bool>>,
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        return each_addr(addr, inner);

        fn inner(addr: &SocketAddr) -> io::Result<TcpListener> {
            let mut addr = *addr;
            let fd = TcpListener::bind_inner(&mut addr)?;
            Ok(TcpListener {
                fd: Arc::new(AtomicU16::new(fd)),
                local: addr,
                handle_count: Arc::new(AtomicUsize::new(1)),
                nonblocking: Arc::new(AtomicBool::new(false)),
            })
        }
    }

    /// 这会返回一个 Listener 的原始 fd，以便 accept 例程在 Listener 的
    /// handle 被转换成一个 TcpStream 对象之后，也能用它来补充（重建）
    /// 该 Listener 对象。
    fn bind_inner(addr: &mut SocketAddr) -> io::Result<u16> {
        // 构造请求
        let mut connect_request = ConnectRequest { raw: [0u8; 4096] };

        // 序列化 StdUdpBind 结构体。这里之所以“手动”进行，是因为我们不想让
        // 某个自动 serdes（如 bincode 或 rkyv）crate 成为 Xous 的依赖。
        let port_bytes = addr.port().to_le_bytes();
        connect_request.raw[0] = port_bytes[0];
        connect_request.raw[1] = port_bytes[1];
        match addr.ip() {
            IpAddr::V4(addr) => {
                connect_request.raw[2] = 4;
                for (dest, src) in connect_request.raw[3..].iter_mut().zip(addr.octets()) {
                    *dest = src;
                }
            }
            IpAddr::V6(addr) => {
                connect_request.raw[2] = 6;
                for (dest, src) in connect_request.raw[3..].iter_mut().zip(addr.octets()) {
                    *dest = src;
                }
            }
        }

        let Ok((_, valid)) = crate::os::xous::ffi::lend_mut(
            services::net_server(),
            services::NetLendMut::StdTcpListen.into(),
            &mut connect_request.raw,
            0,
            4096,
        ) else {
            return Err(io::const_error!(io::ErrorKind::InvalidInput, "invalid response"));
        };

        // 成功时前四个字节应当为零，出错时则会是非零值。
        let response = connect_request.raw;
        if response[0] != 0 || valid == 0 {
            let errcode = response[1];
            if errcode == NetError::SocketInUse as u8 {
                return Err(io::const_error!(io::ErrorKind::ResourceBusy, "socket in use"));
            } else if errcode == NetError::Invalid as u8 {
                return Err(io::const_error!(io::ErrorKind::AddrNotAvailable, "invalid address"));
            } else if errcode == NetError::LibraryError as u8 {
                return Err(io::const_error!(io::ErrorKind::Other, "library error"));
            } else {
                return Err(io::const_error!(
                    io::ErrorKind::Other,
                    "unable to connect or internal error",
                ));
            }
        }
        let fd = response[1] as usize;
        if addr.port() == 0 {
            // 奇怪的是，这是一个有效的端口，它的含义是“给我一个有效的端口，
            // 具体是什么由你决定”
            let assigned_port = u16::from_le_bytes(response[2..4].try_into().unwrap());
            addr.set_port(assigned_port);
        }
        // println!("TcpListening with file handle of {}\r\n", fd);
        Ok(fd.try_into().unwrap())
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let mut receive_request = ReceiveData { raw: [0u8; 4096] };

        if self.nonblocking.load(Ordering::Relaxed) {
            // 非阻塞
            receive_request.raw[0] = 0;
        } else {
            // 阻塞
            receive_request.raw[0] = 1;
        }

        if let Ok((_offset, _valid)) = crate::os::xous::ffi::lend_mut(
            services::net_server(),
            services::NetLendMut::StdTcpAccept(self.fd.load(Ordering::Relaxed)).into(),
            &mut receive_request.raw,
            0,
            0,
        ) {
            if receive_request.raw[0] != 0 {
                // 错误情形
                if receive_request.raw[1] == NetError::TimedOut as u8 {
                    return Err(io::const_error!(io::ErrorKind::TimedOut, "accept timed out"));
                } else if receive_request.raw[1] == NetError::WouldBlock as u8 {
                    return Err(io::const_error!(io::ErrorKind::WouldBlock, "accept would block"));
                } else if receive_request.raw[1] == NetError::LibraryError as u8 {
                    return Err(io::const_error!(io::ErrorKind::Other, "library error"));
                } else {
                    return Err(io::const_error!(io::ErrorKind::Other, "library error"));
                }
            } else {
                // accept 成功
                let rr = &receive_request.raw;
                let stream_fd = u16::from_le_bytes(rr[1..3].try_into().unwrap());
                let port = u16::from_le_bytes(rr[20..22].try_into().unwrap());
                let addr = if rr[3] == 4 {
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(rr[4], rr[5], rr[6], rr[7])), port)
                } else if rr[3] == 6 {
                    SocketAddr::new(
                        IpAddr::V6(Ipv6Addr::new(
                            u16::from_be_bytes(rr[4..6].try_into().unwrap()),
                            u16::from_be_bytes(rr[6..8].try_into().unwrap()),
                            u16::from_be_bytes(rr[8..10].try_into().unwrap()),
                            u16::from_be_bytes(rr[10..12].try_into().unwrap()),
                            u16::from_be_bytes(rr[12..14].try_into().unwrap()),
                            u16::from_be_bytes(rr[14..16].try_into().unwrap()),
                            u16::from_be_bytes(rr[16..18].try_into().unwrap()),
                            u16::from_be_bytes(rr[18..20].try_into().unwrap()),
                        )),
                        port,
                    )
                } else {
                    return Err(io::const_error!(io::ErrorKind::Other, "library error"));
                };

                // 补充（重建）该 listener
                let mut local_copy = self.local.clone(); // 到此时 port 已非 0，但该方法签名需要一个 mut
                let new_fd = TcpListener::bind_inner(&mut local_copy)?;
                self.fd.store(new_fd, Ordering::Relaxed);

                // 现在返回一个由旧 stream 的 fd 转换而来的 stream
                Ok((TcpStream::from_listener(stream_fd, self.local.port(), port, addr), addr))
            }
        } else {
            Err(io::const_error!(io::ErrorKind::InvalidInput, "unable to accept"))
        }
    }

    pub fn duplicate(&self) -> io::Result<TcpListener> {
        self.handle_count.fetch_add(1, Ordering::Relaxed);
        Ok(self.clone())
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        if ttl > 255 {
            return Err(io::const_error!(io::ErrorKind::InvalidInput, "TTL must be less than 256"));
        }
        crate::os::xous::ffi::blocking_scalar(
            services::net_server(),
            services::NetBlockingScalar::StdSetTtlTcp(self.fd.load(Ordering::Relaxed), ttl).into(),
        )
        .or(Err(io::const_error!(io::ErrorKind::InvalidInput, "unexpected return value")))
        .map(|_| ())
    }

    pub fn ttl(&self) -> io::Result<u32> {
        Ok(crate::os::xous::ffi::blocking_scalar(
            services::net_server(),
            services::NetBlockingScalar::StdGetTtlTcp(self.fd.load(Ordering::Relaxed)).into(),
        )
        .or(Err(io::const_error!(io::ErrorKind::InvalidInput, "unexpected return value")))
        .map(|res| res[0] as _)?)
    }

    pub fn set_only_v6(&self, _: bool) -> io::Result<()> {
        unimpl!();
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        unimpl!();
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        // 这个调用在我们的平台上没有意义，但至少在它被使用时我们可以不 panic。
        Ok(None)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.nonblocking.store(nonblocking, Ordering::Relaxed);
        Ok(())
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TCP listening on {:?}", self.local)
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        if self.handle_count.fetch_sub(1, Ordering::Relaxed) == 1 {
            // 只有当我们是最后一个克隆体时才进行 drop
            crate::os::xous::ffi::blocking_scalar(
                services::net_server(),
                crate::os::xous::services::NetBlockingScalar::StdTcpClose(
                    self.fd.load(Ordering::Relaxed),
                )
                .into(),
            )
            .unwrap();
        }
    }
}
