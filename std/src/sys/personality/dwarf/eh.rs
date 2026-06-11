//! 对 GCC 风格的语言专属数据区（Language-Specific Data Area，LSDA）的解析。
//! 详情参见：
//!  * <https://refspecs.linuxfoundation.org/LSB_3.0.0/LSB-PDA/LSB-PDA/ehframechpt.html>
//!  * <https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/dwarfext.html>
//!  * <https://itanium-cxx-abi.github.io/cxx-abi/exceptions.pdf>
//!  * <https://www.airs.com/blog/archives/460>
//!  * <https://www.airs.com/blog/archives/464>
//!
//! 一份参考实现可在 GCC 源码树中找到
//!（截至撰写本文时位于 `<root>/libgcc/unwind-c.c`）。

#![allow(non_upper_case_globals)]
#![allow(unused)]

use core::ptr;

use super::DwarfReader;

pub const DW_EH_PE_omit: u8 = 0xFF;
pub const DW_EH_PE_absptr: u8 = 0x00;

pub const DW_EH_PE_uleb128: u8 = 0x01;
pub const DW_EH_PE_udata2: u8 = 0x02;
pub const DW_EH_PE_udata4: u8 = 0x03;
pub const DW_EH_PE_udata8: u8 = 0x04;
pub const DW_EH_PE_sleb128: u8 = 0x09;
pub const DW_EH_PE_sdata2: u8 = 0x0A;
pub const DW_EH_PE_sdata4: u8 = 0x0B;
pub const DW_EH_PE_sdata8: u8 = 0x0C;

pub const DW_EH_PE_pcrel: u8 = 0x10;
pub const DW_EH_PE_textrel: u8 = 0x20;
pub const DW_EH_PE_datarel: u8 = 0x30;
pub const DW_EH_PE_funcrel: u8 = 0x40;
pub const DW_EH_PE_aligned: u8 = 0x50;

pub const DW_EH_PE_indirect: u8 = 0x80;

#[derive(Copy, Clone)]
pub struct EHContext<'a> {
    pub ip: *const u8,                             // 当前指令指针（instruction pointer）
    pub func_start: *const u8,                     // 指向当前函数的指针
    pub get_text_start: &'a dyn Fn() -> *const u8, // 获取指向代码节（code section）的指针
    pub get_data_start: &'a dyn Fn() -> *const u8, // 获取指向数据节（data section）的指针
}

/// 落地区（Landing pad）。
type LPad = *const u8;
pub enum EHAction {
    None,
    Cleanup(LPad),
    Catch(LPad),
    Filter(LPad),
    Terminate,
}

/// 32 位 ARM Darwin 平台使用 SjLj 异常。
///
/// 例外是 watchOS armv7k（具体而言是该子架构），它转而使用
/// DWARF 调用帧信息（Call Frame Information，CFI）来进行栈展开。
///
/// <https://github.com/llvm/llvm-project/blob/llvmorg-18.1.4/clang/lib/Driver/ToolChains/Darwin.cpp#L3107-L3119>
pub const USING_SJLJ_EXCEPTIONS: bool =
    cfg!(all(target_vendor = "apple", not(target_os = "watchos"), target_arch = "arm"));

