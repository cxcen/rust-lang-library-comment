//! [`CStr`] 及其相关类型。

use crate::cmp::Ordering;
use crate::error::Error;
use crate::ffi::c_char;
use crate::intrinsics::const_eval_select;
use crate::iter::FusedIterator;
use crate::marker::PhantomData;
use crate::ptr::NonNull;
use crate::slice::memchr;
use crate::{fmt, ops, slice, str};

// FIXME: 由于这里会被 doc(inline),实际链接取决于条目被文档化的位置,因此必须使用
// intra-doc links。但这里是 libcore,不能在 intra-doc links 中实际引用 libstd 或 liballoc。
// 所以在找到方案前,目前最好的办法是先移除指向 `CString` 和 `String` 的链接。

/// C 字符串的动态大小视图。
///
/// `&CStr` 表示对借来的、以 nul 终止的字节数组的引用。它可以安全地从
/// <code>&[[u8]]</code> 切片构造,也可以 unsafe 地从原始 `*const c_char` 构造。
/// 它还可以写成 `c"Hello world"` 形式的字面量。
///
/// 随后可通过 UTF-8 校验把 `&CStr` 转成 Rust <code>&[str]</code>,或转成拥有所有权的
/// `CString`。
///
/// `&CStr` 与 `CString` 的关系,就像 <code>&[str]</code> 与 `String` 的关系:
/// 每组前者是借用引用,后者是拥有所有权的字符串。
///
/// 注意,本结构**没有**保证布局(即使带有 `repr(transparent)`),不应出现在 FFI 函数签名中。
/// FFI 函数的安全包装应改用 [`CStr::as_ptr`] 与 unsafe 构造器 [`CStr::from_ptr`],
/// 为其他调用方提供安全接口。
///
/// # 示例
///
/// 检查外部 C 字符串:
///
/// ```
/// use std::ffi::CStr;
/// use std::os::raw::c_char;
///
/// # /* 外部函数在文档注释里很别扭——这里改用伪造的方式
/// extern "C" { fn my_string() -> *const c_char; }
/// # */ unsafe extern "C" fn my_string() -> *const c_char { c"hello".as_ptr() }
///
/// unsafe {
///     let slice = CStr::from_ptr(my_string());
///     println!("string buffer size without nul terminator: {}", slice.to_bytes().len());
/// }
/// ```
///
/// 传递源自 Rust 的 C 字符串:
///
/// ```
/// use std::ffi::CStr;
/// use std::os::raw::c_char;
///
/// fn work(data: &CStr) {
///     unsafe extern "C" fn work_with(s: *const c_char) {}
///     unsafe { work_with(data.as_ptr()) }
/// }
///
/// let s = c"Hello world!";
/// work(&s);
/// ```
///
/// 把外部 C 字符串转换为 Rust `String`:
///
/// ```
/// use std::ffi::CStr;
/// use std::os::raw::c_char;
///
/// # /* 外部函数在文档注释里很别扭——这里改用伪造的方式
/// extern "C" { fn my_string() -> *const c_char; }
/// # */ unsafe extern "C" fn my_string() -> *const c_char { c"hello".as_ptr() }
///
/// fn my_string_safe() -> String {
///     let cstr = unsafe { CStr::from_ptr(my_string()) };
///     // 取得 copy-on-write 的 Cow<'_, str>,再取出其中已分配的 String
///     // (必要时会新分配)。
///     cstr.to_string_lossy().into_owned()
/// }
///
/// println!("string: {}", my_string_safe());
/// ```
///
/// [str]: prim@str "str"
#[derive(PartialEq, Eq, Hash)]
#[stable(feature = "core_c_str", since = "1.64.0")]
#[rustc_diagnostic_item = "cstr_type"]
#[rustc_has_incoherent_inherent_impls]
#[lang = "CStr"]
// 当前 `impl From<&CStr> for Box<CStr>` 中的 `fn from` 实现依赖 `CStr` 与 `[u8]`
// 布局兼容。但 `CStr` 布局被视为实现细节,调用方不得依赖。我们需要 `repr(transparent)`,
// 又不希望它出现在 rustdoc 中,因此把它藏在 `cfg(doc)` 下。这是属性隐私的临时实现。
#[repr(transparent)]
pub struct CStr {
    // FIXME: 这里不应以 DST 切片表示,而应只用一个原始 `c_char` 加某种 marker
    //        让它成为 unsized 类型。本质上,`sizeof(&CStr)` 应与 `sizeof(&c_char)` 相同,
    //        但 `CStr` 自身应是 unsized 类型。
    inner: [c_char],
}

