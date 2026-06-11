//! 针对通用 I/O 基础类型的 Unix 特有扩展。
//!
//! 正如裸指针一样，裸文件描述符指向具有动态生命周期的资源，如果它们的存活时间
//! 超过了其所指向的资源，就可能变成悬垂（dangle）；如果它们由无效值构造出来，
//! 则可能是伪造的（forged）。
//!
//! 本模块提供了三种用于表示文件描述符的类型，它们具有不同的所有权属性：raw（裸）、
//! borrowed（借用）和 owned（拥有所有权），这与用于表示指针的那些类型相对应。
//! 这些类型反映了 Unix 上的 [I/O 安全][io-safety]概念。
//!
//! | 类型               | 类比于       |
//! | ------------------ | ------------ |
//! | [`RawFd`]          | `*const _`   |
//! | [`BorrowedFd<'a>`] | `&'a Arc<_>` |
//! | [`OwnedFd`]        | `Arc<_>`     |
//!
//! 与裸指针一样，`RawFd` 值是原生（primitive）值。在新代码中，应当将对它们进行 I/O
//! 视为 unsafe（类比于解引用裸指针）。Rust 并非一直都给出这一指导，因此 Rust 生态中
//! 现有的代码往往没有把 `RawFd` 的使用标记为 unsafe。
//! 鼓励各类库进行迁移，方式有二：要么为那些会解引用 `RawFd` 值的 API 加上 `unsafe`，
//! 要么改用 `BorrowedFd` 或 `OwnedFd`。
//!
//! 对借用/拥有所有权的文件描述符使用 `Arc` 来类比可能令人意外。Unix 文件描述符不过是
//! 对一种被称为“打开文件描述（open file descriptions）”的内核内部对象的引用，而同一个
//! 打开文件描述可以被多个文件描述符引用（例如使用了 `dup`）。诸如文件内偏移量这样的状态
//! 会被所有引用同一打开文件描述的文件描述符所共享，且内核内部会做引用计数，只有当所有
//! 引用某资源的文件描述符都被关闭后，才会关闭底层资源。这正是为什么 `Arc`（而非 `Box`）
//! 才是“拥有所有权”的文件描述符在 Rust 中最贴切的类比。
//!
//! 与引用一样，`BorrowedFd` 值与某个生命周期绑定，以确保它们的存活时间不会超过其所指向的
//! 资源。它们的使用是安全（safe）的。`BorrowedFd` 值可用于为以下系统调用之外的任意系统调用
//! 提供安全访问的 API 中：
//!
//!  - `close`，因为这会在文件描述符的生命周期尚未结束的情况下，结束该资源的动态生命周期。
//!    （等价地说：一个 `&Arc<_>` 不能被 `drop`。）
//!
//!  - `dup2`/`dup3` 的第二个实参，因为该实参会被关闭并被赋予一个新的资源，这可能会破坏
//!    其他使用该文件描述符的代码所做的假设。
//!
//! `BorrowedFd` 值可用于为 `dup` 系统调用提供安全访问的 API 中，因此使用 `OwnedFd` 的代码
//! 不能假定自己对底层的打开文件描述拥有独占访问权。（等价地说：`&Arc` 可用于为 `clone`
//! 提供安全访问的 API 中，因此使用 `Arc` 的代码不能假定其引用计数为 1。）
//!
//! `BorrowedFd` 值也可用于 `mmap`，因为 `mmap` 使用所提供文件描述符的方式与 `dup` 类似，
//! 并不要求传给它的 `BorrowedFd` 在所产生映射的整个生命周期内都保持存活。话虽如此，`mmap`
//! 因其他原因而是 unsafe 的：它操作裸指针，且如果底层存储被改动，可能产生未定义行为。
//! 这种改动可能来自其他进程，也可能来自同一进程——如果该 API 提供了 `BorrowedFd` 访问，
//! 因为如前所述，`BorrowedFd` 值可用于为任意系统调用提供安全访问的 API 中。因此，使用 `mmap`
//! 并对外提供安全 API 的代码必须全权负责确保安全的 Rust 代码无法借由它引发未定义行为。
//!
//! 与 `Arc` 一样，`OwnedFd` 值在概念上拥有对其所指向资源的一个引用，并在被 drop 时
//!（通过调用 `close`）将引用计数减一。当引用计数归零时，内核将释放底层的打开文件描述。
//!
//! 关于 I/O 安全的一般性解释，参见 [`io` 模块文档][io-safety]。
//!
//! ## `/proc/self/mem` 及类似的操作系统特性
//!
//! 某些平台具有特殊文件，例如 `/proc/self/mem`，它们提供对进程内存的读写访问。
//! 这类读写发生在 Rust 编译器的控制之外，因此它们并不维护 Rust 的内存安全保证。
//!
//! 这并不意味着所有可能允许打开 `/proc/self/mem` 并对其读写的 API 都必须是 `unsafe` 的。
//! Rust 的安全保证只覆盖程序自身能做什么，而不覆盖程序外部的实体能对它做什么。
//! `/proc/self/mem` 被视为这样一个外部实体，与之同列的还有 `/proc/self/fd/*`、调试接口，
//! 以及对硬件拥有物理访问权的人。即便在程序正控制着该外部实体的情况下，这一点依然成立。
//!
//! 如果你希望全面防止程序伸手触发外部实体反过来侵入并破坏内存安全，就有必要使用*沙箱
//!（sandboxing）*，而这超出了 `std` 的范畴。
//!
//! [`BorrowedFd<'a>`]: crate::os::unix::io::BorrowedFd
//! [io-safety]: crate::io#io-safety

#![stable(feature = "rust1", since = "1.0.0")]

