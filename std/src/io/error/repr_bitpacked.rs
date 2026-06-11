//! 这是一种密集压缩的错误表示，用于带 64 位指针的目标平台。
//!
//! （注意：这里的 `bitpacked`（位压缩）与 `unpacked`（未压缩）跟 `#[repr(packed)]`
//! 毫无关系，它仅仅是指：以比 `rustc` 默认布局算法更巧妙的方式，去利用任何可用的空闲位）。
//!
//! 在概念上，它存储的数据与我们在其他平台上使用的「未压缩」版本完全相同。具体而言，
//! 你可以把它想象成下面这个枚举的优化版本（这个枚举大致等价于 `repr_unpacked::Repr`
//! 所存储的内容，即 `super::ErrorData<Box<Custom>>`）：
//!
//! ```ignore (exposition-only)
//! enum ErrorData {
//!    Os(i32),
//!    Simple(ErrorKind),
//!    SimpleMessage(&'static SimpleMessage),
//!    Custom(Box<Custom>),
//! }
//! ```
//!
//! 不过，它把这些数据打包进一个 64 位的非零值里。
//!
//! 这项优化不仅让 `io::Error` 只占用一个指针的宽度，还顺带改善了 `io::Result`，
//! 尤其是 `io::Result<()>`（现在是 64 位）或 `io::Result<u64>`（现在是 128 位）
//! 这类相当常见的情况。
//!
//! # 布局（Layout）
//! 带标签的值是 64 位的，最低的 2 位用作标签（tag）。这意味着存在 4 种「变体」：
//!
//! - **标签 0b00**：第一种变体等价于 `ErrorData::SimpleMessage`，它直接持有一个
//!   `&'static SimpleMessage`。
//!
//!   `SimpleMessage` 的对齐 >= 4（这通过 `#[repr(align)]` 请求，并在本文件末尾做静态检查），
//!   这意味着每个 `&'static SimpleMessage` 的两个标签位都应该是 0，也就是说它带标签和不带标签
//!   的表示是相同的。
//!
//!   这意味着我们可以跳过对它的打标签操作，而这是必需的：因为该变体可以由 `const fn` 构造，
//!   而 `const fn` 很可能无法给指针打标签（或者至少会很困难）。
//!
//! - **标签 0b01**：另一种指针变体持有 `ErrorData::Custom` 的数据，剩下的 62 位用来存储一个
//!   `Box<Custom>`。`Custom` 的对齐同样 >= 4，所以最低两位可以自由用作标签。
//!
//!   唯一需要注意的重点是：给指针打标签时用的是 `ptr::wrapping_add` 和 `ptr::wrapping_sub`，
//!   而不是按位运算。这样能保留指针的来源（provenance），否则来源信息会丢失。
//!
//! - **标签 0b10**：持有 `ErrorData::Os(i32)` 的数据。我们把这个 `i32` 存放在指针最高的 32 位里，
//!   而第 `2..32` 位则不作任何用途。使用最高的 32 位只是为了让我们能够方便地以正确的符号恢复出
//!   `i32` 错误码。
//!
//! - **标签 0b11**：持有 `ErrorData::Simple(ErrorKind)` 的数据。它同样把 `ErrorKind` 存在最高的
//!   32 位里，尽管它远远用不了那么多位。这里大部分位都是闲置的，不过反正我们暂时也没别的用处。
//!
//! # 关于 `NonNull<()>` 的使用
//!
//! 所有内容都存储在一个 `NonNull<()>` 里。这看起来有点奇怪，但实际上是有意为之。
//!
//! 在概念上，你或许会更倾向于这样去理解它：
//!
//! ```ignore (exposition-only)
//! union Repr {
//!     // 持有整数（Simple/Os）变体，
//!     // 并提供对标签位的访问。
//!     bits: NonZero<u64>,
//!     // 标签为 0，所以这个不带标签存储。
//!     msg: &'static SimpleMessage,
//!     // 带标签（带偏移）的 `Box<Custom>` 指针。
//!     tagged_custom: NonNull<()>,
//! }
//! ```
//!
//! 但这种做法有几个问题：
//!
//! 1. union 的访问等价于一次 transmute，所以这种表示至少会要求我们在某一个方向上在整数和指针之间
//!    做 transmute，而这可能是 UB（即便不是，编译器对它进行推理也很可能比对显式的「指针->整数」
//!    操作更困难）。
//!
//! 2. 即便一个 union 的所有字段都有 niche，该 union 本身也没有 niche（尽管这一点未来可能会改变）。
//!    这会让 `io::Result<()>` 和 `io::Result<usize>` 之类的类型变大，从而抵消了这套位压缩的部分动机。
//!
//! 把所有内容存进一个 `NonZero<usize>`（或别的某种整数）会是更传统的指针打标签方式，但那样会丢失
//! 来源（provenance）信息，无法由 `const fn` 构造，而且很可能还会撞上其他问题。
//!
//! `NonNull<()>` 似乎是唯一的替代方案，尽管用一个指针类型来存储某些时候可能持有整数的东西确实相当奇怪。

