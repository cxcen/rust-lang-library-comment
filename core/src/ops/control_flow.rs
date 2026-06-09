use crate::marker::Destruct;
use crate::{convert, ops};

/// 用于告诉某个操作:它应当提前退出,还是照常继续。
///
/// 在暴露诸如图遍历或访问者(visitor)之类、希望让用户能够选择是否提前退出的
/// 东西时,会用到它。用这个枚举会让意图更清晰——再也不用纳闷“等等,`false` 到底
/// 是什么意思来着?”——而且它还能携带一个值。
///
/// 与 [`Option`] 和 [`Result`] 类似,这个枚举可以配合 `?` 运算符使用:当出现
/// [`Break`] 变体时立即返回,否则就带着 [`Continue`] 变体内部的值照常继续。
///
/// # 示例
///
/// 从 [`Iterator::try_for_each`] 中提前退出:
/// ```
/// use std::ops::ControlFlow;
///
/// let r = (2..100).try_for_each(|x| {
///     if 403 % x == 0 {
///         return ControlFlow::Break(x)
///     }
///
///     ControlFlow::Continue(())
/// });
/// assert_eq!(r, ControlFlow::Break(13));
/// ```
///
/// 一个基本的树遍历:
/// ```
/// use std::ops::ControlFlow;
///
/// pub struct TreeNode<T> {
///     value: T,
///     left: Option<Box<TreeNode<T>>>,
///     right: Option<Box<TreeNode<T>>>,
/// }
///
/// impl<T> TreeNode<T> {
///     pub fn traverse_inorder<B>(&self, f: &mut impl FnMut(&T) -> ControlFlow<B>) -> ControlFlow<B> {
///         if let Some(left) = &self.left {
///             left.traverse_inorder(f)?;
///         }
///         f(&self.value)?;
///         if let Some(right) = &self.right {
///             right.traverse_inorder(f)?;
///         }
///         ControlFlow::Continue(())
///     }
///     fn leaf(value: T) -> Option<Box<TreeNode<T>>> {
///         Some(Box::new(Self { value, left: None, right: None }))
///     }
/// }
///
/// let node = TreeNode {
///     value: 0,
///     left: TreeNode::leaf(1),
///     right: Some(Box::new(TreeNode {
///         value: -1,
///         left: TreeNode::leaf(5),
///         right: TreeNode::leaf(2),
///     }))
/// };
/// let mut sum = 0;
///
/// let res = node.traverse_inorder(&mut |val| {
///     if *val < 0 {
///         ControlFlow::Break(*val)
///     } else {
///         sum += *val;
///         ControlFlow::Continue(())
///     }
/// });
/// assert_eq!(res, ControlFlow::Break(-1));
/// assert_eq!(sum, 6);
/// ```
///
/// [`Break`]: ControlFlow::Break
/// [`Continue`]: ControlFlow::Continue
#[stable(feature = "control_flow_enum_type", since = "1.55.0")]
#[rustc_diagnostic_item = "ControlFlow"]
#[must_use]
// 按照 RFC 3058,ControlFlow 不应实现 PartialOrd 或 Ord:
// https://rust-lang.github.io/rfcs/3058-try-trait-v2.html#traits-for-controlflow
#[derive(Copy, Debug, Hash)]
#[derive_const(Clone, PartialEq, Eq)]
pub enum ControlFlow<B, C = ()> {
    /// 照常推进到操作的下一阶段。
    #[stable(feature = "control_flow_enum_type", since = "1.55.0")]
    #[lang = "Continue"]
    Continue(C),
    /// 退出该操作,不再运行后续阶段。
    #[stable(feature = "control_flow_enum_type", since = "1.55.0")]
    #[lang = "Break"]
    Break(B),
    // 是的,变体的顺序与类型参数的顺序并不一致。
    // 它们之所以采用这个顺序,是为了让 `Try` 实现中的
    // `ControlFlow<A, B>` <-> `Result<B, A>` 转换成为一个空操作(no-op)。
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
impl<B, C> const ops::Try for ControlFlow<B, C> {
    type Output = C;
    type Residual = ControlFlow<B, convert::Infallible>;

