#![doc = include_str!("error.md")]
#![stable(feature = "error_in_core", since = "1.81.0")]

use crate::any::TypeId;
use crate::fmt::{self, Debug, Display, Formatter};

/// `Error` 是描述错误值基本约定的 trait，也就是 [`Result<T, E>`] 中 `E` 这类值的公共接口。
///
/// 错误类型必须通过 [`Display`] 和 [`Debug`] 描述自身。面向用户的错误消息通常应是简短的
/// 小写句子，并且不带句末标点：
///
/// ```
/// let err = "NaN".parse::<u32>().unwrap_err();
/// assert_eq!(err.to_string(), "invalid digit found in string");
/// ```
///
/// # 错误来源
///
/// 错误可以提供原因信息。[`Error::source()`] 通常用于错误跨越“抽象边界”的场景：如果上层模块
/// 需要报告一个由下层模块错误引起的错误，它可以通过 `Error::source()` 暴露那个下层错误。这样
/// 上层模块既能提供符合自身抽象的错误类型，又能在调试时保留足够的实现细节。
///
/// 对包装底层错误的错误类型，底层错误应当二选一：要么由外层错误的 `Error::source()` 返回，
/// 要么由外层错误的 `Display` 实现渲染出来，但不要两者同时做。这样可以避免错误报告中重复输出
/// 同一层原因。
///
/// # 示例
///
/// 实现 `Error` trait 只要求该类型同时实现 `Debug` 和 `Display`。`Display` 负责面向人的短消息，
/// `Debug` 负责调试表示，而 `source` 链则用于表达错误之间的因果关系。
///
/// ```
/// use std::error::Error;
/// use std::fmt;
/// use std::path::PathBuf;
///
/// #[derive(Debug)]
/// struct ReadConfigError {
///     path: PathBuf
/// }
///
/// impl fmt::Display for ReadConfigError {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         let path = self.path.display();
///         write!(f, "unable to read configuration at {path}")
///     }
/// }
///
/// impl Error for ReadConfigError {}
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Error"]
#[rustc_has_incoherent_inherent_impls]
#[allow(multiple_supertrait_upcastable)]
pub trait Error: Debug + Display {
    /// 如果存在，返回此错误的下层来源。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::error::Error;
    /// use std::fmt;
    ///
    /// #[derive(Debug)]
    /// struct SuperError {
    ///     source: SuperErrorSideKick,
    /// }
    ///
    /// impl fmt::Display for SuperError {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         write!(f, "SuperError is here!")
    ///     }
    /// }
    ///
    /// impl Error for SuperError {
    ///     fn source(&self) -> Option<&(dyn Error + 'static)> {
    ///         Some(&self.source)
    ///     }
    /// }
    ///
    /// #[derive(Debug)]
    /// struct SuperErrorSideKick;
    ///
    /// impl fmt::Display for SuperErrorSideKick {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         write!(f, "SuperErrorSideKick is here!")
    ///     }
    /// }
    ///
    /// impl Error for SuperErrorSideKick {}
    ///
    /// fn get_super_error() -> Result<(), SuperError> {
    ///     Err(SuperError { source: SuperErrorSideKick })
    /// }
    ///
    /// fn main() {
    ///     match get_super_error() {
    ///         Err(e) => {
    ///             println!("Error: {e}");
    ///             println!("Caused by: {}", e.source().unwrap());
    ///         }
    ///         _ => println!("No error"),
    ///     }
    /// }
    /// ```
    #[stable(feature = "error_source", since = "1.30.0")]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    /// 取得 `self` 的 `TypeId`。
    #[doc(hidden)]
    #[unstable(
        feature = "error_type_id",
        reason = "this is memory-unsafe to override in user code",
        issue = "60784"
    )]
    fn type_id(&self, _: private::Internal) -> TypeId
    where
        Self: 'static,
    {
        TypeId::of::<Self>()
    }

    /// ```
    /// if let Err(e) = "xc".parse::<u32>() {
    ///     // 直接打印 `e` 本身，不再需要 description()。
    ///     eprintln!("Error: {e}");
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(since = "1.42.0", note = "use the Display impl or to_string()")]
    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(
        since = "1.33.0",
        note = "replaced by Error::source, which can support downcasting"
    )]
    #[allow(missing_docs)]
    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }

    /// 为错误报告上下文提供基于类型的访问。
    ///
    /// 该方法和 [`Request::provide_value`]、[`Request::provide_ref`] 配合使用，用于从
    /// `dyn Error` trait object 中提取成员变量的值或引用。它让错误报告器可以按类型请求
    /// backtrace、span、错误码等上下文，而不需要每种上下文都在 `Error` trait 上新增方法。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(error_generic_member_access)]
    /// use core::fmt;
    /// use core::error::{request_ref, Request};
    ///
    /// #[derive(Debug)]
    /// enum MyLittleTeaPot {
    ///     Empty,
    /// }
    ///
    /// #[derive(Debug)]
    /// struct MyBacktrace {
    ///     // ...
    /// }
    ///
    /// impl MyBacktrace {
    ///     fn new() -> MyBacktrace {
    ///         // ...
    ///         # MyBacktrace {}
    ///     }
    /// }
    ///
    /// #[derive(Debug)]
    /// struct Error {
    ///     backtrace: MyBacktrace,
    /// }
    ///
    /// impl fmt::Display for Error {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         write!(f, "Example Error")
    ///     }
    /// }
    ///
    /// impl std::error::Error for Error {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         request
    ///             .provide_ref::<MyBacktrace>(&self.backtrace);
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let backtrace = MyBacktrace::new();
    ///     let error = Error { backtrace };
    ///     let dyn_error = &error as &dyn std::error::Error;
    ///     let backtrace_ref = request_ref::<MyBacktrace>(dyn_error).unwrap();
    ///
    ///     assert!(core::ptr::eq(&error.backtrace, backtrace_ref));
    ///     assert!(request_ref::<MyLittleTeaPot>(dyn_error).is_none());
    /// }
    /// ```
    ///
    /// # 委托实现
    ///
    /// <div class="warning">
    ///
    /// **警告**：建议实现者避免把 `provide` 的实现委托给 source 错误的 `provide` 实现。
    ///
    /// </div>
    ///
    /// 此方法应只暴露 source 链当前这一层的上下文，而不是继续暴露 source 链后续错误已经暴露的
    /// 上下文。委托 `provide` 实现会让同一上下文被 source 链上的多个错误重复提供，可能导致错误
    /// 报告中出现非预期的信息重复，或者迫使报告器使用启发式规则去重。
    ///
    /// 换句话说，下面这种 `provide` 实现模式不推荐使用；对暴露给第三方的公共 API 中的
    /// [`Error`] 类型尤其不应这样写。
    ///
    /// ```rust
    /// # #![feature(error_generic_member_access)]
    /// # use core::fmt;
    /// # use core::error::Request;
    /// # #[derive(Debug)]
    /// struct MyError {
    ///     source: Error,
    /// }
    /// # #[derive(Debug)]
    /// # struct Error;
    /// # impl fmt::Display for Error {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "Example Source Error")
    /// #     }
    /// # }
    /// # impl fmt::Display for MyError {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "Example Error")
    /// #     }
    /// # }
    /// # impl std::error::Error for Error { }
    ///
    /// impl std::error::Error for MyError {
    ///     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    ///         Some(&self.source)
    ///     }
    ///
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         self.source.provide(request) // <--- 不推荐
    ///     }
    /// }
    /// ```
    #[unstable(feature = "error_generic_member_access", issue = "99301")]
    #[allow(unused_variables)]
    fn provide<'a>(&'a self, request: &mut Request<'a>) {}
}

