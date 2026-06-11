#[cfg(not(any(
    target_env = "newlib",
    target_os = "l4re",
    target_os = "emscripten",
    target_os = "redox",
    target_os = "hurd",
    target_os = "aix",
    target_os = "wasi",
)))]
use crate::ffi::CStr;
use crate::mem::{self, DropGuard, ManuallyDrop};
use crate::num::NonZero;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
use crate::sys::weak::dlsym;
#[cfg(any(target_os = "solaris", target_os = "illumos", target_os = "nto",))]
use crate::sys::weak::weak;
use crate::thread::ThreadInit;
use crate::time::Duration;
use crate::{cmp, io, ptr, sys};
#[cfg(not(any(
    target_os = "l4re",
    target_os = "vxworks",
    target_os = "espidf",
    target_os = "nuttx"
)))]
pub const DEFAULT_MIN_STACK_SIZE: usize = 2 * 1024 * 1024;
#[cfg(target_os = "l4re")]
pub const DEFAULT_MIN_STACK_SIZE: usize = 1024 * 1024;
#[cfg(target_os = "vxworks")]
pub const DEFAULT_MIN_STACK_SIZE: usize = 256 * 1024;
#[cfg(any(target_os = "espidf", target_os = "nuttx"))]
pub const DEFAULT_MIN_STACK_SIZE: usize = 0; // 0 表示应使用 ESP-IDF/NuttX menuconfig 系统中配置的栈大小

pub struct Thread {
    id: libc::pthread_t,
}

// 在某些平台上，pthread_t 可能是一个指针，此时我们仍然希望
// Thread 是 Send/Sync 的。
unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

impl Thread {
    // unsafe：安全性要求参见 thread::Builder::spawn_unchecked
    #[cfg_attr(miri, track_caller)] // 即使没有 panic，这也有助于 Miri 的 backtrace
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        // FIXME：一旦 wasi-sdk 更新并包含来自
        // https://github.com/WebAssembly/wasi-libc/pull/716 的修复，就移除此代码块。
        // WASI 不支持通过 pthreads 实现线程。尽管 wasi-libc 提供了 pthread 桩函数，
        // 但 pthread_create 会返回 EAGAIN，从而导致令人困惑的错误。
        // 因此我们直接返回 UNSUPPORTED_PLATFORM。
        if cfg!(target_os = "wasi") {
            return Err(io::Error::UNSUPPORTED_PLATFORM);
        }

        let data = init;
        let mut attr: mem::MaybeUninit<libc::pthread_attr_t> = mem::MaybeUninit::uninit();
        assert_eq!(libc::pthread_attr_init(attr.as_mut_ptr()), 0);
        let mut attr = DropGuard::new(&mut attr, |attr| {
            assert_eq!(libc::pthread_attr_destroy(attr.as_mut_ptr()), 0)
        });

        #[cfg(any(target_os = "espidf", target_os = "nuttx"))]
        if stack > 0 {
            // 仅当传入的值非零时才设置栈大小。
            // 0 用作一个标志，表示应使用 ESP-IDF/NuttX menuconfig 系统中配置的默认栈大小。
            assert_eq!(
                libc::pthread_attr_setstacksize(
                    attr.as_mut_ptr(),
                    cmp::max(stack, min_stack_size(attr.as_ptr()))
                ),
                0
            );
        }

        #[cfg(not(any(target_os = "espidf", target_os = "nuttx")))]
        {
            let stack_size = cmp::max(stack, min_stack_size(attr.as_ptr()));

            match libc::pthread_attr_setstacksize(attr.as_mut_ptr(), stack_size) {
                0 => {}
                n => {
                    assert_eq!(n, libc::EINVAL);
                    // EINVAL 意味着 |stack_size| 要么太小，要么不是系统页大小的整数倍。
                    // 由于它肯定 >= PTHREAD_STACK_MIN，所以一定是对齐问题。
                    // 向上取整到最近的页边界后再试一次。
                    let page_size = sys::os::page_size();
                    let stack_size =
                        (stack_size + page_size - 1) & (-(page_size as isize - 1) as usize - 1);

                    // 某些 libc 实现（例如 musl）对栈大小设置了上限，
                    // 这种情况下我们只能在此优雅地返回一个错误。
                    if libc::pthread_attr_setstacksize(attr.as_mut_ptr(), stack_size) != 0 {
                        return Err(io::const_error!(
                            io::ErrorKind::InvalidInput,
                            "invalid stack size"
                        ));
                    }
                }
            };
        }

