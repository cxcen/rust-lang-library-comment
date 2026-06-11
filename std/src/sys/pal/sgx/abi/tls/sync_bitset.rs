#[cfg(test)]
mod tests;

use super::{TLS_KEYS_BITSET_SIZE, USIZE_BITS};
use crate::iter::{Enumerate, Peekable};
use crate::slice::Iter;
use crate::sync::atomic::{Atomic, AtomicUsize, Ordering};

/// 一个可以同步使用的 bitset（位集合）。
pub(super) struct SyncBitset([Atomic<usize>; TLS_KEYS_BITSET_SIZE]);

pub(super) const SYNC_BITSET_INIT: SyncBitset =
    SyncBitset([AtomicUsize::new(0), AtomicUsize::new(0)]);

impl SyncBitset {
    pub fn get(&self, index: usize) -> bool {
        let (hi, lo) = Self::split(index);
        (self.0[hi].load(Ordering::Relaxed) & lo) != 0
    }

    /// 非原子操作。
    pub fn iter(&self) -> SyncBitsetIter<'_> {
        SyncBitsetIter { iter: self.0.iter().enumerate().peekable(), elem_idx: 0 }
    }

    pub fn clear(&self, index: usize) {
        let (hi, lo) = Self::split(index);
        self.0[hi].fetch_and(!lo, Ordering::Relaxed);
    }

    /// 设置任意一个未置位的 bit。非原子操作。如果观察到所有 bit 都已被置位，
    /// 则返回 `None`。
    pub fn set(&self) -> Option<usize> {
        'elems: for (idx, elem) in self.0.iter().enumerate() {
            let mut current = elem.load(Ordering::Relaxed);
            loop {
                if 0 == !current {
                    continue 'elems;
                }
                let trailing_ones = (!current).trailing_zeros() as usize;
                match elem.compare_exchange(
                    current,
                    current | (1 << trailing_ones),
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return Some(idx * USIZE_BITS + trailing_ones),
                    Err(previous) => current = previous,
                }
            }
        }
        None
    }

    fn split(index: usize) -> (usize, usize) {
        (index / USIZE_BITS, 1 << (index % USIZE_BITS))
    }
}

pub(super) struct SyncBitsetIter<'a> {
    iter: Peekable<Enumerate<Iter<'a, Atomic<usize>>>>,
    elem_idx: usize,
}

impl<'a> Iterator for SyncBitsetIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        self.iter.peek().cloned().and_then(|(idx, elem)| {
            let elem = elem.load(Ordering::Relaxed);
            let low_mask = (1 << self.elem_idx) - 1;
            let next = elem & !low_mask;
            let next_idx = next.trailing_zeros() as usize;
            self.elem_idx = next_idx + 1;
            if self.elem_idx >= 64 {
                self.elem_idx = 0;
                self.iter.next();
            }
            match next_idx {
                64 => self.next(),
                _ => Some(idx * USIZE_BITS + next_idx),
            }
        })
    }
}
