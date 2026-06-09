/// 允许把一个值以独占(exclusive)方式重新借用(reborrow),创建该值的一份副本,
/// 并在这份副本的生命周期内禁止源值进行读写。
#[lang = "reborrow"]
#[unstable(feature = "reborrow", issue = "145612")]
pub trait Reborrow {
    // 空。
}

/// 允许把一个可重新借用的值以共享(shared)方式重新借用,创建一份副本,并在这份
/// 副本的生命周期内禁止源值进行写入。
#[lang = "coerce_shared"]
#[unstable(feature = "reborrow", issue = "145612")]
pub trait CoerceShared: Reborrow {
    /// 该值以共享方式重新借用之后的类型。
    type Target: Copy;
}
