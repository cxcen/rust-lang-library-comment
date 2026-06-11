//! 这是一种相当简单的「未压缩」错误表示，用于非 64 位目标平台。在这些平台上，
//! 那种压缩进 64 位的表示无法工作，而且也带不来任何好处。

use super::{Custom, ErrorData, ErrorKind, RawOsError, SimpleMessage};

type Inner = ErrorData<Box<Custom>>;

pub(super) struct Repr(Inner);

impl Repr {
    #[inline]
    pub(super) fn new_custom(b: Box<Custom>) -> Self {
        Self(Inner::Custom(b))
    }
    #[inline]
    pub(super) fn new_os(code: RawOsError) -> Self {
        Self(Inner::Os(code))
    }
    #[inline]
    pub(super) fn new_simple(kind: ErrorKind) -> Self {
        Self(Inner::Simple(kind))
    }
    #[inline]
    pub(super) const fn new_simple_message(m: &'static SimpleMessage) -> Self {
        Self(Inner::SimpleMessage(m))
    }
    #[inline]
    pub(super) fn into_data(self) -> ErrorData<Box<Custom>> {
        self.0
    }
    #[inline]
    pub(super) fn data(&self) -> ErrorData<&Custom> {
        match &self.0 {
            Inner::Os(c) => ErrorData::Os(*c),
            Inner::Simple(k) => ErrorData::Simple(*k),
            Inner::SimpleMessage(m) => ErrorData::SimpleMessage(*m),
            Inner::Custom(m) => ErrorData::Custom(&*m),
        }
    }
    #[inline]
    pub(super) fn data_mut(&mut self) -> ErrorData<&mut Custom> {
        match &mut self.0 {
            Inner::Os(c) => ErrorData::Os(*c),
            Inner::Simple(k) => ErrorData::Simple(*k),
            Inner::SimpleMessage(m) => ErrorData::SimpleMessage(*m),
            Inner::Custom(m) => ErrorData::Custom(&mut *m),
        }
    }
}
