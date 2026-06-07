//! 定义用于 Objective-C 互操作的类型和宏。

#![unstable(feature = "darwin_objc", issue = "145496")]
#![allow(nonstandard_style)]

use crate::fmt;

/// 等价于 Objective-C 的 `struct objc_class` 类型。
#[repr(u8)]
pub enum objc_class {
    #[unstable(
        feature = "objc_class_variant",
        reason = "temporary implementation detail",
        issue = "none"
    )]
    #[doc(hidden)]
    __variant1,
    #[unstable(
        feature = "objc_class_variant",
        reason = "temporary implementation detail",
        issue = "none"
    )]
    #[doc(hidden)]
    __variant2,
}

impl fmt::Debug for objc_class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("objc_class").finish()
    }
}

/// 等价于 Objective-C 的 `struct objc_selector` 类型。
#[repr(u8)]
pub enum objc_selector {
    #[unstable(
        feature = "objc_selector_variant",
        reason = "temporary implementation detail",
        issue = "none"
    )]
    #[doc(hidden)]
    __variant1,
    #[unstable(
        feature = "objc_selector_variant",
        reason = "temporary implementation detail",
        issue = "none"
    )]
    #[doc(hidden)]
    __variant2,
}

impl fmt::Debug for objc_selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("objc_selector").finish()
    }
}

/// 等价于 Objective-C 的 `Class` 类型。
pub type Class = *mut objc_class;

/// 等价于 Objective-C 的 `SEL` 类型。
pub type SEL = *mut objc_selector;

/// 获取 Objective-C class 的引用。
///
/// 对给定的 class 名称字符串字面量，此宏会生成类型为 [`Class`] 的表达式。
///
/// # 示例
///
/// ```no_run
/// #![feature(darwin_objc)]
/// use core::os::darwin::objc;
///
/// let string_class = objc::class!("NSString");
/// ```
#[allow_internal_unstable(rustc_attrs)]
pub macro class($classname:expr) {{
    // 静态 Objective-C class 引用最终会在 dylib 边界上产生多个定义，
    // 因此这里只暴露该 static 的值，不提供获取该 static 地址或引用的方法。
    unsafe extern "C" {
        #[rustc_objc_class = $classname]
        safe static VAL: $crate::os::darwin::objc::Class;
    }
    VAL
}}

/// 获取 Objective-C selector 的引用。
///
/// 对给定的方法名字符串字面量，此宏会生成类型为 [`SEL`] 的表达式。
///
/// 它类似于 Objective-C 的 `@selector` 指令。
///
/// # 示例
///
/// ```no_run
/// #![feature(darwin_objc)]
/// use core::os::darwin::objc;
///
/// let alloc_sel = objc::selector!("alloc");
/// let init_sel = objc::selector!("initWithCString:encoding:");
/// ```
#[allow_internal_unstable(rustc_attrs)]
pub macro selector($methname:expr) {{
    // 静态 Objective-C selector 引用最终会在 dylib 边界上产生多个定义，
    // 因此这里只暴露该 static 的值，不提供获取该 static 地址或引用的方法。
    unsafe extern "C" {
        #[rustc_objc_selector = $methname]
        safe static VAL: $crate::os::darwin::objc::SEL;
    }
    VAL
}}
