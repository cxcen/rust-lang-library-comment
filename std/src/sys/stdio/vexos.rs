use crate::io;

pub struct Stdin;
pub struct Stdout;
pub type Stderr = Stdout;

pub const STDIO_CHANNEL: u32 = 1;

impl Stdin {
    pub const fn new() -> Stdin {
        Stdin
    }
}

impl io::Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut count = 0;

        for out_byte in buf.iter_mut() {
            let byte = unsafe { vex_sdk::vexSerialReadChar(STDIO_CHANNEL) };
            if byte < 0 {
                break;
            }

            *out_byte = byte as u8;
            count += 1;
        }

        Ok(count)
    }
}

impl Stdout {
    pub const fn new() -> Stdout {
        Stdout
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut written = 0;

        // HACK: VEXos 为串口写入维护一个内部环形缓冲区（ringbuffer），它由 `vexTasksRun`
        // 大约每毫秒刷新（flush）到 USB1 一次。对于大于 2048 字节的写入，我们必须先阻塞
        // 直到该缓冲区被刷新到 USB1，然后再写入 `buf` 的剩余部分。
        //
        // 对于一个 `write` 实现而言，这相当不标准，但它避免了在使用诸如 `print!` 之类的宏
        // 一次性向 stdout 写入大量数据（buf.len() > 2048）时必然发生的递归 panic。
        for chunk in buf.chunks(STDOUT_BUF_SIZE) {
            if unsafe { vex_sdk::vexSerialWriteFree(STDIO_CHANNEL) as usize } < chunk.len() {
                self.flush().unwrap();
            }

            let count: usize = unsafe {
                vex_sdk::vexSerialWriteBuffer(STDIO_CHANNEL, chunk.as_ptr(), chunk.len() as u32)
            }
            .try_into()
            .map_err(|_| {
                io::const_error!(io::ErrorKind::Uncategorized, "internal write error occurred")
            })?;

            written += count;

            // 这是一项合理性检查（sanity check），用于确保我们不会出现非连续的
            // 缓冲区写入。例如，某个 chunk 只被部分写入，而我们却继续尝试写入
            // 剩余的 chunk。
            //
            // 实际上，这种情况应该基本不会发生，因为前面的 flush 确保了 FIFO 中
            // 有足够的空间，可以把整个 chunk 写入 vexSerialWriteBuffer。
            if count != chunk.len() {
                break;
            }
        }

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        // 这可能会阻塞最多一毫秒。
        unsafe {
            while (vex_sdk::vexSerialWriteFree(STDIO_CHANNEL) as usize) != STDOUT_BUF_SIZE {
                vex_sdk::vexTasksRun();
            }
        }

        Ok(())
    }
}

pub const STDIN_BUF_SIZE: usize = 4096;
pub const STDOUT_BUF_SIZE: usize = 2048;

pub fn is_ebadf(_err: &io::Error) -> bool {
    false
}

pub fn panic_output() -> Option<impl io::Write> {
    Some(Stdout::new())
}
