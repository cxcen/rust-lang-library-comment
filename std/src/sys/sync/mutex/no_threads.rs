use crate::cell::Cell;

pub struct Mutex {
    // 本平台没有线程，所以我们可以在这里使用 Cell。
    locked: Cell<bool>,
}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {} // 本平台没有线程

impl Mutex {
    #[inline]
    pub const fn new() -> Mutex {
        Mutex { locked: Cell::new(false) }
    }

    #[inline]
    pub fn lock(&self) {
        assert_eq!(self.locked.replace(true), false, "cannot recursively acquire mutex");
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        self.locked.set(false);
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        self.locked.replace(true) == false
    }
}