        let data = Box::into_raw(data);
        let mut native: libc::pthread_t = mem::zeroed();
        let ret = libc::pthread_create(&mut native, attr.as_ptr(), thread_start, data as *mut _);
        return if ret == 0 {
            Ok(Thread { id: native })
        } else {
            // 线程启动失败，因此 `data` 没有被消费掉。
            // 所以重新构造这个 box 以便它被释放是安全的。
            drop(Box::from_raw(data));
            Err(io::Error::from_raw_os_error(ret))
        };

        extern "C" fn thread_start(data: *mut libc::c_void) -> *mut libc::c_void {
            unsafe {
                // SAFETY：我们只是在重建先前被泄漏的 box。
                let init = Box::from_raw(data as *mut ThreadInit);
                let rust_start = init.init();

                // 现在线程信息已经设置完毕，建立我们的栈溢出处理器（stack
                // overflow handler）。
                let _handler = sys::stack_overflow::Handler::new();

                rust_start();
            }
            ptr::null_mut()
        }
    }

    pub fn join(self) {
        let id = self.into_id();
        let ret = unsafe { libc::pthread_join(id, ptr::null_mut()) };
        assert!(ret == 0, "failed to join thread: {}", io::Error::from_raw_os_error(ret));
    }

    #[cfg(not(target_os = "wasi"))]
    pub fn id(&self) -> libc::pthread_t {
        self.id
    }

    pub fn into_id(self) -> libc::pthread_t {
        ManuallyDrop::new(self).id
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        let ret = unsafe { libc::pthread_detach(self.id) };
        debug_assert_eq!(ret, 0);
    }
}

pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    cfg_select! {
        any(
            target_os = "android",
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "hurd",
            target_os = "linux",
            target_os = "aix",
            target_vendor = "apple",
            target_os = "cygwin",
        ) => {
            #[allow(unused_assignments)]
            #[allow(unused_mut)]
            let mut quota = usize::MAX;

            #[cfg(any(target_os = "android", target_os = "linux"))]
            {
                quota = cgroups::quota().max(1);
                let mut set: libc::cpu_set_t = unsafe { mem::zeroed() };
                unsafe {
                    if libc::sched_getaffinity(0, size_of::<libc::cpu_set_t>(), &mut set) == 0 {
                        let count = libc::CPU_COUNT(&set) as usize;
                        let count = count.min(quota);

                        // 根据 sched_getaffinity 的 API，它应当总是非零；
                        // 但某些老旧的 MIPS 内核存在 bug，当没有显式设置
                        // 亲和性掩码时会将其零初始化。
                        // 在那种情况下我们退回到 sysconf 方案。
                        if let Some(count) = NonZero::new(count) {
                            return Ok(count)
                        }
                    }
                }
            }
            match unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) } {
                -1 => Err(io::Error::last_os_error()),
                0 => Err(io::Error::UNKNOWN_THREAD_COUNT),
                cpus => {
                    let count = cpus as usize;
                    // 处理这样一种不寻常的情形：我们能够获取到配额（quota），但无法获取亲和性掩码
                    let count = count.min(quota);
                    Ok(unsafe { NonZero::new_unchecked(count) })
                }
            }
        }
        any(
           target_os = "freebsd",
           target_os = "dragonfly",
           target_os = "openbsd",
           target_os = "netbsd",
        ) => {
            use crate::ptr;

            #[cfg(target_os = "freebsd")]
            {
                let mut set: libc::cpuset_t = unsafe { mem::zeroed() };
                unsafe {
                    if libc::cpuset_getaffinity(
                        libc::CPU_LEVEL_WHICH,
                        libc::CPU_WHICH_PID,
                        -1,
                        size_of::<libc::cpuset_t>(),
                        &mut set,
                    ) == 0 {
                        let count = libc::CPU_COUNT(&set) as usize;
                        if count > 0 {
                            return Ok(NonZero::new_unchecked(count));
                        }
                    }
                }
            }

            #[cfg(target_os = "netbsd")]
            {
                unsafe {
                    let set = libc::_cpuset_create();
                    if !set.is_null() {
                        let mut count: usize = 0;
                        if libc::pthread_getaffinity_np(libc::pthread_self(), libc::_cpuset_size(set), set) == 0 {
                            for i in 0..libc::cpuid_t::MAX {
                                match libc::_cpuset_isset(i, set) {
                                    -1 => break,
                                    0 => continue,
                                    _ => count = count + 1,
                                }
                            }
                        }
                        libc::_cpuset_destroy(set);
                        if let Some(count) = NonZero::new(count) {
                            return Ok(count);
                        }
                    }
                }
            }

            let mut cpus: libc::c_uint = 0;
            let mut cpus_size = size_of_val(&cpus);

            unsafe {
                cpus = libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as libc::c_uint;
            }

            // 出错或没有硬件线程时的回退方案。
            if cpus < 1 {
                let mut mib = [libc::CTL_HW, libc::HW_NCPU, 0, 0];
                let res = unsafe {
                    libc::sysctl(
                        mib.as_mut_ptr(),
                        2,
                        (&raw mut cpus) as *mut _,
                        (&raw mut cpus_size) as *mut _,
                        ptr::null_mut(),
                        0,
                    )
                };

                // 若有错误则处理之。
                if res == -1 {
                    return Err(io::Error::last_os_error());
                } else if cpus == 0 {
                    return Err(io::Error::UNKNOWN_THREAD_COUNT);
                }
            }

            Ok(unsafe { NonZero::new_unchecked(cpus as usize) })
        }
        target_os = "nto" => {
            unsafe {
                use libc::_syspage_ptr;
                if _syspage_ptr.is_null() {
                    Err(io::const_error!(io::ErrorKind::NotFound, "no syspage available"))
                } else {
                    let cpus = (*_syspage_ptr).num_cpu;
                    NonZero::new(cpus as usize)
                        .ok_or(io::Error::UNKNOWN_THREAD_COUNT)
                }
            }
        }
        any(target_os = "solaris", target_os = "illumos") => {
            let mut cpus = 0u32;
            if unsafe { libc::pset_info(libc::PS_MYID, core::ptr::null_mut(), &mut cpus, core::ptr::null_mut()) } != 0 {
                return Err(io::Error::UNKNOWN_THREAD_COUNT);
            }
            Ok(unsafe { NonZero::new_unchecked(cpus as usize) })
        }
        target_os = "haiku" => {
            // system_info 的 cpu_count 字段获取的是启动时通过 `smp_set_num_cpus` 设定的静态数据
            // `get_system_info` 内部随后调用 `smp_get_num_cpus`
            unsafe {
                let mut sinfo: libc::system_info = crate::mem::zeroed();
                let res = libc::get_system_info(&mut sinfo);

                if res != libc::B_OK {
                    return Err(io::Error::UNKNOWN_THREAD_COUNT);
                }

                Ok(NonZero::new_unchecked(sinfo.cpu_count as usize))
            }
        }
        target_os = "vxworks" => {
            // 注意：还有一个 `vxCpuConfiguredGet`，它比实际可用核心数
            // 更接近 _SC_NPROCESSORS_CONF 的语义。

            // SAFETY：`vxCpuEnabledGet` 总是返回一个至少有一位被置位的掩码
            unsafe{
                let set = libc::vxCpuEnabledGet();
                Ok(NonZero::new_unchecked(set.count_ones() as usize))
            }
        }
        _ => {
            // FIXME：在 Redox、l4re 上实现
            Err(io::const_error!(io::ErrorKind::Unsupported, "getting the number of hardware threads is not supported on the target platform"))
        }
    }
}

