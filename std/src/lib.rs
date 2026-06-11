//! # Rust 标准库（The Rust Standard Library）
//!
//! Rust 标准库是可移植 Rust 软件的基石，它为[更广阔的 Rust 生态系统][crates.io]
//! 提供了一组最小化且经过实战检验的共享抽象。它提供了诸如 [`Vec<T>`] 和
//! [`Option<T>`] 这样的核心类型、[针对语言基本类型的库级操作](#primitives)、
//! [标准宏](#macros)、[I/O] 与[多线程][multithreading]，以及[许多其它东西][other]。
//!
//! `std` 默认对所有 Rust crate 可用。因此，标准库可以通过路径 `std` 在 [`use`]
//! 语句中访问，例如 [`use std::env`]。
//!
//! 设计背景：在 Rust 的库分层中，`core` 是无内存分配、无 OS 依赖的最底层，`alloc`
//! 在其上引入堆分配（`Box`、`Vec`、`String` 等），而 `std` 处于最高层——它假定有堆、
//! 有操作系统、有运行时，把对各平台（最显著的是 Windows 与 Unix 衍生系统）差异的
//! 抽象统一封装起来，并提供文件系统、网络、线程、进程、时间等需要 OS 支持的能力。
//!
//! # 如何阅读本文档
//!
//! 如果你已经知道要找的东西的名字，最快的方式是使用页面顶部的<a href="#" onclick="window.searchState.focus();">搜索
//! 按钮</a>。
//!
//! 否则，你可能想跳到下面这些有用的章节之一：
//!
//! * [`std::*` 模块](#modules)
//! * [基本类型（Primitive types）](#primitives)
//! * [标准宏（Standard macros）](#macros)
//! * [Rust 预导入（The Rust Prelude）][The Rust Prelude]
//!
//! 如果这是你第一次阅读，标准库文档的撰写风格适合随意浏览。点击感兴趣的内容通常
//! 会带你到同样有趣的地方。不过，仍有一些你不想错过的重要部分，所以请继续往下读，
//! 来一次标准库及其文档的巡览！
//!
//! 一旦你熟悉了标准库的内容，可能会觉得这些散文式的叙述过于啰嗦。到了开发的这个
//! 阶段，你或许想点击页面顶部附近的
//! "<svg style="width:0.75rem;height:0.75rem" viewBox="0 0 12 12" stroke="currentColor" fill="none"><path d="M2,2l4,4l4,-4M2,6l4,4l4,-4"/></svg>&nbsp;Summary"
//! 按钮，把文档折叠成更便于略读的视图。
//!
//! 当你看着页面顶部时，也请注意那个 "Source"（源代码）链接。Rust 的 API 文档随附
//! 源代码，我们鼓励你去读它。标准库源代码质量通常很高，窥探幕后往往令人豁然开朗。
//!
//! # 标准库文档里都有什么？
//!
//! 首先，Rust 标准库被划分为若干个聚焦的模块，[全部列在本页面下方](#modules)。这些
//! 模块是整个 Rust 赖以构筑的基岩，它们有着像 [`std::slice`] 和 [`std::cmp`] 这样
//! 响亮的名字。模块文档通常包括对该模块的概览以及示例，是开始熟悉本库的明智起点。
//!
//! 其次，[基本类型（primitive types）][primitive types]上的隐式方法也记录在这里。
//! 这可能因两个原因造成困惑：
//!
//! 1. 虽然基本类型由编译器实现，但标准库直接在这些基本类型上实现方法（而且它是唯一
//!    这样做的库），这些方法[记录在“基本类型”一节](#primitives)中。
//! 2. 标准库导出了许多*与基本类型同名的*模块。这些模块定义了与该基本类型相关的额外
//!    项，但不包括那些至关重要的方法本身。
//!
//! 举例来说，既有一个[基本类型 `char` 的页面](primitive::char)，列出可以在字符上
//! 调用的所有方法（非常有用）；又有一个[模块 `std::char` 的页面](crate::char)，记录
//! 由这些方法创建的迭代器和错误类型（很少用到）。
//!
//! 注意基本类型 [`str`] 和 [`[T]`][prim@slice]（也称为“切片（slice）”）的文档。
//! [`String`] 和 [`Vec<T>`] 上的许多方法调用，实际上是分别通过 [deref
//! 强制转换][deref-coercions]去调用 [`str`] 和 [`[T]`][prim@slice] 上的方法。
//!
//! 第三，标准库定义了 [Rust 预导入（The Rust Prelude）][The Rust Prelude]，这是一小组
//! 项——大多是 trait——它们被导入到每个 crate 的每个模块中。预导入中的 trait 无处不在，
//! 这使得预导入文档成为了解本库的一个良好入口。
//!
//! 最后，标准库导出了若干标准宏，并[在本页面列出它们](#macros)（严格来说，并非所有
//! 标准宏都由标准库定义——有些由编译器定义——但它们同样记录在这里）。与预导入一样，
//! 这些标准宏默认被导入到所有 crate 中。
//!
//! # 向文档贡献修改
//!
//! 请查阅 Rust 贡献指南[此处](
//! https://rustc-dev-guide.rust-lang.org/contributing.html#writing-documentation)。
//! 本文档的源代码可在
//! [GitHub](https://github.com/rust-lang/rust) 上的 'library/std/' 目录中找到。
//! 若要贡献修改，请先阅读指南，然后为你建议的改动提交 pull request。
//!
//! 我们感谢你的贡献！如果你发现文档中有可改进之处，请提交 PR，或先在 [Zulip][rust-zulip]
//! 的 #docs 频道上与我们交流。
//!
//! # Rust 标准库巡览（A Tour of The Rust Standard Library）
//!
//! 本 crate 文档接下来的部分，致力于指出 Rust 标准库中那些值得注意的特性。
//!
//! ## 容器与集合（Containers and collections）
//!
//! [`option`] 和 [`result`] 模块定义了用于“可选值”和“错误处理”的类型 [`Option<T>`]
//! 和 [`Result<T, E>`]。[`iter`] 模块定义了 Rust 的迭代器 trait [`Iterator`]，它与
//! [`for`] 循环协作来访问集合。
//!
//! 标准库提供了三种处理连续内存区域的常见方式：
//!
//! * [`Vec<T>`] —— 一个堆分配的*向量（vector）*，可在运行时改变大小。
//! * [`[T; N]`][prim@array] —— 一个内联*数组（array）*，大小在编译期固定。
//! * [`[T]`][prim@slice] —— 一个动态大小的*切片（slice）*，指向任何其它种类的连续
//!   存储，无论它是否堆分配。
//!
//! 切片只能通过某种*指针（pointer）*来处理，因此有多种形态，例如：
//!
//! * `&[T]` —— *共享切片（shared slice）*
//! * `&mut [T]` —— *可变切片（mutable slice）*
//! * [`Box<[T]>`][owned slice] —— *拥有所有权的切片（owned slice）*
//!
//! [`str`] 是一个 UTF-8 字符串切片，属于基本类型，标准库为它定义了许多方法。Rust 的
//! [`str`] 通常以不可变引用 `&str` 的形式被访问。要构建和修改字符串，请使用拥有所有权
//! 的 [`String`]。
//!
//! 要转换*为*字符串，使用 [`format!`] 宏；要*从*字符串转换，使用 [`FromStr`] trait。
//!
//! 数据可以通过放入引用计数的 box（即 [`Rc`] 类型）来共享；若进一步将其包入 [`Cell`]
//! 或 [`RefCell`]，则在共享的同时还可被修改。同样地，在并发场景下，常见做法是将一个
//! 原子引用计数的 box [`Arc`] 与一个 [`Mutex`] 配对，以获得相同的效果。
//!
//! [`collections`] 模块定义了映射、集合、链表等典型的集合类型，包括常用的
//! [`HashMap<K, V>`]。
//!
//! ## 平台抽象与 I/O（Platform abstractions and I/O）
//!
//! 除了基本的数据类型之外，标准库主要还致力于抽象掉常见平台（最显著的是 Windows 与
//! Unix 衍生系统）之间的差异。
//!
//! 常见的 I/O 类型，包括[文件][files]、[TCP] 和 [UDP]，分别定义在 [`io`]、[`fs`]
//! 和 [`net`] 模块中。
//!
//! [`thread`] 模块包含 Rust 的线程抽象。[`sync`] 包含更多底层的共享内存类型，包括
//! [`atomic`]、[`mpmc`] 和 [`mpsc`]，后者包含用于消息传递的 channel 类型。
//!
//! # 在 `main()` 之前与之后的使用（Use before and after `main()`）
//!
//! 标准库的许多部分被期望在 `main()` 之前和之后都能工作；但这一点并没有被测试所保证
//! 或确保。建议你为自己希望支持的每个平台编写并运行你自己的测试。
//! 这意味着，在 main 之前/之后使用 `std`——尤其是那些与 OS 或全局状态交互的特性——不受
//! 稳定性和可移植性保证的约束，仅以尽力而为（best-effort）的方式提供。尽管如此，我们
//! 仍欢迎 bug 报告。
//!
//! 另一方面，`core` 和 `alloc` 最有可能在此类环境中工作，但需注意：任何可被钩子（hook）
//! 接管的行为，例如 panic、内存不足（oom）处理或分配器（allocator），同样取决于这些
//! 钩子的兼容性。
//!
//! 某些特性在 main 之外也可能表现不同，例如 stdio 可能变为无缓冲（unbuffered）、某些
//! panic 可能转为 abort、backtrace 可能无法被符号化（symbolicate）等等。
//!
//! 已知限制的非穷尽列表：
//!
//! - 在 main 之后使用线程局部变量（thread-local），这也会影响以下额外特性：
//!   - [`thread::current()`]
//! - 在 UNIX 上，于 main 之前，文件描述符 0、1、2 可能保持原样
//!   （它们保证在 main 期间是打开的；如果它们在程序启动时未打开，则会以
//!    O_RDWR 模式打开到 /dev/null）
//!
//!
//! [I/O]: io
//! [TCP]: net::TcpStream
//! [The Rust Prelude]: prelude
//! [UDP]: net::UdpSocket
//! [`Arc`]: sync::Arc
//! [owned slice]: boxed
//! [`Cell`]: cell::Cell
//! [`FromStr`]: str::FromStr
//! [`HashMap<K, V>`]: collections::HashMap
//! [`Mutex`]: sync::Mutex
//! [`Option<T>`]: option::Option
//! [`Rc`]: rc::Rc
//! [`RefCell`]: cell::RefCell
//! [`Result<T, E>`]: result::Result
//! [`Vec<T>`]: vec::Vec
//! [`atomic`]: sync::atomic
//! [`for`]: ../book/ch03-05-control-flow.html#looping-through-a-collection-with-for
//! [`str`]: prim@str
//! [`mpmc`]: sync::mpmc
//! [`mpsc`]: sync::mpsc
//! [`std::cmp`]: cmp
//! [`std::slice`]: mod@slice
//! [`use std::env`]: env/index.html
//! [`use`]: ../book/ch07-02-defining-modules-to-control-scope-and-privacy.html
//! [crates.io]: https://crates.io
//! [deref-coercions]: ../book/ch15-02-deref.html#implicit-deref-coercions-with-functions-and-methods
//! [files]: fs::File
//! [multithreading]: thread
//! [other]: #what-is-in-the-standard-library-documentation
//! [primitive types]: ../book/ch03-02-data-types.html
//! [rust-zulip]: https://rust-lang.zulipchat.com/
//! [array]: prim@array
//! [slice]: prim@slice

