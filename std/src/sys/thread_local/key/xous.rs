//! 线程本地存储（Thread Local Storage）
//!
//! 目前，我们被限制在 1023 个 TLS 条目。这些条目存放在一页内存中，
//! 该页对每个进程唯一，并保存在 `$tp` 寄存器中。如果该寄存器为 0，
//! 则说明 TLS 尚未初始化，可以跳过线程清理。
//!
//! 进入该寄存器的索引就是 `key`。这个 key 在所有线程之间是相同的，
//! 但在该指针内部索引到不同的偏移量。
//!
//! # 析构函数注册（抄自 Windows）
//!
//! Xous 没有运行析构函数（destructors）的原生支持，所以我们自己管理一份
//! 析构函数列表，以追踪如何销毁各个 key。当某个线程或进程退出时，
//! `run_dtors` 会被调用，它会遍历该列表并运行析构函数。
//!
//! 目前不支持从该列表中注销（unregistration）。析构函数可以被注册，
//! 但无法被注销。这样做有若干简化方面的原因，主要是：
//!
//! 1. 目前我们甚至不支持释放 TLS key，所以正常操作不需要释放析构函数。
//! 2. 不存在某个时间点能让我们确知可以注销某个析构函数，因为它随时
//!    可能正被某个远程线程运行。
//!
//! 通常进程拥有一组静态已知且相当小的 TLS key，而且无论如何我们其实都希望
//! 让这块内存在整个进程生命周期内保持存活。
//!
//! 也许有一天我们可以把这里的 `Box` 折叠进一处静态分配，扩展 `LazyKey`
//! 结构，使其不仅含有一个用于 TLS key 的槽位，还像 windows 上那样含有一个
//! 用于析构函数队列的槽位。这是留待将来的优化！

// FIXME(joboet)：改为实现对原生 TLS 的支持。

use core::arch::asm;

use crate::alloc::System;
use crate::mem::ManuallyDrop;
use crate::os::xous::ffi::{MemoryFlags, map_memory, unmap_memory};
use crate::ptr;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicPtr, AtomicUsize};

pub type Key = usize;
pub type Dtor = unsafe extern "C" fn(*mut u8);

const TLS_MEMORY_SIZE: usize = 4096;

/// TLS key 从 `1` 开始。索引 `0` 未被使用
#[cfg(not(test))]
#[unsafe(export_name = "_ZN16__rust_internals3std3sys4xous16thread_local_key13TLS_KEY_INDEXE")]
static TLS_KEY_INDEX: Atomic<usize> = AtomicUsize::new(1);

#[cfg(not(test))]
#[unsafe(export_name = "_ZN16__rust_internals3std3sys4xous16thread_local_key9DTORSE")]
static DTORS: Atomic<*mut Node> = AtomicPtr::new(ptr::null_mut());

#[cfg(test)]
unsafe extern "Rust" {
    #[link_name = "_ZN16__rust_internals3std3sys4xous16thread_local_key13TLS_KEY_INDEXE"]
    static TLS_KEY_INDEX: Atomic<usize>;

    #[link_name = "_ZN16__rust_internals3std3sys4xous16thread_local_key9DTORSE"]
    static DTORS: Atomic<*mut Node>;
}

fn tls_ptr_addr() -> *mut *mut u8 {
    let mut tp: usize;
    unsafe {
        asm!(
            "mv {}, tp",
            out(reg) tp,
        );
    }
    core::ptr::with_exposed_provenance_mut::<*mut u8>(tp)
}