use core::marker::PhantomData;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

use super::{Custom, ErrorData, ErrorKind, RawOsError, SimpleMessage};

// 最低的 2 位用作标签（tag）。
const TAG_MASK: usize = 0b11;
const TAG_SIMPLE_MESSAGE: usize = 0b00;
const TAG_CUSTOM: usize = 0b01;
const TAG_OS: usize = 0b10;
const TAG_SIMPLE: usize = 0b11;

/// 内部表示。
///
/// 详见模块文档。这里的 trait bound 只是用来塞进一个检查，以验证我们确实不是 unwind-safe 的。
///
/// ```compile_fail,E0277
/// fn is_unwind_safe<T: core::panic::UnwindSafe>() {}
/// is_unwind_safe::<std::io::Error>();
/// ```
#[repr(transparent)]
#[rustc_insignificant_dtor]
pub(super) struct Repr(NonNull<()>, PhantomData<ErrorData<Box<Custom>>>);

// `Repr` 内部存储的所有类型都是 Send + Sync 的，所以它本身也是。
unsafe impl Send for Repr {}
unsafe impl Sync for Repr {}

impl Repr {
    pub(super) fn new_custom(b: Box<Custom>) -> Self {
        let p = Box::into_raw(b).cast::<u8>();
        // 只有当分配器交回了一个对齐错误的指针时才可能触发。
        debug_assert_eq!(p.addr() & TAG_MASK, 0);
        // 注意：我们知道 `TAG_CUSTOM <= size_of::<Custom>()`（见本文件末尾的 static_assert），
        // 并且由于 `Box` 的语义，该表达式的起点和终点都必须有效、不会发生地址空间回绕。
        //
        // 这意味着用 `ptr::add`（而非 `ptr::wrapping_add`）来实现也是正确的，但尚不清楚这样做
        // 是否有任何收益，所以我们干脆就用 `wrapping_add`。
        let tagged = p.wrapping_add(TAG_CUSTOM).cast::<()>();
        // 安全性：`TAG_CUSTOM + p` 与 `TAG_CUSTOM | p` 相同，因为 `p` 的对齐意味着它不可能
        // 设置任何一个 `TAG_BITS` 位（你可以用真值表验证：当两个操作数没有任何公共置位时，
        // 加法和按位或是等价的）。
        //
        // 接着，`TAG_CUSTOM | p` 不为零，因为那样要求 `TAG_CUSTOM` 和 `p` 都为零，而两者都不是
        //（`p` 来自一个 box，而 `TAG_CUSTOM` 嘛……它就不是零——它是 `0b01`）。因此
        // `TAG_CUSTOM + p` 不为零，从而 `tagged` 也不可能为零，于是 `new_unchecked` 是安全的。
        let res = Self(unsafe { NonNull::new_unchecked(tagged) }, PhantomData);
        // 快速冒烟检查我们编码的内容是否正确（这通常只在 std 的测试中运行，除非用户使用
        // -Zbuild-std）
        debug_assert!(matches!(res.data(), ErrorData::Custom(_)), "repr(custom) encoding failed");
        res
    }

    #[inline]
    pub(super) fn new_os(code: RawOsError) -> Self {
        let utagged = ((code as usize) << 32) | TAG_OS;
        // 安全性：`TAG_OS` 不为零，所以 `|` 的结果不为 0。
        let res = Self(
            NonNull::without_provenance(unsafe { NonZeroUsize::new_unchecked(utagged) }),
            PhantomData,
        );
        // 快速冒烟检查我们编码的内容是否正确（这通常只在 std 的测试中运行，除非用户使用
        // -Zbuild-std）
        debug_assert!(
            matches!(res.data(), ErrorData::Os(c) if c == code),
            "repr(os) encoding failed for {code}"
        );
        res
    }

    #[inline]
    pub(super) fn new_simple(kind: ErrorKind) -> Self {
        let utagged = ((kind as usize) << 32) | TAG_SIMPLE;
        // 安全性：`TAG_SIMPLE` 不为零，所以 `|` 的结果不为 0。
        let res = Self(
            NonNull::without_provenance(unsafe { NonZeroUsize::new_unchecked(utagged) }),
            PhantomData,
        );
        // 快速冒烟检查我们编码的内容是否正确（这通常只在 std 的测试中运行，除非用户使用
        // -Zbuild-std）
        debug_assert!(
            matches!(res.data(), ErrorData::Simple(k) if k == kind),
            "repr(simple) encoding failed {:?}",
            kind,
        );
        res
    }

