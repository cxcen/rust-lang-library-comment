#![allow(unused_imports)]

use crate::fmt::{self, Debug, Formatter};

struct PadAdapter<'buf, 'state> {
    buf: &'buf mut (dyn fmt::Write + 'buf),
    state: &'state mut PadAdapterState,
}

struct PadAdapterState {
    on_newline: bool,
}

impl Default for PadAdapterState {
    fn default() -> Self {
        PadAdapterState { on_newline: true }
    }
}

impl<'buf, 'state> PadAdapter<'buf, 'state> {
    fn wrap<'slot, 'fmt: 'buf + 'slot>(
        fmt: &'fmt mut fmt::Formatter<'_>,
        slot: &'slot mut Option<Self>,
        state: &'state mut PadAdapterState,
    ) -> fmt::Formatter<'slot> {
        fmt.wrap_buf(move |buf| slot.insert(PadAdapter { buf, state }))
    }
}

impl fmt::Write for PadAdapter<'_, '_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for s in s.split_inclusive('\n') {
            if self.state.on_newline {
                self.buf.write_str("    ")?;
            }

            self.state.on_newline = s.ends_with('\n');
            self.buf.write_str(s)?;
        }

        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        if self.state.on_newline {
            self.buf.write_str("    ")?;
        }
        self.state.on_newline = c == '\n';
        self.buf.write_char(c)
    }
}

/// 辅助编写 [`fmt::Debug`](Debug) 实现的结构体。
///
/// 当你希望在 [`Debug::fmt`] 实现中输出结构体形式的格式化结果时,此类型很有用。
///
/// 可通过 [`Formatter::debug_struct`] 方法构造它。
///
/// # 示例
///
/// ```
/// use std::fmt;
///
/// struct Foo {
///     bar: i32,
///     baz: String,
/// }
///
/// impl fmt::Debug for Foo {
///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
///         fmt.debug_struct("Foo")
///            .field("bar", &self.bar)
///            .field("baz", &self.baz)
///            .finish()
///     }
/// }
///
/// assert_eq!(
///     format!("{:?}", Foo { bar: 10, baz: "Hello World".to_string() }),
///     r#"Foo { bar: 10, baz: "Hello World" }"#,
/// );
/// ```
#[must_use = "must eventually call `finish()` on Debug builders"]
#[allow(missing_debug_implementations)]
#[stable(feature = "debug_builders", since = "1.2.0")]
#[rustc_diagnostic_item = "DebugStruct"]
pub struct DebugStruct<'a, 'b: 'a> {
    fmt: &'a mut fmt::Formatter<'b>,
    result: fmt::Result,
    has_fields: bool,
}

pub(super) fn debug_struct_new<'a, 'b>(
    fmt: &'a mut fmt::Formatter<'b>,
    name: &str,
) -> DebugStruct<'a, 'b> {
    let result = fmt.write_str(name);
    DebugStruct { fmt, result, has_fields: false }
}