pub unsafe fn find_eh_action(lsda: *const u8, context: &EHContext<'_>) -> Result<EHAction, ()> {
    if lsda.is_null() {
        return Ok(EHAction::None);
    }

    let func_start = context.func_start;
    let mut reader = DwarfReader::new(lsda);
    let lpad_base = unsafe {
        let start_encoding = reader.read::<u8>();
        // 落地区（landing pad）偏移量的基地址
        if start_encoding != DW_EH_PE_omit {
            read_encoded_pointer(&mut reader, context, start_encoding)?
        } else {
            func_start
        }
    };
    let call_site_encoding = unsafe {
        let ttype_encoding = reader.read::<u8>();
        if ttype_encoding != DW_EH_PE_omit {
            // Rust 不分析异常类型，所以我们不关心类型表（type table）
            reader.read_uleb128();
        }

        reader.read::<u8>()
    };
    let action_table = unsafe {
        let call_site_table_length = reader.read_uleb128();
        reader.ptr.add(call_site_table_length as usize)
    };
    let ip = context.ip;

    if !USING_SJLJ_EXCEPTIONS {
        // 读取调用点表（callsite table）
        while reader.ptr < action_table {
            unsafe {
                // 这些是偏移量而非指针；
                let cs_start = read_encoded_offset(&mut reader, call_site_encoding)?;
                let cs_len = read_encoded_offset(&mut reader, call_site_encoding)?;
                let cs_lpad = read_encoded_offset(&mut reader, call_site_encoding)?;
                let cs_action_entry = reader.read_uleb128();
                // 调用点表按 cs_start 排序，所以如果我们已经越过了 ip，就可以停止搜索。
                if ip < func_start.wrapping_add(cs_start) {
                    break;
                }
                if ip < func_start.wrapping_add(cs_start + cs_len) {
                    if cs_lpad == 0 {
                        return Ok(EHAction::None);
                    } else {
                        let lpad = lpad_base.wrapping_add(cs_lpad);
                        return Ok(interpret_cs_action(action_table, cs_action_entry, lpad));
                    }
                }
            }
        }
        // ip 不在表中。这表示一次 nounwind（不会展开）的调用。
        Ok(EHAction::Terminate)
    } else {
        // SjLj 版本：
        // 这个 "IP" 是进入调用点表的一个索引，但有两个例外：
        // -1 表示 'no-action'（无动作），0 表示 'terminate'（终止）。
        match ip.addr() as isize {
            -1 => return Ok(EHAction::None),
            0 => return Ok(EHAction::Terminate),
            _ => (),
        }
        let mut idx = ip.addr();
        loop {
            let cs_lpad = unsafe { reader.read_uleb128() };
            let cs_action_entry = unsafe { reader.read_uleb128() };
            idx -= 1;
            if idx == 0 {
                // 对于 sjlj 绝不可能有空的落地区——那种情况本应由值为 -1 的
                // 调用点索引来表示。
                // FIXME(strict provenance)
                let lpad = ptr::with_exposed_provenance((cs_lpad + 1) as usize);
                return Ok(unsafe { interpret_cs_action(action_table, cs_action_entry, lpad) });
            }
        }
    }
}

unsafe fn interpret_cs_action(
    action_table: *const u8,
    cs_action_entry: u64,
    lpad: LPad,
) -> EHAction {
    if cs_action_entry == 0 {
        // 如果 cs_action_entry 为 0，则这是一次 cleanup（清理，即 Drop::drop）。
        // 对 Rust panic 和外部异常（foreign exceptions），我们都会运行它们。
        EHAction::Cleanup(lpad)
    } else {
        // 如果 lpad != 0 且 cs_action_entry != 0，我们就必须检查 ttype_index。
        // 如果在此条件下 ttype_index == 0，我们就采取 cleanup 动作。
        let action_record = unsafe { action_table.offset(cs_action_entry as isize - 1) };
        let mut action_reader = DwarfReader::new(action_record);
        let ttype_index = unsafe { action_reader.read_sleb128() };
        if ttype_index == 0 {
            EHAction::Cleanup(lpad)
        } else if ttype_index > 0 {
            // 在 catch_unwind 处停止对 Rust panic 的展开。
            EHAction::Catch(lpad)
        } else {
            EHAction::Filter(lpad)
        }
    }
}

#[inline]
fn round_up(unrounded: usize, align: usize) -> Result<usize, ()> {
    if align.is_power_of_two() { Ok((unrounded + align - 1) & !(align - 1)) } else { Err(()) }
}

