//! 包含大部分共享的 UEFI 特定内容。如有需要，其中一些可能会被移动到
//! `std::os::uefi`；但在 UEFI 本身尚未获得 Std 支持的前提下，没有必要额外
//! 添加公开 API。
//!
//! 一些术语
//! * Protocol（协议）：
//! - Protocol 用于在各自独立构建的模块（包括驱动）之间实现通信。
//! - 每个 protocol 都关联一个 GUID。该 GUID 充当该 protocol 的名称。
//! - Protocol 有生产方与消费方。
//! - 关于 protocol 的更多信息可参见[此处](https://edk2-docs.gitbook.io/edk-ii-uefi-driver-writer-s-guide/3_foundation/36_protocols_and_handles)

use r_efi::efi::{self, Guid};
use r_efi::protocols::{device_path, device_path_to_text, file, service_binding, shell};

use crate::alloc::Layout;
use crate::ffi::{OsStr, OsString};
use crate::io::{self, const_error};
use crate::marker::PhantomData;
use crate::mem::MaybeUninit;
use crate::os::uefi::env::boot_services;
use crate::os::uefi::ffi::{OsStrExt, OsStringExt};
use crate::os::uefi::{self};
use crate::path::Path;
use crate::ptr::NonNull;
use crate::slice;
use crate::sync::atomic::{Atomic, AtomicPtr, Ordering};
use crate::sys::helpers::WStrUnits;

type BootInstallMultipleProtocolInterfaces =
    unsafe extern "efiapi" fn(_: *mut r_efi::efi::Handle, _: ...) -> r_efi::efi::Status;

type BootUninstallMultipleProtocolInterfaces =
    unsafe extern "efiapi" fn(_: r_efi::efi::Handle, _: ...) -> r_efi::efi::Status;

const BOOT_SERVICES_UNAVAILABLE: io::Error =
    const_error!(io::ErrorKind::Other, "Boot Services are no longer available");

/// 定位带有特定 Protocol GUID 的 Handle。
///
/// 使用 `EFI_BOOT_SERVICES.LocateHandles()` 实现。
///
/// 返回一个支持指定 protocol 的 [Handle](r_efi::efi::Handle) 数组。
pub(crate) fn locate_handles(mut guid: Guid) -> io::Result<Vec<NonNull<crate::ffi::c_void>>> {
    fn inner(
        guid: &mut Guid,
        boot_services: NonNull<r_efi::efi::BootServices>,
        buf_size: &mut usize,
        buf: *mut r_efi::efi::Handle,
    ) -> io::Result<()> {
        let r = unsafe {
            ((*boot_services.as_ptr()).locate_handle)(
                r_efi::efi::BY_PROTOCOL,
                guid,
                crate::ptr::null_mut(),
                buf_size,
                buf,
            )
        };

        if r.is_error() { Err(crate::io::Error::from_raw_os_error(r.as_usize())) } else { Ok(()) }
    }

    let boot_services = boot_services().ok_or(BOOT_SERVICES_UNAVAILABLE)?.cast();
    let mut buf_len = 0usize;

    // 这一调用应当总是失败，因为缓冲区大小为 0。该调用应当把所需的缓冲区长度
    // 更新到 buf_len 变量中
    match inner(&mut guid, boot_services, &mut buf_len, crate::ptr::null_mut()) {
        Ok(()) => unreachable!(),
        Err(e) => match e.kind() {
            io::ErrorKind::FileTooLarge => {}
            _ => return Err(e),
        },
    }

    // 返回的 buf_len 以字节为单位
    assert_eq!(buf_len % size_of::<r_efi::efi::Handle>(), 0);
    let num_of_handles = buf_len / size_of::<r_efi::efi::Handle>();
    let mut buf: Vec<r_efi::efi::Handle> = Vec::with_capacity(num_of_handles);
    match inner(&mut guid, boot_services, &mut buf_len, buf.as_mut_ptr()) {
        Ok(()) => {
            // 这是安全的，因为只有当 buf_len >= 所需长度时该调用才会成功。
            // 此外，在成功时 `buf_len` 会被更新为已写入缓冲区的大小（以字节为单位）
            unsafe { buf.set_len(num_of_handles) };
            Ok(buf.into_iter().filter_map(|x| NonNull::new(x)).collect())
        }
        Err(e) => Err(e),
    }
}