/// 表示 nul 字节不在预期位置的错误。
///
/// 用于创建 [`CStr`] 的切片必须有且仅有一个 nul 字节,并且该字节位于末尾。
///
/// 本错误由 [`CStr::from_bytes_with_nul`] 方法创建。更多信息见该方法文档。
///
/// # 示例
///
/// ```
/// use std::ffi::{CStr, FromBytesWithNulError};
///
/// let _: FromBytesWithNulError = CStr::from_bytes_with_nul(b"f\0oo").unwrap_err();
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[stable(feature = "core_c_str", since = "1.64.0")]
pub enum FromBytesWithNulError {
    /// 提供的数据在字节 `position` 处包含内部 nul 字节。
    InteriorNul {
        /// 内部 nul 字节的位置。
        position: usize,
    },
    /// 提供的数据没有以 nul 终止。
    NotNulTerminated,
}

#[stable(feature = "frombyteswithnulerror_impls", since = "1.17.0")]
impl fmt::Display for FromBytesWithNulError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InteriorNul { position } => {
                write!(f, "data provided contains an interior nul byte at byte position {position}")
            }
            Self::NotNulTerminated => write!(f, "data provided is not nul terminated"),
        }
    }
}

#[stable(feature = "frombyteswithnulerror_impls", since = "1.17.0")]
impl Error for FromBytesWithNulError {}

/// 表示不存在 nul 字节的错误。
///
/// 用于创建 [`CStr`] 的切片必须在某处包含一个 nul 字节。
///
/// 本错误由 [`CStr::from_bytes_until_nul`] 方法创建。
#[derive(Clone, PartialEq, Eq, Debug)]
#[stable(feature = "cstr_from_bytes_until_nul", since = "1.69.0")]
pub struct FromBytesUntilNulError(());

#[stable(feature = "cstr_from_bytes_until_nul", since = "1.69.0")]
impl fmt::Display for FromBytesUntilNulError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "data provided does not contain a nul")
    }
}

/// 把底层字节显示为普通字符串;无效 UTF-8 会以十六进制转义序列呈现。
#[stable(feature = "cstr_debug", since = "1.3.0")]
impl fmt::Debug for CStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(crate::bstr::ByteStr::from_bytes(self.to_bytes()), f)
    }
}

#[stable(feature = "cstr_default", since = "1.10.0")]
impl Default for &CStr {
    #[inline]
    fn default() -> Self {
        c""
    }
}

impl CStr {
    /// 用安全的 C 字符串包装器包裹原始 C 字符串。
    ///
    /// 本函数会把提供的 `ptr` 包装成 `CStr`,从而允许检查和互操作未拥有所有权的 C 字符串。
    /// 已终止缓冲区在内存中的总大小必须小于 [`isize::MAX`] **字节**
    /// (这是 [`slice::from_raw_parts`] 的限制)。
    ///
    /// # 安全性(Safety）
    ///
    /// * `ptr` 指向的内存必须在字符串末尾包含一个有效的 nul 终止符。
    ///
    /// * `ptr` 对直到并包含 nul 终止符的全部字节读取都必须[有效][valid]。
    ///   这尤其意味着:
    ///
    ///     * 这个 `CStr` 的整个内存范围必须位于同一个 allocation 内!
    ///     * 即使是零长度 cstr,`ptr` 也必须非空。
    ///
    /// * 返回的 `CStr` 所引用的内存在生命周期 `'a` 期间不得被修改。
    ///
    /// * nul 终止符与 `ptr` 的距离必须小于等于 `isize::MAX`。
    ///
    /// > **注意**:本操作意图上是零成本转换,但当前实现会先计算字符串长度。
    /// > 未来不保证始终如此。
    ///
    /// # 注意事项
    ///
    /// 返回切片的生命周期会从使用方式中推断。为防止意外误用,建议把该生命周期绑定到当前上下文中
    /// 安全的来源生命周期上,例如提供一个辅助函数,让它接收承载该切片的宿主值生命周期,
    /// 或显式写出生命周期标注。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::{c_char, CStr};
    ///
    /// fn my_string() -> *const c_char {
    ///     c"hello".as_ptr()
    /// }
    ///
    /// unsafe {
    ///     let slice = CStr::from_ptr(my_string());
    ///     assert_eq!(slice.to_str().unwrap(), "hello");
    /// }
    /// ```
    ///
    /// ```
    /// use std::ffi::{c_char, CStr};
    ///
    /// const HELLO_PTR: *const c_char = {
    ///     const BYTES: &[u8] = b"Hello, world!\0";
    ///     BYTES.as_ptr().cast()
    /// };
    /// const HELLO: &CStr = unsafe { CStr::from_ptr(HELLO_PTR) };
    ///
    /// assert_eq!(c"Hello, world!", HELLO);
    /// ```
    ///
    /// [valid]: core::ptr#safety
    #[inline] // inline 是必要的，让 codegen 能看到 strlen。
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_cstr_from_ptr", since = "1.81.0")]
    pub const unsafe fn from_ptr<'a>(ptr: *const c_char) -> &'a CStr {
        // SAFETY: 调用方已经提供指向有效 C 字符串的指针,且 NUL 终止符距离 `ptr`
        // 小于 `isize::MAX`。
        let len = unsafe { strlen(ptr) };

        // SAFETY: 调用方提供的指针有效,且长度小于 `isize::MAX`,因此 `from_raw_parts`
        // 是安全的。内容在返回的 `CStr` 生命周期内保持有效且不改变,所以调用
        // `from_bytes_with_nul_unchecked` 是正确的。
        //
        // 从 c_char 转成 u8 是可以的,因为 c_char 始终是一个字节。
        unsafe { Self::from_bytes_with_nul_unchecked(slice::from_raw_parts(ptr.cast(), len + 1)) }
    }