    #[inline]
    fn from_output(output: Self::Output) -> Self {
        ControlFlow::Continue(output)
    }

    #[inline]
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            ControlFlow::Continue(c) => ControlFlow::Continue(c),
            ControlFlow::Break(b) => ControlFlow::Break(ControlFlow::Break(b)),
        }
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
// 注意:这里手动指定 residual 类型而非使用默认值,是为了绕过
// https://github.com/rust-lang/rust/issues/99940
impl<B, C> const ops::FromResidual<ControlFlow<B, convert::Infallible>> for ControlFlow<B, C> {
    #[inline]
    fn from_residual(residual: ControlFlow<B, convert::Infallible>) -> Self {
        match residual {
            ControlFlow::Break(b) => ControlFlow::Break(b),
        }
    }
}

#[unstable(feature = "try_trait_v2_residual", issue = "91285")]
impl<B, C> ops::Residual<C> for ControlFlow<B, convert::Infallible> {
    type TryType = ControlFlow<B, C>;
}

impl<B, C> ControlFlow<B, C> {
    /// 如果这是一个 `Break` 变体,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::ControlFlow;
    ///
    /// assert!(ControlFlow::<&str, i32>::Break("Stop right there!").is_break());
    /// assert!(!ControlFlow::<&str, i32>::Continue(3).is_break());
    /// ```
    #[inline]
    #[stable(feature = "control_flow_enum_is", since = "1.59.0")]
    #[rustc_const_unstable(feature = "min_const_control_flow", issue = "148738")]
    pub const fn is_break(&self) -> bool {
        matches!(*self, ControlFlow::Break(_))
    }

    /// 如果这是一个 `Continue` 变体,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::ControlFlow;
    ///
    /// assert!(!ControlFlow::<&str, i32>::Break("Stop right there!").is_continue());
    /// assert!(ControlFlow::<&str, i32>::Continue(3).is_continue());
    /// ```
    #[inline]
    #[stable(feature = "control_flow_enum_is", since = "1.59.0")]
    #[rustc_const_unstable(feature = "min_const_control_flow", issue = "148738")]
    pub const fn is_continue(&self) -> bool {
        matches!(*self, ControlFlow::Continue(_))
    }

    /// 把 `ControlFlow` 转换为一个 `Option`:若该 `ControlFlow` 为 `Break` 则
    /// 为 `Some`,否则为 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::ControlFlow;
    ///
    /// assert_eq!(ControlFlow::<&str, i32>::Break("Stop right there!").break_value(), Some("Stop right there!"));
    /// assert_eq!(ControlFlow::<&str, i32>::Continue(3).break_value(), None);
    /// ```
    #[inline]
    #[stable(feature = "control_flow_enum", since = "1.83.0")]
    #[rustc_const_unstable(feature = "const_control_flow", issue = "148739")]
    pub const fn break_value(self) -> Option<B>
    where
        Self: [const] Destruct,
    {
        match self {
            ControlFlow::Continue(..) => None,
            ControlFlow::Break(x) => Some(x),
        }
    }