pub fn current_os_id() -> Option<u64> {
    // 大多数 Unix 平台都有办法查询当前线程的整数 ID，只是拼写各不相同。
    //
    // 这里使用 OS 线程 ID 而非 `pthread_self`，是为了与进程检视工具
    // （调试器、trace、`top` 等）所显示的内容相匹配。
    cfg_select! {
        // 大多数平台都有一个返回 `pid_t` 或 int（即 `i32`）的函数。
        any(target_os = "android", target_os = "linux") => {
            use crate::sys::pal::weak::syscall;

            // `libc::gettid` 仅在 glibc 2.30+ 上可用，但该系统调用自
            // Linux 2.4.11 起即已存在。
            syscall!(fn gettid() -> libc::pid_t;);

            // SAFETY：无前置条件的 FFI 调用。
            let id: libc::pid_t = unsafe { gettid() };
            Some(id as u64)
        }
        target_os = "nto" => {
            // SAFETY：无前置条件的 FFI 调用。
            let id: libc::pid_t = unsafe { libc::gettid() };
            Some(id as u64)
        }
        target_os = "openbsd" => {
            // SAFETY：无前置条件的 FFI 调用。
            let id: libc::pid_t = unsafe { libc::getthrid() };
            Some(id as u64)
        }
        target_os = "freebsd" => {
            // SAFETY：无前置条件的 FFI 调用。
            let id: libc::c_int = unsafe { libc::pthread_getthreadid_np() };
            Some(id as u64)
        }
        target_os = "netbsd" => {
            // SAFETY：无前置条件的 FFI 调用。
            let id: libc::lwpid_t = unsafe { libc::_lwp_self() };
            Some(id as u64)
        }
        any(target_os = "illumos", target_os = "solaris") => {
            // 在 Illumos 和 Solaris 上，`pthread_t` 与 OS 线程 ID 相同。
            // SAFETY：无前置条件的 FFI 调用。
            let id: libc::pthread_t = unsafe { libc::pthread_self() };
            Some(id as u64)
        }
        target_vendor = "apple" => {
            // Apple 允许查询任意线程 ID，`thread=NULL` 表示查询当前线程。
            let mut id = 0u64;
            // SAFETY：`thread_id` 是有效指针，无其他前置条件。
            let status: libc::c_int = unsafe { libc::pthread_threadid_np(0, &mut id) };
            if status == 0 {
                Some(id)
            } else {
                None
            }
        }
        // 其他平台没有 OS 线程 ID，或者没有办法访问它。
        _ => None,
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "nto",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "vxworks",
    target_os = "cygwin",
    target_vendor = "apple",
))]
fn truncate_cstr<const MAX_WITH_NUL: usize>(cstr: &CStr) -> [libc::c_char; MAX_WITH_NUL] {
    let mut result = [0; MAX_WITH_NUL];
    for (src, dst) in cstr.to_bytes().iter().zip(&mut result[..MAX_WITH_NUL - 1]) {
        *dst = *src as libc::c_char;
    }
    result
}

#[cfg(target_os = "android")]
pub fn set_name(name: &CStr) {
    const PR_SET_NAME: libc::c_int = 15;
    unsafe {
        let res = libc::prctl(
            PR_SET_NAME,
            name.as_ptr(),
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        );
        // 我们这里无法很好地传播错误，但在 debug 构建中，让我们检查一下它确实生效了。
        debug_assert_eq!(res, 0);
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "nuttx",
    target_os = "cygwin"
))]
pub fn set_name(name: &CStr) {
    unsafe {
        cfg_select! {
            any(target_os = "linux", target_os = "cygwin") => {
                // Linux 和 Cygwin 限制了名称的允许长度。
                const TASK_COMM_LEN: usize = 16;
                let name = truncate_cstr::<{ TASK_COMM_LEN }>(name);
            }
            _ => {
                // FreeBSD、DragonFly BSD 和 NuttX 不强制长度限制。
            }
        };
        // 在 Linux 上自 glibc 2.12、musl 1.1.16 和 uClibc 1.0.20 起可用，
        // 在 FreeBSD 12.2 和 13.0、以及 DragonFly BSD 6.0 上可用。
        let res = libc::pthread_setname_np(libc::pthread_self(), name.as_ptr());
        // 我们这里无法很好地传播错误，但在 debug 构建中，让我们检查一下它确实生效了。
        debug_assert_eq!(res, 0);
    }
}

#[cfg(target_os = "openbsd")]
pub fn set_name(name: &CStr) {
    unsafe {
        libc::pthread_set_name_np(libc::pthread_self(), name.as_ptr());
    }
}