mod private {
    // 这是一个防止 `Error` 实现覆盖 `type_id` 的技巧；
    // 如果用户实现能伪造 `TypeId`，就可能启用不健全的 downcasting。
    #[unstable(feature = "error_type_id", issue = "60784")]
    #[derive(Debug)]
    pub struct Internal;
}

#[unstable(feature = "never_type", issue = "35121")]
impl Error for ! {}

// 从 `any.rs` 复制而来。
impl dyn Error + 'static {
    /// 如果内部具体类型与 `T` 相同，则返回 `true`。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn is<T: Error + 'static>(&self) -> bool {
        // 取得此函数被单态化时使用的类型 `T` 的 `TypeId`。
        let t = TypeId::of::<T>();

        // 取得 trait object (`self`) 内部具体类型的 `TypeId`。
        let concrete = self.type_id(private::Internal);

        // 比较两个 `TypeId` 是否相等。
        t == concrete
    }

    /// 如果内部值的具体类型是 `T`，则返回它的共享引用；否则返回 `None`。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn downcast_ref<T: Error + 'static>(&self) -> Option<&T> {
        if self.is::<T>() {
            // SAFETY: `is` 已确认 trait object 内部具体类型就是 `T`，因此这个指针转换正确。
            unsafe { Some(&*(self as *const dyn Error as *const T)) }
        } else {
            None
        }
    }

    /// 如果内部值的具体类型是 `T`，则返回它的可变引用；否则返回 `None`。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn downcast_mut<T: Error + 'static>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            // SAFETY: `is` 已确认 trait object 内部具体类型就是 `T`，因此这个指针转换正确。
            unsafe { Some(&mut *(self as *mut dyn Error as *mut T)) }
        } else {
            None
        }
    }
}

