use crate::cell::UnsafeCell;
use crate::{fmt, mem};

/// 一种名义上只能被写入一次的 cell。
///
/// 它允许在不复制、不替换内部值的情况下取得指向其内部值的共享引用 `&T`(不同于 [`Cell`]),
/// 而且无需运行时借用检查(不同于 [`RefCell`])。不过,除非你持有指向该 cell 本身的可变引用,
/// 否则只能取得不可变引用。同理,也只有持有这样的可变引用,才能对该 cell 进行重新初始化。
///
/// 可以把 `OnceCell` 看作是对未初始化数据的一层安全抽象:数据一旦被写入,就变为已初始化状态。
///
/// 本结构体的线程安全版本,参见 [`std::sync::OnceLock`]。
///
/// [`RefCell`]: crate::cell::RefCell
/// [`Cell`]: crate::cell::Cell
/// [`std::sync::OnceLock`]: ../../std/sync/struct.OnceLock.html
///
/// # 示例
///
/// ```
/// use std::cell::OnceCell;
///
/// let cell = OnceCell::new();
/// assert!(cell.get().is_none());
///
/// let value: &String = cell.get_or_init(|| {
///     "Hello, World!".to_string()
/// });
/// assert_eq!(value, "Hello, World!");
/// assert!(cell.get().is_some());
/// ```
#[stable(feature = "once_cell", since = "1.70.0")]
pub struct OnceCell<T> {
    // 不变量:至多被写入一次。
    inner: UnsafeCell<Option<T>>,
}

impl<T> OnceCell<T> {
    /// 创建一个新的、未初始化的 cell。
    #[inline]
    #[must_use]
    #[stable(feature = "once_cell", since = "1.70.0")]
    #[rustc_const_stable(feature = "once_cell", since = "1.70.0")]
    pub const fn new() -> OnceCell<T> {
        OnceCell { inner: UnsafeCell::new(None) }
    }

    /// 取得指向内部值的引用。
    ///
    /// 如果该 cell 尚未初始化,则返回 `None`。
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    pub fn get(&self) -> Option<&T> {
        // SAFETY:借助 `inner` 的不变量,此操作是安全的
        unsafe { &*self.inner.get() }.as_ref()
    }

