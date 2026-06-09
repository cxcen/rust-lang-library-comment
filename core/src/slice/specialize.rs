use crate::clone::TrivialClone;
use crate::ptr;

pub(super) trait SpecFill<T> {
    fn spec_fill(&mut self, value: T);
}

impl<T: Clone> SpecFill<T> for [T] {
    default fn spec_fill(&mut self, value: T) {
        if let Some((last, elems)) = self.split_last_mut() {
            for el in elems {
                el.clone_from(&value);
            }

            *last = value
        }
    }
}

impl<T: TrivialClone> SpecFill<T> for [T] {
    default fn spec_fill(&mut self, value: T) {
        for item in self.iter_mut() {
            // SAFETY: `TrivialClone` 表示按位读取等价于调用 `Clone::clone`。
            *item = unsafe { ptr::read(&value) };
        }
    }
}

impl SpecFill<u8> for [u8] {
    fn spec_fill(&mut self, value: u8) {
        // SAFETY: 指针来自可变引用，因此可写。
        unsafe {
            crate::intrinsics::write_bytes(self.as_mut_ptr(), value, self.len());
        }
    }
}

impl SpecFill<i8> for [i8] {
    fn spec_fill(&mut self, value: i8) {
        // SAFETY: 指针来自可变引用，因此可写。
        unsafe {
            crate::intrinsics::write_bytes(self.as_mut_ptr(), value.cast_unsigned(), self.len());
        }
    }
}

macro spec_fill_int {
    ($($type:ty)*) => {$(
        impl SpecFill<$type> for [$type] {
            #[inline]
            fn spec_fill(&mut self, value: $type) {
                // 在 Miri 中处理长切片时始终走这条 fastpath，因为手写 `for` 循环可能慢到不可接受。
                if (cfg!(miri) && self.len() > 32) || crate::intrinsics::is_val_statically_known(value) {
                    let bytes = value.to_ne_bytes();
                    if value == <$type>::from_ne_bytes([bytes[0]; size_of::<$type>()]) {
                        // SAFETY: 指针来自可变引用，因此可写。
                        unsafe {
                            crate::intrinsics::write_bytes(self.as_mut_ptr(), bytes[0], self.len());
                        }
                        return;
                    }
                }
                for item in self.iter_mut() {
                    *item = value;
                }
            }
        }
    )*}
}

spec_fill_int! { u16 i16 u32 i32 u64 i64 u128 i128 usize isize }