#![cfg_attr(not(restricted_std), stable(feature = "rust1", since = "1.0.0"))]
#![cfg_attr(
    restricted_std,
    unstable(
        feature = "restricted_std",
        issue = "none",
        reason = "You have attempted to use a standard library built for a platform that it doesn't \
            know how to support. Consider building it for a known environment, disabling it with \
            `#![no_std]` or overriding this warning by enabling this feature."
    )
)]
#![rustc_preserve_ub_checks]
#![doc(
    html_playground_url = "https://play.rust-lang.org/",
    issue_tracker_base_url = "https://github.com/rust-lang/rust/issues/",
    test(no_crate_inject, attr(deny(warnings))),
    test(attr(allow(dead_code, deprecated, unused_variables, unused_mut)))
)]
#![doc(rust_logo)]
#![doc(auto_cfg(hide(no_global_oom_handling)))]
// 不要链接到 std，因为我们自己就是 std。
#![no_std]
// 告诉编译器链接到 panic_abort 或 panic_unwind 之一。
#![needs_panic_runtime]
//
// Lints（各类 lint 设置）:
#![warn(deprecated_in_future)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![allow(explicit_outlives_requirements)]
#![allow(unused_lifetimes)]
#![allow(internal_features)]
#![deny(fuzzy_provenance_casts)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(rustdoc::redundant_explicit_links)]
#![warn(rustdoc::unescaped_backticks)]
// 确保 std 即便在以 `-C panic=unwind` 编译时，仍能与 panic_abort 链接。
#![deny(ffi_unwind_calls)]
// std 可能会以平台特定的方式使用某些 feature。
#![allow(unused_features)]
//
// Features（特性门控）:
#![cfg_attr(test, feature(internal_output_capture, print_internals, update_panic_count, rt))]
#![cfg_attr(
    all(target_vendor = "fortanix", target_env = "sgx"),
    feature(slice_index_methods, coerce_unsized, sgx_platform)
)]
#![cfg_attr(target_family = "wasm", feature(stdarch_wasm_atomic_wait))]
#![cfg_attr(target_arch = "wasm64", feature(simd_wasm64))]
//
// Language features（语言特性）:
// tidy-alphabetical-start
#![feature(alloc_error_handler)]
#![feature(allocator_internals)]
#![feature(allow_internal_unsafe)]
#![feature(allow_internal_unstable)]
#![feature(asm_experimental_arch)]
#![feature(autodiff)]
#![feature(cfg_sanitizer_cfi)]
#![feature(cfg_target_thread_local)]
#![feature(cfi_encoding)]
#![feature(const_default)]
#![feature(const_trait_impl)]
#![feature(core_float_math)]
#![feature(decl_macro)]
#![feature(deprecated_suggestion)]
#![feature(doc_cfg)]
#![feature(doc_masked)]
#![feature(doc_notable_trait)]
#![feature(dropck_eyepatch)]
#![feature(f16)]
#![feature(f128)]
#![feature(ffi_const)]
#![feature(formatting_options)]
#![feature(funnel_shifts)]
#![feature(if_let_guard)]
#![feature(intra_doc_pointers)]
#![feature(iter_advance_by)]
#![feature(iter_next_chunk)]
#![feature(lang_items)]
#![feature(link_cfg)]
#![feature(linkage)]
#![feature(macro_metavar_expr_concat)]
#![feature(maybe_uninit_fill)]
#![feature(min_specialization)]
#![feature(must_not_suspend)]
#![feature(needs_panic_runtime)]
#![feature(negative_impls)]
#![feature(never_type)]
#![feature(optimize_attribute)]
#![feature(prelude_import)]
#![feature(rustc_attrs)]
#![feature(rustdoc_internals)]
#![feature(staged_api)]
#![feature(stmt_expr_attributes)]
#![feature(strict_provenance_lints)]
#![feature(thread_local)]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(type_alias_impl_trait)]
// tidy-alphabetical-end
//
// Library features (core)（来自 core 的库特性）:
// tidy-alphabetical-start
#![feature(bstr)]
#![feature(bstr_internals)]
#![feature(cast_maybe_uninit)]
#![feature(cfg_select)]
#![feature(char_internals)]
#![feature(clone_to_uninit)]
#![feature(const_convert)]
#![feature(core_intrinsics)]
#![feature(core_io_borrowed_buf)]
#![feature(drop_guard)]
#![feature(duration_constants)]
#![feature(error_generic_member_access)]
#![feature(error_iter)]
#![feature(exact_size_is_empty)]
#![feature(exclusive_wrapper)]
#![feature(extend_one)]
#![feature(float_algebraic)]
#![feature(float_gamma)]
#![feature(float_minimum_maximum)]
#![feature(fmt_internals)]
#![feature(fn_ptr_trait)]
#![feature(generic_atomic)]
#![feature(hasher_prefixfree_extras)]
#![feature(hashmap_internals)]
#![feature(hint_must_use)]
#![feature(int_from_ascii)]
#![feature(ip)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(panic_can_unwind)]
#![feature(panic_internals)]
#![feature(pin_coerce_unsized_trait)]
#![feature(pointer_is_aligned_to)]
#![feature(portable_simd)]
#![feature(ptr_as_uninit)]
#![feature(ptr_mask)]
#![feature(random)]
#![feature(slice_internals)]
#![feature(slice_ptr_get)]
#![feature(slice_range)]
#![feature(slice_split_once)]
#![feature(std_internals)]
#![feature(str_internals)]
#![feature(sync_unsafe_cell)]
#![feature(temporary_niche_types)]
#![feature(ub_checks)]
#![feature(used_with_arg)]
// tidy-alphabetical-end
//
// Library features (alloc)（来自 alloc 的库特性）:
// tidy-alphabetical-start
#![feature(alloc_layout_extra)]
#![feature(allocator_api)]
#![feature(clone_from_ref)]
#![feature(get_mut_unchecked)]
#![feature(map_try_insert)]
#![feature(slice_concat_trait)]
#![feature(thin_box)]
#![feature(try_reserve_kind)]
#![feature(try_with_capacity)]
#![feature(unique_rc_arc)]
#![feature(wtf8_internals)]
// tidy-alphabetical-end
//
// Library features (unwind)（来自 unwind 的库特性）:
// tidy-alphabetical-start
#![feature(panic_unwind)]
// tidy-alphabetical-end
//
// Library features (std_detect)（来自 std_detect 的库特性）:
// tidy-alphabetical-start
#![feature(stdarch_internal)]
// tidy-alphabetical-end
//
// Only for re-exporting（仅用于重导出）:
// tidy-alphabetical-start
#![feature(assert_matches)]
#![feature(async_iterator)]
#![feature(c_variadic)]
#![feature(cfg_accessible)]
#![feature(cfg_eval)]
#![feature(concat_bytes)]
#![feature(const_format_args)]
#![feature(custom_test_frameworks)]
#![feature(edition_panic)]
#![feature(format_args_nl)]
#![feature(log_syntax)]
#![feature(test)]
#![feature(trace_macros)]
// tidy-alphabetical-end
//
// Only used in tests/benchmarks（仅在测试/基准测试中使用）:
//
// Only for const-ness（仅为满足 const 性质）:
// tidy-alphabetical-start
#![feature(io_const_error)]
// tidy-alphabetical-end
//
#![default_lib_allocator]

