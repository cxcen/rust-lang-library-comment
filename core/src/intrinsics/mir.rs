//! rustc 内部使用的、手写 MIR 的工具。
//!
//! 如果你出于某种原因并不是在写 rustc 测试，却发现自己在考虑使用这个特性，请就此打住。
//! 它*极其*不稳定。这里完全没有去保证任何东西能用，能用的只是 rustc 测试套件恰好需要的那部分。
//! 一个笔误就很可能让编译器 ICE（内部编译错误）。说真的，这不是用来解决你问题的方案。
//! 请转而考虑支持 [stable MIR 项目组](https://github.com/rust-lang/project-stable-mir)。
//!
//! 本模块的文档描述了如何使用这个特性。如果你想动手改它的实现，相关文档大多在
//! `rustc_mir_build/src/build/custom/mod.rs`。
//!
//! 设计背景：MIR（中级中间表示）是 rustc 内部的实现细节；这里的所有内容都是编译器内建的契约层，
//! 通过 `mir!` 宏和一组 `#[rustc_diagnostic_item]` 标注的函数把 MIR 结构暴露给手写者，
//! 编译器在 `#[custom_mir]` 函数体内会特殊处理它们。
//!
//! 典型用法如下：
//!
//! ```rust
//! #![feature(core_intrinsics, custom_mir)]
//! #![allow(internal_features)]
//!
//! use core::intrinsics::mir::*;
//!
//! #[custom_mir(dialect = "built")]
//! pub fn simple(x: i32) -> i32 {
//!     mir! {
//!         let temp2: i32;
//!
//!         {
//!             let temp1 = x;
//!             Goto(my_second_block)
//!         }
//!
//!         my_second_block = {
//!             temp2 = Move(temp1);
//!             RET = temp2;
//!             Return()
//!         }
//!     }
//! }
//! ```
//!
//! `custom_mir` 属性告诉编译器把该函数当作自定义 MIR 来对待。该属性只对函数有效——
//! 没有办法把自定义 MIR 插入到另一个函数的中间。`dialect` 和 `phase` 参数指明你在这里插入的是
//! [哪个版本的 MIR][dialect 文档]。一般来说，如果你希望你的 MIR 经过完整的 MIR 流水线处理，
//! 就用 `#![custom_mir(dialect = "built")]`；如果不希望，就用
//! `#![custom_mir(dialect = "runtime", phase = "optimized")]`。
//!
//! [dialect 文档]:
//!     https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/mir/enum.MirPhase.html
//!
//! [`mir!`] 宏的输入是：
//!
//!  - 一个可选的返回类型标注，形如 `type RET = ...;`。当编译器无法推断 RET 的类型时，这是必需的。
//!  - 一份（可能为空的）局部变量声明列表。局部变量也可以在赋值时通过 `let` 就地声明。类型推断一般可用。
//!    不支持遮蔽（shadowing）。
//!  - 一份基本块（basic block）列表。其中第一个是起始块，执行从这里开始。除起始块外的所有块都必须命名，
//!    以便后续引用。
//!     - 每个块都是一串以分号结尾的语句，后跟一个终结符（terminator）。各种语句和终结符的语法被设计得
//!       尽量贴近 Rust 中对应概念的原生语法。完整列表见下文。
//!
//! # 示例
//!
//! ```rust
//! #![feature(core_intrinsics, custom_mir)]
//! #![allow(internal_features)]
//! #![allow(unused_assignments)]
//!
//! use core::intrinsics::mir::*;
//!
//! #[custom_mir(dialect = "built")]
//! pub fn choose_load(a: &i32, b: &i32, c: bool) -> i32 {
//!     mir! {
//!         {
//!             match c {
//!                 true => t,
//!                 _ => f,
//!             }
//!         }
//!
//!         t = {
//!             let temp = a;
//!             Goto(load_and_exit)
//!         }
//!
//!         f = {
//!             temp = b;
//!             Goto(load_and_exit)
//!         }
//!
//!         load_and_exit = {
//!             RET = *temp;
//!             Return()
//!         }
//!     }
//! }
//!
//! #[custom_mir(dialect = "built")]
//! fn unwrap_unchecked<T>(opt: Option<T>) -> T {
//!     mir! {
//!         {
//!             RET = Move(Field(Variant(opt, 1), 0));
//!             Return()
//!         }
//!     }
//! }
//!
//! #[custom_mir(dialect = "runtime", phase = "optimized")]
//! fn push_and_pop<T>(v: &mut Vec<T>, value: T) {
//!     mir! {
//!         let _unused;
//!         let popped;
//!
//!         {
//!             Call(_unused = Vec::push(v, value), ReturnTo(pop), UnwindContinue())
//!         }
//!
//!         pop = {
//!             Call(popped = Vec::pop(v), ReturnTo(drop), UnwindContinue())
//!         }
//!
//!         drop = {
//!             Drop(popped, ReturnTo(ret), UnwindContinue())
//!         }
//!
//!         ret = {
//!             Return()
//!         }
//!     }
//! }
//!
//! #[custom_mir(dialect = "runtime", phase = "optimized")]
//! fn annotated_return_type() -> (i32, bool) {
//!     mir! {
//!         type RET = (i32, bool);
//!         {
//!             RET.0 = 1;
//!             RET.1 = true;
//!             Return()
//!         }
//!     }
//! }
//! ```
//!
//! 我们也可以故意触发那些发生在编译器足够靠后阶段的编译失败：
//!
//! ```rust,compile_fail
//! #![feature(core_intrinsics, custom_mir)]
//!
//! extern crate core;
//! use core::intrinsics::mir::*;
//!
//! #[custom_mir(dialect = "built")]
//! fn borrow_error(should_init: bool) -> i32 {
//!     mir! {
//!         let temp: i32;
//!
//!         {
//!             match should_init {
//!                 true => init,
//!                 _ => use_temp,
//!             }
//!         }
//!
//!         init = {
//!             temp = 0;
//!             Goto(use_temp)
//!         }
//!
//!         use_temp = {
//!             RET = temp;
//!             Return()
//!         }
//!     }
//! }
//! ```
//!
//! ```text
//! error[E0381]: used binding is possibly-uninitialized
//!   --> test.rs:24:13
//!    |
//! 8  | /     mir! {
//! 9  | |         let temp: i32;
//! 10 | |
//! 11 | |         {
//! ...  |
//! 19 | |             temp = 0;
//!    | |             -------- binding initialized here in some conditions
//! ...  |
//! 24 | |             RET = temp;
//!    | |             ^^^^^^^^^^ value used here but it is possibly-uninitialized
//! 25 | |             Return()
//! 26 | |         }
//! 27 | |     }
//!    | |_____- binding declared here but left uninitialized
//!
//! error: aborting due to 1 previous error
//!
//! For more information about this error, try `rustc --explain E0381`.
//! ```
//!
//! # 语法
//!
//! 下面的列表详尽地描述了各种 MIR 构造的创建方式。任何未列出的东西都应假定为不被支持，欢迎提 PR。
//!
//! #### 局部变量（Locals）
//!
//!  - 编号为 `_0` 的返回局部变量始终可以通过 `RET` 访问。
//!  - 参数可以通过它们各自的常规名字访问。
//!  - 其余所有局部变量都需要在某处用 `let` 声明，之后即可按名字访问。
//!
//! #### 位置（Places）
//!  - 局部变量会隐式转换为位置（place）。
//!  - 字段访问、解引用、下标索引都按常规方式工作。
//!  - 变体（variant）中的字段可以通过 [`Variant`] 和 [`Field`] 这两个关联函数访问，详见它们的文档。
//!
//! #### 操作数（Operands）
//!  - 位置会隐式转换为 `Copy` 操作数。
//!  - `Move` 操作数可以通过 [`Move`] 创建。
//!  - const 块、字面量、具名常量以及 const 泛型参数都可直接使用。
//!  - [`Static`] 和 [`StaticMut`] 可用于创建指向 static 的 `&T` 和 `*mut T`。它们在 MIR 中是常量，
//!    也是访问 static 的唯一途径。
//!
//! #### 语句（Statements）
//!  - 赋值语句通过常规的 Rust 赋值实现。
//!  - [`Retag`]、[`StorageLive`]、[`StorageDead`] 这几种语句各有一个对应的关联函数。
//!
//! #### 右值（Rvalues）
//!
//!  - 操作数会隐式转换为 `Use` 右值。
//!  - `&`、`&mut`、`addr_of!`、`addr_of_mut!` 都可用于创建各自对应的右值。
//!  - [`CastTransmute`]、[`CastPtrToPtr`]、[`CastUnsize`] 和 [`Discriminant`] 各有对应的关联函数。
//!  - 一元和二元运算使用其常规 Rust 语法——`a * b`、`!c` 等等。
//!  - 二元运算 `Offset` 可以通过 [`Offset`] 创建。
//!  - 带溢出检查的二元运算通过把对应的 binop 包进 [`Checked`] 来表示。
//!  - 数组重复语法（`[foo; 10]`）会创建对应的右值。
//!
//! #### 终结符（Terminators）
//!
//!  - [`Goto`]、[`Return`]、[`Unreachable`] 和 [`Drop`](Drop()) 各有对应的关联函数。
//!  - `match some_int_operand` 会变成一个 `SwitchInt`。每个分支臂应写成 `literal => basic_block`。
//!     - 唯一的例外是最后一个分支臂，它必须是 `_ => basic_block`，对应 otherwise（其余情况）分支。
//!  - [`Call`] 同样有对应的关联函数，并带有特殊语法：
//!    `Call(ret_val = function(arg1, arg2, ...), ReturnTo(next_block), UnwindContinue())`。
//!  - [`TailCall`] 没有返回目标，也没有后继块，所以它的语法就只是
//!    `TailCall(function(arg1, arg2, ...))`。
//!
//! #### 调试信息（Debuginfo）
//!
//! 调试信息把源码中的变量名（这些变量可能已经不复存在）与表明该变量值存放位置的 MIR 表达式关联起来。
//! 其语法为：
//! ```text
//! debug source_var_name => expression;
//! ```
//! `expression` 中既支持位置（place），也支持常量。
//!
//! ```rust
//! #![allow(internal_features)]
//! #![feature(core_intrinsics, custom_mir)]
//!
//! use core::intrinsics::mir::*;
//!
//! #[custom_mir(dialect = "built")]
//! fn debuginfo(arg: Option<&i32>) {
//!     mir!(
//!         // 源码变量 `plain_local` 的调试信息，它只是单纯复制了 `arg`。
//!         debug plain_local => arg;
//!         // 源码变量 `projection` 的调试信息，其值可通过解引用 `arg` 的某个字段计算得到。
//!         debug projection => *Field::<&i32>(Variant(arg, 1), 0);
//!         // 源码变量 `constant` 的调试信息，它始终持有值 `5`。
//!         debug constant => 5_usize;
//!         {
//!             Return()
//!         }
//!     )
//! }
//! ```