impl dyn Error + 'static + Send {
    /// 转发到 `dyn Error` 类型上定义的同名方法。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn is<T: Error + 'static>(&self) -> bool {
        <dyn Error + 'static>::is::<T>(self)
    }

    /// 转发到 `dyn Error` 类型上定义的同名方法。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn downcast_ref<T: Error + 'static>(&self) -> Option<&T> {
        <dyn Error + 'static>::downcast_ref::<T>(self)
    }

    /// 转发到 `dyn Error` 类型上定义的同名方法。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn downcast_mut<T: Error + 'static>(&mut self) -> Option<&mut T> {
        <dyn Error + 'static>::downcast_mut::<T>(self)
    }
}

impl dyn Error + 'static + Send + Sync {
    /// 转发到 `dyn Error` 类型上定义的同名方法。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn is<T: Error + 'static>(&self) -> bool {
        <dyn Error + 'static>::is::<T>(self)
    }

    /// 转发到 `dyn Error` 类型上定义的同名方法。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn downcast_ref<T: Error + 'static>(&self) -> Option<&T> {
        <dyn Error + 'static>::downcast_ref::<T>(self)
    }

    /// 转发到 `dyn Error` 类型上定义的同名方法。
    #[stable(feature = "error_downcast", since = "1.3.0")]
    #[inline]
    pub fn downcast_mut<T: Error + 'static>(&mut self) -> Option<&mut T> {
        <dyn Error + 'static>::downcast_mut::<T>(self)
    }
}

impl dyn Error {
    /// 返回一个迭代器：从当前错误开始，然后通过递归调用 [`Error::source`] 沿 source 链前进。
    ///
    /// 如果想跳过当前错误、只处理它的来源错误，可使用 `skip(1)`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(error_iter)]
    /// use std::error::Error;
    /// use std::fmt;
    ///
    /// #[derive(Debug)]
    /// struct A;
    ///
    /// #[derive(Debug)]
    /// struct B(Option<Box<dyn Error + 'static>>);
    ///
    /// impl fmt::Display for A {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         write!(f, "A")
    ///     }
    /// }
    ///
    /// impl fmt::Display for B {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         write!(f, "B")
    ///     }
    /// }
    ///
    /// impl Error for A {}
    ///
    /// impl Error for B {
    ///     fn source(&self) -> Option<&(dyn Error + 'static)> {
    ///         self.0.as_ref().map(|e| e.as_ref())
    ///     }
    /// }
    ///
    /// let b = B(Some(Box::new(A)));
    ///
    /// // let err : Box<Error> = b.into(); // 或者
    /// let err = &b as &dyn Error;
    ///
    /// let mut iter = err.sources();
    ///
    /// assert_eq!("B".to_string(), iter.next().unwrap().to_string());
    /// assert_eq!("A".to_string(), iter.next().unwrap().to_string());
    /// assert!(iter.next().is_none());
    /// assert!(iter.next().is_none());
    /// ```
    #[unstable(feature = "error_iter", issue = "58520")]
    #[inline]
    pub fn sources(&self) -> Source<'_> {
        // 你可能会觉得这个方法更适合放在 `Error` trait 中，这个判断是对的。
        // 遗憾的是那样行不通：原因不是普通的 dyn-incompatibility 规则，而是下面的
        // `Source` 需要把 `self` 的引用保存为 trait object。
        // 如果此方法声明在 `Error` 中，那么 `self` 的类型会是 `&T`，其中 `T`
        // 是某个实现了 `Error` 的具体类型。我们需要把 `self` 强制转换为 `&dyn Error`，
        // 但这要求 `Self` 大小已知（即 `Self: Sized`）。这个约束不能加在 `Error`
        // 上，因为那会禁止 `Error` trait object；也不能加在方法上，因为那会导致此方法
        // 不能在 trait object 上调用（并且我们还需要 `'static` 约束，但除了 `Sized`
        // 之外对 `Self` 加其他约束的方法是 dyn-incompatible）。要求 `Unsize` 约束也不具备
        // 向后兼容性。

        Source { current: Some(self) }
    }
}

/// 从给定的 `impl Error` 请求类型为 `T` 的值。
///
/// # 示例
///
/// 从错误中取得一个 `String` 值。
///
/// ```rust
/// #![feature(error_generic_member_access)]
/// use std::error::Error;
/// use core::error::request_value;
///
/// fn get_string(err: &impl Error) -> String {
///     request_value::<String>(err).unwrap()
/// }
/// ```
#[unstable(feature = "error_generic_member_access", issue = "99301")]
pub fn request_value<'a, T>(err: &'a (impl Error + ?Sized)) -> Option<T>
where
    T: 'static,
{
    request_by_type_tag::<'a, tags::Value<T>>(err)
}

