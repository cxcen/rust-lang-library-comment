/// Xous 把指向参数块（parameter block）的指针作为第二个参数传入。
/// 它用于传递诸如环境变量之类的标志。该参数块的格式为：
///
/// #[repr(C)]
/// struct BlockHeader {
///     /// 标识此块的魔数。必须是可打印的 ASCII。
///     magic: [u8; 4],
///
///     /// 数据块的大小。不包含此 header。可以为 0。
///     size: u32,
///
///     /// 此块的内容。依块类型而异。
///     data: [u8; 0],
/// }
///
/// 起始处有一个魔数为 `AppP` 的 BlockHeader，其后的数据为存在的块的数量：
///
/// #[repr(C)]
/// struct ApplicationParameters {
///     magic: b"AppP",
///     size: 4u32,
///
///     /// 整个 application slice 的大小（以字节计），包含所有 header
///     length: u32,
///
///     /// 存在的 application 参数的数量。必须至少为 1（即此块本身）
///     entries: (parameter_count as u32).to_bytes_le(),
/// }
///
/// #[repr(C)]
/// struct EnvironmentBlock {
///     magic: b"EnvB",
///
///     /// 总字节数，不含此 header
///     size: 2+data.len(),
///
///     /// 环境变量的数量
///     count: u16,
///
///     /// 环境变量迭代
///     data: [u8; 0],
/// }
///
/// 环境变量存在于一个 `EnvB` 块中。其 `data` 段是如下形式的字节序列：
///
///      (u16 /* key_len */; [0u8; key_len as usize] /* key */,
///       u16 /* val_len */ [0u8; val_len as usize])
///
/// #[repr(C)]
/// struct ArgumentList {
///     magic: b"ArgL",
///
///     /// 总字节数，不含此 header
///     size: 2+data.len(),
///
///     /// 参数变量的数量
///     count: u16,
///
///     /// 参数变量迭代
///     data: [u8; 0],
/// }
///
/// Args 只是一个表示命令行参数的字符串数组。
/// 它们是如下形式的序列：
///
///      (u16 /* val_len */ [0u8; val_len as usize])
use core::slice;

use crate::ffi::OsString;

/// 表示存在一个环境块的魔数
const ENV_MAGIC: [u8; 4] = *b"EnvB";

/// 命令行参数列表
const ARGS_MAGIC: [u8; 4] = *b"ArgL";

/// 表示加载器（loader）已传入 application 参数的魔数
const PARAMS_MAGIC: [u8; 4] = *b"AppP";

#[cfg(test)]
mod tests;

pub(crate) struct ApplicationParameters {
    data: &'static [u8],
    offset: usize,
    _entries: usize,
}

impl ApplicationParameters {
    pub(crate) unsafe fn new_from_ptr(data: *const u8) -> Option<ApplicationParameters> {
        if data.is_null() {
            return None;
        }

        let magic = unsafe { core::slice::from_raw_parts(data, 4) };
        let block_length = unsafe {
            u32::from_le_bytes(slice::from_raw_parts(data.add(4), 4).try_into().ok()?) as usize
        };
        let data_length = unsafe {
            u32::from_le_bytes(slice::from_raw_parts(data.add(8), 4).try_into().ok()?) as usize
        };
        let entries = unsafe {
            u32::from_le_bytes(slice::from_raw_parts(data.add(12), 4).try_into().ok()?) as usize
        };

        // 检查主 header
        if data_length < 16 || magic != PARAMS_MAGIC || block_length != 8 {
            return None;
        }

        let data = unsafe { slice::from_raw_parts(data, data_length) };

        Some(ApplicationParameters { data, offset: 0, _entries: entries })
    }
}

impl Iterator for ApplicationParameters {
    type Item = ApplicationParameter;

