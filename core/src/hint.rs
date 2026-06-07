#![stable(feature = "core_hint", since = "1.27.0")]

//! 给编译器的提示,用于影响代码应如何生成或优化。
//!
//! 这些提示可以作用于编译期,也可以作用于运行期。

use crate::marker::Destruct;
use crate::mem::MaybeUninit;
use crate::{intrinsics, ub_checks};

/// 告诉编译器调用本函数的位置不可达,从而可能启用更多优化。
///
/// # 安全性(Safety）
///
/// 执行到本函数是*未定义行为*。
///
/// 由于编译器假定所有形式的未定义行为都绝不会发生,它会删除周围代码中
/// 所有能够判定必然通向 `unreachable_unchecked()` 调用的分支。
///
/// 如果使用本函数时嵌入的假设后来被证明是错误的,也就是调用
/// `unreachable_unchecked()` 的位置在运行时实际上可达,编译器可能已经为
/// 这种情形生成了无意义的机器指令,甚至影响看起来无关的代码,从而造成
/// 难以调试的问题。
///
/// 请谨慎使用本函数。可以考虑使用 [`unreachable!`] 宏;它可能阻止部分优化,
/// 但如果运行时真的执行到该位置,会安全地 panic。请通过基准测试确认
/// `unreachable_unchecked()` 是否确实带来性能收益。
///
/// # 示例
///
/// 当编译器无法证明此前已经建立的不变量时,可以使用
/// `unreachable_unchecked()`。如果这些不变量由编译器无法分析的外部代码维护,
/// 这种情况更容易出现。
/// ```
/// fn prepare_inputs(divisors: &mut Vec<u32>) {
///     // 给之后修改代码的自己:这里建立的不变量不会在 `do_computation()` 中检查;
///     // 如果这里发生变化,必须同步修改 `do_computation()`。
///     divisors.retain(|divisor| *divisor != 0)
/// }
///
/// /// # 安全性(Safety）
/// /// `divisor` 的所有元素都必须非零。
/// unsafe fn do_computation(i: u32, divisors: &[u32]) -> u32 {
///     divisors.iter().fold(i, |acc, divisor| {
///         // 告诉编译器这里不可能发生除零,因此下面不需要检查。
///         if *divisor == 0 {
///             // 安全性:`prepare_inputs` 保证 `divisor` 不可能为零,
///             // 但编译器不知道这一点。我们*承诺*总是先调用 `prepare_inputs`。
///             unsafe { std::hint::unreachable_unchecked() }
///         }
///         // 编译器通常会在这里引入防止除零的检查。但如果 `divisor` 为零,
///         // 上面的分支会到达我们显式标记为不可达的位置。
///         // 编译器由此推断此处 `divisor` 不可能为零,并删除这个已被证明
///         // 无用的检查。
///         acc / divisor
///     })
/// }
///
/// let mut divisors = vec![2, 0, 4];
/// prepare_inputs(&mut divisors);
/// let result = unsafe {
///     // 安全性:prepare_inputs() 保证 divisors 中的元素都非零。
///     do_computation(100, &divisors)
/// };
/// assert_eq!(result, 12);
///
/// ```
///
/// 在下面的示例中使用 `unreachable_unchecked()` 本身是完全可靠的,
/// 因为编译器能够证明不可能发生除零;但基准测试表明,相比 [`unreachable!`],
/// `unreachable_unchecked()` 没有带来收益,而后者不会引入未定义行为的可能性。
///
/// ```
/// fn div_1(a: u32, b: u32) -> u32 {
///     use std::hint::unreachable_unchecked;
///
///     // `b.saturating_add(1)` 始终为正(非零),
///     // 因此 `checked_div` 永远不会返回 `None`。
///     // 所以 else 分支不可达。
///     a.checked_div(b.saturating_add(1))
///         .unwrap_or_else(|| unsafe { unreachable_unchecked() })
/// }
///
/// assert_eq!(div_1(7, 0), 7);
/// assert_eq!(div_1(9, 1), 4);
/// assert_eq!(div_1(11, u32::MAX), 0);
/// ```
#[inline]
#[stable(feature = "unreachable", since = "1.27.0")]
#[rustc_const_stable(feature = "const_unreachable_unchecked", since = "1.57.0")]
#[track_caller]
pub const unsafe fn unreachable_unchecked() -> ! {
    ub_checks::assert_unsafe_precondition!(
        check_language_ub,
        "hint::unreachable_unchecked must never be reached",
        () => false
    );
    // SAFETY: `intrinsics::unreachable` 的安全契约必须由调用者维护。
    unsafe { intrinsics::unreachable() }
}