/// 在某个 handle 上打开 Protocol。
/// 内部其实只是对 `EFI_BOOT_SERVICES.OpenProtocol()` 的一次调用。
///
/// 查询某个 handle 以确定其是否支持指定 protocol。如果该 handle 支持此 protocol，
/// 则代表调用方将该 protocol 打开。
///
/// 该 protocol 以 GET_PROTOCOL 属性打开，这意味着调用方不需要通过
/// `EFI_BOOT_SERVICES.CloseProtocol()` 关闭该 protocol 接口
pub(crate) fn open_protocol<T>(
    handle: NonNull<crate::ffi::c_void>,
    mut protocol_guid: Guid,
) -> io::Result<NonNull<T>> {
    let boot_services: NonNull<efi::BootServices> =
        boot_services().ok_or(BOOT_SERVICES_UNAVAILABLE)?.cast();
    let system_handle = uefi::env::image_handle();
    let mut protocol: MaybeUninit<*mut T> = MaybeUninit::uninit();

    let r = unsafe {
        ((*boot_services.as_ptr()).open_protocol)(
            handle.as_ptr(),
            &mut protocol_guid,
            protocol.as_mut_ptr().cast(),
            system_handle.as_ptr(),
            crate::ptr::null_mut(),
            r_efi::system::OPEN_PROTOCOL_GET_PROTOCOL,
        )
    };

    if r.is_error() {
        Err(crate::io::Error::from_raw_os_error(r.as_usize()))
    } else {
        NonNull::new(unsafe { protocol.assume_init() })
            .ok_or(const_error!(io::ErrorKind::Other, "null protocol"))
    }
}

/// 获取当前系统 handle 的 Protocol。
///
/// 注意：某些 protocol 需要手动释放。这是调用方的责任。
pub(crate) fn image_handle_protocol<T>(protocol_guid: Guid) -> io::Result<NonNull<T>> {
    let system_handle = uefi::env::try_image_handle()
        .ok_or(io::const_error!(io::ErrorKind::NotFound, "protocol not found in Image handle"))?;
    open_protocol(system_handle, protocol_guid)
}

pub(crate) fn device_path_to_text(path: NonNull<device_path::Protocol>) -> io::Result<OsString> {
    fn path_to_text(
        protocol: NonNull<device_path_to_text::Protocol>,
        path: NonNull<device_path::Protocol>,
    ) -> io::Result<OsString> {
        let path_ptr: *mut r_efi::efi::Char16 = unsafe {
            ((*protocol.as_ptr()).convert_device_path_to_text)(
                path.as_ptr(),
                // DisplayOnly
                r_efi::efi::Boolean::FALSE,
                // AllowShortcuts
                r_efi::efi::Boolean::FALSE,
            )
        };

        let path = os_string_from_raw(path_ptr)
            .ok_or(io::const_error!(io::ErrorKind::InvalidData, "invalid path"))?;

        if let Some(boot_services) = crate::os::uefi::env::boot_services() {
            let boot_services: NonNull<r_efi::efi::BootServices> = boot_services.cast();
            unsafe {
                ((*boot_services.as_ptr()).free_pool)(path_ptr.cast());
            }
        }

        Ok(path)
    }

    static LAST_VALID_HANDLE: Atomic<*mut crate::ffi::c_void> =
        AtomicPtr::new(crate::ptr::null_mut());

    if let Some(handle) = NonNull::new(LAST_VALID_HANDLE.load(Ordering::Acquire)) {
        if let Ok(protocol) = open_protocol::<device_path_to_text::Protocol>(
            handle,
            device_path_to_text::PROTOCOL_GUID,
        ) {
            return path_to_text(protocol, path);
        }
    }

    let device_path_to_text_handles = locate_handles(device_path_to_text::PROTOCOL_GUID)?;
    for handle in device_path_to_text_handles {
        if let Ok(protocol) = open_protocol::<device_path_to_text::Protocol>(
            handle,
            device_path_to_text::PROTOCOL_GUID,
        ) {
            LAST_VALID_HANDLE.store(handle.as_ptr(), Ordering::Release);
            return path_to_text(protocol, path);
        }
    }

    Err(io::const_error!(io::ErrorKind::NotFound, "no device path to text protocol found"))
}