    fn next(&mut self) -> Option<Self::Item> {
        // 读取 magic，确保不会越过末尾
        if self.offset + 4 > self.data.len() {
            return None;
        }
        let magic = &self.data[self.offset..self.offset + 4];
        self.offset += 4;

        // 读取 header size
        if self.offset + 4 > self.data.len() {
            return None;
        }
        let size = u32::from_le_bytes(self.data[self.offset..self.offset + 4].try_into().unwrap())
            as usize;
        self.offset += 4;

        // 读取 data 内容
        if self.offset + size > self.data.len() {
            return None;
        }
        let data = &self.data[self.offset..self.offset + size];
        self.offset += size;

        Some(ApplicationParameter { data, magic: magic.try_into().unwrap() })
    }
}

pub(crate) struct ApplicationParameter {
    data: &'static [u8],
    magic: [u8; 4],
}

pub(crate) struct ApplicationParameterError;

pub(crate) struct EnvironmentBlock {
    _count: usize,
    data: &'static [u8],
    offset: usize,
}

impl TryFrom<&ApplicationParameter> for EnvironmentBlock {
    type Error = ApplicationParameterError;

    fn try_from(value: &ApplicationParameter) -> Result<Self, Self::Error> {
        if value.data.len() < 2 || value.magic != ENV_MAGIC {
            return Err(ApplicationParameterError);
        }

        let count = u16::from_le_bytes(value.data[0..2].try_into().unwrap()) as usize;

        Ok(EnvironmentBlock { data: &value.data[2..], offset: 0, _count: count })
    }
}

pub(crate) struct EnvironmentEntry {
    pub key: &'static str,
    pub value: &'static str,
}

impl Iterator for EnvironmentBlock {
    type Item = EnvironmentEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset + 2 > self.data.len() {
            return None;
        }
        let key_len =
            u16::from_le_bytes(self.data[self.offset..self.offset + 2].try_into().ok()?) as usize;
        self.offset += 2;

        if self.offset + key_len > self.data.len() {
            return None;
        }
        let key = core::str::from_utf8(&self.data[self.offset..self.offset + key_len]).ok()?;
        self.offset += key_len;

        if self.offset + 2 > self.data.len() {
            return None;
        }
        let value_len =
            u16::from_le_bytes(self.data[self.offset..self.offset + 2].try_into().ok()?) as usize;
        self.offset += 2;

        if self.offset + value_len > self.data.len() {
            return None;
        }
        let value = core::str::from_utf8(&self.data[self.offset..self.offset + value_len]).ok()?;
        self.offset += value_len;

        Some(EnvironmentEntry { key, value })
    }
}

pub(crate) struct ArgumentList {
    data: &'static [u8],
    _count: usize,
    offset: usize,
}

impl TryFrom<&ApplicationParameter> for ArgumentList {
    type Error = ApplicationParameterError;

    fn try_from(value: &ApplicationParameter) -> Result<Self, Self::Error> {
        if value.data.len() < 2 || value.magic != ARGS_MAGIC {
            return Err(ApplicationParameterError);
        }
        let count =
            u16::from_le_bytes(value.data[0..2].try_into().or(Err(ApplicationParameterError))?)
                as usize;
        Ok(ArgumentList { data: &value.data[2..], _count: count, offset: 0 })
    }
}

pub(crate) struct ArgumentEntry {
    value: &'static str,
}

impl Into<&str> for ArgumentEntry {
    fn into(self) -> &'static str {
        self.value
    }
}

impl Into<OsString> for ArgumentEntry {
    fn into(self) -> OsString {
        self.value.into()
    }
}

impl Iterator for ArgumentList {
    type Item = ArgumentEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset + 2 > self.data.len() {
            return None;
        }
        let value_len =
            u16::from_le_bytes(self.data[self.offset..self.offset + 2].try_into().ok()?) as usize;
        self.offset += 2;

        if self.offset + value_len > self.data.len() {
            return None;
        }
        let value = core::str::from_utf8(&self.data[self.offset..self.offset + value_len]).ok()?;
        self.offset += value_len;

        Some(ArgumentEntry { value })
    }
}