#[cfg(target_vendor = "apple")]
pub fn set_name(name: &CStr) {
    unsafe {
        let name = truncate_cstr::<{ libc::MAXTHREADNAMESIZE }>(name);
        let res = libc::pthread_setname_np(name.as_ptr());
        // 我们这里无法很好地传播错误，但在 debug 构建中，让我们检查一下它确实生效了。
        debug_assert_eq!(res, 0);
    }
}

#[cfg(target_os = "netbsd")]
pub fn set_name(name: &CStr) {
    unsafe {
        let res = libc::pthread_setname_np(
            libc::pthread_self(),
            c"%s".as_ptr(),
            name.as_ptr() as *mut libc::c_void,
        );
        debug_assert_eq!(res, 0);
    }
}

#[cfg(any(target_os = "solaris", target_os = "illumos", target_os = "nto"))]
pub fn set_name(name: &CStr) {
    weak!(
        fn pthread_setname_np(thread: libc::pthread_t, name: *const libc::c_char) -> libc::c_int;
    );

    if let Some(f) = pthread_setname_np.get() {
        #[cfg(target_os = "nto")]
        const THREAD_NAME_MAX: usize = libc::_NTO_THREAD_NAME_MAX as usize;
        #[cfg(any(target_os = "solaris", target_os = "illumos"))]
        const THREAD_NAME_MAX: usize = 32;

        let name = truncate_cstr::<{ THREAD_NAME_MAX }>(name);
        let res = unsafe { f(libc::pthread_self(), name.as_ptr()) };
        debug_assert_eq!(res, 0);
    }
}

#[cfg(target_os = "fuchsia")]
pub fn set_name(name: &CStr) {
    use crate::sys::pal::fuchsia::*;
    unsafe {
        zx_object_set_property(
            zx_thread_self(),
            ZX_PROP_NAME,
            name.as_ptr() as *const libc::c_void,
            name.to_bytes().len(),
        );
    }
}

#[cfg(target_os = "haiku")]
pub fn set_name(name: &CStr) {
    unsafe {
        let thread_self = libc::find_thread(ptr::null_mut());
        let res = libc::rename_thread(thread_self, name.as_ptr());
        // 我们这里无法很好地传播错误，但在 debug 构建中，让我们检查一下它确实生效了。
        debug_assert_eq!(res, libc::B_OK);
    }
}

#[cfg(target_os = "vxworks")]
pub fn set_name(name: &CStr) {
    let mut name = truncate_cstr::<{ (libc::VX_TASK_RENAME_LENGTH - 1) as usize }>(name);
    let res = unsafe { libc::taskNameSet(libc::taskIdSelf(), name.as_mut_ptr()) };
    debug_assert_eq!(res, libc::OK);
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub fn sleep(dur: Duration) {
    let mut secs = dur.as_secs();
    let mut nsecs = dur.subsec_nanos() as _;

    // 如果我们被某个信号唤醒，那么返回值会是 -1，并且
    // nanosleep 会用剩余的时间填充 `ts`。
    unsafe {
        while secs > 0 || nsecs > 0 {
            let mut ts = libc::timespec {
                tv_sec: cmp::min(libc::time_t::MAX as u64, secs) as libc::time_t,
                tv_nsec: nsecs,
            };
            secs -= ts.tv_sec as u64;
            let ts_ptr = &raw mut ts;
            if libc::nanosleep(ts_ptr, ts_ptr) == -1 {
                assert_eq!(sys::io::errno(), libc::EINTR);
                secs += ts.tv_sec as u64;
                nsecs = ts.tv_nsec;
            } else {
                nsecs = 0;
            }
        }
    }
}

#[cfg(any(
    target_os = "espidf",
    // WebAssembly/wasi-libc#696 之前的 wasi-libc 对 `nanosleep`（即上面大多数
    // 平台所用的函数）的实现是有问题的，所以在该修复传遍整个生态之前，
    // 这里改用 `usleep`。
    target_os = "wasi",
))]
pub fn sleep(dur: Duration) {
    // ESP-IDF 没有 `nanosleep`，所以我们改用 `usleep`。
    // 根据 `usleep` 的文档，它至少应支持长达 1 秒的睡眠时间。
    //
    // ESP-IDF 实际上支持几乎到 `u32::MAX` 的范围，但由于其 `usleep`
    // 实现中存在潜在的整数溢出
    // (https://github.com/espressif/esp-idf/blob/d7ca8b94c852052e3bc33292287ef4dd62c9eeb1/components/newlib/time.c#L210)，
    // 我们将睡眠时间限制在不会导致底层 `usleep` 实现溢出的最大值
    // （`portTICK_PERIOD_MS` 可以是 1 到 1000 之间的任意值，默认是 10）。
    const MAX_MICROS: u32 = u32::MAX - 1_000_000 - 1;

    // 把任何小于一微秒的纳秒部分作为额外的一微秒加上，
    // 以遵循 `std::thread::sleep` 的契约——它要求实现至少睡眠所提供的 `dur`。
    // 我们不会让 `micros` 溢出，因为它是 `u128`，而 `Duration` 是一对
    //（`u64` 秒，`u32` 纳秒），其中纳秒严格小于 1 秒
    //（即 < 1_000_000_000）。
    let mut micros = dur.as_micros() + if dur.subsec_nanos() % 1_000 > 0 { 1 } else { 0 };

    while micros > 0 {
        let st = if micros > MAX_MICROS as u128 { MAX_MICROS } else { micros as u32 };
        unsafe {
            libc::usleep(st);
        }

        micros -= st as u128;
    }
}