/// 从给定的 `impl Error` 请求类型为 `T` 的引用。
///
/// # 示例
///
/// 从错误中取得一个 `str` 引用。
///
/// ```rust
/// #![feature(error_generic_member_access)]
/// use core::error::Error;
/// use core::error::request_ref;
///
/// fn get_str(err: &impl Error) -> &str {
///     request_ref::<str>(err).unwrap()
/// }
/// ```
#[unstable(feature = "error_generic_member_access", issue = "99301")]
pub fn request_ref<'a, T>(err: &'a (impl Error + ?Sized)) -> Option<&'a T>
where
    T: 'static + ?Sized,
{
    request_by_type_tag::<'a, tags::Ref<tags::MaybeSizedValue<T>>>(err)
}

/// 按类型标签从 `Error` 请求一个特定值。
fn request_by_type_tag<'a, I>(err: &'a (impl Error + ?Sized)) -> Option<I::Reified>
where
    I: tags::Type<'a>,
{
    let mut tagged = Tagged { tag_id: TypeId::of::<I>(), value: TaggedOption::<'a, I>(None) };
    err.provide(tagged.as_request());
    tagged.value.0
}

///////////////////////////////////////////////////////////////////////////////
// Request 及其方法
///////////////////////////////////////////////////////////////////////////////

/// `Request` 支持泛型的、由类型驱动的数据访问。目前它的使用限制在标准库内部，服务于这样
/// 的场景：trait 作者希望允许 trait 实现者跨 trait 边界共享泛型信息。驱动这个设计的典型用例是
/// `core::error::Error`；如果没有 `Request`，它就需要为每一种具体上下文类型（例如实现者想向
/// 用户暴露的 `std::backtrace::Backtrace`）各增加一个方法。
///
/// # 数据流
///
/// 为了描述 `Request` 对象的预期数据流，可以把 API 边界两侧看作两类概念用户：
///
/// * Consumer：使用 `Request` 实例请求对象的一方。例如，一个为用户提供高级 `Error`/`Result`
///   报告的 crate，可能想从给定的 `dyn Error` 中请求 Backtrace。
///
/// * Producer：在 `Request` 请求到来时提供对象的一方。例如，一个库的 `Error` 实现在创建错误
///   实例时自动捕获 backtrace，并在被请求时暴露它。
///
/// Consumer 只需要知道把请求提交到哪里，并且必须通过 Producer 响应中的 `Option<T>` 处理请求
/// 未被满足的情况。
///
/// * Producer 初始化某个特定类型字段的值，或者准备好在被请求时生成该值，例如
///   `backtrace::Backtrace` 或 `std::backtrace::Backtrace`。
/// * Consumer 请求某个具体类型的对象，例如 `std::backtrace::Backtrace`。在 Producer 是
///   `dyn Error` trait object 的情况下，`request_ref` 和 `request_value` 两个函数用于简化
///   “为给定类型取得 `Option<T>`” 的流程。
/// * Producer 在被请求时填充给定的 `Request` 对象；该对象以可变引用形式传入。
/// * Consumer 从 `Request` 对象中取出请求类型对应的值或引用，并通过 `Option<T>` 表达是否成功。
///   对 `dyn Error` 来说，上述 `request_ref` 和 `request_value` 意味着 `dyn Error` 用户完全不必
///   直接处理 `Request` 类型（但 `Error` 实现者需要）。`Option` 的 `None` 只表示 Producer 当前
///   不能提供所请求类型的实例，并不表示它理论上不能或永远不会提供。
///
/// # 示例
///
/// 最直接的演示方式是实现一次 `Error` 的 `provide` trait 方法：
///
/// ```
/// #![feature(error_generic_member_access)]
/// use core::fmt;
/// use core::error::Request;
/// use core::error::request_ref;
///
/// #[derive(Debug)]
/// enum MyLittleTeaPot {
///     Empty,
/// }
///
/// #[derive(Debug)]
/// struct MyBacktrace {
///     // ...
/// }
///
/// impl MyBacktrace {
///     fn new() -> MyBacktrace {
///         // ...
///         # MyBacktrace {}
///     }
/// }
///
/// #[derive(Debug)]
/// struct Error {
///     backtrace: MyBacktrace,
/// }
///
/// impl fmt::Display for Error {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "Example Error")
///     }
/// }
///
/// impl std::error::Error for Error {
///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
///         request
///             .provide_ref::<MyBacktrace>(&self.backtrace);
///     }
/// }
///
/// fn main() {
///     let backtrace = MyBacktrace::new();
///     let error = Error { backtrace };
///     let dyn_error = &error as &dyn std::error::Error;
///     let backtrace_ref = request_ref::<MyBacktrace>(dyn_error).unwrap();
///
///     assert!(core::ptr::eq(&error.backtrace, backtrace_ref));
///     assert!(request_ref::<MyLittleTeaPot>(dyn_error).is_none());
/// }
/// ```
///
#[unstable(feature = "error_generic_member_access", issue = "99301")]
#[repr(transparent)]
pub struct Request<'a>(Tagged<dyn Erased<'a> + 'a>);

impl<'a> Request<'a> {
    /// 提供一个只包含 static 生命周期的值或其他类型。
    ///
    /// # 示例
    ///
    /// 提供一个 `u8`。
    ///
    /// ```rust
    /// #![feature(error_generic_member_access)]
    ///
    /// use core::error::Request;
    ///
    /// #[derive(Debug)]
    /// struct SomeConcreteType { field: u8 }
    ///
    /// impl std::fmt::Display for SomeConcreteType {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "{} failed", self.field)
    ///     }
    /// }
    ///
    /// impl std::error::Error for SomeConcreteType {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         request.provide_value::<u8>(self.field);
    ///     }
    /// }
    /// ```
    #[unstable(feature = "error_generic_member_access", issue = "99301")]
    pub fn provide_value<T>(&mut self, value: T) -> &mut Self
    where
        T: 'static,
    {
        self.provide::<tags::Value<T>>(value)
    }

    /// 通过闭包计算并提供一个只包含 static 生命周期的值或其他类型。
    ///
    /// # 示例
    ///
    /// 通过克隆提供一个 `String`。
    ///
    /// ```rust
    /// #![feature(error_generic_member_access)]
    ///
    /// use core::error::Request;
    ///
    /// #[derive(Debug)]
    /// struct SomeConcreteType { field: String }
    ///
    /// impl std::fmt::Display for SomeConcreteType {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "{} failed", self.field)
    ///     }
    /// }
    ///
    /// impl std::error::Error for SomeConcreteType {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         request.provide_value_with::<String>(|| self.field.clone());
    ///     }
    /// }
    /// ```
    #[unstable(feature = "error_generic_member_access", issue = "99301")]
    pub fn provide_value_with<T>(&mut self, fulfil: impl FnOnce() -> T) -> &mut Self
    where
        T: 'static,
    {
        self.provide_with::<tags::Value<T>>(fulfil)
    }

    /// 提供一个引用。被引用类型必须受 `'static` 约束，但可以是未定大小类型。
    ///
    /// # 示例
    ///
    /// 以 `&str` 形式提供指向字段的引用。
    ///
    /// ```rust
    /// #![feature(error_generic_member_access)]
    ///
    /// use core::error::Request;
    ///
    /// #[derive(Debug)]
    /// struct SomeConcreteType { field: String }
    ///
    /// impl std::fmt::Display for SomeConcreteType {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "{} failed", self.field)
    ///     }
    /// }
    ///
    /// impl std::error::Error for SomeConcreteType {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         request.provide_ref::<str>(&self.field);
    ///     }
    /// }
    /// ```
    #[unstable(feature = "error_generic_member_access", issue = "99301")]
    pub fn provide_ref<T: ?Sized + 'static>(&mut self, value: &'a T) -> &mut Self {
        self.provide::<tags::Ref<tags::MaybeSizedValue<T>>>(value)
    }

    /// 通过闭包计算并提供一个引用。被引用类型必须受 `'static` 约束，但可以是未定大小类型。
    ///
    /// # 示例
    ///
    /// 以 `&str` 形式提供指向字段的引用。
    ///
    /// ```rust
    /// #![feature(error_generic_member_access)]
    ///
    /// use core::error::Request;
    ///
    /// #[derive(Debug)]
    /// struct SomeConcreteType { business: String, party: String }
    /// fn today_is_a_weekday() -> bool { true }
    ///
    /// impl std::fmt::Display for SomeConcreteType {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "{} failed", self.business)
    ///     }
    /// }
    ///
    /// impl std::error::Error for SomeConcreteType {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         request.provide_ref_with::<str>(|| {
    ///             if today_is_a_weekday() {
    ///                 &self.business
    ///             } else {
    ///                 &self.party
    ///             }
    ///         });
    ///     }
    /// }
    /// ```
    #[unstable(feature = "error_generic_member_access", issue = "99301")]
    pub fn provide_ref_with<T: ?Sized + 'static>(
        &mut self,
        fulfil: impl FnOnce() -> &'a T,
    ) -> &mut Self {
        self.provide_with::<tags::Ref<tags::MaybeSizedValue<T>>>(fulfil)
    }

    /// 使用给定的 `Type` 标签提供一个值。
    fn provide<I>(&mut self, value: I::Reified) -> &mut Self
    where
        I: tags::Type<'a>,
    {
        if let Some(res @ TaggedOption(None)) = self.0.downcast_mut::<I>() {
            res.0 = Some(value);
        }
        self
    }

    /// 使用给定的 `Type` 标签提供一个值，并通过闭包避免不必要的计算。
    fn provide_with<I>(&mut self, fulfil: impl FnOnce() -> I::Reified) -> &mut Self
    where
        I: tags::Type<'a>,
    {
        if let Some(res @ TaggedOption(None)) = self.0.downcast_mut::<I>() {
            res.0 = Some(fulfil());
        }
        self
    }

    /// 检查如果提供指定类型的值，当前 `Request` 是否会被满足。
    ///
    /// 如果类型不匹配，或者该类型的值已经被提供，则返回 `false`。
    ///
    /// # 示例
    ///
    /// 检查是否仍需要提供 `u8`，然后按需提供它。
    ///
    /// ```rust
    /// #![feature(error_generic_member_access)]
    ///
    /// use core::error::Request;
    /// use core::error::request_value;
    ///
    /// #[derive(Debug)]
    /// struct Parent(Option<u8>);
    ///
    /// impl std::fmt::Display for Parent {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "a parent failed")
    ///     }
    /// }
    ///
    /// impl std::error::Error for Parent {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         if let Some(v) = self.0 {
    ///             request.provide_value::<u8>(v);
    ///         }
    ///     }
    /// }
    ///
    /// #[derive(Debug)]
    /// struct Child {
    ///     parent: Parent,
    /// }
    ///
    /// impl Child {
    ///     // 假设这个计算需要消耗大量资源。
    ///     fn an_expensive_computation(&self) -> Option<u8> {
    ///         Some(99)
    ///     }
    /// }
    ///
    /// impl std::fmt::Display for Child {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "child failed: \n  because of parent: {}", self.parent)
    ///     }
    /// }
    ///
    /// impl std::error::Error for Child {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         // 一般来说，我们不知道这次调用是否会提供
    ///         // 一个 `u8` 值……
    ///         self.parent.provide(request);
    ///
    ///         // ……因此在运行昂贵计算前，先检查是否还需要 `u8`。
    ///         if request.would_be_satisfied_by_value_of::<u8>() {
    ///             if let Some(v) = self.an_expensive_computation() {
    ///                 request.provide_value::<u8>(v);
    ///             }
    ///         }
    ///
    ///         // 现在无论是 parent 提供了值，还是我们提供了值，
    ///         // 这个请求都已经被满足。
    ///         assert!(!request.would_be_satisfied_by_value_of::<u8>());
    ///     }
    /// }
    ///
    /// let parent = Parent(Some(42));
    /// let child = Child { parent };
    /// assert_eq!(Some(42), request_value::<u8>(&child));
    ///
    /// let parent = Parent(None);
    /// let child = Child { parent };
    /// assert_eq!(Some(99), request_value::<u8>(&child));
    ///
    /// ```
    #[unstable(feature = "error_generic_member_access", issue = "99301")]
    pub fn would_be_satisfied_by_value_of<T>(&self) -> bool
    where
        T: 'static,
    {
        self.would_be_satisfied_by::<tags::Value<T>>()
    }

    /// 检查如果提供指定类型值的引用，当前 `Request` 是否会被满足。
    ///
    /// 如果类型不匹配，或者该类型的引用已经被提供，则返回 `false`。
    ///
    /// # 示例
    ///
    /// 检查是否仍需要提供 `&str`，然后按需提供它。
    ///
    /// ```rust
    /// #![feature(error_generic_member_access)]
    ///
    /// use core::error::Request;
    /// use core::error::request_ref;
    ///
    /// #[derive(Debug)]
    /// struct Parent(Option<String>);
    ///
    /// impl std::fmt::Display for Parent {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "a parent failed")
    ///     }
    /// }
    ///
    /// impl std::error::Error for Parent {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         if let Some(v) = &self.0 {
    ///             request.provide_ref::<str>(v);
    ///         }
    ///     }
    /// }
    ///
    /// #[derive(Debug)]
    /// struct Child {
    ///     parent: Parent,
    ///     name: String,
    /// }
    ///
    /// impl Child {
    ///     // 假设这个计算需要消耗大量资源。
    ///     fn an_expensive_computation(&self) -> Option<&str> {
    ///         Some(&self.name)
    ///     }
    /// }
    ///
    /// impl std::fmt::Display for Child {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "{} failed: \n  {}", self.name, self.parent)
    ///     }
    /// }
    ///
    /// impl std::error::Error for Child {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         // 一般来说，我们不知道这次调用是否会提供
    ///         // 一个 `str` 引用……
    ///         self.parent.provide(request);
    ///
    ///         // ……因此在运行昂贵计算前，先检查是否还需要 `&str`。
    ///         if request.would_be_satisfied_by_ref_of::<str>() {
    ///             if let Some(v) = self.an_expensive_computation() {
    ///                 request.provide_ref::<str>(v);
    ///             }
    ///         }
    ///
    ///         // 现在无论是 parent 提供了引用，还是我们提供了引用，
    ///         // 这个请求都已经被满足。
    ///         assert!(!request.would_be_satisfied_by_ref_of::<str>());
    ///     }
    /// }
    ///
    /// let parent = Parent(Some("parent".into()));
    /// let child = Child { parent, name: "child".into() };
    /// assert_eq!(Some("parent"), request_ref::<str>(&child));
    ///
    /// let parent = Parent(None);
    /// let child = Child { parent, name: "child".into() };
    /// assert_eq!(Some("child"), request_ref::<str>(&child));
    /// ```
    #[unstable(feature = "error_generic_member_access", issue = "99301")]
    pub fn would_be_satisfied_by_ref_of<T>(&self) -> bool
    where
        T: ?Sized + 'static,
    {
        self.would_be_satisfied_by::<tags::Ref<tags::MaybeSizedValue<T>>>()
    }

    fn would_be_satisfied_by<I>(&self) -> bool
    where
        I: tags::Type<'a>,
    {
        matches!(self.0.downcast::<I>(), Some(TaggedOption(None)))
    }
}