fn device_node_to_text(path: NonNull<device_path::Protocol>) -> io::Result<OsString> {
    fn node_to_text(
        protocol: NonNull<device_path_to_text::Protocol>,
        path: NonNull<device_path::Protocol>,
    ) -> io::Result<OsString> {
        let path_ptr: *mut r_efi::efi::Char16 = unsafe {
            ((*protocol.as_ptr()).convert_device_node_to_text)(
                path.as_ptr(),
                // DisplayOnly
                r_efi::efi::Boolean::FALSE,
                // AllowShortcuts
                r_efi::efi::Boolean::FALSE,
            )
        };

        let path = os_string_from_raw(path_ptr)
            .ok_or(io::const_error!(io::ErrorKind::InvalidData, "Invalid path"))?;

        if let Some(boot_services) = crate::os::uefi::env::boot_services() {
            let boot_services: NonNull<r_efi::efi::BootServices> = boot_services.cast();
            unsafe {
                ((*boot_services.as_ptr()).free_pool)(path_ptr.cast());
            }
        }

        Ok(path)
    }

    static LAST_VALID_HANDLE: AtomicPtr<crate::ffi::c_void> =
        AtomicPtr::new(crate::ptr::null_mut());

    if let Some(handle) = NonNull::new(LAST_VALID_HANDLE.load(Ordering::Acquire)) {
        if let Ok(protocol) = open_protocol::<device_path_to_text::Protocol>(
            handle,
            device_path_to_text::PROTOCOL_GUID,
        ) {
            return node_to_text(protocol, path);
        }
    }

    let device_path_to_text_handles = locate_handles(device_path_to_text::PROTOCOL_GUID)?;
    for handle in device_path_to_text_handles {
        if let Ok(protocol) = open_protocol::<device_path_to_text::Protocol>(
            handle,
            device_path_to_text::PROTOCOL_GUID,
        ) {
            LAST_VALID_HANDLE.store(handle.as_ptr(), Ordering::Release);
            return node_to_text(protocol, path);
        }
    }

    Err(io::const_error!(io::ErrorKind::NotFound, "No device path to text protocol found"))
}

/// 获取 RuntimeServices。
pub(crate) fn runtime_services() -> Option<NonNull<r_efi::efi::RuntimeServices>> {
    let system_table: NonNull<r_efi::efi::SystemTable> =
        crate::os::uefi::env::try_system_table()?.cast();
    let runtime_services = unsafe { (*system_table.as_ptr()).runtime_services };
    NonNull::new(runtime_services)
}

pub(crate) struct OwnedDevicePath(NonNull<r_efi::protocols::device_path::Protocol>);

impl OwnedDevicePath {
    pub(crate) fn from_text(p: &OsStr) -> io::Result<Self> {
        fn inner(
            p: &OsStr,
            protocol: NonNull<r_efi::protocols::device_path_from_text::Protocol>,
        ) -> io::Result<OwnedDevicePath> {
            let path_vec = p.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
            if path_vec[..path_vec.len() - 1].contains(&0) {
                return Err(const_error!(
                    io::ErrorKind::InvalidInput,
                    "strings passed to UEFI cannot contain NULs",
                ));
            }

            let path =
                unsafe { ((*protocol.as_ptr()).convert_text_to_device_path)(path_vec.as_ptr()) };

            NonNull::new(path)
                .map(OwnedDevicePath)
                .ok_or_else(|| const_error!(io::ErrorKind::InvalidFilename, "invalid Device Path"))
        }

        static LAST_VALID_HANDLE: Atomic<*mut crate::ffi::c_void> =
            AtomicPtr::new(crate::ptr::null_mut());

        if let Some(handle) = NonNull::new(LAST_VALID_HANDLE.load(Ordering::Acquire)) {
            if let Ok(protocol) = open_protocol::<r_efi::protocols::device_path_from_text::Protocol>(
                handle,
                r_efi::protocols::device_path_from_text::PROTOCOL_GUID,
            ) {
                return inner(p, protocol);
            }
        }

        let handles = locate_handles(r_efi::protocols::device_path_from_text::PROTOCOL_GUID)?;
        for handle in handles {
            if let Ok(protocol) = open_protocol::<r_efi::protocols::device_path_from_text::Protocol>(
                handle,
                r_efi::protocols::device_path_from_text::PROTOCOL_GUID,
            ) {
                LAST_VALID_HANDLE.store(handle.as_ptr(), Ordering::Release);
                return inner(p, protocol);
            }
        }

        io::Result::Err(const_error!(
            io::ErrorKind::NotFound,
            "DevicePathFromText Protocol not found",
        ))
    }