// 任何拥有 clock_nanosleep 的 unix
// 如果此列表发生变化，请更新 MIRI 的 clock_nanosleep shim
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "linux",
    target_os = "android",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "dragonfly",
    target_os = "hurd",
    target_os = "fuchsia",
    target_os = "vxworks",
    target_os = "wasi",
))]
pub fn sleep_until(deadline: crate::time::Instant) {
    use crate::time::Instant;

    let Some(ts) = deadline.into_inner().into_timespec().to_timespec() else {
        // 该截止时间在未来太远，无法传给 clock_nanosleep。
        // 我们只好改用 Self::sleep。这种情况可能发生在 32 位平台上，
        // 尤其是临近 2038 年时。
        let now = Instant::now();
        if let Some(delay) = deadline.checked_duration_since(now) {
            sleep(delay);
        }
        return;
    };

    unsafe {
        // 当我们被中断（res = EINTR）时，再次调用 clock_nanosleep
        loop {
            let res = libc::clock_nanosleep(
                crate::sys::time::Instant::CLOCK_ID,
                libc::TIMER_ABSTIME,
                &ts,
                core::ptr::null_mut(), // 使用 TIMER_ABSTIME 时不需要
            );

            if res == 0 {
                break;
            } else {
                assert_eq!(
                    res,
                    libc::EINTR,
                    "timespec is in range,
                         clockid is valid and kernel should support it"
                );
            }
        }
    }
}

pub fn yield_now() {
    let ret = unsafe { libc::sched_yield() };
    debug_assert_eq!(ret, 0);
}

#[cfg(any(target_os = "android", target_os = "linux"))]
mod cgroups {
    //! 目前未覆盖的情况
    //! * 位于非标准挂载点的 cgroup v2
    //! * 路径中包含控制字符或空格的情形，因为这些字符在 procfs 输出中会被转义，
    //!   而我们并不做反转义

    use crate::borrow::Cow;
    use crate::ffi::OsString;
    use crate::fs::{File, exists};
    use crate::io::{BufRead, Read};
    use crate::os::unix::ffi::OsStringExt;
    use crate::path::{Path, PathBuf};
    use crate::str::from_utf8;

    #[derive(PartialEq)]
    enum Cgroup {
        V1,
        V2,
    }

