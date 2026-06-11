//! 由 risc0-zkvm-platform 导出的符号的 ABI 定义。

// 在此处包含这些定义，以免我们必须依赖 risc0-zkvm-platform。
//
// FIXME: 我们是否应该把这部分移动到 "libc" crate？看起来其他架构
// 都把很多这类东西放在那里。但目前还没有 risc0 版本的 libc crate fork，
// 因此我们要么得 fork 它，要么得把它上游化（upstream）。

#![allow(dead_code)]
pub const DIGEST_WORDS: usize = 8;

/// 用于配合 sys_read 和 sys_write 使用的标准 IO 文件描述符。
pub mod fileno {
    pub const STDIN: u32 = 0;
    pub const STDOUT: u32 = 1;
    pub const STDERR: u32 = 2;
    pub const JOURNAL: u32 = 3;
}

unsafe extern "C" {
    // 对 risc0-zkvm-platform 所提供系统调用的封装：
    pub fn sys_halt();
    pub fn sys_output(output_id: u32, output_value: u32);
    pub fn sys_sha_compress(
        out_state: *mut [u32; DIGEST_WORDS],
        in_state: *const [u32; DIGEST_WORDS],
        block1_ptr: *const [u32; DIGEST_WORDS],
        block2_ptr: *const [u32; DIGEST_WORDS],
    );
    pub fn sys_sha_buffer(
        out_state: *mut [u32; DIGEST_WORDS],
        in_state: *const [u32; DIGEST_WORDS],
        buf: *const u8,
        count: u32,
    );
    pub fn sys_rand(recv_buf: *mut u32, words: usize);
    pub fn sys_panic(msg_ptr: *const u8, len: usize) -> !;
    pub fn sys_log(msg_ptr: *const u8, len: usize);
    pub fn sys_cycle_count() -> usize;
    pub fn sys_read(fd: u32, recv_buf: *mut u8, nrequested: usize) -> usize;
    pub fn sys_write(fd: u32, write_buf: *const u8, nbytes: usize);
    pub fn sys_getenv(
        recv_buf: *mut u32,
        words: usize,
        varname: *const u8,
        varname_len: usize,
    ) -> usize;
    pub fn sys_argc() -> usize;
    pub fn sys_argv(out_words: *mut u32, out_nwords: usize, arg_index: usize) -> usize;

    // 从全局 HEAP 中分配内存。
    pub fn sys_alloc_words(nwords: usize) -> *mut u32;
    pub fn sys_alloc_aligned(nwords: usize, align: usize) -> *mut u8;
}