    /// 从包含任意数量 nul 的字节切片创建 C 字符串包装器。
    ///
    /// 本方法可从任意至少包含一个 nul 字节的字节切片创建 `CStr`。不同于
    /// [`CStr::from_bytes_with_nul`],调用方不需要知道 nul 字节位于何处。
    ///
    /// 若第一个字节就是 nul 字符,本方法返回空 `CStr`。若存在多个 nul 字符,`CStr`
    /// 会在第一个 nul 处结束。
    ///
    /// 若切片仅在末尾有一个 nul 字节,本方法等价于 [`CStr::from_bytes_with_nul`]。
    ///
    /// # 示例
    /// ```
    /// use std::ffi::CStr;
    ///
    /// let mut buffer = [0u8; 16];
    /// unsafe {
    ///     // 这里可能会调用一个 unsafe C 函数,把字符串写入缓冲区。
    ///     let buf_ptr = buffer.as_mut_ptr();
    ///     buf_ptr.write_bytes(b'A', 8);
    /// }
    /// // 尝试从缓冲区中提取以 nul 终止的 C 字符串。
    /// let c_str = CStr::from_bytes_until_nul(&buffer[..]).unwrap();
    /// assert_eq!(c_str.to_str().unwrap(), "AAAAAAAA");
    /// ```
    ///
    #[stable(feature = "cstr_from_bytes_until_nul", since = "1.69.0")]
    #[rustc_const_stable(feature = "cstr_from_bytes_until_nul", since = "1.69.0")]
    pub const fn from_bytes_until_nul(bytes: &[u8]) -> Result<&CStr, FromBytesUntilNulError> {
        let nul_pos = memchr::memchr(0, bytes);
        match nul_pos {
            Some(nul_pos) => {
                // FIXME(const-hack) 替换为范围索引。
                // SAFETY: `nul_pos + 1 <= bytes.len()`。
                let subslice = unsafe { crate::slice::from_raw_parts(bytes.as_ptr(), nul_pos + 1) };
                // SAFETY: 已知 `nul_pos` 处存在 nul 字节,因此这个以 nul 字节结尾的切片
                // 是格式良好的 C 字符串。
                Ok(unsafe { CStr::from_bytes_with_nul_unchecked(subslice) })
            }
            None => Err(FromBytesUntilNulError(())),
        }
    }

