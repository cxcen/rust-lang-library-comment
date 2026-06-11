//! 用于解析 DWARF 编码数据流的工具。
//! 参见 <http://www.dwarfstd.org>，
//! DWARF-4 标准，第 7 节 —— "Data Representation"

// 目前此模块仅被 x86_64-pc-windows-gnu 使用，但我们在所有平台上都编译它，
// 以避免回归（regressions）。
#![allow(unused)]
#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
mod tests;

pub mod eh;

pub struct DwarfReader {
    pub ptr: *const u8,
}

impl DwarfReader {
    pub fn new(ptr: *const u8) -> DwarfReader {
        DwarfReader { ptr }
    }

    /// 读取一个类型 T，然后把指针前移相应的字节数。
    ///
    /// DWARF 流是“紧凑（packed）”的，因此所有类型都必须按对齐 1 来读取。
    pub unsafe fn read<T: Copy>(&mut self) -> T {
        unsafe {
            let result = self.ptr.cast::<T>().read_unaligned();
            self.ptr = self.ptr.byte_add(size_of::<T>());
            result
        }
    }

    /// ULEB128 和 SLEB128 编码定义于第 7.6 节 —— "Variable Length Data"。
    pub unsafe fn read_uleb128(&mut self) -> u64 {
        let mut shift: usize = 0;
        let mut result: u64 = 0;
        let mut byte: u8;
        loop {
            byte = unsafe { self.read::<u8>() };
            result |= ((byte & 0x7F) as u64) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        result
    }

    pub unsafe fn read_sleb128(&mut self) -> i64 {
        let mut shift: u32 = 0;
        let mut result: u64 = 0;
        let mut byte: u8;
        loop {
            byte = unsafe { self.read::<u8>() };
            result |= ((byte & 0x7F) as u64) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        // 符号扩展（sign-extend）
        if shift < u64::BITS && (byte & 0x40) != 0 {
            result |= (!0 as u64) << shift;
        }
        result as i64
    }
}