#[unstable(feature = "error_generic_member_access", issue = "99301")]
impl<'a> Debug for Request<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request").finish_non_exhaustive()
    }
}

///////////////////////////////////////////////////////////////////////////////
// 类型标签
///////////////////////////////////////////////////////////////////////////////

pub(crate) mod tags {
    //! 类型标签使用一个单独的值来标识类型。本模块包含若干常见类型模式对应的类型标签。
    //!
    //! 目前类型标签不会暴露给用户。未来如果要让 `Request` API 支持更复杂的类型（通常是带有
    //! 生命周期参数的类型），就需要编写自定义标签来描述这些类型。

    use crate::marker::PhantomData;

    /// 由具体标签类型实现，用来描述在给定生命周期 `'a` 下可被请求的类型。
    ///
    /// 本模块中包含几个由类型驱动的标签实现示例；crate 也可以为带内部生命周期的复杂类型实现
    /// 自己的标签。
    pub(crate) trait Type<'a>: Sized + 'static {
        /// 在给定生命周期下，可由此标签标识的值类型。
        type Reified: 'a;
    }

    /// 类似 [`Type`] trait，但表示可能为未定大小的类型（即带 `?Sized` 约束），例如 `str`。
    pub(crate) trait MaybeSizedType<'a>: Sized + 'static {
        type Reified: 'a + ?Sized;
    }