#![unstable(
    feature = "custom_mir",
    reason = "MIR is an implementation detail and extremely unstable",
    issue = "none"
)]
#![allow(unused_variables, non_snake_case, missing_debug_implementations)]

/// 表示基本块（basic block）的类型。
///
/// 所有终结符（terminator）都以本类型作为返回类型。这有助于获得一定的类型安全。
#[rustc_diagnostic_item = "mir_basic_block"]
pub enum BasicBlock {
    /// 非清理（non-cleanup）基本块。
    Normal,
    /// 位于栈展开（unwind）路径上的基本块。
    Cleanup,
}

/// 在栈展开（unwinding）过程中我们终止进程的原因。
#[rustc_diagnostic_item = "mir_unwind_terminate_reason"]
pub enum UnwindTerminateReason {
    /// 给定本函数的 ABI，栈展开根本就不可能进行。
    Abi,
    /// 我们本就在为一次正在进行的展开做清理，而此时 drop 胶水代码又触发了*第二次*、*嵌套的*展开。
    InCleanup,
}

pub use UnwindTerminateReason::{Abi as ReasonAbi, InCleanup as ReasonInCleanup};

macro_rules! define {
    ($name:literal, $( #[ $meta:meta ] )* fn $($sig:tt)*) => {
        #[rustc_diagnostic_item = $name]
        #[inline]
        $( #[ $meta ] )*
        pub fn $($sig)* { panic!() }
    }
}

// 栈展开动作
pub struct UnwindActionArg;
define!(
    "mir_unwind_continue",
    /// 一个继续向上展开（unwind）的栈展开动作。
    fn UnwindContinue() -> UnwindActionArg
);
define!(
    "mir_unwind_unreachable",
    /// 一个触发未定义行为（UB）的栈展开动作。
    fn UnwindUnreachable() -> UnwindActionArg
);
define!(
    "mir_unwind_terminate",
    /// 一个终止执行的栈展开动作。
    ///
    /// `UnwindTerminate` 也可以用作终结符（terminator）。
    fn UnwindTerminate(reason: UnwindTerminateReason) -> UnwindActionArg
);
define!(
    "mir_unwind_cleanup",
    /// 一个在给定基本块中继续执行的栈展开动作。
    fn UnwindCleanup(goto: BasicBlock) -> UnwindActionArg
);

// `Call` 的返回目标
pub struct ReturnToArg;
define!("mir_return_to", fn ReturnTo(goto: BasicBlock) -> ReturnToArg);

// 终结符（Terminators）
define!("mir_return", fn Return() -> BasicBlock);
define!("mir_goto", fn Goto(destination: BasicBlock) -> BasicBlock);
define!("mir_unreachable", fn Unreachable() -> BasicBlock);
define!("mir_drop",
    /// Drop（析构）某个位置（place）的内容。
    ///
    /// 第一个参数必须是一个位置（place）。
    ///
    /// 第二个参数必须形如 `ReturnTo(bb)`，其中 `bb` 是析构函数返回后将跳转到的基本块。
    ///
    /// 第三个参数描述栈展开时发生什么。它可以是以下之一：
    /// - [`UnwindContinue`]
    /// - [`UnwindUnreachable`]
    /// - [`UnwindTerminate`]
    /// - [`UnwindCleanup`]
    fn Drop<T>(place: T, goto: ReturnToArg, unwind_action: UnwindActionArg)
);
define!("mir_call",
    /// 调用一个函数。
    ///
    /// 第一个参数必须形如 `ret_val = fun(arg1, arg2, ...)`。
    ///
    /// 第二个参数必须形如 `ReturnTo(bb)`，其中 `bb` 是函数返回后将跳转到的基本块。
    ///
    /// 第三个参数描述栈展开时发生什么。它可以是以下之一：
    /// - [`UnwindContinue`]
    /// - [`UnwindUnreachable`]
    /// - [`UnwindTerminate`]
    /// - [`UnwindCleanup`]
    fn Call(call: (), goto: ReturnToArg, unwind_action: UnwindActionArg)
);
define!("mir_tail_call",
    /// 调用一个函数（尾调用）。
    ///
    /// 参数必须形如 `fun(arg1, arg2, ...)`。
    fn TailCall<T>(call: T)
);
define!("mir_unwind_resume",
    /// 一个恢复（继续）栈展开的终结符。
    fn UnwindResume()
);

define!("mir_storage_live", fn StorageLive<T>(local: T));
define!("mir_storage_dead", fn StorageDead<T>(local: T));
define!("mir_assume", fn Assume(operand: bool));
define!("mir_checked", fn Checked<T>(binop: T) -> (T, bool));
define!(
    "mir_ptr_metadata",
    fn PtrMetadata<P: ?Sized>(place: *const P) -> <P as ::core::ptr::Pointee>::Metadata
);
define!("mir_retag", fn Retag<T>(place: T));
define!("mir_move", fn Move<T>(place: T) -> T);
define!("mir_static", fn Static<T>(s: T) -> &'static T);
define!("mir_static_mut", fn StaticMut<T>(s: T) -> *mut T);
define!(
    "mir_discriminant",
    /// 取某个位置（place）的判别值（discriminant）。
    fn Discriminant<T>(place: T) -> <T as ::core::marker::DiscriminantKind>::Discriminant
);
define!("mir_set_discriminant", fn SetDiscriminant<T>(place: T, index: u32));
define!("mir_offset", fn Offset<T, U>(ptr: T, count: U) -> T);
define!(
    "mir_field",
    /// 访问某个位置（place）中给定索引的字段。
    ///
    /// 只有与 [`Variant`] 配合使用才有意义。如果你要访问字段的那个类型没有变体（variant），
    /// 直接用普通的字段投影（field projection）语法即可。
    ///
    /// 在 Rust 中没有正经的办法把位置投影到某个变体上，所以这两个函数是一种变通手段。
    /// 你可以通过 `Field(Variant(place, var_idx), field_idx)` 来访问某个变体的字段，
    /// 其中 `var_idx` 和 `field_idx` 是合适的字面量。有几点需要注意：
    ///
    ///  - `Variant` 的返回类型始终是 `()`。不用担心，正确的 MIR 仍然会被生成出来。
    ///  - 在某些情况下，`Field` 的返回类型无法被推断。这时你可能需要在函数上标注它。
    ///  - 由于 `Field` 是一次函数调用而非位置表达式（place expression），把它用在表达式左侧
    ///    会被编译器拒绝。[`place!`] 这个宏就是用来绕开该问题的。把赋值的左侧用该宏包起来，
    ///    就能让编译器相信这是可以的。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![allow(internal_features)]
    /// #![feature(custom_mir, core_intrinsics)]
    ///
    /// use core::intrinsics::mir::*;
    ///
    /// #[custom_mir(dialect = "built")]
    /// fn unwrap_deref(opt: Option<&i32>) -> i32 {
    ///     mir! {
    ///         {
    ///             RET = *Field::<&i32>(Variant(opt, 1), 0);
    ///             Return()
    ///         }
    ///     }
    /// }
    ///
    /// #[custom_mir(dialect = "built")]
    /// fn set(opt: &mut Option<i32>) {
    ///     mir! {
    ///         {
    ///             place!(Field(Variant(*opt, 1), 0)) = 5;
    ///             Return()
    ///         }
    ///     }
    /// }
    /// ```
    fn Field<F>(place: (), field: u32) -> F
);
define!(
    "mir_variant",
    /// 给位置（place）添加一个指定索引的变体投影（variant projection）。
    ///
    /// 文档见 [`Field`]。
    fn Variant<T>(place: T, index: u32) -> ()
);
define!(
    "mir_cast_transmute",
    /// 发出一个 `CastKind::Transmute` 转换。
    ///
    /// 用于测试 `sizeof(T) != sizeof(U)` 时的 UB——这种情况无法通过常规的 `mem::transmute` 生成。
    fn CastTransmute<T, U>(operand: T) -> U
);
define!(
    "mir_cast_ptr_to_ptr",
    /// 发出一个 `CastKind::PtrToPtr` 转换。
    ///
    /// 这允许绕过常规校验，从而生成各种奇怪的转换。
    fn CastPtrToPtr<T, U>(operand: T) -> U
);
define!(
    "mir_cast_unsize",
    /// 发出一个 `CastKind::PointerCoercion(Unsize)` 转换。
    ///
    /// 这允许绕过常规校验，从而生成各种奇怪的转换。
    fn CastUnsize<T, U>(operand: T) -> U
);
define!(
    "mir_make_place",
    #[doc(hidden)]
    fn __internal_make_place<T>(place: T) -> *mut T
);
define!(
    "mir_debuginfo",
    #[doc(hidden)]
    fn __debuginfo<T>(name: &'static str, s: T)
);

/// 用于生成自定义 MIR 的宏。
///
/// 语法细节见模块文档。这个宏并不神奇——它只是把你的 MIR 转换成在编译器里更易于解析的形式。
#[rustc_macro_transparency = "transparent"]
pub macro mir {
    {
        $(type RET = $ret_ty:ty ;)?
        $(let $local_decl:ident $(: $local_decl_ty:ty)? ;)*
        $(debug $dbg_name:ident => $dbg_data:expr ;)*

        {
            $($entry:tt)*
        }

        $(
            $block_name:ident $(($block_cleanup:ident))? = {
                $($block:tt)*
            }
        )*
    } => {{
        // 首先，声明所有基本块。
        __internal_declare_basic_blocks!($(
            $block_name $(($block_cleanup))?
        )*);
        {
            // 接着是所有局部变量
            #[allow(non_snake_case)]
            let RET $(: $ret_ty)?;
            $(
                let $local_decl $(: $local_decl_ty)? ;
            )*
            ::core::intrinsics::mir::__internal_extract_let!($($entry)*);
            $(
                ::core::intrinsics::mir::__internal_extract_let!($($block)*);
            )*

            {
                // 然后是调试信息
                $(
                    __debuginfo(stringify!($dbg_name), $dbg_data);
                )*

                {
                    // 最后，是各基本块的内容
                    ::core::intrinsics::mir::__internal_remove_let!({
                        {}
                        { $($entry)* }
                    });
                    $(
                        ::core::intrinsics::mir::__internal_remove_let!({
                            {}
                            { $($block)* }
                        });
                    )*

                    RET
                }
            }
        }
    }}
}

/// 辅助宏，让你能把一个值表达式（value expression）当作位置表达式（place expression）来用。
///
/// 关于为什么需要它以及如何使用它，见 [`Variant`] 的文档。
pub macro place($e:expr) {
    (*::core::intrinsics::mir::__internal_make_place($e))
}

/// 辅助宏，从一堆语句中把 `let` 声明提取出来。
///
/// 这个宏采用“语句吞噬（statement muncher）”策略编写。每次调用都从输入里解析出第一条语句，
/// 对它做相应处理，然后对剩下的输入递归调用同一个宏。
#[doc(hidden)]
pub macro __internal_extract_let {
    // 如果是类似 `let` 的语句，就保留这个 `let`
    (
        let $var:ident $(: $ty:ty)? = $expr:expr; $($rest:tt)*
    ) => {
        let $var $(: $ty)?;
        ::core::intrinsics::mir::__internal_extract_let!($($rest)*);
    },
    // 由于 #86730，我们必须单独处理 const 块
    (
        let $var:ident $(: $ty:ty)? = const $block:block; $($rest:tt)*
    ) => {
        let $var $(: $ty)?;
        ::core::intrinsics::mir::__internal_extract_let!($($rest)*);
    },
    // 否则，不输出任何东西
    (
        $stmt:stmt; $($rest:tt)*
    ) => {
        ::core::intrinsics::mir::__internal_extract_let!($($rest)*);
    },
    (
        $expr:expr
    ) => {}
}

/// 辅助宏，从一堆语句中把 `let` 声明移除掉。
///
/// 因为表达式位置的宏不能展开成“语句 + 表达式”，这里我们得稍微动点脑筋。总体策略同样是上面那种语句吞噬，
/// 但宏的输出被“暂存”在随后的那次宏调用里。通过例子最容易理解：
/// ```text
/// invoke!(
///     {
///         {
///             x = 5;
///         }
///         {
///             let d = e;
///             Call()
///         }
///     }
/// )
/// ```
/// 会变成
/// ```text
/// invoke!(
///     {
///         {
///             x = 5;
///             d = e;
///         }
///         {
///             Call()
///         }
///     }
/// )
/// ```
#[doc(hidden)]
pub macro __internal_remove_let {
    // 如果是类似 `let` 的语句，就移除这个 `let`
    (
        {
            {
                $($already_parsed:tt)*
            }
            {
                let $var:ident $(: $ty:ty)? = $expr:expr;
                $($rest:tt)*
            }
        }
    ) => { ::core::intrinsics::mir::__internal_remove_let!(
        {
            {
                $($already_parsed)*
                $var = $expr;
            }
            {
                $($rest)*
            }
        }
    )},
    // 由于 #86730，我们必须单独处理 const 块
    (
        {
            {
                $($already_parsed:tt)*
            }
            {
                let $var:ident $(: $ty:ty)? = const $block:block;
                $($rest:tt)*
            }
        }
    ) => { ::core::intrinsics::mir::__internal_remove_let!(
        {
            {
                $($already_parsed)*
                $var = const $block;
            }
            {
                $($rest)*
            }
        }
    )},
    // 否则，继续往下处理
    (
        {
            {
                $($already_parsed:tt)*
            }
            {
                $stmt:stmt;
                $($rest:tt)*
            }
        }
    ) => { ::core::intrinsics::mir::__internal_remove_let!(
        {
            {
                $($already_parsed)*
                $stmt;
            }
            {
                $($rest)*
            }
        }
    )},
    (
        {
            {
                $($already_parsed:tt)*
            }
            {
                $expr:expr
            }
        }
    ) => {
        {
            $($already_parsed)*
            $expr
        }
    },
}

/// 辅助宏，用于声明各基本块。
#[doc(hidden)]
pub macro __internal_declare_basic_blocks {
    () => {},
    ($name:ident (cleanup) $($rest:tt)*) => {
        let $name = ::core::intrinsics::mir::BasicBlock::Cleanup;
        __internal_declare_basic_blocks!($($rest)*)
    },
    ($name:ident $($rest:tt)*) => {
        let $name = ::core::intrinsics::mir::BasicBlock::Normal;
        __internal_declare_basic_blocks!($($rest)*)
    },
}