    #[inline]
    pub(super) const fn new_simple_message(m: &'static SimpleMessage) -> Self {
        // 安全性：引用永远不会为 null。
        Self(unsafe { NonNull::new_unchecked(m as *const _ as *mut ()) }, PhantomData)
    }

    #[inline]
    pub(super) fn data(&self) -> ErrorData<&Custom> {
        // 安全性：我们是一个 Repr，所以调用 decode_repr 没问题。
        unsafe { decode_repr(self.0, |c| &*c) }
    }

    #[inline]
    pub(super) fn data_mut(&mut self) -> ErrorData<&mut Custom> {
        // 安全性：我们是一个 Repr，所以调用 decode_repr 没问题。
        unsafe { decode_repr(self.0, |c| &mut *c) }
    }

    #[inline]
    pub(super) fn into_data(self) -> ErrorData<Box<Custom>> {
        let this = core::mem::ManuallyDrop::new(self);
        // 安全性：我们是一个 Repr，所以调用 decode_repr 没问题。`Box::from_raw` 是安全的，
        // 因为我们用 `ManuallyDrop` 防止了重复析构（double-drop）。
        unsafe { decode_repr(this.0, |p| Box::from_raw(p)) }
    }
}

impl Drop for Repr {
    #[inline]
    fn drop(&mut self) {
        // 安全性：我们是一个 Repr，所以调用 decode_repr 没问题。`Box::from_raw` 是安全的，
        // 因为我们正在被析构。
        unsafe {
            let _ = decode_repr(self.0, |p| Box::<Custom>::from_raw(p));
        }
    }
}

// 共享的辅助函数，用于把一个 `Repr` 的内部指针解码为 ErrorData。
//
// 安全性：`ptr` 的各位必须按本文件顶部所述方式编码（它应该是 `some_repr.0`）。
#[inline]
unsafe fn decode_repr<C, F>(ptr: NonNull<()>, make_custom: F) -> ErrorData<C>
where
    F: FnOnce(*mut Custom) -> C,
{
    let bits = ptr.as_ptr().addr();
    match bits & TAG_MASK {
        TAG_OS => {
            let code = ((bits as i64) >> 32) as RawOsError;
            ErrorData::Os(code)
        }
        TAG_SIMPLE => {
            let kind_bits = (bits >> 32) as u32;
            let kind = kind_from_prim(kind_bits).unwrap_or_else(|| {
                debug_assert!(false, "Invalid io::error::Repr bits: `Repr({:#018x})`", bits);
                // 这意味着传入的 `ptr` 是无效的，违反了 `decode_repr` 的 unsafe 契约。
                //
                // 相比 unwrap，使用这种写法能切实改善那些只关心某一个变体（通常是 `Custom`）
                // 的调用方所生成的代码。
                unsafe { core::hint::unreachable_unchecked() };
            });
            ErrorData::Simple(kind)
        }
        TAG_SIMPLE_MESSAGE => {
            // 安全性：依据标签可知。
            unsafe { ErrorData::SimpleMessage(&*ptr.cast::<SimpleMessage>().as_ptr()) }
        }
        TAG_CUSTOM => {
            // 这里使用 `ptr::byte_sub` 也是正确的（原因见 `new_custom` 中 `wrapping_add`
            // 调用上方的注释），但尚不清楚这是否有什么区别，所以我们没这么做。
            let custom = ptr.as_ptr().wrapping_byte_sub(TAG_CUSTOM).cast::<Custom>();
            ErrorData::Custom(make_custom(custom))
        }
        _ => {
            // 不可能发生，编译器也能推断出来。
            unreachable!();
        }
    }
}