    impl<'a, T: Type<'a>> MaybeSizedType<'a> for T {
        type Reified = T::Reified;
    }

    /// 基于类型的标签，用于受 `'static` 约束、也就是不含借用元素的类型。
    #[derive(Debug)]
    pub(crate) struct Value<T: 'static>(PhantomData<T>);

    impl<'a, T: 'static> Type<'a> for Value<T> {
        type Reified = T;
    }

    /// 类似 [`Value`] 的基于类型标签，但允许被标识类型是未定大小类型（即带 `?Sized` 约束）。
    #[derive(Debug)]
    pub(crate) struct MaybeSizedValue<T: ?Sized + 'static>(PhantomData<T>);

    impl<'a, T: ?Sized + 'static> MaybeSizedType<'a> for MaybeSizedValue<T> {
        type Reified = T;
    }

    /// 基于类型的引用标签，用于引用类型（`&'a T`），其中 T 由
    /// `<I as MaybeSizedType<'a>>::Reified` 表示。
    #[derive(Debug)]
    pub(crate) struct Ref<I>(PhantomData<I>);

    impl<'a, I: MaybeSizedType<'a>> Type<'a> for Ref<I> {
        type Reified = &'a I::Reified;
    }
}

/// 带有类型标签 `I` 的 `Option`。
///
/// 由于此结构体实现了 `Erased`，它的类型可以被擦除，从而形成动态类型的 option。随后可以通过
/// `Tagged::tag_id` 动态检查类型；而具体类型在构造时经过静态检查，因此仍保留一定程度的类型安全。
#[repr(transparent)]
pub(crate) struct TaggedOption<'a, I: tags::Type<'a>>(pub Option<I::Reified>);