impl<'a, 'b: 'a> DebugStruct<'a, 'b> {
    /// 向生成的结构体输出中添加新字段。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Bar {
    ///     bar: i32,
    ///     another: String,
    /// }
    ///
    /// impl fmt::Debug for Bar {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_struct("Bar")
    ///            .field("bar", &self.bar) // 添加 `bar` 字段。
    ///            .field("another", &self.another) // 添加 `another` 字段。
    ///            // 甚至添加一个并不存在的字段。
    ///            .field("nonexistent_field", &1)
    ///            .finish() // 到这里就完成了。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Bar { bar: 10, another: "Hello World".to_string() }),
    ///     r#"Bar { bar: 10, another: "Hello World", nonexistent_field: 1 }"#,
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn field(&mut self, name: &str, value: &dyn fmt::Debug) -> &mut Self {
        self.field_with(name, |f| value.fmt(f))
    }

    /// 向生成的结构体输出中添加新字段。
    ///
    /// 此方法等价于 [`DebugStruct::field`],但会使用传入闭包格式化值,
    /// 而不是调用 [`Debug::fmt`]。
    #[unstable(feature = "debug_closure_helpers", issue = "117729")]
    pub fn field_with<F>(&mut self, name: &str, value_fmt: F) -> &mut Self
    where
        F: FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        self.result = self.result.and_then(|_| {
            if self.is_pretty() {
                if !self.has_fields {
                    self.fmt.write_str(" {\n")?;
                }
                let mut slot = None;
                let mut state = Default::default();
                let mut writer = PadAdapter::wrap(self.fmt, &mut slot, &mut state);
                writer.write_str(name)?;
                writer.write_str(": ")?;
                value_fmt(&mut writer)?;
                writer.write_str(",\n")
            } else {
                let prefix = if self.has_fields { ", " } else { " { " };
                self.fmt.write_str(prefix)?;
                self.fmt.write_str(name)?;
                self.fmt.write_str(": ")?;
                value_fmt(self.fmt)
            }
        });

        self.has_fields = true;
        self
    }

    /// 将该结构体标记为 non-exhaustive,向读者说明 debug 表示中还有其他未显示字段。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Bar {
    ///     bar: i32,
    ///     hidden: f32,
    /// }
    ///
    /// impl fmt::Debug for Bar {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_struct("Bar")
    ///            .field("bar", &self.bar)
    ///            .finish_non_exhaustive() // 表示还存在其他字段。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Bar { bar: 10, hidden: 1.0 }),
    ///     "Bar { bar: 10, .. }",
    /// );
    /// ```
    #[stable(feature = "debug_non_exhaustive", since = "1.53.0")]
    pub fn finish_non_exhaustive(&mut self) -> fmt::Result {
        self.result = self.result.and_then(|_| {
            if self.has_fields {
                if self.is_pretty() {
                    let mut slot = None;
                    let mut state = Default::default();
                    let mut writer = PadAdapter::wrap(self.fmt, &mut slot, &mut state);
                    writer.write_str("..\n")?;
                    self.fmt.write_str("}")
                } else {
                    self.fmt.write_str(", .. }")
                }
            } else {
                self.fmt.write_str(" { .. }")
            }
        });
        self.result
    }

    /// 完成输出,并返回过程中遇到的任何错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Bar {
    ///     bar: i32,
    ///     baz: String,
    /// }
    ///
    /// impl fmt::Debug for Bar {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_struct("Bar")
    ///            .field("bar", &self.bar)
    ///            .field("baz", &self.baz)
    ///            .finish() // 需要调用它来“结束”
    ///                      // 结构体格式化。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Bar { bar: 10, baz: "Hello World".to_string() }),
    ///     r#"Bar { bar: 10, baz: "Hello World" }"#,
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn finish(&mut self) -> fmt::Result {
        if self.has_fields {
            self.result = self.result.and_then(|_| {
                if self.is_pretty() { self.fmt.write_str("}") } else { self.fmt.write_str(" }") }
            });
        }
        self.result
    }

    fn is_pretty(&self) -> bool {
        self.fmt.alternate()
    }
}

/// 辅助编写 [`fmt::Debug`](Debug) 实现的结构体。
///
/// 当你希望在 [`Debug::fmt`] 实现中输出元组结构体形式的格式化结果时,此类型很有用。
///
/// 可通过 [`Formatter::debug_tuple`] 方法构造它。
///
/// # 示例
///
/// ```
/// use std::fmt;
///
/// struct Foo(i32, String);
///
/// impl fmt::Debug for Foo {
///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
///         fmt.debug_tuple("Foo")
///            .field(&self.0)
///            .field(&self.1)
///            .finish()
///     }
/// }
///
/// assert_eq!(
///     format!("{:?}", Foo(10, "Hello World".to_string())),
///     r#"Foo(10, "Hello World")"#,
/// );
/// ```
#[must_use = "must eventually call `finish()` on Debug builders"]
#[allow(missing_debug_implementations)]
#[stable(feature = "debug_builders", since = "1.2.0")]
pub struct DebugTuple<'a, 'b: 'a> {
    fmt: &'a mut fmt::Formatter<'b>,
    result: fmt::Result,
    fields: usize,
    empty_name: bool,
}

