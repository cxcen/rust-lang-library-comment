use crate::alloc::System;
use crate::cell::RefCell;
use crate::sys::thread_local::guard;

#[thread_local]
static DTORS: RefCell<Vec<(*mut u8, unsafe extern "C" fn(*mut u8)), System>> =
    RefCell::new(Vec::new_in(System));

pub unsafe fn register(t: *mut u8, dtor: unsafe extern "C" fn(*mut u8)) {
    let Ok(mut dtors) = DTORS.try_borrow_mut() else {
        rtabort!("the System allocator may not use TLS with destructors")
    };
    guard::enable();
    dtors.push((t, dtor));
}

/// [`guard`] 模块包含平台相关的函数；如果已调用过 [`guard::enable`]，
/// 它们会在线程退出时运行此函数。
///
/// # 安全性(Safety）
///
/// 只能在线程退出时运行，以保证在 TLS 变量被销毁期间不存在指向它们的
/// 活引用（live references）。
pub unsafe fn run() {
    loop {
        let mut dtors = DTORS.borrow_mut();
        match dtors.pop() {
            Some((t, dtor)) => {
                drop(dtors);
                unsafe {
                    dtor(t);
                }
            }
            None => {
                // 释放列表所占用的内存。
                *dtors = Vec::new_in(System);
                break;
            }
        }
    }
}