// Rust 预导入（The Rust prelude）
// 编译器要求预导入的定义出现在其 use 语句之前。
pub mod prelude;

// 显式导入预导入。当构建依赖 std 的 crate 时，编译器也使用这个相同的不稳定属性
// 来隐式导入预导入。
#[prelude_import]
#[allow(unused)]
use prelude::rust_2024::*;

// 访问 Bencher 等内容。
#[cfg(test)]
extern crate test;

#[allow(unused_imports)] // 来自 `alloc` 的宏并非在所有平台上都会被用到
#[macro_use]
extern crate alloc as alloc_crate;

// 许多编译器测试依赖于 libc 被 std 拉入，
// 所以即便它未被使用，也在这里包含进来。
#[doc(masked)]
#[allow(unused_extern_crates)]
#[cfg(not(all(windows, target_env = "msvc")))]
extern crate libc;

// 目前我们总是需要一个 unwinder 来支持 backtrace。
#[doc(masked)]
#[allow(unused_extern_crates)]
extern crate unwind;

// FIXME: #94122 这个 extern crate 定义之所以存在于此，仅仅是为了阻止
// miniz_oxide 的文档泄漏到 std 的文档中。需要找到更好的办法来处理它。
// 当它被移除时，请一并把它从 tidy 的平台检查排除项中删去。
#[doc(masked)]
#[allow(unused_extern_crates)]
#[cfg(all(
    not(all(windows, target_env = "msvc", not(target_vendor = "uwp"))),
    feature = "miniz_oxide"
))]
extern crate miniz_oxide;