/// 向编译器做出一个关于 `cond` 成立的*可靠性(soundness)*承诺。
///
/// 这可能允许优化器简化代码,但也可能让生成的代码变慢。无论如何,
/// 调用它很可能会增加编译时间。
///
/// 在其他地方,你可能见过类似的
/// [`llvm.assume`](https://llvm.org/docs/LangRef.html#llvm-assume-intrinsic),或在 C 中的
/// [`__builtin_assume`](https://clang.llvm.org/docs/LanguageExtensions.html#builtin-assume).
///
/// 它会把一个正确性要求提升为可靠性要求。没有充分理由时不要这样做。
///
/// # 用法
///
/// 这是一个用于微优化的情境性工具,并且允许什么都不做。任何使用都应当
/// 附带可重复的基准测试来证明其价值,并预期在优化器变得更聪明、不再
/// 需要它之后移除。
///
/// 条件越复杂,它越不可能有用。例如 `assert_unchecked(foo.is_sorted())`
/// 已经足够复杂,编译器不太可能从中获益。
///
/// 也没有必要对基本属性使用 `assert_unchecked`。例如,编译器已经知道
/// `count_ones` 的取值范围,因此
/// `let n = u32::count_ones(x); assert_unchecked(n <= u32::BITS);` 没有收益。
///
/// `assert_unchecked` 在逻辑上等价于 `if !cond { unreachable_unchecked(); }`。
/// 如果你想写 `assert_unchecked(false)`,应当直接使用 [`unreachable_unchecked()`]。
///
/// # 安全性(Safety）
///
/// `cond` 必须为 `true`。用 `false` 调用本函数会立即产生 UB。
///
/// # 示例
///
/// ```
/// use core::hint;
///
/// /// # 安全性(Safety）
/// ///
/// /// `p` 必须非空且有效。
/// pub unsafe fn next_value(p: *const i32) -> i32 {
///     // SAFETY: 调用者维护的不变量保证 `p` 非空。
///     unsafe { hint::assert_unchecked(!p.is_null()) }
///
///     if p.is_null() {
///         return -1;
///     } else {
///         // SAFETY: 调用者维护的不变量保证 `p` 有效。
///         unsafe { *p + 1 }
///     }
/// }
/// ```
///
/// 如果不使用 `assert_unchecked`,在启用优化时上面的函数会生成如下代码:
///
/// ```asm
/// next_value:
///         test    rdi, rdi
///         je      .LBB0_1
///         mov     eax, dword ptr [rdi]
///         inc     eax
///         ret
/// .LBB0_1:
///         mov     eax, -1
///         ret
/// ```
///
/// 添加断言后,优化器可以移除额外检查:
///
/// ```asm
/// next_value:
///         mov     eax, dword ptr [rdi]
///         inc     eax
///         ret
/// ```
///
/// 这个示例与现实代码相当不同:在检查同一件事的代码旁边放置断言是冗余的,
/// 而且解引用指针本身就带有该指针非空的内建假设。不过它展示了优化器
/// 即使在行为关联不那么明显时也可能做出的变化。
#[track_caller]
#[inline(always)]
#[doc(alias = "assume")]
#[stable(feature = "hint_assert_unchecked", since = "1.81.0")]
#[rustc_const_stable(feature = "hint_assert_unchecked", since = "1.81.0")]
pub const unsafe fn assert_unchecked(cond: bool) {
    // SAFETY: 调用者承诺 `cond` 为 true。
    unsafe {
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "hint::assert_unchecked must never be called when the condition is false",
            (cond: bool = cond) => cond,
        );
        crate::intrinsics::assume(cond);
    }
}