pub(super) fn debug_tuple_new<'a, 'b>(
    fmt: &'a mut fmt::Formatter<'b>,
    name: &str,
) -> DebugTuple<'a, 'b> {
    let result = fmt.write_str(name);
    DebugTuple { fmt, result, fields: 0, empty_name: name.is_empty() }
}

impl<'a, 'b: 'a> DebugTuple<'a, 'b> {
    /// 向生成的元组结构体输出中添加新字段。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32, String);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_tuple("Foo")
    ///            .field(&self.0) // 添加第一个字段。
    ///            .field(&self.1) // 添加第二个字段。
    ///            .finish() // 到这里就完成了。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(10, "Hello World".to_string())),
    ///     r#"Foo(10, "Hello World")"#,
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn field(&mut self, value: &dyn fmt::Debug) -> &mut Self {
        self.field_with(|f| value.fmt(f))
    }

    /// 向生成的元组结构体输出中添加新字段。
    ///
    /// 此方法等价于 [`DebugTuple::field`],但会使用传入闭包格式化值,
    /// 而不是调用 [`Debug::fmt`]。
    #[unstable(feature = "debug_closure_helpers", issue = "117729")]
    pub fn field_with<F>(&mut self, value_fmt: F) -> &mut Self
    where
        F: FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        self.result = self.result.and_then(|_| {
            if self.is_pretty() {
                if self.fields == 0 {
                    self.fmt.write_str("(\n")?;
                }
                let mut slot = None;
                let mut state = Default::default();
                let mut writer = PadAdapter::wrap(self.fmt, &mut slot, &mut state);
                value_fmt(&mut writer)?;
                writer.write_str(",\n")
            } else {
                let prefix = if self.fields == 0 { "(" } else { ", " };
                self.fmt.write_str(prefix)?;
                value_fmt(self.fmt)
            }
        });

        self.fields += 1;
        self
    }

    /// 将该元组结构体标记为 non-exhaustive,向读者说明 debug 表示中还有其他未显示字段。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32, String);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_tuple("Foo")
    ///            .field(&self.0)
    ///            .finish_non_exhaustive() // 表示还存在其他字段。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(10, "secret!".to_owned())),
    ///     "Foo(10, ..)",
    /// );
    /// ```
    #[stable(feature = "debug_more_non_exhaustive", since = "1.83.0")]
    pub fn finish_non_exhaustive(&mut self) -> fmt::Result {
        self.result = self.result.and_then(|_| {
            if self.fields > 0 {
                if self.is_pretty() {
                    let mut slot = None;
                    let mut state = Default::default();
                    let mut writer = PadAdapter::wrap(self.fmt, &mut slot, &mut state);
                    writer.write_str("..\n")?;
                    self.fmt.write_str(")")
                } else {
                    self.fmt.write_str(", ..)")
                }
            } else {
                self.fmt.write_str("(..)")
            }
        });
        self.result
    }

    /// 完成输出,并返回过程中遇到的任何错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(i32, String);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_tuple("Foo")
    ///            .field(&self.0)
    ///            .field(&self.1)
    ///            .finish() // 需要调用它来“结束”
    ///                      // 元组格式化。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(10, "Hello World".to_string())),
    ///     r#"Foo(10, "Hello World")"#,
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn finish(&mut self) -> fmt::Result {
        if self.fields > 0 {
            self.result = self.result.and_then(|_| {
                if self.fields == 1 && self.empty_name && !self.is_pretty() {
                    self.fmt.write_str(",")?;
                }
                self.fmt.write_str(")")
            });
        }
        self.result
    }

    fn is_pretty(&self) -> bool {
        self.fmt.alternate()
    }
}

/// 用于打印类列表条目的辅助结构,不添加特殊格式化语义。
struct DebugInner<'a, 'b: 'a> {
    fmt: &'a mut fmt::Formatter<'b>,
    result: fmt::Result,
    has_fields: bool,
}