// 在测试期间，本 crate 并不是“真正的” std 库，而是链接到那个真正的 std 库——后者
// 正是从这同一份源代码编译而来。因此，std 定义的任何 lang item 都会被有条件地排除
// （否则会产生重复 lang item 的错误），并且它定义的任何全局变量都*不是*“真正的”
// std 所使用的全局变量。所以这个仅在测试期间定义的 import，给了 test-std 访问
// real-std 的 lang item 和全局变量的途径。参见 #2912
#[cfg(test)]
extern crate std as realstd;

// 那些并非编译器内建的标准宏。
#[macro_use]
mod macros;

// 运行时入口点，以及一些供编译器使用的不稳定公开函数。
#[macro_use]
pub mod rt;

#[stable(feature = "rust1", since = "1.0.0")]
pub use core::any;
#[stable(feature = "core_array", since = "1.35.0")]
pub use core::array;
#[unstable(feature = "async_iterator", issue = "79024")]
pub use core::async_iter;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::cell;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::char;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::clone;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::cmp;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::convert;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::default;
#[stable(feature = "futures_api", since = "1.36.0")]
pub use core::future;
#[stable(feature = "core_hint", since = "1.27.0")]
pub use core::hint;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::i8;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::i16;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::i32;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::i64;
#[stable(feature = "i128", since = "1.26.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::i128;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::intrinsics;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::isize;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::iter;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::marker;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::mem;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::ops;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::option;
#[stable(feature = "pin", since = "1.33.0")]
pub use core::pin;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::ptr;
#[unstable(feature = "new_range_api", issue = "125687")]
pub use core::range;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::result;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::u8;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::u16;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::u32;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::u64;
#[stable(feature = "i128", since = "1.26.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::u128;
#[unstable(feature = "unsafe_binders", issue = "130516")]
pub use core::unsafe_binder;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::usize;