/// 发出一条机器指令,向处理器表明当前正在运行忙等自旋循环("spin lock")。
///
/// 处理器收到自旋循环信号后,可以优化自身行为,例如节省功耗或切换超线程。
///
/// 本函数不同于 [`thread::yield_now`]:后者会直接让出给系统调度器,
/// 而 `spin_loop` 不与操作系统交互。
///
/// `spin_loop` 的常见用法是在同步原语的 CAS 循环中实现有界的乐观自旋。
/// 为避免优先级反转等问题,强烈建议在有限次迭代后终止自旋循环,
/// 并改用合适的阻塞系统调用。
///
/// **注意**:在不支持接收自旋循环提示的平台上,本函数完全不做任何事。
///
/// # 示例
///
/// ```ignore-wasm
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use std::sync::Arc;
/// use std::{hint, thread};
///
/// // 供线程用于协调的共享原子值。
/// let live = Arc::new(AtomicBool::new(false));
///
/// // 在后台线程中,我们最终会设置该值。
/// let bg_work = {
///     let live = live.clone();
///     thread::spawn(move || {
///         // 执行一些工作,然后让该值变为 live。
///         do_some_work();
///         live.store(true, Ordering::Release);
///     })
/// };
///
/// // 回到当前线程,等待该值被设置。
/// while !live.load(Ordering::Acquire) {
///     // 自旋循环提示 CPU 我们正在等待,但大概率不会等很久。
///     hint::spin_loop();
/// }
///
/// // 该值现在已经设置。
/// # fn do_some_work() {}
/// do_some_work();
/// bg_work.join()?;
/// # Ok::<(), Box<dyn core::any::Any + Send + 'static>>(())
/// ```
///
/// [`thread::yield_now`]: ../../std/thread/fn.yield_now.html
#[inline(always)]
#[stable(feature = "renamed_spin_loop", since = "1.49.0")]
pub fn spin_loop() {
    crate::cfg_select! {
        miri => {
            unsafe extern "Rust" {
                safe fn miri_spin_loop();
            }

            // Miri 支持下面调用的部分 intrinsics,但为了保证跨目标行为一致,
            // 这里使用这个自定义函数。
            miri_spin_loop();
        }
        target_arch = "x86" => {
            // SAFETY: `cfg` 属性保证这段代码只会在 x86 目标上执行。
            crate::arch::x86::_mm_pause()
        }
        target_arch = "x86_64" => {
            // SAFETY: `cfg` 属性保证这段代码只会在 x86_64 目标上执行。
            crate::arch::x86_64::_mm_pause()
        }
        target_arch = "riscv32" => crate::arch::riscv32::pause(),
        target_arch = "riscv64" => crate::arch::riscv64::pause(),
        any(target_arch = "aarch64", target_arch = "arm64ec") => {
            // SAFETY: `cfg` 属性保证这段代码只会在 aarch64 目标上执行。
            unsafe { crate::arch::aarch64::__isb(crate::arch::aarch64::SY) }
        }
        all(target_arch = "arm", target_feature = "v6") => {
            // SAFETY: `cfg` 属性保证这段代码只会在支持 v6 feature 的 arm 目标上执行。
            unsafe { crate::arch::arm::__yield() }
        }
        target_arch = "loongarch32" => crate::arch::loongarch32::ibar::<0>(),
        target_arch = "loongarch64" => crate::arch::loongarch64::ibar::<0>(),
        _ => { /* 什么都不做 */ }
    }
}