impl<'a, I: tags::Type<'a>> Tagged<TaggedOption<'a, I>> {
    pub(crate) fn as_request(&mut self) -> &mut Request<'a> {
        let erased = self as &mut Tagged<dyn Erased<'a> + 'a>;
        // SAFETY: `Request` 是 repr(transparent)，因此把 `&mut Tagged<dyn Erased<'a> + 'a>`
        // 转换为 `&mut Request<'a>` 保持布局一致。
        unsafe { &mut *(erased as *mut Tagged<dyn Erased<'a>> as *mut Request<'a>) }
    }
}

/// 表示类型已擦除但仍可识别的对象。
///
/// 该 trait 只由 `TaggedOption` 类型实现。
unsafe trait Erased<'a>: 'a {}

unsafe impl<'a, I: tags::Type<'a>> Erased<'a> for TaggedOption<'a, I> {}

struct Tagged<E: ?Sized> {
    tag_id: TypeId,
    value: E,
}

impl<'a> Tagged<dyn Erased<'a> + 'a> {
    /// 如果动态值带有标签 `I`，则返回其共享引用；否则返回 `None`。
    #[inline]
    fn downcast<I>(&self) -> Option<&TaggedOption<'a, I>>
    where
        I: tags::Type<'a>,
    {
        if self.tag_id == TypeId::of::<I>() {
            // SAFETY: 刚刚已经检查过当前对象确实指向标签 `I` 对应的值。
            Some(&unsafe { &*(self as *const Self).cast::<Tagged<TaggedOption<'a, I>>>() }.value)
        } else {
            None
        }
    }

    /// 如果动态值带有标签 `I`，则返回其可变引用；否则返回 `None`。
    #[inline]
    fn downcast_mut<I>(&mut self) -> Option<&mut TaggedOption<'a, I>>
    where
        I: tags::Type<'a>,
    {
        if self.tag_id == TypeId::of::<I>() {
            Some(
                // SAFETY: 刚刚已经检查过当前对象确实指向标签 `I` 对应的值。
                &mut unsafe { &mut *(self as *mut Self).cast::<Tagged<TaggedOption<'a, I>>>() }
                    .value,
            )
        } else {
            None
        }
    }
}

/// 遍历某个 [`Error`] 及其 source 链的迭代器。
///
/// 如果想跳过起始错误、只处理其来源错误，可使用 `skip(1)`。
#[unstable(feature = "error_iter", issue = "58520")]
#[derive(Clone, Debug)]
pub struct Source<'a> {
    current: Option<&'a (dyn Error + 'static)>,
}