    pub(crate) const fn as_ptr(&self) -> *mut r_efi::protocols::device_path::Protocol {
        self.0.as_ptr()
    }

    pub(crate) const fn borrow<'a>(&'a self) -> BorrowedDevicePath<'a> {
        BorrowedDevicePath::new(self.0)
    }
}

impl Drop for OwnedDevicePath {
    fn drop(&mut self) {
        if let Some(bt) = boot_services() {
            let bt: NonNull<r_efi::efi::BootServices> = bt.cast();
            unsafe {
                ((*bt.as_ptr()).free_pool)(self.0.as_ptr() as *mut crate::ffi::c_void);
            }
        }
    }
}

impl crate::fmt::Debug for OwnedDevicePath {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        match self.borrow().to_text() {
            Ok(p) => p.fmt(f),
            Err(_) => f.debug_struct("OwnedDevicePath").finish_non_exhaustive(),
        }
    }
}

pub(crate) struct BorrowedDevicePath<'a> {
    protocol: NonNull<r_efi::protocols::device_path::Protocol>,
    phantom: PhantomData<&'a r_efi::protocols::device_path::Protocol>,
}

impl<'a> BorrowedDevicePath<'a> {
    pub(crate) const fn new(protocol: NonNull<r_efi::protocols::device_path::Protocol>) -> Self {
        Self { protocol, phantom: PhantomData }
    }

    pub(crate) fn to_text(&self) -> io::Result<OsString> {
        device_path_to_text(self.protocol)
    }

    pub(crate) const fn iter(&'a self) -> DevicePathIterator<'a> {
        DevicePathIterator::new(DevicePathNode::new(self.protocol))
    }
}

impl<'a> crate::fmt::Debug for BorrowedDevicePath<'a> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        match self.to_text() {
            Ok(p) => p.fmt(f),
            Err(_) => f.debug_struct("BorrowedDevicePath").finish_non_exhaustive(),
        }
    }
}

pub(crate) struct DevicePathIterator<'a>(Option<DevicePathNode<'a>>);

impl<'a> DevicePathIterator<'a> {
    const fn new(node: DevicePathNode<'a>) -> Self {
        if node.is_end() { Self(None) } else { Self(Some(node)) }
    }
}

impl<'a> Iterator for DevicePathIterator<'a> {
    type Item = DevicePathNode<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let cur_node = self.0?;

        let next_node = unsafe { cur_node.next_node() };
        self.0 = if next_node.is_end() { None } else { Some(next_node) };

        Some(cur_node)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct DevicePathNode<'a> {
    protocol: NonNull<r_efi::protocols::device_path::Protocol>,
    phantom: PhantomData<&'a r_efi::protocols::device_path::Protocol>,
}

impl<'a> DevicePathNode<'a> {
    pub(crate) const fn new(protocol: NonNull<r_efi::protocols::device_path::Protocol>) -> Self {
        Self { protocol, phantom: PhantomData }
    }

    pub(crate) const fn length(&self) -> u16 {
        let len = unsafe { (*self.protocol.as_ptr()).length };
        u16::from_le_bytes(len)
    }