#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::borrow;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::boxed;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::fmt;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::format;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::rc;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::slice;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::str;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::string;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::vec;

#[path = "num/f128.rs"]
pub mod f128;
#[path = "num/f16.rs"]
pub mod f16;
#[path = "num/f32.rs"]
pub mod f32;
#[path = "num/f64.rs"]
pub mod f64;

#[macro_use]
pub mod thread;
pub mod ascii;
pub mod backtrace;
#[unstable(feature = "bstr", issue = "134915")]
pub mod bstr;
pub mod collections;
pub mod env;
pub mod error;
pub mod ffi;
pub mod fs;
pub mod hash;
pub mod io;
pub mod net;
pub mod num;
pub mod os;
pub mod panic;
#[unstable(feature = "pattern_type_macro", issue = "123646")]
pub mod pat;
pub mod path;
pub mod process;
#[unstable(feature = "random", issue = "130703")]
pub mod random;
pub mod sync;
pub mod time;

// 将 `std_float` crate 拉入 std。`std_float` 的内容位于另一个仓库
// rust-lang/portable-simd 中。
#[path = "../../portable-simd/crates/std_float/src/lib.rs"]
#[allow(missing_debug_implementations, dead_code, unsafe_op_in_unsafe_fn)]
#[allow(rustdoc::bare_urls)]
#[unstable(feature = "portable_simd", issue = "86656")]
mod std_float;