    /// 以“核心当量”（core-equivalents）形式返回 cgroup 的 CPU 配额，向下取整；
    /// 若配额无法确定或未设置，则返回 usize::MAX。
    pub(super) fn quota() -> usize {
        let mut quota = usize::MAX;
        if cfg!(miri) {
            // 在默认标志下，由于隔离（isolation）的缘故尝试打开文件会失败。
            // 而且 Miri 本来也不具备并行能力。
            return quota;
        }

        let _: Option<()> = try {
            let mut buf = Vec::with_capacity(128);
            // 找到我们在 cgroup 层级中的位置
            File::open("/proc/self/cgroup").ok()?.read_to_end(&mut buf).ok()?;
            let (cgroup_path, version) =
                buf.split(|&c| c == b'\n').fold(None, |previous, line| {
                    let mut fields = line.splitn(3, |&c| c == b':');
                    // 第 2 个字段对 v1 是控制器列表，对 v2 则为空
                    let version = match fields.nth(1) {
                        Some(b"") => Cgroup::V2,
                        Some(controllers)
                            if from_utf8(controllers)
                                .is_ok_and(|c| c.split(',').any(|c| c == "cpu")) =>
                        {
                            Cgroup::V1
                        }
                        _ => return previous,
                    };

                    // 已找到的 v1 优先于 v2，因为它显式指定了自己的控制器
                    if previous.is_some() && version == Cgroup::V2 {
                        return previous;
                    }

                    let path = fields.last()?;
                    // 跳过开头的斜杠
                    Some((path[1..].to_owned(), version))
                })?;
            let cgroup_path = PathBuf::from(OsString::from_vec(cgroup_path));

            quota = match version {
                Cgroup::V1 => quota_v1(cgroup_path),
                Cgroup::V2 => quota_v2(cgroup_path),
            };
        };

        quota
    }

    fn quota_v2(group_path: PathBuf) -> usize {
        let mut quota = usize::MAX;

        let mut path = PathBuf::with_capacity(128);
        let mut read_buf = String::with_capacity(20);

        // file-hierarchy(7) 手册页中定义的标准挂载位置
        let cgroup_mount = "/sys/fs/cgroup";

        path.push(cgroup_mount);
        path.push(&group_path);

        path.push("cgroup.controllers");

        // 如果我们面对的不是 cgroup2 则跳过
        if matches!(exists(&path), Err(_) | Ok(false)) {
            return usize::MAX;
        };

        path.pop();

        let _: Option<()> = try {
            while path.starts_with(cgroup_mount) {
                path.push("cpu.max");

                read_buf.clear();

                if File::open(&path).and_then(|mut f| f.read_to_string(&mut read_buf)).is_ok() {
                    let raw_quota = read_buf.lines().next()?;
                    let mut raw_quota = raw_quota.split(' ');
                    let limit = raw_quota.next()?;
                    let period = raw_quota.next()?;
                    match (limit.parse::<usize>(), period.parse::<usize>()) {
                        (Ok(limit), Ok(period)) if period > 0 => {
                            quota = quota.min(limit / period);
                        }
                        _ => {}
                    }
                }

                path.pop(); // 弹出文件名
                path.pop(); // 弹出目录
            }
        };

        quota
    }

    fn quota_v1(group_path: PathBuf) -> usize {
        let mut quota = usize::MAX;
        let mut path = PathBuf::with_capacity(128);
        let mut read_buf = String::with_capacity(20);

        // 硬编码 cgroups(7) 手册页中提到的常用位置；
        // 如果那不奏效，则扫描 mountinfo 并为绑定挂载（bind-mount）调整 `group_path`
        let mounts: &[fn(&Path) -> Option<(_, &Path)>] = &[
            |p| Some((Cow::Borrowed("/sys/fs/cgroup/cpu"), p)),
            |p| Some((Cow::Borrowed("/sys/fs/cgroup/cpu,cpuacct"), p)),
            // 在挂载点数量庞大的系统上这可能开销很大，
            // 但只有当 /proc/self/cgroups 明确表明本进程属于某个 cpu-controller
            // cgroup v1、且默认位置都不奏效时，我们才会走到这一步
            find_mountpoint,
        ];

        for mount in mounts {
            let Some((mount, group_path)) = mount(&group_path) else { continue };

            path.clear();
            path.push(mount.as_ref());
            path.push(&group_path);

            // 如果我们对挂载点的猜测有误则跳过
            if matches!(exists(&path), Err(_) | Ok(false)) {
                continue;
            }

            while path.starts_with(mount.as_ref()) {
                let mut parse_file = |name| {
                    path.push(name);
                    read_buf.clear();

                    let f = File::open(&path);
                    path.pop(); // 在任何提前返回之前先恢复缓冲区
                    f.ok()?.read_to_string(&mut read_buf).ok()?;
                    let parsed = read_buf.trim().parse::<usize>().ok()?;

                    Some(parsed)
                };

                let limit = parse_file("cpu.cfs_quota_us");
                let period = parse_file("cpu.cfs_period_us");

                match (limit, period) {
                    (Some(limit), Some(period)) if period > 0 => quota = quota.min(limit / period),
                    _ => {}
                }

                path.pop();
            }

            // 走到这一行时，由于我们通过了上面的 try_exists 检查，
            // 因此应当已经遍历了正确的层级
            break;
        }

        quota
    }