/// 一个恒等函数,它向编译器 *__提示__* 应当尽可能悲观地看待 `black_box`
/// 可能做的事情。
///
/// 不同于 [`std::convert::identity`],Rust 编译器会被鼓励去假定 `black_box`
/// 可以用 Rust 代码允许的任何有效方式使用 `dummy`,只要不在调用代码中引入
/// 未定义行为。这个性质让 `black_box` 适合编写不希望发生某些优化的代码,
/// 例如基准测试。
///
/// <div class="warning">
///
/// 但请注意,`black_box` 只能以“尽力而为”的方式提供。它能阻止优化到什么程度,
/// 可能随平台和所使用的代码生成后端而变化。除了它表现为恒等函数这一点之外,
/// 程序不能依赖 `black_box` 来保证*正确性*。因此,**绝不能依赖它来控制关键程序行为。**
/// 这也意味着本函数不会为密码学或安全用途提供任何保证。
///
/// 这个限制并非 `black_box` 独有;整个 Rust 语言都没有一种机制可以提供
/// 常数时间密码学所需的保证。(LLVM 中也没有这样的机制,因此所有基于 LLVM
/// 的其他编译器也是如此。)
///
/// </div>
///
/// [`std::convert::identity`]: crate::convert::identity
///
/// # 何时有用?
///
/// 虽然它不适合上述关键场景,但 `black_box` 的功能通常可用于基准测试,
/// 并且也应当在此类场景中使用。它会尝试确保编译器不会根据上下文优化掉
/// 原本打算测试的部分代码。例如:
///
/// ```
/// fn contains(haystack: &[&str], needle: &str) -> bool {
///     haystack.iter().any(|x| x == &needle)
/// }
///
/// pub fn benchmark() {
///     let haystack = vec!["abc", "def", "ghi", "jkl", "mno"];
///     let needle = "ghi";
///     for _ in 0..10 {
///         contains(&haystack, needle);
///     }
/// }
/// ```
///
/// 理论上,编译器可能做出如下优化:
///
/// - `needle` 和 `haystack` 不变,把 `contains` 调用移到循环外并删除循环
/// - 内联 `contains`
/// - `needle` 和 `haystack` 的值在编译期已知,`contains` 总是 true,于是删除调用并替换为 `true`
/// - `contains` 的结果没有被使用:完全删除这次函数调用
/// - `benchmark` 已经没有用途:删除这个函数
///
/// 上述优化不太可能全部发生,但编译器确实能够做出某些会让基准结果严重失真的优化。
/// 这正是 `black_box` 的用武之地:
///
/// ```
/// use std::hint::black_box;
///
/// // 同一个 `contains` 函数。
/// fn contains(haystack: &[&str], needle: &str) -> bool {
///     haystack.iter().any(|x| x == &needle)
/// }
///
/// pub fn benchmark() {
///     let haystack = vec!["abc", "def", "ghi", "jkl", "mno"];
///     let needle = "ghi";
///     for _ in 0..10 {
///         // 强制编译器运行 `contains`,即使它是纯函数且结果未被使用。
///         black_box(contains(
///             // 阻止编译器对输入做出假设。
///             black_box(&haystack),
///             black_box(needle),
///         ));
///     }
/// }
/// ```
///
/// 这本质上是在告诉编译器阻断跨越任何 `black_box` 调用的优化。因此现在:
///
/// - 把 `contains` 的两个参数都视为不可预测:`contains` 的函数体无法再根据参数值优化
/// - 把 `contains` 调用及其结果视为 volatile:`benchmark` 的函数体不能把它优化掉
///
/// 这会让基准测试更接近函数的真实使用方式:参数通常不会在编译期已知,
/// 结果也会以某种方式被使用。
///
/// # 如何使用
///
/// 实践中,`black_box` 有两个用途:
///
/// 1. 阻止编译器围绕 `black_box` 返回的值进行相关优化
/// 2. 强制计算传给 `black_box` 的值,即使 `black_box` 的返回值未被使用
///
/// ```
/// use std::hint::black_box;
///
/// let zero = 0;
/// let five = 5;
///
/// // 编译器会看到这一点并删除 `* five` 调用,因为它知道任何整数乘以 0 都会得到 0。
/// let c = zero * five;
///
/// // 在这里添加 `black_box` 会禁用编译器推理乘法第一个操作数的能力。
/// // 它被迫假定该操作数可能是任意数,因此不能删除 `* five` 操作。
/// let c = black_box(zero) * five;
/// ```
///
/// 大多数情况不会像上面的示例这样清晰,但它仍然展示了 `black_box` 的用法。
/// 对函数做基准测试时,通常希望用 `black_box` 包裹其输入,使编译器无法做出
/// 与真实使用不符的优化。
///
/// ```
/// use std::hint::black_box;
///
/// // 这是一个简单函数,会把输入加 1。注意它是纯函数,也就是说没有副作用。
/// // 如果结果未被使用,该函数就没有效果。(带有副作用的函数示例是 `println!()`。)
/// fn increment(x: u8) -> u8 {
///     x + 1
/// }
///
/// // 这里我们调用 `increment` 但丢弃它的结果。编译器看到这一点并知道
/// // `increment` 是纯函数后,会完全消除这个函数调用。但这可能不是想要的结果,
/// // 尤其是在我们尝试测量 `increment` 执行耗时的时候。
/// let _ = increment(black_box(5));
///
/// // 这里我们强制执行 `increment`。这是因为编译器会把 `black_box`
/// // 当作有副作用处理,因此必须计算它的输入。
/// let _ = black_box(increment(black_box(5)));
/// ```
///
/// 还可能有其他场景需要用 `black_box` 包裹函数结果来强制执行该函数。
/// 这取决于具体场景,也可能没有任何效果(例如函数返回 [`()` unit][unit]
/// 这样的零大小类型时)。
///
/// 注意,`black_box` 只影响其输出如何被处理,不影响其输入如何被处理。
/// 因此,传给 `black_box` 的表达式仍可能被优化:
///
/// ```
/// use std::hint::black_box;
///
/// // 编译器看到的是这个...
/// let y = black_box(5 * 10);
///
/// // ...类似于这个。因此它很可能会把 `5 * 10` 直接简化为 `50`。
/// let _0 = 5 * 10;
/// let y = black_box(_0);
/// ```
///
/// 在上面的示例中,`5 * 10` 表达式被认为与 `black_box` 调用不同,
/// 因而仍会被编译器优化。可以通过把乘法操作移到 `black_box` 外部来阻止这一点:
///
/// ```
/// use std::hint::black_box;
///
/// // 不能对任一操作数做出假设,因此乘法不会被优化掉。
/// let y = black_box(5) * black_box(10);
/// ```
///
/// 在常量求值期间,`black_box` 会被视为无操作。
#[inline]
#[stable(feature = "bench_black_box", since = "1.66.0")]
#[rustc_const_stable(feature = "const_black_box", since = "1.86.0")]
pub const fn black_box<T>(dummy: T) -> T {
    crate::intrinsics::black_box(dummy)
}