    /// 把 `ControlFlow` 转换为一个 `Result`:若该 `ControlFlow` 为 `Break` 则
    /// 为 `Ok`,否则为 `Err`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(control_flow_ok)]
    ///
    /// use std::ops::ControlFlow;
    ///
    /// struct TreeNode<T> {
    ///     value: T,
    ///     left: Option<Box<TreeNode<T>>>,
    ///     right: Option<Box<TreeNode<T>>>,
    /// }
    ///
    /// impl<T> TreeNode<T> {
    ///     fn find<'a>(&'a self, mut predicate: impl FnMut(&T) -> bool) -> Result<&'a T, ()> {
    ///         let mut f = |t: &'a T| -> ControlFlow<&'a T> {
    ///             if predicate(t) {
    ///                 ControlFlow::Break(t)
    ///             } else {
    ///                 ControlFlow::Continue(())
    ///             }
    ///         };
    ///
    ///         self.traverse_inorder(&mut f).break_ok()
    ///     }
    ///
    ///     fn traverse_inorder<'a, B>(
    ///         &'a self,
    ///         f: &mut impl FnMut(&'a T) -> ControlFlow<B>,
    ///     ) -> ControlFlow<B> {
    ///         if let Some(left) = &self.left {
    ///             left.traverse_inorder(f)?;
    ///         }
    ///         f(&self.value)?;
    ///         if let Some(right) = &self.right {
    ///             right.traverse_inorder(f)?;
    ///         }
    ///         ControlFlow::Continue(())
    ///     }
    ///
    ///     fn leaf(value: T) -> Option<Box<TreeNode<T>>> {
    ///         Some(Box::new(Self {
    ///             value,
    ///             left: None,
    ///             right: None,
    ///         }))
    ///     }
    /// }
    ///
    /// let node = TreeNode {
    ///     value: 0,
    ///     left: TreeNode::leaf(1),
    ///     right: Some(Box::new(TreeNode {
    ///         value: -1,
    ///         left: TreeNode::leaf(5),
    ///         right: TreeNode::leaf(2),
    ///     })),
    /// };
    ///
    /// let res = node.find(|val: &i32| *val > 3);
    /// assert_eq!(res, Ok(&5));
    /// ```
    #[inline]
    #[unstable(feature = "control_flow_ok", issue = "140266")]
    #[rustc_const_unstable(feature = "min_const_control_flow", issue = "148738")]
    pub const fn break_ok(self) -> Result<B, C> {
        match self {
            ControlFlow::Continue(c) => Err(c),
            ControlFlow::Break(b) => Ok(b),
        }
    }

    /// 通过对 break 值(如果存在)应用一个函数,把 `ControlFlow<B, C>` 映射为
    /// `ControlFlow<T, C>`。
    #[inline]
    #[stable(feature = "control_flow_enum", since = "1.83.0")]
    #[rustc_const_unstable(feature = "const_control_flow", issue = "148739")]
    pub const fn map_break<T, F>(self, f: F) -> ControlFlow<T, C>
    where
        F: [const] FnOnce(B) -> T + [const] Destruct,
    {
        match self {
            ControlFlow::Continue(x) => ControlFlow::Continue(x),
            ControlFlow::Break(x) => ControlFlow::Break(f(x)),
        }
    }

    /// 把 `ControlFlow` 转换为一个 `Option`:若该 `ControlFlow` 为 `Continue`
    /// 则为 `Some`,否则为 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::ControlFlow;
    ///
    /// assert_eq!(ControlFlow::<&str, i32>::Break("Stop right there!").continue_value(), None);
    /// assert_eq!(ControlFlow::<&str, i32>::Continue(3).continue_value(), Some(3));
    /// ```
    #[inline]
    #[stable(feature = "control_flow_enum", since = "1.83.0")]
    #[rustc_const_unstable(feature = "const_control_flow", issue = "148739")]
    pub const fn continue_value(self) -> Option<C>
    where
        Self: [const] Destruct,
    {
        match self {
            ControlFlow::Continue(x) => Some(x),
            ControlFlow::Break(..) => None,
        }
    }