// 这编译出的代码与「检查 + transmute」相同，但不需要 unsafe，也不需要以编译器无法验证的方式
// 硬编码 ErrorKind 的最大值或其大小。
#[inline]
fn kind_from_prim(ek: u32) -> Option<ErrorKind> {
    macro_rules! from_prim {
        ($prim:expr => $Enum:ident { $($Variant:ident),* $(,)? }) => {{
            // 如果这份列表过时，则强制产生一个编译错误。
            const _: fn(e: $Enum) = |e: $Enum| match e {
                $($Enum::$Variant => ()),*
            };
            match $prim {
                $(v if v == ($Enum::$Variant as _) => Some($Enum::$Variant),)*
                _ => None,
            }
        }}
    }
    from_prim!(ek => ErrorKind {
        NotFound,
        PermissionDenied,
        ConnectionRefused,
        ConnectionReset,
        HostUnreachable,
        NetworkUnreachable,
        ConnectionAborted,
        NotConnected,
        AddrInUse,
        AddrNotAvailable,
        NetworkDown,
        BrokenPipe,
        AlreadyExists,
        WouldBlock,
        NotADirectory,
        IsADirectory,
        DirectoryNotEmpty,
        ReadOnlyFilesystem,
        FilesystemLoop,
        StaleNetworkFileHandle,
        InvalidInput,
        InvalidData,
        TimedOut,
        WriteZero,
        StorageFull,
        NotSeekable,
        QuotaExceeded,
        FileTooLarge,
        ResourceBusy,
        ExecutableFileBusy,
        Deadlock,
        CrossesDevices,
        TooManyLinks,
        InvalidFilename,
        ArgumentListTooLong,
        Interrupted,
        Other,
        UnexpectedEof,
        Unsupported,
        OutOfMemory,
        InProgress,
        Uncategorized,
    })
}

// 做一些静态检查，以便在某项改动破坏了我们的编码所依赖的正确性与可靠性（soundness）假设时
// 向我们发出警示。（必须承认，其中有些检查略显过度周全/谨慎）
//
// 如果在某个 std 支持的平台上触发了其中任何一项，那我们多半应该在那个平台改用
// `repr_unpacked.rs`（除非修复很容易）。
macro_rules! static_assert {
    ($condition:expr) => {
        const _: () = assert!($condition);
    };
    (@usize_eq: $lhs:expr, $rhs:expr) => {
        const _: [(); $lhs] = [(); $rhs];
    };
}

// 我们使用的位压缩要求指针恰好是 64 位。
static_assert!(@usize_eq: size_of::<NonNull<()>>(), 8);

// 我们还要求指针和 usize 大小相同。
static_assert!(@usize_eq: size_of::<NonNull<()>>(), size_of::<usize>());

// `Custom` 和 `SimpleMessage` 需要是「瘦」指针（thin pointer）。
static_assert!(@usize_eq: size_of::<&'static SimpleMessage>(), 8);
static_assert!(@usize_eq: size_of::<Box<Custom>>(), 8);

static_assert!((TAG_MASK + 1).is_power_of_two());
// 而且它们必须有足够的对齐。
static_assert!(align_of::<SimpleMessage>() >= TAG_MASK + 1);
static_assert!(align_of::<Custom>() >= TAG_MASK + 1);

static_assert!(@usize_eq: TAG_MASK & TAG_SIMPLE_MESSAGE, TAG_SIMPLE_MESSAGE);
static_assert!(@usize_eq: TAG_MASK & TAG_CUSTOM, TAG_CUSTOM);
static_assert!(@usize_eq: TAG_MASK & TAG_OS, TAG_OS);
static_assert!(@usize_eq: TAG_MASK & TAG_SIMPLE, TAG_SIMPLE);

// 这显然为真（`TAG_CUSTOM` 是 `0b01`），但在 `Repr::new_custom` 中我们会把一个指针偏移这个值，
// 并期望偏移后的指针既位于同一个对象之内，又不会在地址空间里发生回绕。详见该函数中的注释。
//
// 实际上，目前我们用的是 `ptr::wrapping_add` 而非 `ptr::add`，所以对那处而言这个检查并非必需，
// 不过「在那次 wrapping_add 中我们确实不会发生回绕」这一断言，确实能大大简化别处的安全性推理。
static_assert!(size_of::<Custom>() >= TAG_CUSTOM);

// 这两者存储的载荷允许为零，所以它们本身必须非零，以维持 `NonNull` 的取值范围不变量。
static_assert!(TAG_OS != 0);
static_assert!(TAG_SIMPLE != 0);
// 我们无法给 `SimpleMessage` 打标签，其标签必须为 0。
static_assert!(@usize_eq: TAG_SIMPLE_MESSAGE, 0);

// 检查这一切的初衷是否仍然成立。
//
// 我们本想拿 `io::Error` 来检查，但「技术上」它的大小是允许变化的，因为它并非
// `#[repr(transparent)]`/`#[repr(C)]`。我们可以加上那个属性，但 `#[repr()]` 会出现在 rustdoc 中，
// 这可能被视为一种稳定性承诺。
static_assert!(@usize_eq: size_of::<Repr>(), 8);
static_assert!(@usize_eq: size_of::<Option<Repr>>(), 8);
static_assert!(@usize_eq: size_of::<Result<(), Repr>>(), 8);
static_assert!(@usize_eq: size_of::<Result<usize, Repr>>(), 16);