    /// 从恰好包含一个 nul 终止符的字节切片创建 C 字符串包装器。
    ///
    /// 本函数会先确认字节切片以 nul 终止且不含任何内部 nul 字节,再把提供的 `bytes`
    /// 转换为 `CStr` 包装器。
    ///
    /// 若 nul 字节可能不在末尾,可改用 [`CStr::from_bytes_until_nul`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::CStr;
    ///
    /// let cstr = CStr::from_bytes_with_nul(b"hello\0");
    /// assert_eq!(cstr, Ok(c"hello"));
    /// ```
    ///
    /// 创建缺少尾随 nul 终止符的 `CStr` 会报错:
    ///
    /// ```
    /// use std::ffi::{CStr, FromBytesWithNulError};
    ///
    /// let cstr = CStr::from_bytes_with_nul(b"hello");
    /// assert_eq!(cstr, Err(FromBytesWithNulError::NotNulTerminated));
    /// ```
    ///
    /// 创建包含内部 nul 字节的 `CStr` 会报错:
    ///
    /// ```
    /// use std::ffi::{CStr, FromBytesWithNulError};
    ///
    /// let cstr = CStr::from_bytes_with_nul(b"he\0llo\0");
    /// assert_eq!(cstr, Err(FromBytesWithNulError::InteriorNul { position: 2 }));
    /// ```
    #[stable(feature = "cstr_from_bytes", since = "1.10.0")]
    #[rustc_const_stable(feature = "const_cstr_methods", since = "1.72.0")]
    pub const fn from_bytes_with_nul(bytes: &[u8]) -> Result<&Self, FromBytesWithNulError> {
        let nul_pos = memchr::memchr(0, bytes);
        match nul_pos {
            Some(nul_pos) if nul_pos + 1 == bytes.len() => {
                // SAFETY: 已知字节切片中只有一个 nul 字节,且它位于末尾。
                Ok(unsafe { Self::from_bytes_with_nul_unchecked(bytes) })
            }
            Some(position) => Err(FromBytesWithNulError::InteriorNul { position }),
            None => Err(FromBytesWithNulError::NotNulTerminated),
        }
    }

    /// 不经检查地从字节切片创建 C 字符串包装器。
    ///
    /// 本函数会把提供的 `bytes` 直接转换为 `CStr` 包装器,不执行任何健全性检查。
    ///
    /// # 安全性(Safety）
    ///
    /// 提供的切片**必须**以 nul 终止,且不得包含任何内部 nul 字节。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::CStr;
    ///
    /// let bytes = b"Hello world!\0";
    ///
    /// let cstr = unsafe { CStr::from_bytes_with_nul_unchecked(bytes) };
    /// assert_eq!(cstr.to_bytes_with_nul(), bytes);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "cstr_from_bytes", since = "1.10.0")]
    #[rustc_const_stable(feature = "const_cstr_unchecked", since = "1.59.0")]
    #[rustc_allow_const_fn_unstable(const_eval_select)]
    pub const unsafe fn from_bytes_with_nul_unchecked(bytes: &[u8]) -> &CStr {
        const_eval_select!(
            @capture { bytes: &[u8] } -> &CStr:
            if const {
                // 使用 saturating,让空切片在 assert 中用清晰消息 panic,
                // 而不是在这里因下溢出错。
                let mut i = bytes.len().saturating_sub(1);
                assert!(!bytes.is_empty() && bytes[i] == 0, "input was not nul-terminated");

                // 末尾 nul 字节已经存在,继续检查其余部分。
                while i != 0 {
                    i -= 1;
                    let byte = bytes[i];
                    assert!(byte != 0, "input contained interior nul");
                }

                // SAFETY: 见下方运行时转换注释。
                unsafe { &*(bytes as *const [u8] as *const CStr) }
            } else {
                // 在 debug 构建中尽量捕获部分 UB。
                debug_assert!(!bytes.is_empty() && bytes[bytes.len() - 1] == 0);

                // SAFETY: 转换到 CStr 是安全的,因为其内部表示同样是 [u8](只在 std 内部安全)。
                // 取得的指针来自引用,因此可安全解引用。随后创建引用也是安全的,
                // 因为其生命周期受给定 `bytes` 的生命周期约束。
                unsafe { &*(bytes as *const [u8] as *const CStr) }
            }
        )
    }

