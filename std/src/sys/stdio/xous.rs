#[expect(dead_code)]
#[path = "unsupported.rs"]
mod unsupported_stdio;

use crate::io;
use crate::os::xous::ffi::{Connection, lend, try_lend, try_scalar};
use crate::os::xous::services::{LogLend, LogScalar, log_server, try_connect};

pub type Stdin = unsupported_stdio::Stdin;
pub struct Stdout;
pub struct Stderr;

impl Stdout {
    pub const fn new() -> Stdout {
        Stdout
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        #[repr(C, align(4096))]
        struct LendBuffer([u8; 4096]);
        let mut lend_buffer = LendBuffer([0u8; 4096]);
        let connection = log_server();
        for chunk in buf.chunks(lend_buffer.0.len()) {
            for (dest, src) in lend_buffer.0.iter_mut().zip(chunk) {
                *dest = *src;
            }
            lend(connection, LogLend::StandardOutput.into(), &lend_buffer.0, 0, chunk.len())
                .unwrap();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Stderr {
    pub const fn new() -> Stderr {
        Stderr
    }
}

impl io::Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        #[repr(C, align(4096))]
        struct LendBuffer([u8; 4096]);
        let mut lend_buffer = LendBuffer([0u8; 4096]);
        let connection = log_server();
        for chunk in buf.chunks(lend_buffer.0.len()) {
            for (dest, src) in lend_buffer.0.iter_mut().zip(chunk) {
                *dest = *src;
            }
            lend(connection, LogLend::StandardError.into(), &lend_buffer.0, 0, chunk.len())
                .unwrap();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub const STDIN_BUF_SIZE: usize = unsupported_stdio::STDIN_BUF_SIZE;

pub fn is_ebadf(_err: &io::Error) -> bool {
    true
}

#[derive(Copy, Clone)]
pub struct PanicWriter {
    log: Connection,
    gfx: Option<Connection>,
}

impl io::Write for PanicWriter {
    fn write(&mut self, s: &[u8]) -> core::result::Result<usize, io::Error> {
        for c in s.chunks(size_of::<usize>() * 4) {
            // 文本被分组为 4 个 `usize` 字（words）。其 id 为 1100 加上
            // 本条消息中的字符数。
            // 忽略错误，因为我们已经在 panic 中了。
            try_scalar(self.log, LogScalar::AppendPanicMessage(&c).into()).ok();
        }

        // 仅当我们能够获取到与图形 panic 处理器的连接时，才把文本序列化发送给它。
        // 文本长度编码在 `valid` 字段中，数据本身则放在缓冲区里。通常需要若干条消息
        // 才能完整传输整条 panic 消息。
        if let Some(gfx) = self.gfx {
            #[repr(C, align(4096))]
            struct Request([u8; 4096]);
            let mut request = Request([0u8; 4096]);
            for (&s, d) in s.iter().zip(request.0.iter_mut()) {
                *d = s;
            }
            try_lend(gfx, 0 /* AppendPanicText */, &request.0, 0, s.len()).ok();
        }
        Ok(s.len())
    }

    // 测试表明，它似乎不会在一次 panic 打印结束时被可靠地调用，
    // 因此我们不能依赖它来做诸如触发一次图形更新之类的事情。
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn panic_output() -> Option<impl io::Write> {
    // 一般来说这不会失败，因为每个服务器都已经连接过了，因此这很可能会成功。
    let log = log_server();

    // 发送 "We're panicking"（我们正在 panic）消息（1000）。
    try_scalar(log, LogScalar::BeginPanic.into()).ok();

    // 在连接表已满、或图形服务器未运行的情况下，这会失败。
    // 大多数服务器都还没有建立这个连接。
    let gfx = try_connect("panic-to-screen!");

    Some(PanicWriter { log, gfx })
}