    pub(crate) const fn node_type(&self) -> u8 {
        unsafe { (*self.protocol.as_ptr()).r#type }
    }

    pub(crate) const fn sub_type(&self) -> u8 {
        unsafe { (*self.protocol.as_ptr()).sub_type }
    }

    pub(crate) fn data(&self) -> &[u8] {
        let length: usize = self.length().into();

        // 某些节点没有任何特殊数据
        if length > 4 {
            let raw_ptr: *const u8 = self.protocol.as_ptr().cast();
            let data = unsafe { raw_ptr.add(4) };
            unsafe { crate::slice::from_raw_parts(data, length - 4) }
        } else {
            &[]
        }
    }

    pub(crate) const fn is_end(&self) -> bool {
        self.node_type() == r_efi::protocols::device_path::TYPE_END
            && self.sub_type() == r_efi::protocols::device_path::End::SUBTYPE_ENTIRE
    }

    pub(crate) const fn is_end_instance(&self) -> bool {
        self.node_type() == r_efi::protocols::device_path::TYPE_END
            && self.sub_type() == r_efi::protocols::device_path::End::SUBTYPE_INSTANCE
    }

    pub(crate) unsafe fn next_node(&self) -> Self {
        let node = unsafe {
            self.protocol
                .cast::<u8>()
                .add(self.length().into())
                .cast::<r_efi::protocols::device_path::Protocol>()
        };
        Self::new(node)
    }

    pub(crate) fn to_path(&'a self) -> BorrowedDevicePath<'a> {
        BorrowedDevicePath::new(self.protocol)
    }

    pub(crate) fn to_text(&self) -> io::Result<OsString> {
        device_node_to_text(self.protocol)
    }
}

impl<'a> PartialEq for DevicePathNode<'a> {
    fn eq(&self, other: &Self) -> bool {
        // 作为单个缓冲区整体比较，而非逐字段比较，因为这样优化效果更好。
        //
        // SAFETY: `Protocol` 之后紧跟着一个长度为 `length - sizeof::<Protocol>()` 的缓冲区。
        // `Protocol` 没有填充字节，因此把它解释为切片是合理的。
        unsafe {
            let s1 =
                slice::from_raw_parts(self.protocol.as_ptr().cast::<u8>(), self.length().into());
            let s2 =
                slice::from_raw_parts(other.protocol.as_ptr().cast::<u8>(), other.length().into());
            s1 == s2
        }
    }
}

impl<'a> crate::fmt::Debug for DevicePathNode<'a> {
    fn fmt(&self, f: &mut crate::fmt::Formatter<'_>) -> crate::fmt::Result {
        match self.to_text() {
            Ok(p) => p.fmt(f),
            Err(_) => f
                .debug_struct("DevicePathNode")
                .field("type", &self.node_type())
                .field("sub_type", &self.sub_type())
                .field("length", &self.length())
                .field("specific_device_path_data", &self.data())
                .finish(),
        }
    }
}

/// 由 Rust 侧安装在某个 handle 上的 Protocol。
pub(crate) struct OwnedProtocol<T> {
    guid: r_efi::efi::Guid,
    handle: NonNull<crate::ffi::c_void>,
    protocol: *mut T,
}

impl<T> OwnedProtocol<T> {
    // FIXME: 考虑使用 unsafe trait 来匹配 protocol 与 guid
    pub(crate) unsafe fn create(protocol: T, mut guid: r_efi::efi::Guid) -> io::Result<Self> {
        let bt: NonNull<r_efi::efi::BootServices> =
            boot_services().ok_or(BOOT_SERVICES_UNAVAILABLE)?.cast();
        let protocol: *mut T = Box::into_raw(Box::new(protocol));
        let mut handle: r_efi::efi::Handle = crate::ptr::null_mut();

        // FIXME: 一旦 extended_varargs_abi_support 稳定下来就移入 r-efi
        let func: BootInstallMultipleProtocolInterfaces =
            unsafe { crate::mem::transmute((*bt.as_ptr()).install_multiple_protocol_interfaces) };

        let r = unsafe {
            func(
                &mut handle,
                &mut guid as *mut _ as *mut crate::ffi::c_void,
                protocol as *mut crate::ffi::c_void,
                crate::ptr::null_mut() as *mut crate::ffi::c_void,
            )
        };

        if r.is_error() {
            drop(unsafe { Box::from_raw(protocol) });
            return Err(crate::io::Error::from_raw_os_error(r.as_usize()));
        };

        let handle = NonNull::new(handle)
            .ok_or(io::const_error!(io::ErrorKind::Uncategorized, "found null handle"))?;

        Ok(Self { guid, handle, protocol })
    }

    pub(crate) fn handle(&self) -> NonNull<crate::ffi::c_void> {
        self.handle
    }
}

impl<T> Drop for OwnedProtocol<T> {
    fn drop(&mut self) {
        // 不要释放运行时（runtime）protocol
        if let Some(bt) = boot_services() {
            let bt: NonNull<r_efi::efi::BootServices> = bt.cast();
            // FIXME: 一旦 extended_varargs_abi_support 稳定下来就移入 r-efi
            let func: BootUninstallMultipleProtocolInterfaces = unsafe {
                crate::mem::transmute((*bt.as_ptr()).uninstall_multiple_protocol_interfaces)
            };
            let status = unsafe {
                func(
                    self.handle.as_ptr(),
                    &mut self.guid as *mut _ as *mut crate::ffi::c_void,
                    self.protocol as *mut crate::ffi::c_void,
                    crate::ptr::null_mut() as *mut crate::ffi::c_void,
                )
            };

            // 万一卸载失败，则故意泄漏该 protocol
            if status == r_efi::efi::Status::SUCCESS {
                let _ = unsafe { Box::from_raw(self.protocol) };
            }
        }
    }
}

impl<T> AsRef<T> for OwnedProtocol<T> {
    fn as_ref(&self) -> &T {
        unsafe { self.protocol.as_ref().unwrap() }
    }
}

pub(crate) struct OwnedTable<T> {
    layout: crate::alloc::Layout,
    ptr: *mut T,
}

impl<T> OwnedTable<T> {
    pub(crate) fn from_table_header(hdr: &r_efi::efi::TableHeader) -> Self {
        let header_size = hdr.header_size as usize;
        let layout = crate::alloc::Layout::from_size_align(header_size, 8).unwrap();
        let ptr = unsafe { crate::alloc::alloc(layout) as *mut T };
        Self { layout, ptr }
    }

    pub(crate) const fn as_ptr(&self) -> *const T {
        self.ptr
    }

    pub(crate) const fn as_mut_ptr(&self) -> *mut T {
        self.ptr
    }
}

impl OwnedTable<r_efi::efi::SystemTable> {
    pub(crate) fn from_table(tbl: *const r_efi::efi::SystemTable) -> Self {
        let hdr = unsafe { (*tbl).hdr };

        let owned_tbl = Self::from_table_header(&hdr);
        unsafe {
            crate::ptr::copy_nonoverlapping(
                tbl as *const u8,
                owned_tbl.as_mut_ptr() as *mut u8,
                hdr.header_size as usize,
            )
        };

        owned_tbl
    }
}

impl<T> Drop for OwnedTable<T> {
    fn drop(&mut self) {
        unsafe { crate::alloc::dealloc(self.ptr as *mut u8, self.layout) };
    }
}

/// 从指向以 NULL 结尾的 UTF-16 字符串的指针创建 OsString
pub(crate) fn os_string_from_raw(ptr: *mut r_efi::efi::Char16) -> Option<OsString> {
    let path_len = unsafe { WStrUnits::new(ptr)?.count() };
    Some(OsString::from_wide(unsafe { slice::from_raw_parts(ptr.cast(), path_len) }))
}

/// 创建以 NULL 结尾的 UTF-16 字符串
pub(crate) fn os_string_to_raw(s: &OsStr) -> Option<Box<[r_efi::efi::Char16]>> {
    let temp = s.encode_wide().chain(Some(0)).collect::<Box<[r_efi::efi::Char16]>>();
    if temp[..temp.len() - 1].contains(&0) { None } else { Some(temp) }
}

pub(crate) fn open_shell() -> Option<NonNull<shell::Protocol>> {
    static LAST_VALID_HANDLE: Atomic<*mut crate::ffi::c_void> =
        AtomicPtr::new(crate::ptr::null_mut());

    if let Some(handle) = NonNull::new(LAST_VALID_HANDLE.load(Ordering::Acquire)) {
        if let Ok(protocol) = open_protocol::<shell::Protocol>(handle, shell::PROTOCOL_GUID) {
            return Some(protocol);
        }
    }

    let handles = locate_handles(shell::PROTOCOL_GUID).ok()?;
    for handle in handles {
        if let Ok(protocol) = open_protocol::<shell::Protocol>(handle, shell::PROTOCOL_GUID) {
            LAST_VALID_HANDLE.store(handle.as_ptr(), Ordering::Release);
            return Some(protocol);
        }
    }

    None
}

/// 获取与 shell 映射关联的 device path protocol。
///
/// 如果不存在这样的映射，则返回 None
pub(crate) fn get_device_path_from_map(map: &Path) -> io::Result<BorrowedDevicePath<'static>> {
    let shell =
        open_shell().ok_or(io::const_error!(io::ErrorKind::NotFound, "UEFI Shell not found"))?;
    let mut path = os_string_to_raw(map.as_os_str())
        .ok_or(io::const_error!(io::ErrorKind::InvalidFilename, "invalid UEFI shell mapping"))?;

    // UEFI shell 返回的 Device Path Protocol 指针归 shell 所有，在其整个生命周期内
    // 都不会被释放。因此它具有 'static 生命周期。
    let protocol = unsafe { ((*shell.as_ptr()).get_device_path_from_map)(path.as_mut_ptr()) };
    let protocol = NonNull::new(protocol)
        .ok_or(io::const_error!(io::ErrorKind::NotFound, "UEFI Shell mapping not found"))?;

    Ok(BorrowedDevicePath::new(protocol))
}

/// 用于那些通过
/// [EFI_SERVICE_BINDING_PROTOCOL](https://uefi.org/specs/UEFI/2.11/11_Protocols_UEFI_Driver_Model.html#efi-service-binding-protocol)
/// 创建和销毁的 UEFI Protocol 的辅助工具
///
/// # 不变量(Invariant)
/// - `handle` 必须始终是与 `service_guid` 对应的有效 UEFI handle。
/// - 只要 `handle` 保持有效，复制 `ServiceProtocol` 就是合理的。
/// - 对大多数 service binding protocol（在 edk2 实现中），这类 handle 在整个 UEFI
///   环境的生命周期内都保持有效——实际上相当于 `'static`。
#[derive(Clone, Copy)]
pub(crate) struct ServiceProtocol {
    service_guid: r_efi::efi::Guid,
    handle: NonNull<crate::ffi::c_void>,
}

impl ServiceProtocol {
    /// 在 service_binding protocol 上打开一个子 handle。
    pub(crate) fn open(
        service_guid: r_efi::efi::Guid,
    ) -> io::Result<(Self, NonNull<crate::ffi::c_void>)> {
        let handles = locate_handles(service_guid)?;

        for handle in handles {
            if let Ok(protocol) = open_protocol::<service_binding::Protocol>(handle, service_guid) {
                if let Ok(child_handle) = unsafe { Self::create_child(protocol) } {
                    return Ok((Self { service_guid, handle }, child_handle));
                }
            }
        }

        Err(io::const_error!(io::ErrorKind::NotFound, "no service binding protocol found"))
    }