impl<'a, 'b: 'a> DebugInner<'a, 'b> {
    fn entry_with<F>(&mut self, entry_fmt: F)
    where
        F: FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        self.result = self.result.and_then(|_| {
            if self.is_pretty() {
                if !self.has_fields {
                    self.fmt.write_str("\n")?;
                }
                let mut slot = None;
                let mut state = Default::default();
                let mut writer = PadAdapter::wrap(self.fmt, &mut slot, &mut state);
                entry_fmt(&mut writer)?;
                writer.write_str(",\n")
            } else {
                if self.has_fields {
                    self.fmt.write_str(", ")?
                }
                entry_fmt(self.fmt)
            }
        });

        self.has_fields = true;
    }

    fn is_pretty(&self) -> bool {
        self.fmt.alternate()
    }
}

/// 辅助编写 [`fmt::Debug`](Debug) 实现的结构体。
///
/// 当你希望在 [`Debug::fmt`] 实现中输出集合形式的条目时,此类型很有用。
///
/// 可通过 [`Formatter::debug_set`] 方法构造它。
///
/// # 示例
///
/// ```
/// use std::fmt;
///
/// struct Foo(Vec<i32>);
///
/// impl fmt::Debug for Foo {
///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
///         fmt.debug_set().entries(self.0.iter()).finish()
///     }
/// }
///
/// assert_eq!(
///     format!("{:?}", Foo(vec![10, 11])),
///     "{10, 11}",
/// );
/// ```
#[must_use = "must eventually call `finish()` on Debug builders"]
#[allow(missing_debug_implementations)]
#[stable(feature = "debug_builders", since = "1.2.0")]
pub struct DebugSet<'a, 'b: 'a> {
    inner: DebugInner<'a, 'b>,
}

pub(super) fn debug_set_new<'a, 'b>(fmt: &'a mut fmt::Formatter<'b>) -> DebugSet<'a, 'b> {
    let result = fmt.write_str("{");
    DebugSet { inner: DebugInner { fmt, result, has_fields: false } }
}

impl<'a, 'b: 'a> DebugSet<'a, 'b> {
    /// 向集合输出中添加新条目。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>, Vec<u32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_set()
    ///            .entry(&self.0) // 添加第一个“entry”。
    ///            .entry(&self.1) // 添加第二个“entry”。
    ///            .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![10, 11], vec![12, 13])),
    ///     "{[10, 11], [12, 13]}",
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn entry(&mut self, entry: &dyn fmt::Debug) -> &mut Self {
        self.inner.entry_with(|f| entry.fmt(f));
        self
    }

    /// 向集合输出中添加新条目。
    ///
    /// 此方法等价于 [`DebugSet::entry`],但会使用传入闭包格式化条目,
    /// 而不是调用 [`Debug::fmt`]。
    #[unstable(feature = "debug_closure_helpers", issue = "117729")]
    pub fn entry_with<F>(&mut self, entry_fmt: F) -> &mut Self
    where
        F: FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        self.inner.entry_with(entry_fmt);
        self
    }

    /// 把一个 entry 迭代器的内容添加到集合输出中。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>, Vec<u32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_set()
    ///            .entries(self.0.iter()) // 添加第一组“entry”。
    ///            .entries(self.1.iter()) // 添加第二组“entry”。
    ///            .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![10, 11], vec![12, 13])),
    ///     "{10, 11, 12, 13}",
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn entries<D, I>(&mut self, entries: I) -> &mut Self
    where
        D: fmt::Debug,
        I: IntoIterator<Item = D>,
    {
        for entry in entries {
            self.entry(&entry);
        }
        self
    }

    /// 将该集合标记为 non-exhaustive,向读者说明 debug 表示中还有其他未显示元素。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         // 最多打印两个元素,其余部分缩写。
    ///         let mut f = fmt.debug_set();
    ///         let mut f = f.entries(self.0.iter().take(2));
    ///         if self.0.len() > 2 {
    ///             f.finish_non_exhaustive()
    ///         } else {
    ///             f.finish()
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![1, 2, 3, 4])),
    ///     "{1, 2, ..}",
    /// );
    /// ```
    #[stable(feature = "debug_more_non_exhaustive", since = "1.83.0")]
    pub fn finish_non_exhaustive(&mut self) -> fmt::Result {
        self.inner.result = self.inner.result.and_then(|_| {
            if self.inner.has_fields {
                if self.inner.is_pretty() {
                    let mut slot = None;
                    let mut state = Default::default();
                    let mut writer = PadAdapter::wrap(self.inner.fmt, &mut slot, &mut state);
                    writer.write_str("..\n")?;
                    self.inner.fmt.write_str("}")
                } else {
                    self.inner.fmt.write_str(", ..}")
                }
            } else {
                self.inner.fmt.write_str("..}")
            }
        });
        self.inner.result
    }

    /// 完成输出,并返回过程中遇到的任何错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_set()
    ///            .entries(self.0.iter())
    ///            .finish() // 结束集合格式化。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![10, 11])),
    ///     "{10, 11}",
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn finish(&mut self) -> fmt::Result {
        self.inner.result = self.inner.result.and_then(|_| self.inner.fmt.write_str("}"));
        self.inner.result
    }
}

