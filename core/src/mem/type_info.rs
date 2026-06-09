//! 一个 MVP（最小可用产品），用于以一种运行时或 const-eval 可处理的方式
//! 暴露关于类型的编译期信息。

use crate::any::TypeId;
use crate::intrinsics::type_of;

/// 编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[lang = "type_info"]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Type {
    /// 各类型独有的信息
    pub kind: TypeKind,
    /// 该类型的大小。若它是不定长（unsized）类型则为 `None`
    pub size: Option<usize>,
}

impl TypeId {
    /// 计算某个具体类型的类型信息。
    /// 它只能在编译期被调用。
    #[unstable(feature = "type_info", issue = "146922")]
    #[rustc_const_unstable(feature = "type_info", issue = "146922")]
    pub const fn info(self) -> Type {
        type_of(self)
    }
}

impl Type {
    /// 返回该泛型类型参数的类型信息。
    #[unstable(feature = "type_info", issue = "146922")]
    #[rustc_const_unstable(feature = "type_info", issue = "146922")]
    // FIXME(reflection): 不要求 'static 约束
    pub const fn of<T: ?Sized + 'static>() -> Self {
        const { TypeId::of::<T>().info() }
    }
}

/// 编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub enum TypeKind {
    /// 元组。
    Tuple(Tuple),
    /// 数组。
    Array(Array),
    /// 原生布尔类型。
    Bool(Bool),
    /// 原生字符类型。
    Char(Char),
    /// 原生有符号与无符号整数类型。
    Int(Int),
    /// 原生浮点数类型。
    Float(Float),
    /// 字符串切片类型。
    Str(Str),
    /// 引用。
    Reference(Reference),
    /// FIXME(#146922): 补全所有常见类型
    Other,
}

/// 关于元组的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Tuple {
    /// 元组的所有字段。
    pub fields: &'static [Field],
}

/// 关于元组、结构体和枚举变体的字段的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Field {
    /// 该字段的类型。
    pub ty: TypeId,
    /// 相对于父类型的字节偏移量
    pub offset: usize,
}

/// 关于数组的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Array {
    /// 数组中每个元素的类型。
    pub element_ty: TypeId,
    /// 数组的长度。
    pub len: usize,
}

/// 关于 `bool` 的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Bool {
    // 目前没有额外信息可供提供。
}

/// 关于 `char` 的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Char {
    // 目前没有额外信息可供提供。
}

/// 关于有符号与无符号整数类型的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Int {
    /// 该有符号整数类型的位宽（bit width）。
    pub bit_width: usize,
    /// 该整数类型是否为有符号。
    pub signed: bool,
}

/// 关于浮点数类型的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Float {
    /// 该浮点数类型的位宽（bit width）。
    pub bit_width: usize,
}

/// 关于字符串切片类型的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Str {
    // 目前没有额外信息可供提供。
}

/// 关于引用的编译期类型信息。
#[derive(Debug)]
#[non_exhaustive]
#[unstable(feature = "type_info", issue = "146922")]
pub struct Reference {
    /// 被引用的值的类型。
    pub pointee: TypeId,
    /// 此引用是否可变。
    pub mutable: bool,
}
