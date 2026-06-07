//! impl bool {}

impl bool {
    /// 如果此 `bool` 为 [`true`](../std/keyword.true.html)，返回 `Some(t)`；
    /// 否则返回 `None`。
    ///
    /// 传给 `then_some` 的参数会被立即求值；如果要传入函数调用的结果，
    /// 建议改用惰性求值的 [`then`]。
    ///
    /// [`then`]: bool::then
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(false.then_some(0), None);
    /// assert_eq!(true.then_some(0), Some(0));
    /// ```
    ///
    /// ```
    /// let mut a = 0;
    /// let mut function_with_side_effects = || { a += 1; };
    ///
    /// true.then_some(function_with_side_effects());
    /// false.then_some(function_with_side_effects());
    ///
    /// // `a` 会递增两次，因为传给 `then_some` 的值会被立即求值。
    /// assert_eq!(a, 2);
    /// ```
    #[stable(feature = "bool_to_option", since = "1.62.0")]
    #[inline]
    pub fn then_some<T>(self, t: T) -> Option<T> {
        if self { Some(t) } else { None }
    }

    /// 如果此 `bool` 为 [`true`](../std/keyword.true.html)，返回 `Some(f())`；
    /// 否则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(false.then(|| 0), None);
    /// assert_eq!(true.then(|| 0), Some(0));
    /// ```
    ///
    /// ```
    /// let mut a = 0;
    ///
    /// true.then(|| { a += 1; });
    /// false.then(|| { a += 1; });
    ///
    /// // `a` 只会递增一次，因为闭包由 `then` 惰性求值。
    /// assert_eq!(a, 1);
    /// ```
    #[doc(alias = "then_with")]
    #[stable(feature = "lazy_bool_to_option", since = "1.50.0")]
    #[rustc_diagnostic_item = "bool_then"]
    #[inline]
    pub fn then<T, F: FnOnce() -> T>(self, f: F) -> Option<T> {
        if self { Some(f()) } else { None }
    }

    /// 如果此 `bool` 为 [`true`](../std/keyword.true.html)，返回 `Ok(())`；
    /// 否则返回 `Err(err)`。
    ///
    /// 传给 `ok_or` 的参数会被立即求值；如果要传入函数调用的结果，
    /// 建议改用惰性求值的 [`ok_or_else`]。
    ///
    /// [`ok_or_else`]: bool::ok_or_else
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(bool_to_result)]
    ///
    /// assert_eq!(false.ok_or(0), Err(0));
    /// assert_eq!(true.ok_or(0), Ok(()));
    /// ```
    ///
    /// ```
    /// #![feature(bool_to_result)]
    ///
    /// let mut a = 0;
    /// let mut function_with_side_effects = || { a += 1; };
    ///
    /// assert!(true.ok_or(function_with_side_effects()).is_ok());
    /// assert!(false.ok_or(function_with_side_effects()).is_err());
    ///
    /// // `a` 会递增两次，因为传给 `ok_or` 的值会被立即求值。
    /// assert_eq!(a, 2);
    /// ```
    #[unstable(feature = "bool_to_result", issue = "142748")]
    #[inline]
    pub fn ok_or<E>(self, err: E) -> Result<(), E> {
        if self { Ok(()) } else { Err(err) }
    }

    /// 如果此 `bool` 为 [`true`](../std/keyword.true.html)，返回 `Ok(())`；
    /// 否则返回 `Err(f())`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(bool_to_result)]
    ///
    /// assert_eq!(false.ok_or_else(|| 0), Err(0));
    /// assert_eq!(true.ok_or_else(|| 0), Ok(()));
    /// ```
    ///
    /// ```
    /// #![feature(bool_to_result)]
    ///
    /// let mut a = 0;
    ///
    /// assert!(true.ok_or_else(|| { a += 1; }).is_ok());
    /// assert!(false.ok_or_else(|| { a += 1; }).is_err());
    ///
    /// // `a` 只会递增一次，因为闭包由 `ok_or_else` 惰性求值。
    /// assert_eq!(a, 1);
    /// ```
    #[unstable(feature = "bool_to_result", issue = "142748")]
    #[inline]
    pub fn ok_or_else<E, F: FnOnce() -> E>(self, f: F) -> Result<(), E> {
        if self { Ok(()) } else { Err(f()) }
    }
}