#[unstable(feature = "error_iter", issue = "58520")]
impl<'a> Iterator for Source<'a> {
    type Item = &'a (dyn Error + 'static);

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        self.current = self.current.and_then(Error::source);
        current
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.current.is_some() { (1, None) } else { (0, Some(0)) }
    }
}

#[unstable(feature = "error_iter", issue = "58520")]
impl<'a> crate::iter::FusedIterator for Source<'a> {}

#[stable(feature = "error_by_ref", since = "1.51.0")]
impl<'a, T: Error + ?Sized> Error for &'a T {
    #[allow(deprecated)]
    fn cause(&self) -> Option<&dyn Error> {
        Error::cause(&**self)
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Error::source(&**self)
    }

    fn provide<'b>(&'b self, request: &mut Request<'b>) {
        Error::provide(&**self, request);
    }
}

#[stable(feature = "fmt_error", since = "1.11.0")]
impl Error for crate::fmt::Error {}

#[stable(feature = "try_borrow", since = "1.13.0")]
impl Error for crate::cell::BorrowError {}

#[stable(feature = "try_borrow", since = "1.13.0")]
impl Error for crate::cell::BorrowMutError {}

#[stable(feature = "try_from", since = "1.34.0")]
impl Error for crate::char::CharTryFromError {}

#[stable(feature = "duration_checked_float", since = "1.66.0")]
impl Error for crate::time::TryFromFloatSecsError {}

#[stable(feature = "cstr_from_bytes_until_nul", since = "1.69.0")]
impl Error for crate::ffi::FromBytesUntilNulError {}

#[stable(feature = "get_many_mut", since = "1.86.0")]
impl Error for crate::slice::GetDisjointMutError {}