    /// 返回这个 C 字符串的内部指针。
    ///
    /// 只要 `self` 仍有效,返回的指针就有效。它指向一段连续内存,该内存以 0 字节表示
    /// 字符串结尾。
    ///
    /// 返回指针的类型是 [`*const c_char`][crate::ffi::c_char];它究竟是 `*const i8`
    /// 还是 `*const u8` 的别名取决于平台。
    ///
    /// **警告**
    ///
    /// 返回的指针是只读的;写入它(包括把它传给会写入的 C 代码)会导致未定义行为。
    ///
    /// 调用方负责确保底层内存不会过早释放。例如,下面的代码在 `unsafe` 块中使用 `ptr`
    /// 时会导致未定义行为:
    ///
    /// ```no_run
    /// # #![expect(dangling_pointers_from_temporaries)]
    /// use std::ffi::{CStr, CString};
    ///
    /// // 由于包含一次未定义行为,整个程序的含义都是未定义的,
    /// // 其任何行为都没有保证,甚至不保证行为看起来像源码所写。
    ///
    /// // 创建指向临时 `CString` 的 dangling pointer;
    /// // 该 `CString` 会在语句结束时释放。
    /// let ptr = CString::new("Hi!".to_uppercase()).unwrap().as_ptr();
    ///
    /// // 若没有未定义行为,你可能会期望 `ptr` 等于:
    /// dbg!(CStr::from_bytes_with_nul(b"HI!\0").unwrap());
    ///
    /// // 程序此前可能碰巧表现得符合预期,这里只显示 `ptr` 如今是垃圾值...
    /// // 但这已经违反 `CStr::from_ptr` 的安全契约,会解引用 dangling pointer,
    /// // 立即导致未定义行为。
    /// dbg!(unsafe { CStr::from_ptr(ptr) });
    /// ```
    ///
    /// 这是因为 `as_ptr` 返回的指针不携带任何生命周期信息,而 `CString` 会在所在表达式
    /// 求值结束后立即释放。修复办法是把 `CString` 绑定到局部变量:
    ///
    /// ```
    /// use std::ffi::{CStr, CString};
    ///
    /// let c_str = CString::new("Hi!".to_uppercase()).unwrap();
    /// let ptr = c_str.as_ptr();
    ///
    /// assert_eq!(unsafe { CStr::from_ptr(ptr) }, c"HI!");
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_str_as_ptr", since = "1.32.0")]
    #[rustc_as_ptr]
    #[rustc_never_returns_null_ptr]
    pub const fn as_ptr(&self) -> *const c_char {
        self.inner.as_ptr()
    }

    /// 如果未来需要,可以把它公开暴露。
    #[inline]
    #[must_use]
    const fn as_non_null_ptr(&self) -> NonNull<c_char> {
        // FIXME(const_trait_impl) 替换为 `NonNull::from`。
        // SAFETY: 引用永远非空。
        unsafe { NonNull::new_unchecked(&self.inner as *const [c_char] as *mut [c_char]) }
            .as_non_null_ptr()
    }

    /// 返回 `self` 的长度。与 C 的 `strlen` 一样,不包含 nul 终止符。
    ///
    /// > **注意**:本方法当前实现为常量时间转换,但计划未来修改定义,
    /// > 让每次调用本方法时都重新计算长度。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(c"foo".count_bytes(), 3);
    /// assert_eq!(c"".count_bytes(), 0);
    /// ```
    #[inline]
    #[must_use]
    #[doc(alias("len", "strlen"))]
    #[stable(feature = "cstr_count_bytes", since = "1.79.0")]
    #[rustc_const_stable(feature = "const_cstr_from_ptr", since = "1.81.0")]
    pub const fn count_bytes(&self) -> usize {
        self.inner.len() - 1
    }

    /// 若 `self.to_bytes()` 的长度为 0,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!(!c"foo".is_empty());
    /// assert!(c"".is_empty());
    /// ```
    #[inline]
    #[stable(feature = "cstr_is_empty", since = "1.71.0")]
    #[rustc_const_stable(feature = "cstr_is_empty", since = "1.71.0")]
    pub const fn is_empty(&self) -> bool {
        // SAFETY: 已知至少有一个字节;对空字符串来说,该字节就是 NUL 终止符。
        // FIXME(const-hack): 使用 get_unchecked。
        unsafe { *self.inner.as_ptr() == 0 }
    }

    /// 把这个 C 字符串转换为字节切片。
    ///
    /// 返回的切片**不**包含这个 C 字符串尾随的 nul 终止符。
    ///
    /// > **注意**:本方法当前实现为常量时间转换,但计划未来修改定义,
    /// > 让每次调用本方法时都重新计算长度。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(c"foo".to_bytes(), b"foo");
    /// ```
    #[inline]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_cstr_methods", since = "1.72.0")]
    pub const fn to_bytes(&self) -> &[u8] {
        let bytes = self.to_bytes_with_nul();
        // FIXME(const-hack) 替换为范围索引。
        // SAFETY: `to_bytes_with_nul` 返回的切片长度至少为 1。
        unsafe { slice::from_raw_parts(bytes.as_ptr(), bytes.len() - 1) }
    }