/// 一个恒等函数:如果给定值未被调用者使用(返回、存入变量等),
/// 它会触发 `unused_must_use` 警告。
///
/// 它主要用于宏生成的代码。在这类代码中,把 [`#[must_use]` 属性][must_use]
/// 放在类型或函数上可能并不方便。
///
/// [must_use]: https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute
///
/// # 示例
///
/// ```
/// #![feature(hint_must_use)]
///
/// use core::fmt;
///
/// pub struct Error(/* ... */);
///
/// #[macro_export]
/// macro_rules! make_error {
///     ($($args:expr),*) => {
///         core::hint::must_use({
///             let error = $crate::make_error(core::format_args!($($args),*));
///             error
///         })
///     };
/// }
///
/// // make_error! 宏的实现细节。
/// #[doc(hidden)]
/// pub fn make_error(args: fmt::Arguments<'_>) -> Error {
///     Error(/* ... */)
/// }
///
/// fn demo() -> Option<Error> {
///     if true {
///         // 糟糕,原本想写的是 `return Some(make_error!("..."));`
///         Some(make_error!("..."));
///     }
///     None
/// }
/// #
/// # // 让 rustdoc 不把整个片段包进 fn main,从而使 $crate::make_error 正常工作。
/// # fn main() {}
/// ```
///
/// 在上面的示例中,我们希望 `unused_must_use` lint 作用于 `make_error!`
/// 创建的值。不过,在结构体上或函数上使用 `#[must_use]` 都不适合这里,
/// 因此宏改为展开成 `core::hint::must_use`。
///
/// - 不希望把 `#[must_use]` 放在 `struct Error` 上,因为那会让下面这段
///   本身没有问题的代码触发警告:
///
///   ```
///   # struct Error;
///   #
///   fn f(arg: &str) -> Result<(), Error>
///   # { Ok(()) }
///
///   #[test]
///   fn t() {
///       // 断言 `f` 在传入空字符串时返回错误。
///       // 这里有一个 `Error` 类型的值未被使用,但这不是问题。
///       f("").unwrap_err();
///   }
///   ```
///
/// - 在 `fn make_error` 上使用 `#[must_use]` 也没有帮助,因为返回值
///   *确实*作为 `let` 语句右侧被使用了。这个 `let` 语句看起来没用,
///   但实际上是必要的:它确保 `format_args` 展开中的临时值不会在创建
///   `Error` 之后继续存活,因为让它们跨过这个点存活可能在 async 代码中
///   引发 autotrait 问题:
///
///   ```
///   # #![feature(hint_must_use)]
///   #
///   # struct Error;
///   #
///   # macro_rules! make_error {
///   #     ($($args:expr),*) => {
///   #         core::hint::must_use({
///   #             // 如果不使用 `let`,那么 `f()` 会生成一个非 Send 的 future。
///   #             let error = make_error(core::format_args!($($args),*));
///   #             error
///   #         })
///   #     };
///   # }
///   #
///   # fn make_error(args: core::fmt::Arguments<'_>) -> Error {
///   #     Error
///   # }
///   #
///   async fn f() {
///       // 在 make_error 展开内部使用 `let`,会让 `unsync()` 等临时值在该
///       // `let` 语句的分号处被丢弃,也就是 await 点之前。否则它们会一直
///       // 存活到*本*语句的分号处,即 await 点之后,而包围它的 Future
///       // 将不会实现 Send。
///       log(make_error!("look: {:p}", unsync())).await;
///   }
///
///   async fn log(error: Error) {/* ... */}
///
///   // 返回一个没有 Sync impl 的东西。
///   fn unsync() -> *const () {
///       0 as *const ()
///   }
///   #
///   # fn test() {
///   #     fn assert_send(_: impl Send) {}
///   #     assert_send(f());
///   # }
///   ```
#[unstable(feature = "hint_must_use", issue = "94745")]
#[must_use] // <-- :)
#[inline(always)]
pub const fn must_use<T>(value: T) -> T {
    value
}

/// 提示编译器某个分支条件很可能为 true。
/// 返回传入的值。
///
/// 它可以与 `if` 或布尔 `match` 表达式配合使用。
///
/// 当在分支条件之外使用时,它仍可能影响附近的分支,但很可能没有效果。
///
/// 它也可以应用于表达式的一部分,例如 `likely(a) && unlikely(b)`,
/// 或应用于复合表达式,例如 `likely(a && b)`。应用于复合表达式时,
/// 它具有如下效果:
/// ```text
///     likely(!a) => !unlikely(a)
///     likely(a && b) => likely(a) && likely(b)
///     likely(a || b) => a || likely(b)
/// ```
///
/// 另请参见 [`cold_path()`] 函数;对惯用 Rust 代码而言,它可能更合适。
///
/// # 示例
///
/// ```
/// #![feature(likely_unlikely)]
/// use core::hint::likely;
///
/// fn foo(x: i32) {
///     if likely(x > 0) {
///         println!("this branch is likely to be taken");
///     } else {
///         println!("this branch is unlikely to be taken");
///     }
///
///     match likely(x > 0) {
///         true => println!("this branch is likely to be taken"),
///         false => println!("this branch is unlikely to be taken"),
///     }
///
///     // 在分支条件之外使用仍可能影响附近的分支。
///     let cond = likely(x != 0);
///     if cond {
///         println!("this branch is likely to be taken");
///     }
/// }
/// ```
#[unstable(feature = "likely_unlikely", issue = "136873")]
#[inline(always)]
pub const fn likely(b: bool) -> bool {
    crate::intrinsics::likely(b)
}