    /// 把 `ControlFlow` 转换为一个 `Result`:若该 `ControlFlow` 为 `Continue`
    /// 则为 `Ok`,否则为 `Err`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(control_flow_ok)]
    ///
    /// use std::ops::ControlFlow;
    ///
    /// struct TreeNode<T> {
    ///     value: T,
    ///     left: Option<Box<TreeNode<T>>>,
    ///     right: Option<Box<TreeNode<T>>>,
    /// }
    ///
    /// impl<T> TreeNode<T> {
    ///     fn validate<B>(&self, f: &mut impl FnMut(&T) -> ControlFlow<B>) -> Result<(), B> {
    ///         self.traverse_inorder(f).continue_ok()
    ///     }
    ///
    ///     fn traverse_inorder<B>(&self, f: &mut impl FnMut(&T) -> ControlFlow<B>) -> ControlFlow<B> {
    ///         if let Some(left) = &self.left {
    ///             left.traverse_inorder(f)?;
    ///         }
    ///         f(&self.value)?;
    ///         if let Some(right) = &self.right {
    ///             right.traverse_inorder(f)?;
    ///         }
    ///         ControlFlow::Continue(())
    ///     }
    ///
    ///     fn leaf(value: T) -> Option<Box<TreeNode<T>>> {
    ///         Some(Box::new(Self {
    ///             value,
    ///             left: None,
    ///             right: None,
    ///         }))
    ///     }
    /// }
    ///
    /// let node = TreeNode {
    ///     value: 0,
    ///     left: TreeNode::leaf(1),
    ///     right: Some(Box::new(TreeNode {
    ///         value: -1,
    ///         left: TreeNode::leaf(5),
    ///         right: TreeNode::leaf(2),
    ///     })),
    /// };
    ///
    /// let res = node.validate(&mut |val| {
    ///     if *val < 0 {
    ///         return ControlFlow::Break("negative value detected");
    ///     }
    ///
    ///     if *val > 4 {
    ///         return ControlFlow::Break("too big value detected");
    ///     }
    ///
    ///     ControlFlow::Continue(())
    /// });
    /// assert_eq!(res, Err("too big value detected"));
    /// ```
    #[inline]
    #[unstable(feature = "control_flow_ok", issue = "140266")]
    #[rustc_const_unstable(feature = "min_const_control_flow", issue = "148738")]
    pub const fn continue_ok(self) -> Result<C, B> {
        match self {
            ControlFlow::Continue(c) => Ok(c),
            ControlFlow::Break(b) => Err(b),
        }
    }

    /// 通过对 continue 值(如果存在)应用一个函数,把 `ControlFlow<B, C>` 映射
    /// 为 `ControlFlow<B, T>`。
    #[inline]
    #[stable(feature = "control_flow_enum", since = "1.83.0")]
    #[rustc_const_unstable(feature = "const_control_flow", issue = "148739")]
    pub const fn map_continue<T, F>(self, f: F) -> ControlFlow<B, T>
    where
        F: [const] FnOnce(C) -> T + [const] Destruct,
    {
        match self {
            ControlFlow::Continue(x) => ControlFlow::Continue(f(x)),
            ControlFlow::Break(x) => ControlFlow::Break(x),
        }
    }
}

impl<T> ControlFlow<T, T> {
    /// 取出被 `ControlFlow<T, T>` 包裹的值 `T`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(control_flow_into_value)]
    /// use std::ops::ControlFlow;
    ///
    /// assert_eq!(ControlFlow::<i32, i32>::Break(1024).into_value(), 1024);
    /// assert_eq!(ControlFlow::<i32, i32>::Continue(512).into_value(), 512);
    /// ```
    #[unstable(feature = "control_flow_into_value", issue = "137461")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    pub const fn into_value(self) -> T {
        match self {
            ControlFlow::Continue(x) | ControlFlow::Break(x) => x,
        }
    }
}

/// 以下这些方法仅作为实现迭代器适配器(iterator adapter)的一部分而使用。
/// 它们的名字平平无奇、语义也不够直观,因此目前并不在迈向潜在稳定化的路线上。
impl<R: ops::Try> ControlFlow<R, R::Output> {
    /// 从任意实现了 `Try` 的类型创建一个 `ControlFlow`。
    #[inline]
    pub(crate) fn from_try(r: R) -> Self {
        match R::branch(r) {
            ControlFlow::Continue(v) => ControlFlow::Continue(v),
            ControlFlow::Break(v) => ControlFlow::Break(R::from_residual(v)),
        }
    }

    /// 把一个 `ControlFlow` 转换为任意实现了 `Try` 的类型。
    #[inline]
    pub(crate) fn into_try(self) -> R {
        match self {
            ControlFlow::Continue(v) => R::from_output(v),
            ControlFlow::Break(v) => v,
        }
    }
}