#[unstable(feature = "portable_simd", issue = "86656")]
pub mod simd {
    #![doc = include_str!("../../portable-simd/crates/core_simd/src/core_simd_docs.md")]

    #[doc(inline)]
    pub use core::simd::*;

    #[doc(inline)]
    pub use crate::std_float::StdFloat;
}

#[unstable(feature = "autodiff", issue = "124509")]
/// 本模块提供对自动微分（automatic differentiation）的支持。
pub mod autodiff {
    /// 本宏处理自动微分。
    pub use core::autodiff::{autodiff_forward, autodiff_reverse};
}

#[stable(feature = "futures_api", since = "1.36.0")]
pub mod task {
    //! 用于处理异步任务（asynchronous tasks）的类型与 trait。

    #[doc(inline)]
    #[stable(feature = "wake_trait", since = "1.51.0")]
    pub use alloc::task::*;
    #[doc(inline)]
    #[stable(feature = "futures_api", since = "1.36.0")]
    pub use core::task::*;
}

#[doc = include_str!("../../stdarch/crates/core_arch/src/core_arch_docs.md")]
#[stable(feature = "simd_arch", since = "1.27.0")]
pub mod arch {
    #[stable(feature = "simd_arch", since = "1.27.0")]
    // 需要 `no_inline` 属性，以便所有目标平台的文档都能可用。
    // 更多信息参见 https://github.com/rust-lang/rust/pull/57808#issuecomment-457390549。
    #[doc(no_inline)] // 注意 (#82861)：为正确生成文档所必需
    pub use core::arch::*;