/// 辅助编写 [`fmt::Debug`](Debug) 实现的结构体。
///
/// 当你希望在 [`Debug::fmt`] 实现中输出列表形式的条目时,此类型很有用。
///
/// 可通过 [`Formatter::debug_list`] 方法构造它。
///
/// # 示例
///
/// ```
/// use std::fmt;
///
/// struct Foo(Vec<i32>);
///
/// impl fmt::Debug for Foo {
///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
///         fmt.debug_list().entries(self.0.iter()).finish()
///     }
/// }
///
/// assert_eq!(
///     format!("{:?}", Foo(vec![10, 11])),
///     "[10, 11]",
/// );
/// ```
#[must_use = "must eventually call `finish()` on Debug builders"]
#[allow(missing_debug_implementations)]
#[stable(feature = "debug_builders", since = "1.2.0")]
pub struct DebugList<'a, 'b: 'a> {
    inner: DebugInner<'a, 'b>,
}

pub(super) fn debug_list_new<'a, 'b>(fmt: &'a mut fmt::Formatter<'b>) -> DebugList<'a, 'b> {
    let result = fmt.write_str("[");
    DebugList { inner: DebugInner { fmt, result, has_fields: false } }
}

impl<'a, 'b: 'a> DebugList<'a, 'b> {
    /// 向列表输出中添加新条目。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>, Vec<u32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_list()
    ///            .entry(&self.0) // 添加第一个“entry”。
    ///            .entry(&self.1) // 添加第二个“entry”。
    ///            .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![10, 11], vec![12, 13])),
    ///     "[[10, 11], [12, 13]]",
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn entry(&mut self, entry: &dyn fmt::Debug) -> &mut Self {
        self.inner.entry_with(|f| entry.fmt(f));
        self
    }

    /// 向列表输出中添加新条目。
    ///
    /// 此方法等价于 [`DebugList::entry`],但会使用传入闭包格式化条目,
    /// 而不是调用 [`Debug::fmt`]。
    #[unstable(feature = "debug_closure_helpers", issue = "117729")]
    pub fn entry_with<F>(&mut self, entry_fmt: F) -> &mut Self
    where
        F: FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        self.inner.entry_with(entry_fmt);
        self
    }

    /// 把一个 entry 迭代器的内容添加到列表输出中。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>, Vec<u32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_list()
    ///            .entries(self.0.iter())
    ///            .entries(self.1.iter())
    ///            .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![10, 11], vec![12, 13])),
    ///     "[10, 11, 12, 13]",
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn entries<D, I>(&mut self, entries: I) -> &mut Self
    where
        D: fmt::Debug,
        I: IntoIterator<Item = D>,
    {
        for entry in entries {
            self.entry(&entry);
        }
        self
    }

    /// 将该列表标记为 non-exhaustive,向读者说明 debug 表示中还有其他未显示元素。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         // 最多打印两个元素,其余部分缩写。
    ///         let mut f = fmt.debug_list();
    ///         let mut f = f.entries(self.0.iter().take(2));
    ///         if self.0.len() > 2 {
    ///             f.finish_non_exhaustive()
    ///         } else {
    ///             f.finish()
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![1, 2, 3, 4])),
    ///     "[1, 2, ..]",
    /// );
    /// ```
    #[stable(feature = "debug_more_non_exhaustive", since = "1.83.0")]
    pub fn finish_non_exhaustive(&mut self) -> fmt::Result {
        self.inner.result.and_then(|_| {
            if self.inner.has_fields {
                if self.inner.is_pretty() {
                    let mut slot = None;
                    let mut state = Default::default();
                    let mut writer = PadAdapter::wrap(self.inner.fmt, &mut slot, &mut state);
                    writer.write_str("..\n")?;
                    self.inner.fmt.write_str("]")
                } else {
                    self.inner.fmt.write_str(", ..]")
                }
            } else {
                self.inner.fmt.write_str("..]")
            }
        })
    }

    /// 完成输出,并返回过程中遇到的任何错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<i32>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_list()
    ///            .entries(self.0.iter())
    ///            .finish() // 结束列表格式化。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![10, 11])),
    ///     "[10, 11]",
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn finish(&mut self) -> fmt::Result {
        self.inner.result = self.inner.result.and_then(|_| self.inner.fmt.write_str("]"));
        self.inner.result
    }
}