    // SAFETY: sbp 必须是一个有效的 service binding protocol 指针
    unsafe fn create_child(
        sbp: NonNull<service_binding::Protocol>,
    ) -> io::Result<NonNull<crate::ffi::c_void>> {
        let mut child_handle: r_efi::efi::Handle = crate::ptr::null_mut();
        // SAFETY: 如果传入指向 NULL 的指针，则会分配一个新的 handle。
        let r = unsafe { ((*sbp.as_ptr()).create_child)(sbp.as_ptr(), &mut child_handle) };

        if r.is_error() {
            Err(crate::io::Error::from_raw_os_error(r.as_usize()))
        } else {
            NonNull::new(child_handle)
                .ok_or(const_error!(io::ErrorKind::Other, "null child handle"))
        }
    }

    // SAFETY: 子 handle 必须由当前 service binding protocol 分配，且必须有效。
    pub(crate) unsafe fn destroy_child(
        &self,
        handle: NonNull<crate::ffi::c_void>,
    ) -> io::Result<()> {
        let sbp = open_protocol::<service_binding::Protocol>(self.handle, self.service_guid)?;

        let r = unsafe { ((*sbp.as_ptr()).destroy_child)(sbp.as_ptr(), handle.as_ptr()) };
        if r.is_error() { Err(crate::io::Error::from_raw_os_error(r.as_usize())) } else { Ok(()) }
    }
}

#[repr(transparent)]
pub(crate) struct OwnedEvent(NonNull<crate::ffi::c_void>);

impl OwnedEvent {
    pub(crate) fn new(
        signal: u32,
        tpl: efi::Tpl,
        handler: Option<efi::EventNotify>,
        context: Option<NonNull<crate::ffi::c_void>>,
    ) -> io::Result<Self> {
        let boot_services: NonNull<efi::BootServices> =
            boot_services().ok_or(BOOT_SERVICES_UNAVAILABLE)?.cast();
        let mut event: r_efi::efi::Event = crate::ptr::null_mut();
        let context = context.map(NonNull::as_ptr).unwrap_or(crate::ptr::null_mut());

        let r = unsafe {
            let create_event = (*boot_services.as_ptr()).create_event;
            (create_event)(signal, tpl, handler, context, &mut event)
        };

        if r.is_error() {
            Err(crate::io::Error::from_raw_os_error(r.as_usize()))
        } else {
            NonNull::new(event)
                .ok_or(const_error!(io::ErrorKind::Other, "failed to create event"))
                .map(Self)
        }
    }