    #[stable(feature = "simd_aarch64", since = "1.60.0")]
    pub use std_detect::is_aarch64_feature_detected;
    #[unstable(feature = "stdarch_arm_feature_detection", issue = "111190")]
    pub use std_detect::is_arm_feature_detected;
    #[unstable(feature = "is_loongarch_feature_detected", issue = "117425")]
    pub use std_detect::is_loongarch_feature_detected;
    #[unstable(feature = "is_riscv_feature_detected", issue = "111192")]
    pub use std_detect::is_riscv_feature_detected;
    #[stable(feature = "stdarch_s390x_feature_detection", since = "1.93.0")]
    pub use std_detect::is_s390x_feature_detected;
    #[stable(feature = "simd_x86", since = "1.27.0")]
    pub use std_detect::is_x86_feature_detected;
    #[unstable(feature = "stdarch_mips_feature_detection", issue = "111188")]
    pub use std_detect::{is_mips_feature_detected, is_mips64_feature_detected};
    #[unstable(feature = "stdarch_powerpc_feature_detection", issue = "111191")]
    pub use std_detect::{is_powerpc_feature_detected, is_powerpc64_feature_detected};
}

// 该项在 crate 根处被稳定化，所以我们必须把它保留在根处。
#[stable(feature = "simd_x86", since = "1.27.0")]
pub use std_detect::is_x86_feature_detected;

mod sys;

pub mod alloc;

// 私有支持模块（Private support modules）
mod panicking;

#[path = "../../backtrace/src/lib.rs"]
#[allow(dead_code, unused_attributes, fuzzy_provenance_casts, unsafe_op_in_unsafe_fn)]
mod backtrace_rs;

#[unstable(feature = "cfg_select", issue = "115585")]
pub use core::cfg_select;
#[unstable(
    feature = "concat_bytes",
    issue = "87555",
    reason = "`concat_bytes` is not stable enough for use and is subject to change"
)]
pub use core::concat_bytes;
#[stable(feature = "matches_macro", since = "1.42.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::matches;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use core::primitive;
#[stable(feature = "todo_macro", since = "1.40.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::todo;
// 重导出通过 core 定义的内建宏。
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
pub use core::{
    assert, assert_matches, cfg, column, compile_error, concat, const_format_args, env, file,
    format_args, format_args_nl, include, include_bytes, include_str, line, log_syntax,
    module_path, option_env, stringify, trace_macros,
};
// 重导出在 core 中定义的宏。
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::{
    assert_eq, assert_ne, debug_assert, debug_assert_eq, debug_assert_ne, r#try, unimplemented,
    unreachable, write, writeln,
};

// 重导出通过 core 定义的不稳定 derive 宏。
#[unstable(feature = "derive_from", issue = "144889")]
/// 包含不稳定的 `From` derive 宏的不稳定模块。
pub mod from {
    #[unstable(feature = "derive_from", issue = "144889")]
    pub use core::from::From;
}

// 包含若干私有模块，它们的唯一存在意义是为基本类型提供 rustdoc 文档。这里使用
// `include!`，因为 rustdoc 只会在 crate 顶层查找这些模块。
include!("../../core/src/primitive_docs.rs");

// 包含若干私有模块，它们的唯一存在意义是为现有的关键字提供 rustdoc 文档。这里使用
// `include!`，因为 rustdoc 只会在 crate 顶层查找这些模块。
include!("keyword_docs.rs");

// 当未启用 `restricted-std` 时，需要这一项来避免出现 unstable 错误。由于
// rustc-std-workspace-std 中对 #![feature(restricted_std)] 的使用是无条件的，
// 因此这个不稳定 feature 需要在某处被定义。
#[unstable(feature = "restricted_std", issue = "none")]
mod __restricted_std_workaround {}

mod sealed {
    /// 该 trait 无法从 crate 外部触及，
    /// 从而阻止外部对我们的扩展 trait 进行实现。
    /// 这让我们将来能够添加更多 trait 方法。
    #[unstable(feature = "sealed", issue = "none")]
    pub trait Sealed {}
}

#[cfg(test)]
#[allow(dead_code)] // 并非在所有配置下都会被用到。
pub(crate) mod test_helpers;