    /// 把这个 C 字符串转换为包含尾随 0 字节的字节切片。
    ///
    /// 本函数等价于 [`CStr::to_bytes`],区别是会保留尾随 nul 终止符,而不是去掉它。
    ///
    /// > **注意**:本方法当前实现为零成本转换,但计划未来修改定义,
    /// > 让每次调用本方法时都重新计算长度。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(c"foo".to_bytes_with_nul(), b"foo\0");
    /// ```
    #[inline]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_cstr_methods", since = "1.72.0")]
    pub const fn to_bytes_with_nul(&self) -> &[u8] {
        // SAFETY: 在所有受支持目标上,把 `c_char` 切片重解释为 `u8` 切片都是安全的。
        unsafe { &*((&raw const self.inner) as *const [u8]) }
    }

    /// 遍历这个 C 字符串中的字节。
    ///
    /// 返回的迭代器**不**包含这个 C 字符串尾随的 nul 终止符。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(cstr_bytes)]
    ///
    /// assert!(c"foo".bytes().eq(*b"foo"));
    /// ```
    #[inline]
    #[unstable(feature = "cstr_bytes", issue = "112115")]
    pub fn bytes(&self) -> Bytes<'_> {
        Bytes::new(self)
    }

    /// 若 `CStr` 包含有效 UTF-8,返回 <code>&[str]</code> 切片。
    ///
    /// 如果 `CStr` 的内容是有效 UTF-8 数据,本函数会返回对应的 <code>&[str]</code>
    /// 切片。否则返回一个错误,其中包含 UTF-8 校验失败位置的细节。
    ///
    /// [str]: prim@str "str"
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(c"foo".to_str(), Ok("foo"));
    /// ```
    #[stable(feature = "cstr_to_str", since = "1.4.0")]
    #[rustc_const_stable(feature = "const_cstr_methods", since = "1.72.0")]
    pub const fn to_str(&self) -> Result<&str, str::Utf8Error> {
        // 注意:如果未来 `CStr` 改为在 `.to_bytes()` 中执行长度检查,而不是在 `from_ptr()`
        // 中执行,则值得考虑把这里改写为在长度计算过程中内联执行 UTF-8 检查,
        // 而不是事后再检查。
        str::from_utf8(self.to_bytes())
    }

    /// 返回一个实现 [`Display`] 的对象,用于安全打印可能包含非 Unicode 数据的 [`CStr`]。
    ///
    /// 行为如同先把 `self` 有损转换为 `str`;无效 UTF-8 会显示为 Unicode 替换字符 �。
    ///
    /// [`Display`]: fmt::Display
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(cstr_display)]
    ///
    /// let cstr = c"Hello, world!";
    /// println!("{}", cstr.display());
    /// ```
    #[unstable(feature = "cstr_display", issue = "139984")]
    #[must_use = "this does not display the `CStr`; \
                  it returns an object that can be displayed"]
    #[inline]
    pub fn display(&self) -> impl fmt::Display {
        crate::bstr::ByteStr::from_bytes(self.to_bytes())
    }

    /// 以字符串切片 `&CStr` 形式返回同一个字符串。
    ///
    /// 直接在 `&CStr` 上使用时,本方法是冗余的;但它有助于把其他类字符串类型解引用为
    /// 字符串切片,例如 `Box<CStr>` 或 `Arc<CStr>` 的引用。
    #[inline]
    #[unstable(feature = "str_as_str", issue = "130366")]
    pub const fn as_c_str(&self) -> &CStr {
        self
    }
}

#[stable(feature = "c_string_eq_c_str", since = "1.90.0")]
impl PartialEq<&Self> for CStr {
    #[inline]
    fn eq(&self, other: &&Self) -> bool {
        *self == **other
    }

    #[inline]
    fn ne(&self, other: &&Self) -> bool {
        *self != **other
    }
}