    /// 扫描 mountinfo，查找带有 cpu 控制器的 cgroup v1 挂载点
    ///
    /// 如果 cgroupfs 是一个绑定挂载（bind mount），则 `group_path` 会被调整，
    /// 以跳过已经包含在内的前缀
    fn find_mountpoint(group_path: &Path) -> Option<(Cow<'static, str>, &Path)> {
        let mut reader = File::open_buffered("/proc/self/mountinfo").ok()?;
        let mut line = String::with_capacity(256);
        loop {
            line.clear();
            if reader.read_line(&mut line).ok()? == 0 {
                break;
            }

            let line = line.trim();
            let mut items = line.split(' ');

            let sub_path = items.nth(3)?;
            let mount_point = items.next()?;
            let mount_opts = items.next_back()?;
            let filesystem_type = items.nth_back(1)?;

            if filesystem_type != "cgroup" || !mount_opts.split(',').any(|opt| opt == "cpu") {
                // 不是 cgroup / 不是 cpu 控制器
                continue;
            }

            let sub_path = Path::new(sub_path).strip_prefix("/").ok()?;

            if !group_path.starts_with(sub_path) {
                // 这是一个绑定挂载（bind-mount），且被绑定的子目录
                // 并不包含本进程所属的 cgroup
                continue;
            }

            let trimmed_group_path = group_path.strip_prefix(sub_path).ok()?;

            return Some((Cow::Owned(mount_point.to_owned()), trimmed_group_path));
        }

        None
    }
}

// glibc >= 2.15 提供了一个 __pthread_get_minstack() 函数，它返回
// PTHREAD_STACK_MIN 加上线程本地存储（thread-local storage）所需的字节数。
// 我们需要这一信息，以避免在一个有大量线程本地存储需求的应用中
// 创建了较小的栈时发生崩溃。
// 关于其依据和细节，参见 #6233。
#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe fn min_stack_size(attr: *const libc::pthread_attr_t) -> usize {
    // 我们使用 dlsym 以避免对 GLIBC_PRIVATE 产生 ELF 版本依赖。(#23628)
    // 我们本不该使用这样一个内部符号，但目前没有其他办法把 TLS 大小计算进去。
    dlsym!(
        fn __pthread_get_minstack(attr: *const libc::pthread_attr_t) -> libc::size_t;
    );

    match __pthread_get_minstack.get() {
        None => libc::PTHREAD_STACK_MIN,
        Some(f) => unsafe { f(attr) },
    }
}

// 在非 glibc 平台上没有必要去查找 __pthread_get_minstack()。
#[cfg(all(
    not(all(target_os = "linux", target_env = "gnu")),
    not(any(target_os = "netbsd", target_os = "nuttx"))
))]
unsafe fn min_stack_size(_: *const libc::pthread_attr_t) -> usize {
    libc::PTHREAD_STACK_MIN
}

#[cfg(any(target_os = "netbsd", target_os = "nuttx"))]
unsafe fn min_stack_size(_: *const libc::pthread_attr_t) -> usize {
    static STACK: crate::sync::OnceLock<usize> = crate::sync::OnceLock::new();

    *STACK.get_or_init(|| {
        let mut stack = unsafe { libc::sysconf(libc::_SC_THREAD_STACK_MIN) };
        if stack < 0 {
            stack = 2048; // 只是一个猜测值
        }

        stack as usize
    })
}