/// 提示编译器某个分支条件不太可能为 true。
/// 返回传入的值。
///
/// 它可以与 `if` 或布尔 `match` 表达式配合使用。
///
/// 当在分支条件之外使用时,它仍可能影响附近的分支,但很可能没有效果。
///
/// 它也可以应用于表达式的一部分,例如 `likely(a) && unlikely(b)`,
/// 或应用于复合表达式,例如 `unlikely(a && b)`。应用于复合表达式时,
/// 它具有如下效果:
/// ```text
///     unlikely(!a) => !likely(a)
///     unlikely(a && b) => a && unlikely(b)
///     unlikely(a || b) => unlikely(a) || unlikely(b)
/// ```
///
/// 另请参见 [`cold_path()`] 函数;对惯用 Rust 代码而言,它可能更合适。
///
/// # 示例
///
/// ```
/// #![feature(likely_unlikely)]
/// use core::hint::unlikely;
///
/// fn foo(x: i32) {
///     if unlikely(x > 0) {
///         println!("this branch is unlikely to be taken");
///     } else {
///         println!("this branch is likely to be taken");
///     }
///
///     match unlikely(x > 0) {
///         true => println!("this branch is unlikely to be taken"),
///         false => println!("this branch is likely to be taken"),
///     }
///
///     // 在分支条件之外使用仍可能影响附近的分支。
///     let cond = unlikely(x != 0);
///     if cond {
///         println!("this branch is likely to be taken");
///     }
/// }
/// ```
#[unstable(feature = "likely_unlikely", issue = "136873")]
#[inline(always)]
pub const fn unlikely(b: bool) -> bool {
    crate::intrinsics::unlikely(b)
}

/// 提示编译器给定路径是 cold 路径,也就是不太可能被执行。编译器可能选择
/// 以牺牲 cold 路径为代价来优化非 cold 路径。
///
/// # 示例
///
/// ```
/// #![feature(cold_path)]
/// use core::hint::cold_path;
///
/// fn foo(x: &[i32]) {
///     if let Some(first) = x.get(0) {
///         // 这是快速路径。
///     } else {
///         // 这条路径不太可能执行。
///         cold_path();
///     }
/// }
///
/// fn bar(x: i32) -> i32 {
///     match x {
///         1 => 10,
///         2 => 100,
///         3 => { cold_path(); 1000 }, // 这个分支不太可能执行。
///         _ => { cold_path(); 10000 }, // 这个分支同样不太可能执行。
///     }
/// }
/// ```
#[unstable(feature = "cold_path", issue = "136873")]
#[inline(always)]
pub const fn cold_path() {
    crate::intrinsics::cold_path()
}