    pub(crate) fn as_ptr(&self) -> efi::Event {
        self.0.as_ptr()
    }

    pub(crate) fn into_raw(self) -> *mut crate::ffi::c_void {
        let r = self.0.as_ptr();
        crate::mem::forget(self);
        r
    }

    /// SAFETY: 假定 ptr 是一个非空的有效 UEFI event
    pub(crate) unsafe fn from_raw(ptr: *mut crate::ffi::c_void) -> Self {
        Self(unsafe { NonNull::new_unchecked(ptr) })
    }
}

impl Drop for OwnedEvent {
    fn drop(&mut self) {
        if let Some(boot_services) = boot_services() {
            let bt: NonNull<r_efi::efi::BootServices> = boot_services.cast();
            unsafe {
                let close_event = (*bt.as_ptr()).close_event;
                (close_event)(self.0.as_ptr())
            };
        }
    }
}

pub(crate) const fn ipv4_to_r_efi(addr: crate::net::Ipv4Addr) -> efi::Ipv4Address {
    efi::Ipv4Address { addr: addr.octets() }
}

pub(crate) const fn ipv4_from_r_efi(ip: efi::Ipv4Address) -> crate::net::Ipv4Addr {
    crate::net::Ipv4Addr::new(ip.addr[0], ip.addr[1], ip.addr[2], ip.addr[3])
}

/// 该类型设计用于 ZST（零大小类型）。由于这类类型是 unsized 的，对它们的引用在
/// Rust 中并不有效。因此，与这类类型交互时只应使用指针。
pub(crate) struct UefiBox<T> {
    inner: NonNull<T>,
    size: usize,
}

impl<T> UefiBox<T> {
    pub(crate) fn new(len: usize) -> io::Result<Self> {
        assert!(len >= size_of::<T>());
        // UEFI 总是要求类型按 8 字节对齐。
        let layout = Layout::from_size_align(len, 8).unwrap();
        let ptr = unsafe { crate::alloc::alloc(layout) };

        match NonNull::new(ptr.cast()) {
            Some(inner) => Ok(Self { inner, size: len }),
            None => Err(const_error!(io::ErrorKind::OutOfMemory, "Allocation failed")),
        }
    }