/// 创建一块对每个线程唯一的内存区域。该区域将包含所有线程本地指针。
fn tls_table() -> &'static mut [*mut u8] {
    let tp = tls_ptr_addr();

    if !tp.is_null() {
        return unsafe {
            core::slice::from_raw_parts_mut(tp, TLS_MEMORY_SIZE / size_of::<*mut u8>())
        };
    }
    // 如果 TP 寄存器为 `0`，则说明该线程尚未初始化其 TLS。
    // 分配一个新页来存放这块内存。
    let tp = unsafe {
        map_memory(
            None,
            None,
            TLS_MEMORY_SIZE / size_of::<*mut u8>(),
            MemoryFlags::R | MemoryFlags::W,
        )
        .expect("Unable to allocate memory for thread local storage")
    };

    for val in tp.iter() {
        assert!(*val as usize == 0);
    }

    unsafe {
        // 设置该线程的 `$tp` 寄存器
        asm!(
            "mv tp, {}",
            in(reg) tp.as_mut_ptr() as usize,
        );
    }
    tp
}

#[inline]
pub fn create(dtor: Option<Dtor>) -> Key {
    // 分配一个新的 TLS key。这些 key 在所有线程之间共享。
    #[allow(unused_unsafe)]
    let key = unsafe { TLS_KEY_INDEX.fetch_add(1, Relaxed) };
    if let Some(f) = dtor {
        unsafe { register_dtor(key, f) };
    }
    key
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    assert!((key < 1022) && (key >= 1));
    let table = tls_table();
    table[key] = value;
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    assert!((key < 1022) && (key >= 1));
    tls_table()[key]
}

#[inline]
pub unsafe fn destroy(_key: Key) {
    // 直接泄漏该 key。在创建大量 TLS 变量的长期运行系统上这或许不太好，
    // 但实践中这并不是问题。
}

struct Node {
    dtor: Dtor,
    key: Key,
    next: *mut Node,
}

unsafe fn register_dtor(key: Key, dtor: Dtor) {
    // 我们这里使用 System 分配器，以避免干扰某个可能使用线程本地存储的
    // Global 分配器。
    let mut node =
        ManuallyDrop::new(Box::new_in(Node { key, dtor, next: ptr::null_mut() }, System));

    #[allow(unused_unsafe)]
    let mut head = unsafe { DTORS.load(Acquire) };
    loop {
        node.next = head;
        #[allow(unused_unsafe)]
        match unsafe { DTORS.compare_exchange(head, &mut **node, Release, Acquire) } {
            Ok(_) => return, // 没有什么要 drop 的，我们已成功把节点加入列表
            Err(cur) => head = cur,
        }
    }
}

pub unsafe fn destroy_tls() {
    let tp = tls_ptr_addr();

    // 如果指针地址为 0，则说明该线程没有 TLS。
    if tp.is_null() {
        return;
    }

    unsafe { run_dtors() };

    // 最后，释放 TLS 数组
    unsafe {
        unmap_memory(core::slice::from_raw_parts_mut(tp, TLS_MEMORY_SIZE / size_of::<usize>()))
            .unwrap()
    };
}

// 此函数被标记为 inline(never)，以防止 dealloc 调用被重排到 TLS 已被销毁之后。
// 更多背景请参见
// https://github.com/rust-lang/rust/pull/144465#pullrequestreview-3289729950 。
#[inline(never)]
unsafe fn run_dtors() {
    let mut any_run = true;

    // 把析构函数运行“若干”次。Windows 上是 5 次，所以我们这里照搬。
    // 这允许 TLS 变量在被销毁时创建出新的 TLS 变量，而这些新变量也会被销毁。
    // 一直进行下去，直到用完尝试次数，或者直到没有任何东西可销毁为止。
    for _ in 0..5 {
        if !any_run {
            break;
        }
        any_run = false;
        #[allow(unused_unsafe)]
        let mut cur = unsafe { DTORS.load(Acquire) };
        while !cur.is_null() {
            let ptr = unsafe { get((*cur).key) };

            if !ptr.is_null() {
                unsafe { set((*cur).key, ptr::null_mut()) };
                unsafe { ((*cur).dtor)(ptr as *mut _) };
                any_run = true;
            }

            unsafe { cur = (*cur).next };
        }
    }

    crate::rt::thread_cleanup();
}