    /// 取得指向内部值的可变引用。
    ///
    /// 如果该 cell 尚未初始化,则返回 `None`。
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.inner.get_mut().as_mut()
    }

    /// 把该 cell 的内容初始化为 `value`。
    ///
    /// # Errors
    ///
    /// 如果该 cell 此前尚未初始化,本方法返回 `Ok(())`;如果它已被初始化,则返回 `Err(value)`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// assert!(cell.get().is_none());
    ///
    /// assert_eq!(cell.set(92), Ok(()));
    /// assert_eq!(cell.set(62), Err(62));
    ///
    /// assert!(cell.get().is_some());
    /// ```
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn set(&self, value: T) -> Result<(), T> {
        match self.try_insert(value) {
            Ok(_) => Ok(()),
            Err((_, value)) => Err(value),
        }
    }

    /// 如果该 cell 此前尚未初始化,则把其内容初始化为 `value`,然后返回一个指向它的引用。
    ///
    /// # Errors
    ///
    /// 如果该 cell 此前尚未初始化,本方法返回 `Ok(&value)`;如果它已被初始化,则返回
    /// `Err((&current_value, value))`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(once_cell_try_insert)]
    ///
    /// use std::cell::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// assert!(cell.get().is_none());
    ///
    /// assert_eq!(cell.try_insert(92), Ok(&92));
    /// assert_eq!(cell.try_insert(62), Err((&92, 62)));
    ///
    /// assert!(cell.get().is_some());
    /// ```
    #[inline]
    #[unstable(feature = "once_cell_try_insert", issue = "116693")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn try_insert(&self, value: T) -> Result<&T, (&T, T)> {
        if let Some(old) = self.get() {
            return Err((old, value));
        }

        // SAFETY:这里是我们唯一会设置这个槽位的地方,不可能因重入/并发而产生竞争;并且我们
        // 已经检查过该槽位当前是 `None`,所以这次写入维持了 `inner` 的不变量。
        let slot = unsafe { &mut *self.inner.get() };
        Ok(slot.insert(value))
    }

    /// 取得该 cell 的内容;如果该 cell 此前尚未初始化,则用 `f()` 将其初始化。
    ///
    /// # Panics
    ///
    /// 如果 `f()` panic,该 panic 会被传播给调用者,而该 cell 仍保持未初始化状态。
    ///
    /// 在 `f` 中重入式地初始化该 cell 是一个错误。这样做会导致 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// let value = cell.get_or_init(|| 92);
    /// assert_eq!(value, &92);
    /// let value = cell.get_or_init(|| unreachable!());
    /// assert_eq!(value, &92);
    /// ```
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        match self.get_or_try_init(|| Ok::<T, !>(f())) {
            Ok(val) => val,
        }
    }

    /// 取得指向该 cell 内容的可变引用;如果该 cell 此前尚未初始化,则用 `f()` 将其初始化。
    ///
    /// # Panics
    ///
    /// 如果 `f()` panic,该 panic 会被传播给调用者,而该 cell 仍保持未初始化状态。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(once_cell_get_mut)]
    ///
    /// use std::cell::OnceCell;
    ///
    /// let mut cell = OnceCell::new();
    /// let value = cell.get_mut_or_init(|| 92);
    /// assert_eq!(*value, 92);
    ///
    /// *value += 2;
    /// assert_eq!(*value, 94);
    ///
    /// let value = cell.get_mut_or_init(|| unreachable!());
    /// assert_eq!(*value, 94);
    /// ```
    #[inline]
    #[unstable(feature = "once_cell_get_mut", issue = "121641")]
    pub fn get_mut_or_init<F>(&mut self, f: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        match self.get_mut_or_try_init(|| Ok::<T, !>(f())) {
            Ok(val) => val,
        }
    }

    /// 取得该 cell 的内容;如果该 cell 此前尚未初始化,则用 `f()` 将其初始化。如果该 cell 此前
    /// 尚未初始化且 `f()` 失败,则返回一个错误。
    ///
    /// # Panics
    ///
    /// 如果 `f()` panic,该 panic 会被传播给调用者,而该 cell 仍保持未初始化状态。
    ///
    /// 在 `f` 中重入式地初始化该 cell 是一个错误。这样做会导致 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(once_cell_try)]
    ///
    /// use std::cell::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// assert_eq!(cell.get_or_try_init(|| Err(())), Err(()));
    /// assert!(cell.get().is_none());
    /// let value = cell.get_or_try_init(|| -> Result<i32, ()> {
    ///     Ok(92)
    /// });
    /// assert_eq!(value, Ok(&92));
    /// assert_eq!(cell.get(), Some(&92))
    /// ```
    #[unstable(feature = "once_cell_try", issue = "109737")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if let Some(val) = self.get() {
            return Ok(val);
        }
        self.try_init(f)
    }

    /// 取得指向该 cell 内容的可变引用;如果该 cell 此前尚未初始化,则用 `f()` 将其初始化。如果
    /// 该 cell 此前尚未初始化且 `f()` 失败,则返回一个错误。
    ///
    /// # Panics
    ///
    /// 如果 `f()` panic,该 panic 会被传播给调用者,而该 cell 仍保持未初始化状态。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(once_cell_get_mut)]
    ///
    /// use std::cell::OnceCell;
    ///
    /// let mut cell: OnceCell<u32> = OnceCell::new();
    ///
    /// // 初始化该 cell 的失败尝试不会改变它的内容
    /// assert!(cell.get_mut_or_try_init(|| "not a number!".parse()).is_err());
    /// assert!(cell.get().is_none());
    ///
    /// let value = cell.get_mut_or_try_init(|| "1234".parse());
    /// assert_eq!(value, Ok(&mut 1234));
    ///
    /// let Ok(value) = value else { return; };
    /// *value += 2;
    /// assert_eq!(cell.get(), Some(&1236))
    /// ```
    #[unstable(feature = "once_cell_get_mut", issue = "121641")]
    pub fn get_mut_or_try_init<F, E>(&mut self, f: F) -> Result<&mut T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if self.get().is_none() {
            self.try_init(f)?;
        }
        Ok(self.get_mut().unwrap())
    }

    // 避免把初始化闭包内联进“取出已初始化值”这条公共路径中
    #[cold]
    fn try_init<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let val = f()?;
        // 注意:*某些*形式的重入式初始化有可能导致 UB(参见 `reentrant_init` 测试)。我认为,
        // 仅仅去掉这里的 `panic`、同时保留 `try_insert`,在健全性上是没问题的;但相比悄无声息地
        // 沿用一个旧值,在此 panic 似乎是更好的选择。
        if let Ok(val) = self.try_insert(val) { Ok(val) } else { panic!("reentrant init") }
    }

    /// 消耗该 cell,返回其所包裹的值。
    ///
    /// 如果该 cell 此前尚未初始化,则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::OnceCell;
    ///
    /// let cell: OnceCell<String> = OnceCell::new();
    /// assert_eq!(cell.into_inner(), None);
    ///
    /// let cell = OnceCell::new();
    /// let _ = cell.set("hello".to_owned());
    /// assert_eq!(cell.into_inner(), Some("hello".to_owned()));
    /// ```
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    #[rustc_const_stable(feature = "const_cell_into_inner", since = "1.83.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    pub const fn into_inner(self) -> Option<T> {
        // 因为 `into_inner` 按值接收 `self`,编译器会静态地验证它当前未被借用。所以把 `Option<T>`
        // move 出来是安全的。
        self.inner.into_inner()
    }

    /// 把值从该 `OnceCell` 中取出,使其重新回到未初始化状态。
    ///
    /// 如果该 `OnceCell` 尚未初始化,则不产生任何效果并返回 `None`。
    ///
    /// 其安全性是通过要求一个可变引用来保证的。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::OnceCell;
    ///
    /// let mut cell: OnceCell<String> = OnceCell::new();
    /// assert_eq!(cell.take(), None);
    ///
    /// let mut cell = OnceCell::new();
    /// let _ = cell.set("hello".to_owned());
    /// assert_eq!(cell.take(), Some("hello".to_owned()));
    /// assert_eq!(cell.get(), None);
    /// ```
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    pub fn take(&mut self) -> Option<T> {
        mem::take(self).into_inner()
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T> const Default for OnceCell<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: fmt::Debug> fmt::Debug for OnceCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("OnceCell");
        match self.get() {
            Some(v) => d.field(v),
            None => d.field(&format_args!("<uninit>")),
        };
        d.finish()
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: Clone> Clone for OnceCell<T> {
    #[inline]
    fn clone(&self) -> OnceCell<T> {
        match self.get() {
            Some(value) => OnceCell::from(value.clone()),
            None => OnceCell::new(),
        }
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: PartialEq> PartialEq for OnceCell<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: Eq> Eq for OnceCell<T> {}

#[stable(feature = "once_cell", since = "1.70.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for OnceCell<T> {
    /// 创建一个已经内含给定 `value` 的新 `OnceCell<T>`。
    #[inline]
    fn from(value: T) -> Self {
        OnceCell { inner: UnsafeCell::new(Some(value)) }
    }
}

// 与 `Cell<T>` 一样,这个 impl 并非必需,但能让错误信息更友好。
#[stable(feature = "once_cell", since = "1.70.0")]
impl<T> !Sync for OnceCell<T> {}
