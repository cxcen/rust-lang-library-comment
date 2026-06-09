use crate::fmt::{self, Display};
use crate::panic::Location;

/// 提供 panic 现场信息的结构体。
///
/// `PanicInfo` 会传给由 `#[panic_handler]` 定义的 panic handler。它描述的是 `core`
/// 层面的 panic：包含格式化消息、发生位置、以及该 panic 是否允许 unwind。`no_std`
/// 程序通常只能通过这个结构决定如何终止或停机。
///
/// `std` 中 panic hook 机制使用的类型请参见 [`std::panic::PanicHookInfo`]；它和这里的
/// `PanicInfo` 角色相近，但用于 `std` 的 hook/runtime 路径。
///
/// [`std::panic::PanicHookInfo`]: ../../std/panic/struct.PanicHookInfo.html
#[lang = "panic_info"]
#[stable(feature = "panic_hooks", since = "1.10.0")]
#[derive(Debug)]
pub struct PanicInfo<'a> {
    message: &'a fmt::Arguments<'a>,
    location: &'a Location<'a>,
    can_unwind: bool,
    force_no_backtrace: bool,
}

/// 传给 `panic!()` 宏的消息。
///
/// 这个类型的 [`Display`] 实现会把 `panic!()` 宏收到的格式化参数一起格式化出来。
/// 因此 panic handler 或 hook 可以延迟到真正输出时再把消息写入目标缓冲区。
///
/// 参见 [`PanicInfo::message`]。
#[stable(feature = "panic_info_message", since = "1.81.0")]
pub struct PanicMessage<'a> {
    message: &'a fmt::Arguments<'a>,
}

impl<'a> PanicInfo<'a> {
    #[inline]
    pub(crate) fn new(
        message: &'a fmt::Arguments<'a>,
        location: &'a Location<'a>,
        can_unwind: bool,
        force_no_backtrace: bool,
    ) -> Self {
        PanicInfo { location, message, can_unwind, force_no_backtrace }
    }

    /// 返回传给 `panic!` 宏的消息。
    ///
    /// # 示例
    ///
    /// 此方法返回的类型实现了 `Display`，因此可以直接传给 [`write!()`] 和类似宏。对
    /// `no_std` panic handler 来说，这通常是把 panic 消息写入串口、调试输出或板级日志的入口。
    ///
    /// [`write!()`]: core::write
    ///
    /// ```ignore (no_std)
    /// #[panic_handler]
    /// fn panic_handler(panic_info: &PanicInfo<'_>) -> ! {
    ///     write!(DEBUG_OUTPUT, "panicked: {}", panic_info.message());
    ///     loop {}
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "panic_info_message", since = "1.81.0")]
    pub fn message(&self) -> PanicMessage<'_> {
        PanicMessage { message: self.message }
    }

    /// 在可用时，返回 panic 起源位置的信息。
    ///
    /// 当前实现总是返回 [`Some`]，但未来版本可能改变这一点。调用方不应把存在位置信息写成
    /// 永久不变量；panic hook 和 panic handler 都应保留处理 [`None`] 的分支。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred in file '{}' at line {}",
    ///             location.file(),
    ///             location.line(),
    ///         );
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn location(&self) -> Option<&Location<'_>> {
        // NOTE: 如果这里将来可能返回 None，需要同步处理
        // std::panicking::default_hook 和 core::panicking::panic_fmt 中的对应分支。
        Some(&self.location)
    }

    /// 返回与 panic 关联的载荷。
    ///
    /// 对 `core::panic::PanicInfo` 这个类型来说，此方法永远不会返回有用内容。它只因与
    /// [`std::panic::PanicHookInfo`] 的兼容性而存在；二者曾经是同一个类型。
    ///
    /// 参见 [`std::panic::PanicHookInfo::payload`]。
    ///
    /// [`std::panic::PanicHookInfo`]: ../../std/panic/struct.PanicHookInfo.html
    /// [`std::panic::PanicHookInfo::payload`]: ../../std/panic/struct.PanicHookInfo.html#method.payload
    #[deprecated(since = "1.81.0", note = "this never returns anything useful")]
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    #[allow(deprecated, deprecated_in_future)]
    pub fn payload(&self) -> &(dyn crate::any::Any + Send) {
        struct NoPayload;
        &NoPayload
    }

    /// 返回 panic handler 是否允许从 panic 发生点开始 unwind 栈。
    ///
    /// 大多数 panic 都会返回 `true`。例外包括试图从 `Drop` 实现中继续 unwind 的 panic，
    /// 以及从 ABI 不支持 unwinding 的函数中向外 unwind 的 panic。
    ///
    /// 即使此函数返回 `false`，panic handler 继续 unwind 在内存安全意义上仍是安全的；但
    /// 结果只是再次调用 panic handler，通常最终走向 abort。这一标志主要用于区分 unwind
    /// 和 abort 策略，而不是给用户代码提供可依赖的恢复机制。
    #[must_use]
    #[unstable(feature = "panic_can_unwind", issue = "92988")]
    pub fn can_unwind(&self) -> bool {
        self.can_unwind
    }

    #[unstable(
        feature = "panic_internals",
        reason = "internal details of the implementation of the `panic!` and related macros",
        issue = "none"
    )]
    #[doc(hidden)]
    #[inline]
    pub fn force_no_backtrace(&self) -> bool {
        self.force_no_backtrace
    }
}

#[stable(feature = "panic_hook_display", since = "1.26.0")]
impl Display for PanicInfo<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("panicked at ")?;
        self.location.fmt(formatter)?;
        formatter.write_str(":\n")?;
        formatter.write_fmt(*self.message)?;
        Ok(())
    }
}

impl<'a> PanicMessage<'a> {
    /// 如果消息没有需要在运行期格式化的参数，则取得格式化后的静态消息。
    ///
    /// 这在某些场景下可用于避免分配，例如 `no_std` 或极早期 panic 输出路径。
    ///
    /// # 保证
    ///
    /// 对 `panic!("just a literal")`，此函数保证返回 `Some("just a literal")`。
    ///
    /// 对大多数带占位符的情况，此函数会返回 `None`。
    ///
    /// 细节见 [`fmt::Arguments::as_str`]。
    #[stable(feature = "panic_info_message", since = "1.81.0")]
    #[rustc_const_stable(feature = "const_arguments_as_str", since = "1.84.0")]
    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> Option<&'static str> {
        self.message.as_str()
    }
}

#[stable(feature = "panic_info_message", since = "1.81.0")]
impl Display for PanicMessage<'_> {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(*self.message)
    }
}

#[stable(feature = "panic_info_message", since = "1.81.0")]
impl fmt::Debug for PanicMessage<'_> {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(*self.message)
    }
}