/// 辅助编写 [`fmt::Debug`](Debug) 实现的结构体。
///
/// 当你希望在 [`Debug::fmt`] 实现中输出映射形式的格式化结果时,此类型很有用。
///
/// 可通过 [`Formatter::debug_map`] 方法构造它。
///
/// # 示例
///
/// ```
/// use std::fmt;
///
/// struct Foo(Vec<(String, i32)>);
///
/// impl fmt::Debug for Foo {
///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
///         fmt.debug_map().entries(self.0.iter().map(|&(ref k, ref v)| (k, v))).finish()
///     }
/// }
///
/// assert_eq!(
///     format!("{:?}", Foo(vec![("A".to_string(), 10), ("B".to_string(), 11)])),
///     r#"{"A": 10, "B": 11}"#,
/// );
/// ```
#[must_use = "must eventually call `finish()` on Debug builders"]
#[allow(missing_debug_implementations)]
#[stable(feature = "debug_builders", since = "1.2.0")]
pub struct DebugMap<'a, 'b: 'a> {
    fmt: &'a mut fmt::Formatter<'b>,
    result: fmt::Result,
    has_fields: bool,
    has_key: bool,
    // 在 key 与 value 之间跟踪换行状态。
    state: PadAdapterState,
}

pub(super) fn debug_map_new<'a, 'b>(fmt: &'a mut fmt::Formatter<'b>) -> DebugMap<'a, 'b> {
    let result = fmt.write_str("{");
    DebugMap { fmt, result, has_fields: false, has_key: false, state: Default::default() }
}