    pub(crate) fn write(&mut self, data: T) {
        unsafe { self.inner.write(data) }
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut T {
        self.inner.as_ptr().cast()
    }

    pub(crate) fn as_ptr(&self) -> *const T {
        self.inner.as_ptr().cast()
    }

    pub(crate) const fn len(&self) -> usize {
        self.size
    }
}

impl<T> Drop for UefiBox<T> {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.size, 8).unwrap();
        unsafe { crate::alloc::dealloc(self.inner.as_ptr().cast(), layout) };
    }
}

impl UefiBox<file::Info> {
    fn size(&self) -> u64 {
        unsafe { (*self.as_ptr()).size }
    }

    fn set_size(&mut self, s: u64) {
        unsafe { (*self.as_mut_ptr()).size = s }
    }

    // 字符串长度（包含 NULL），而非字节数。
    fn file_name_len(&self) -> usize {
        (self.size() as usize - size_of::<file::Info<0>>()) / size_of::<u16>()
    }

    pub(crate) fn file_name(&self) -> &[u16] {
        unsafe {
            crate::slice::from_raw_parts((*self.as_ptr()).file_name.as_ptr(), self.file_name_len())
        }
    }

    fn file_name_mut(&mut self) -> &mut [u16] {
        unsafe {
            crate::slice::from_raw_parts_mut(
                (*self.as_mut_ptr()).file_name.as_mut_ptr(),
                self.file_name_len(),
            )
        }
    }

    pub(crate) fn with_file_name(mut self, name: &OsStr) -> io::Result<Self> {
        // os_string_to_raw 返回以 NULL 结尾的字符串。因此无需单独处理。
        let fname = os_string_to_raw(name)
            .ok_or(const_error!(io::ErrorKind::OutOfMemory, "Allocation failed"))?;
        let new_size = size_of::<file::Info<0>>() + fname.len() * size_of::<u16>();

        // 如果新名称能放进当前结构中，则复用它。
        if self.size() >= new_size as u64 {
            self.file_name_mut()[..fname.len()].copy_from_slice(&fname);
            self.set_size(new_size as u64);

            return Ok(self);
        }

        let mut new_box = UefiBox::new(new_size)?;

        unsafe {
            crate::ptr::copy_nonoverlapping(self.as_ptr(), new_box.as_mut_ptr(), 1);
        }
        new_box.set_size(new_size as u64);
        new_box.file_name_mut().copy_from_slice(&fname);

        Ok(new_box)
    }
}