/// 从 `reader` 中读取一个偏移量（`usize`），其编码由 `encoding` 描述。
///
/// `encoding` 必须是一个 [由 LSB 规范描述的 DWARF 异常头编码][LSB-dwarf-ext]。
/// 此外，高位（“应用 application”）部分必须为零。
///
/// # Errors
/// 在以下情况返回 `Err`：`encoding`
/// * 不是有效的 DWARF 异常头编码，
/// * 为 `DW_EH_PE_omit`，或
/// * 含有非零的应用（application）部分。
///
/// [LSB-dwarf-ext]: https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/dwarfext.html
unsafe fn read_encoded_offset(reader: &mut DwarfReader, encoding: u8) -> Result<usize, ()> {
    if encoding == DW_EH_PE_omit || encoding & 0xF0 != 0 {
        return Err(());
    }
    let result = unsafe {
        match encoding & 0x0F {
            // 尽管名字如此，LLVM 也会把 absptr 用于偏移量而非指针
            DW_EH_PE_absptr => reader.read::<usize>(),
            DW_EH_PE_uleb128 => reader.read_uleb128() as usize,
            DW_EH_PE_udata2 => reader.read::<u16>() as usize,
            DW_EH_PE_udata4 => reader.read::<u32>() as usize,
            DW_EH_PE_udata8 => reader.read::<u64>() as usize,
            DW_EH_PE_sleb128 => reader.read_sleb128() as usize,
            DW_EH_PE_sdata2 => reader.read::<i16>() as usize,
            DW_EH_PE_sdata4 => reader.read::<i32>() as usize,
            DW_EH_PE_sdata8 => reader.read::<i64>() as usize,
            _ => return Err(()),
        }
    };
    Ok(result)
}

/// 从 `reader` 中读取一个指针，其编码由 `encoding` 描述。
///
/// `encoding` 必须是一个 [由 LSB 规范描述的 DWARF 异常头编码][LSB-dwarf-ext]。
///
/// # Errors
/// 在以下情况返回 `Err`：`encoding`
/// * 不是有效的 DWARF 异常头编码，
/// * 为 `DW_EH_PE_omit`，或
/// * 在应用部分使用 `DW_EH_PE_absptr` 或 `DW_EH_PE_aligned`，却又在值格式部分
///   搭配了整数编码（而非 `DW_EH_PE_absptr`）。
///
/// [LSB-dwarf-ext]: https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/dwarfext.html
unsafe fn read_encoded_pointer(
    reader: &mut DwarfReader,
    context: &EHContext<'_>,
    encoding: u8,
) -> Result<*const u8, ()> {
    if encoding == DW_EH_PE_omit {
        return Err(());
    }

    let base_ptr = match encoding & 0x70 {
        DW_EH_PE_absptr => core::ptr::null(),
        // 尽管名字如此，它是相对于该编码值自身的地址而言的
        DW_EH_PE_pcrel => reader.ptr,
        DW_EH_PE_funcrel => {
            if context.func_start.is_null() {
                return Err(());
            }
            context.func_start
        }
        DW_EH_PE_textrel => (*context.get_text_start)(),
        DW_EH_PE_datarel => (*context.get_data_start)(),
        // aligned 意味着该值按一个指针的大小对齐
        DW_EH_PE_aligned => {
            reader.ptr = reader.ptr.with_addr(round_up(reader.ptr.addr(), size_of::<*const u8>())?);
            core::ptr::null()
        }
        _ => return Err(()),
    };

    let mut ptr = if base_ptr.is_null() {
        // 这里使用 absptr 以外的任何值编码都是没有意义的；
        // 因为那样就没有指针出处（pointer provenance）的来源了
        if encoding & 0x0F != DW_EH_PE_absptr {
            return Err(());
        }
        unsafe { reader.read::<*const u8>() }
    } else {
        let offset = unsafe { read_encoded_offset(reader, encoding & 0x0F)? };
        base_ptr.wrapping_add(offset)
    };

    if encoding & DW_EH_PE_indirect != 0 {
        ptr = unsafe { *(ptr.cast::<*const u8>()) };
    }

    Ok(ptr)
}