use crate::io::{self, Stderr, StderrLock, Stdin, StdinLock, Stdout, StdoutLock, Write};
#[stable(feature = "rust1", since = "1.0.0")]
pub use crate::os::fd::*;
#[allow(unused_imports)] // 并非在所有目标平台上都会用到
use crate::sys::cvt;

// 本模块的测试
#[cfg(test)]
mod tests;

#[unstable(feature = "stdio_swap", issue = "150667")]
pub trait StdioExt: crate::sealed::Sealed {
    /// 将标准 I/O 文件描述符重定向，使其指向 `fd` 底层的打开文件描述。
    ///
    /// Rust std::io 的写缓冲区（如果有的话）会被刷新（flush），但其他运行时
    ///（例如 C stdio）或那些已获取该文件描述符克隆副本的库不会感知到这一变更。
    ///
    /// # 平台特定行为
    ///
    /// 该方法[当前][currently]通过以下方式实现
    ///
    /// - 在 wasip1 上使用 `fd_renumber`
    /// - 在大多数 unix 上使用 `dup2`
    ///
    /// [currently]: crate::io#platform-specific-behavior
    ///
    /// ```
    /// #![feature(stdio_swap)]
    /// use std::io::{self, Read, Write};
    /// use std::os::unix::io::StdioExt;
    ///
    /// fn main() -> io::Result<()> {
    ///    let (reader, mut writer) = io::pipe()?;
    ///    let mut stdin = io::stdin();
    ///    stdin.set_fd(reader)?;
    ///    writer.write_all(b"Hello, world!")?;
    ///    let mut buffer = vec![0; 13];
    ///    assert_eq!(stdin.read(&mut buffer)?, 13);
    ///    assert_eq!(&buffer, b"Hello, world!");
    ///    Ok(())
    /// }
    /// ```
    fn set_fd<T: Into<OwnedFd>>(&mut self, fd: T) -> io::Result<()>;

    /// 重定向标准 I/O 文件描述符，并返回一个由先前的打开文件描述支撑的新 `OwnedFd`。
    ///
    /// 详见 [`set_fd()`]。
    ///
    /// [`set_fd()`]: StdioExt::set_fd
    fn replace_fd<T: Into<OwnedFd>>(&mut self, replace_with: T) -> io::Result<OwnedFd>;

    /// 将标准 I/O 文件描述符重定向到空设备（`/dev/null`），
    /// 并返回一个由先前的打开文件描述支撑的新 `OwnedFd`。
    ///
    /// 通过标准 I/O 传递结构化数据的程序可以在 `main()` 早期使用它来提取这些 fd，
    /// 将它们当作其他 IO 类型（`File`、`UnixStream` 等）处理，施加自定义缓冲，
    /// 或避免受到程序后续使用标准 I/O 的干扰。
    ///
    /// 更多细节参见 [`set_fd()`]。
    ///
    /// [`set_fd()`]: StdioExt::set_fd
    fn take_fd(&mut self) -> io::Result<OwnedFd>;
}

macro io_ext_impl($stdio_ty:ty, $stdio_lock_ty:ty, $writer:literal) {
    #[unstable(feature = "stdio_swap", issue = "150667")]
    impl StdioExt for $stdio_ty {
        fn set_fd<T: Into<OwnedFd>>(&mut self, fd: T) -> io::Result<()> {
            self.lock().set_fd(fd)
        }

        fn take_fd(&mut self) -> io::Result<OwnedFd> {
            self.lock().take_fd()
        }

        fn replace_fd<T: Into<OwnedFd>>(&mut self, replace_with: T) -> io::Result<OwnedFd> {
            self.lock().replace_fd(replace_with)
        }
    }

    #[unstable(feature = "stdio_swap", issue = "150667")]
    impl StdioExt for $stdio_lock_ty {
        fn set_fd<T: Into<OwnedFd>>(&mut self, fd: T) -> io::Result<()> {
            #[cfg($writer)]
            self.flush()?;
            replace_stdio_fd(self.as_fd(), fd.into())
        }

        fn take_fd(&mut self) -> io::Result<OwnedFd> {
            let null = null_fd()?;
            let cloned = self.as_fd().try_clone_to_owned()?;
            self.set_fd(null)?;
            Ok(cloned)
        }

        fn replace_fd<T: Into<OwnedFd>>(&mut self, replace_with: T) -> io::Result<OwnedFd> {
            let cloned = self.as_fd().try_clone_to_owned()?;
            self.set_fd(replace_with)?;
            Ok(cloned)
        }
    }
}

io_ext_impl!(Stdout, StdoutLock<'_>, true);
io_ext_impl!(Stdin, StdinLock<'_>, false);
io_ext_impl!(Stderr, StderrLock<'_>, true);

fn null_fd() -> io::Result<OwnedFd> {
    let null_dev = crate::fs::OpenOptions::new().read(true).write(true).open("/dev/null")?;
    Ok(null_dev.into())
}

/// 用来自 `other` 的文件描述符替换底层文件描述符。
/// 不设置 CLOEXEC。
fn replace_stdio_fd(this: BorrowedFd<'_>, other: OwnedFd) -> io::Result<()> {
    cfg_select! {
        all(target_os = "wasi", target_env = "p1") => {
            cvt(unsafe { libc::__wasilibc_fd_renumber(other.as_raw_fd(), this.as_raw_fd()) }).map(|_| ())
        }
        not(any(
            target_arch = "wasm32",
            target_os = "hermit",
            target_os = "trusty",
            target_os = "motor"
        )) => {
            cvt(unsafe {libc::dup2(other.as_raw_fd(), this.as_raw_fd())}).map(|_| ())
        }
        _ => {
            let _ = (this, other);
            Err(io::Error::UNSUPPORTED_PLATFORM)
        }
    }
}