/// 根据 `condition` 的值返回 `true_val` 或 `false_val`,并提示编译器
/// `condition` 不太可能被 CPU 的分支预测器正确预测。
///
/// 本方法在功能上等价于:
/// ```ignore (仅用于说明)
/// fn select_unpredictable<T>(b: bool, true_val: T, false_val: T) -> T {
///     if b { true_val } else { false_val }
/// }
/// ```
/// 但它可能生成不同的汇编。尤其是在具有条件移动或 select 指令的平台上
/// (例如 x86 上的 `cmov` 或 ARM 上的 `csel`),优化器可能使用这些指令
/// 来避免分支。如果分支预测器难以预测 `condition`,例如在二分搜索实现中,
/// 这可能改善性能。
///
/// 但请注意,这种 lowering 在任何平台上都没有保证,因此编写密码学常数时间
/// 代码时不能依赖它。还要注意,如果 `condition` 很容易预测,这种 lowering
/// 反而可能*降低*性能。建议通过基准测试判断本函数是否有用。
///
/// # 示例
///
/// 将值均匀分配到两个桶中:
/// ```
/// use std::hash::BuildHasher;
/// use std::hint;
///
/// fn append<H: BuildHasher>(hasher: &H, v: i32, bucket_one: &mut Vec<i32>, bucket_two: &mut Vec<i32>) {
///     let hash = hasher.hash_one(&v);
///     let bucket = hint::select_unpredictable(hash % 2 == 0, bucket_one, bucket_two);
///     bucket.push(v);
/// }
/// # let hasher = std::collections::hash_map::RandomState::new();
/// # let mut bucket_one = Vec::new();
/// # let mut bucket_two = Vec::new();
/// # append(&hasher, 42, &mut bucket_one, &mut bucket_two);
/// # assert_eq!(bucket_one.len() + bucket_two.len(), 1);
/// ```
#[inline(always)]
#[stable(feature = "select_unpredictable", since = "1.88.0")]
#[rustc_const_unstable(feature = "const_select_unpredictable", issue = "145938")]
pub const fn select_unpredictable<T>(condition: bool, true_val: T, false_val: T) -> T
where
    T: [const] Destruct,
{
    // FIXME(https://github.com/rust-lang/unsafe-code-guidelines/issues/245):
    // 改为使用 ManuallyDrop。
    let mut true_val = MaybeUninit::new(true_val);
    let mut false_val = MaybeUninit::new(false_val);

    struct DropOnPanic<T> {
        // 不变量:这是一个有效指针,指向一个已初始化且之后不再使用的值,
        // 也就是说该值可以由这个 guard 负责 drop。
        inner: *mut T,
    }

    impl<T> Drop for DropOnPanic<T> {
        fn drop(&mut self) {
            // SAFETY: 必须在构造局部类型 `DropOnPanic` 时保证。
            unsafe { self.inner.drop_in_place() }
        }
    }

    let true_ptr = true_val.as_mut_ptr();
    let false_ptr = false_val.as_mut_ptr();

    // SAFETY: 未被选中的值会被 drop,被选中的值会返回。
    // 这是必要的,因为 intrinsic 不会 drop 未被选中的值。
    unsafe {
        // 先提取被选中的值,确保如果 drop 未选中值时 panic,它也会被 drop。
        // 在 drop 未选中值期间,我们围绕被选中的值构造一个临时的按指针 guard。
        // 这里的参数会重叠,因此不能为这些参数使用可变引用。
        let guard = crate::intrinsics::select_unpredictable(condition, true_ptr, false_ptr);
        let drop = crate::intrinsics::select_unpredictable(condition, false_ptr, true_ptr);

        // SAFETY: 两个指针都正确对齐,并且各自指向 `MaybeUninit` 内部的已初始化值。
        // 对 `condition` 的两种可能取值而言,指针 `guard` 和 `drop` 都不会别名
        // (即使我们从中选择的两组参数彼此确实存在别名)。
        let guard = DropOnPanic { inner: guard };
        drop.drop_in_place();
        crate::mem::forget(guard);

        // 注意,这里使用这些值很重要。从拿到的指针读取有时会让 LLVM 忘记
        // !unpredictable 标注(测试中,整数大小的值尤其容易让它困惑;
        // 也可见 llvm/llvm-project #82340)。
        crate::intrinsics::select_unpredictable(condition, true_val, false_val).assume_init()
    }
}

/// 内存预取操作期望的时间局部性。
///
/// 局部性表示预取的数据有多大可能很快被再次使用,
/// 因而也表示应将其带入哪一级缓存。
///
/// 局部性只是一个提示,在某些目标上或由硬件决定时可能被忽略。
///
/// 与 [`prefetch_read`] 和 [`prefetch_write`] 等函数配合使用。
///
/// [`prefetch_read`]: crate::hint::prefetch_read
/// [`prefetch_write`]: crate::hint::prefetch_write
#[unstable(feature = "hint_prefetch", issue = "146941")]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locality {
    /// 数据预计最终会被再次使用。
    ///
    /// 通常预取到 L3 缓存(如果 CPU 支持)。
    L3,
    /// 数据预计在不久后会被再次使用。
    ///
    /// 通常预取到 L2 缓存。
    L2,
    /// 数据预计很快会被再次使用。
    ///
    /// 通常预取到 L1 缓存。
    L1,
}

impl Locality {
    /// 转换为 LLVM 为该局部性关联的常量。
    const fn to_llvm(self) -> i32 {
        match self {
            Self::L3 => 1,
            Self::L2 => 2,
            Self::L1 => 3,
        }
    }
}