// 比较 `.to_bytes()` 表示,而不是内部 `[c_char]`,因为在某些平台上 `c_char`
// 是 `i8` 而不是 `u8`。这就是这里手动实现而不是 derive 的原因。
#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for CStr {
    #[inline]
    fn partial_cmp(&self, other: &CStr) -> Option<Ordering> {
        self.to_bytes().partial_cmp(&other.to_bytes())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for CStr {
    #[inline]
    fn cmp(&self, other: &CStr) -> Ordering {
        self.to_bytes().cmp(&other.to_bytes())
    }
}

#[stable(feature = "cstr_range_from", since = "1.47.0")]
impl ops::Index<ops::RangeFrom<usize>> for CStr {
    type Output = CStr;

    #[inline]
    fn index(&self, index: ops::RangeFrom<usize>) -> &CStr {
        let bytes = self.to_bytes_with_nul();
        // 需要手动检查起始索引以纳入 null 字节;否则可能得到一个不以 null 结尾的空字符串。
        if index.start < bytes.len() {
            // SAFETY: 有效 `CStr` 的非空尾部仍是有效 `CStr`。
            unsafe { CStr::from_bytes_with_nul_unchecked(&bytes[index.start..]) }
        } else {
            panic!(
                "index out of bounds: the len is {} but the index is {}",
                bytes.len(),
                index.start
            );
        }
    }
}

#[stable(feature = "cstring_asref", since = "1.7.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<CStr> for CStr {
    #[inline]
    fn as_ref(&self) -> &CStr {
        self
    }
}

/// 计算以 nul 终止的字符串长度。可行时委托给 C 的 `strlen`。
///
/// # 安全性(Safety）
///
/// 指针必须指向包含 NUL 终止符的有效缓冲区。该 NUL 距离 `ptr` 必须小于等于 `isize::MAX`。
#[inline]
#[unstable(feature = "cstr_internals", issue = "none")]
#[rustc_allow_const_fn_unstable(const_eval_select)]
const unsafe fn strlen(ptr: *const c_char) -> usize {
    const_eval_select!(
        @capture { s: *const c_char = ptr } -> usize:
        if const {
            let mut len = 0;

            // SAFETY: 外层调用方已提供指向有效 C 字符串的指针。
            while unsafe { *s.add(len) } != 0 {
                len += 1;
            }

            len
        } else {
            unsafe extern "C" {
                /// 由 libc 或 compiler_builtins 提供。
                fn strlen(s: *const c_char) -> usize;
            }

            // SAFETY: 外层调用方已提供指向有效 C 字符串的指针。
            unsafe { strlen(s) }
        }
    )
}

/// 遍历 [`CStr`] 字节的迭代器,不包含 nul 终止符。
///
/// 本结构由 [`CStr`] 上的 [`bytes`] 方法创建。更多信息见该方法文档。
///
/// [`bytes`]: CStr::bytes
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[unstable(feature = "cstr_bytes", issue = "112115")]
#[derive(Clone, Debug)]
pub struct Bytes<'a> {
    // 因为已知字符串以 nul 终止,这里只需要一个指针。
    ptr: NonNull<u8>,
    phantom: PhantomData<&'a [c_char]>,
}

#[unstable(feature = "cstr_bytes", issue = "112115")]
unsafe impl Send for Bytes<'_> {}

#[unstable(feature = "cstr_bytes", issue = "112115")]
unsafe impl Sync for Bytes<'_> {}

impl<'a> Bytes<'a> {
    #[inline]
    fn new(s: &'a CStr) -> Self {
        Self { ptr: s.as_non_null_ptr().cast(), phantom: PhantomData }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        // SAFETY: 初始值来自有效 C 字符串,并且永远不会递增越过 nul 终止符,
        // 因此维持了指针始终可解引用的条件。
        unsafe { self.ptr.read() == 0 }
    }
}

#[unstable(feature = "cstr_bytes", issue = "112115")]
impl Iterator for Bytes<'_> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        // SAFETY: 指针只来自有效 C 字符串;它必须非空且至少包含一个值。由于我们总是在
        // 保证存在的 nul 终止符处停止,可假定该指针非空且有效。这使得解引用安全,
        // 并且加 1 后仍会得到新的、非空的有效指针。
        unsafe {
            let ret = self.ptr.read();
            if ret == 0 {
                None
            } else {
                self.ptr = self.ptr.add(1);
                Some(ret)
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_empty() { (0, Some(0)) } else { (1, None) }
    }

    #[inline]
    fn count(self) -> usize {
        // SAFETY: 我们始终持有指向有效 C 字符串的指针。
        unsafe { strlen(self.ptr.as_ptr().cast()) }
    }
}

#[unstable(feature = "cstr_bytes", issue = "112115")]
impl FusedIterator for Bytes<'_> {}