impl<'a, 'b: 'a> DebugMap<'a, 'b> {
    /// 向映射输出中添加新条目。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<(String, i32)>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_map()
    ///            .entry(&"whole", &self.0) // 添加 "whole" 条目。
    ///            .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![("A".to_string(), 10), ("B".to_string(), 11)])),
    ///     r#"{"whole": [("A", 10), ("B", 11)]}"#,
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn entry(&mut self, key: &dyn fmt::Debug, value: &dyn fmt::Debug) -> &mut Self {
        self.key(key).value(value)
    }

    /// 向映射输出中添加新条目的 key 部分。
    ///
    /// 此方法与 `value` 搭配使用,是在一开始还不知道完整 entry 时替代 `entry` 的方式。
    /// 只要能使用 `entry`,就应优先使用 `entry` 方法。
    ///
    /// # Panics
    ///
    /// 必须先调用 `key` 再调用 `value`,且每次 `key` 调用之后都必须跟随一次对应的
    /// `value` 调用。否则此方法会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<(String, i32)>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_map()
    ///            .key(&"whole").value(&self.0) // 添加 "whole" 条目。
    ///            .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![("A".to_string(), 10), ("B".to_string(), 11)])),
    ///     r#"{"whole": [("A", 10), ("B", 11)]}"#,
    /// );
    /// ```
    #[stable(feature = "debug_map_key_value", since = "1.42.0")]
    pub fn key(&mut self, key: &dyn fmt::Debug) -> &mut Self {
        self.key_with(|f| key.fmt(f))
    }

    /// 向映射输出中添加新条目的 key 部分。
    ///
    /// 此方法等价于 [`DebugMap::key`],但会使用传入闭包格式化 key,
    /// 而不是调用 [`Debug::fmt`]。
    #[unstable(feature = "debug_closure_helpers", issue = "117729")]
    pub fn key_with<F>(&mut self, key_fmt: F) -> &mut Self
    where
        F: FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        self.result = self.result.and_then(|_| {
            assert!(
                !self.has_key,
                "attempted to begin a new map entry \
                                    without completing the previous one"
            );

            if self.is_pretty() {
                if !self.has_fields {
                    self.fmt.write_str("\n")?;
                }
                let mut slot = None;
                self.state = Default::default();
                let mut writer = PadAdapter::wrap(self.fmt, &mut slot, &mut self.state);
                key_fmt(&mut writer)?;
                writer.write_str(": ")?;
            } else {
                if self.has_fields {
                    self.fmt.write_str(", ")?
                }
                key_fmt(self.fmt)?;
                self.fmt.write_str(": ")?;
            }

            self.has_key = true;
            Ok(())
        });

        self
    }

    /// 向映射输出中添加新条目的 value 部分。
    ///
    /// 此方法与 `key` 搭配使用,是在一开始还不知道完整 entry 时替代 `entry` 的方式。
    /// 只要能使用 `entry`,就应优先使用 `entry` 方法。
    ///
    /// # Panics
    ///
    /// 必须先调用 `key` 再调用 `value`,且每次 `key` 调用之后都必须跟随一次对应的
    /// `value` 调用。否则此方法会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<(String, i32)>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_map()
    ///            .key(&"whole").value(&self.0) // 添加 "whole" 条目。
    ///            .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![("A".to_string(), 10), ("B".to_string(), 11)])),
    ///     r#"{"whole": [("A", 10), ("B", 11)]}"#,
    /// );
    /// ```
    #[stable(feature = "debug_map_key_value", since = "1.42.0")]
    pub fn value(&mut self, value: &dyn fmt::Debug) -> &mut Self {
        self.value_with(|f| value.fmt(f))
    }

    /// 向映射输出中添加新条目的 value 部分。
    ///
    /// 此方法等价于 [`DebugMap::value`],但会使用传入闭包格式化 value,
    /// 而不是调用 [`Debug::fmt`]。
    #[unstable(feature = "debug_closure_helpers", issue = "117729")]
    pub fn value_with<F>(&mut self, value_fmt: F) -> &mut Self
    where
        F: FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
    {
        self.result = self.result.and_then(|_| {
            assert!(self.has_key, "attempted to format a map value before its key");

            if self.is_pretty() {
                let mut slot = None;
                let mut writer = PadAdapter::wrap(self.fmt, &mut slot, &mut self.state);
                value_fmt(&mut writer)?;
                writer.write_str(",\n")?;
            } else {
                value_fmt(self.fmt)?;
            }

            self.has_key = false;
            Ok(())
        });

        self.has_fields = true;
        self
    }

    /// 把一个 entry 迭代器的内容添加到映射输出中。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<(String, i32)>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_map()
    ///            // 映射这个 vec,让每个 entry 的第一个字段成为 "key"。
    ///            .entries(self.0.iter().map(|&(ref k, ref v)| (k, v)))
    ///            .finish()
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![("A".to_string(), 10), ("B".to_string(), 11)])),
    ///     r#"{"A": 10, "B": 11}"#,
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn entries<K, V, I>(&mut self, entries: I) -> &mut Self
    where
        K: fmt::Debug,
        V: fmt::Debug,
        I: IntoIterator<Item = (K, V)>,
    {
        for (k, v) in entries {
            self.entry(&k, &v);
        }
        self
    }

    /// 将该映射标记为 non-exhaustive,向读者说明 debug 表示中还有其他未显示条目。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<(String, i32)>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         // 最多打印两个元素,其余部分缩写。
    ///         let mut f = fmt.debug_map();
    ///         let mut f = f.entries(self.0.iter().take(2).map(|&(ref k, ref v)| (k, v)));
    ///         if self.0.len() > 2 {
    ///             f.finish_non_exhaustive()
    ///         } else {
    ///             f.finish()
    ///         }
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![
    ///         ("A".to_string(), 10),
    ///         ("B".to_string(), 11),
    ///         ("C".to_string(), 12),
    ///     ])),
    ///     r#"{"A": 10, "B": 11, ..}"#,
    /// );
    /// ```
    #[stable(feature = "debug_more_non_exhaustive", since = "1.83.0")]
    pub fn finish_non_exhaustive(&mut self) -> fmt::Result {
        self.result = self.result.and_then(|_| {
            assert!(!self.has_key, "attempted to finish a map with a partial entry");

            if self.has_fields {
                if self.is_pretty() {
                    let mut slot = None;
                    let mut state = Default::default();
                    let mut writer = PadAdapter::wrap(self.fmt, &mut slot, &mut state);
                    writer.write_str("..\n")?;
                    self.fmt.write_str("}")
                } else {
                    self.fmt.write_str(", ..}")
                }
            } else {
                self.fmt.write_str("..}")
            }
        });
        self.result
    }

    /// 完成输出,并返回过程中遇到的任何错误。
    ///
    /// # Panics
    ///
    /// 必须先调用 `key` 再调用 `value`,且每次 `key` 调用之后都必须跟随一次对应的
    /// `value` 调用。否则此方法会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// struct Foo(Vec<(String, i32)>);
    ///
    /// impl fmt::Debug for Foo {
    ///     fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         fmt.debug_map()
    ///            .entries(self.0.iter().map(|&(ref k, ref v)| (k, v)))
    ///            .finish() // 结束映射格式化。
    ///     }
    /// }
    ///
    /// assert_eq!(
    ///     format!("{:?}", Foo(vec![("A".to_string(), 10), ("B".to_string(), 11)])),
    ///     r#"{"A": 10, "B": 11}"#,
    /// );
    /// ```
    #[stable(feature = "debug_builders", since = "1.2.0")]
    pub fn finish(&mut self) -> fmt::Result {
        self.result = self.result.and_then(|_| {
            assert!(!self.has_key, "attempted to finish a map with a partial entry");

            self.fmt.write_str("}")
        });
        self.result
    }

    fn is_pretty(&self) -> bool {
        self.fmt.alternate()
    }
}