/// 为未来的读取预取包含 `ptr` 的缓存行。
///
/// 如果数据很快会被访问,策略性放置的预取可以降低缓存未命中延迟,
/// 但也可能增加带宽使用或驱逐其他缓存行。
///
/// 预取是一个*提示*,在某些目标上或由硬件决定时可能被忽略。
///
/// 允许传入悬垂或无效指针:内存不会被实际解引用,也不会触发 fault。
///
/// # 示例
///
/// ```
/// #![feature(hint_prefetch)]
/// use std::hint::{Locality, prefetch_read};
/// use std::mem::size_of_val;
///
/// // 将整个 `slice` 预取到 L1 缓存。
/// fn prefetch_slice<T>(slice: &[T]) {
///     // 在多数系统上,缓存行大小为 64 字节。
///     for offset in (0..size_of_val(slice)).step_by(64) {
///         prefetch_read(slice.as_ptr().wrapping_add(offset), Locality::L1);
///     }
/// }
/// ```
#[inline(always)]
#[unstable(feature = "hint_prefetch", issue = "146941")]
pub const fn prefetch_read<T>(ptr: *const T, locality: Locality) {
    match locality {
        Locality::L3 => intrinsics::prefetch_read_data::<T, { Locality::L3.to_llvm() }>(ptr),
        Locality::L2 => intrinsics::prefetch_read_data::<T, { Locality::L2.to_llvm() }>(ptr),
        Locality::L1 => intrinsics::prefetch_read_data::<T, { Locality::L1.to_llvm() }>(ptr),
    }
}

/// 为一次未来读取预取包含 `ptr` 的缓存行,但尝试避免污染缓存。
///
/// 如果数据很快会被访问,策略性放置的预取可以降低缓存未命中延迟,
/// 但也可能增加带宽使用或驱逐其他缓存行。
///
/// 预取是一个*提示*,在某些目标上或由硬件决定时可能被忽略。
///
/// 允许传入悬垂或无效指针:内存不会被实际解引用,也不会触发 fault。
#[inline(always)]
#[unstable(feature = "hint_prefetch", issue = "146941")]
pub const fn prefetch_read_non_temporal<T>(ptr: *const T, locality: Locality) {
    // LLVM intrinsic 目前不支持指定局部性。
    let _ = locality;
    intrinsics::prefetch_read_data::<T, 0>(ptr)
}

/// 为未来的写入预取包含 `ptr` 的缓存行。
///
/// 如果数据很快会被访问,策略性放置的预取可以降低缓存未命中延迟,
/// 但也可能增加带宽使用或驱逐其他缓存行。
///
/// 预取是一个*提示*,在某些目标上或由硬件决定时可能被忽略。
///
/// 允许传入悬垂或无效指针:内存不会被实际解引用,也不会触发 fault。
#[inline(always)]
#[unstable(feature = "hint_prefetch", issue = "146941")]
pub const fn prefetch_write<T>(ptr: *mut T, locality: Locality) {
    match locality {
        Locality::L3 => intrinsics::prefetch_write_data::<T, { Locality::L3.to_llvm() }>(ptr),
        Locality::L2 => intrinsics::prefetch_write_data::<T, { Locality::L2.to_llvm() }>(ptr),
        Locality::L1 => intrinsics::prefetch_write_data::<T, { Locality::L1.to_llvm() }>(ptr),
    }
}

/// 为一次未来写入预取包含 `ptr` 的缓存行,但尝试避免污染缓存。
///
/// 如果数据很快会被访问,策略性放置的预取可以降低缓存未命中延迟,
/// 但也可能增加带宽使用或驱逐其他缓存行。
///
/// 预取是一个*提示*,在某些目标上或由硬件决定时可能被忽略。
///
/// 允许传入悬垂或无效指针:内存不会被实际解引用,也不会触发 fault。
#[inline(always)]
#[unstable(feature = "hint_prefetch", issue = "146941")]
pub const fn prefetch_write_non_temporal<T>(ptr: *const T, locality: Locality) {
    // LLVM intrinsic 目前不支持指定局部性。
    let _ = locality;
    intrinsics::prefetch_write_data::<T, 0>(ptr)
}

/// 为未来的读取把包含 `ptr` 的缓存行预取到指令缓存。
///
/// 如果这些指令很快会被访问,策略性放置的预取可以降低缓存未命中延迟,
/// 但也可能增加带宽使用或驱逐其他缓存行。
///
/// 预取是一个*提示*,在某些目标上或由硬件决定时可能被忽略。
///
/// 允许传入悬垂或无效指针:内存不会被实际解引用,也不会触发 fault。
#[inline(always)]
#[unstable(feature = "hint_prefetch", issue = "146941")]
pub const fn prefetch_read_instruction<T>(ptr: *const T, locality: Locality) {
    match locality {
        Locality::L3 => intrinsics::prefetch_read_instruction::<T, { Locality::L3.to_llvm() }>(ptr),
        Locality::L2 => intrinsics::prefetch_read_instruction::<T, { Locality::L2.to_llvm() }>(ptr),
        Locality::L1 => intrinsics::prefetch_read_instruction::<T, { Locality::L1.to_llvm() }>(ptr),
    }
}