/// 创建一个类型,其 [`fmt::Debug`] 与 [`fmt::Display`] 实现会转发到传入闭包。
///
/// # 示例
///
/// ```
/// use std::fmt;
///
/// let value = 'a';
/// assert_eq!(format!("{}", value), "a");
/// assert_eq!(format!("{:?}", value), "'a'");
///
/// let wrapped = fmt::from_fn(|f| write!(f, "{value:?}"));
/// assert_eq!(format!("{}", wrapped), "'a'");
/// assert_eq!(format!("{:?}", wrapped), "'a'");
/// ```
#[stable(feature = "fmt_from_fn", since = "1.93.0")]
#[must_use = "returns a type implementing Debug and Display, which do not have any effects unless they are used"]
pub fn from_fn<F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result>(f: F) -> FromFn<F> {
    FromFn(f)
}

/// 通过传入闭包实现 [`fmt::Debug`] 与 [`fmt::Display`]。
///
/// 由 [`from_fn`] 创建。
#[stable(feature = "fmt_from_fn", since = "1.93.0")]
pub struct FromFn<F>(F);

#[stable(feature = "fmt_from_fn", since = "1.93.0")]
impl<F> fmt::Debug for FromFn<F>
where
    F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.0)(f)
    }
}

#[stable(feature = "fmt_from_fn", since = "1.93.0")]
impl<F> fmt::Display for FromFn<F>
where
    F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.0)(f)
    }
}
