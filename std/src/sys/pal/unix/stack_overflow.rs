#![cfg_attr(test, allow(dead_code))]

pub use self::imp::{cleanup, init};
use self::imp::{drop_handler, make_handler};

pub struct Handler {
    data: *mut libc::c_void,
}

impl Handler {
    pub unsafe fn new() -> Handler {
        make_handler(false)
    }

    fn null() -> Handler {
        Handler { data: crate::ptr::null_mut() }
    }
}

impl Drop for Handler {
    fn drop(&mut self) {
        unsafe {
            drop_handler(self.data);
        }
    }
}

#[cfg(all(
    not(miri),
    any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "hurd",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "illumos",
    ),
))]
mod thread_info;

// miri 既不对信号、也不对栈溢出建模，而且这段代码具有一些我们不想暴露给用户代码的
// 同步特性，因此我们在 miri 上禁用它。
#[cfg(all(
    not(miri),
    any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "hurd",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "illumos",
    )
))]
mod imp {
    use libc::{
        MAP_ANON, MAP_FAILED, MAP_FIXED, MAP_PRIVATE, PROT_NONE, PROT_READ, PROT_WRITE, SA_ONSTACK,
        SA_SIGINFO, SIG_DFL, SIGBUS, SIGSEGV, SS_DISABLE, sigaction, sigaltstack, sighandler_t,
    };
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    use libc::{mmap as mmap64, mprotect, munmap};
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    use libc::{mmap64, mprotect, munmap};

    use super::Handler;
    use super::thread_info::{delete_current_info, set_current_info, with_current_info};
    use crate::ops::Range;
    use crate::sync::atomic::{Atomic, AtomicBool, AtomicPtr, AtomicUsize, Ordering};
    use crate::sys::pal::unix::os;
    use crate::{io, mem, ptr};

    // 用于 SIGSEGV 和 SIGBUS 处理程序的信号处理函数。我们在每个线程栈的末端都放置
    // 了 guard page（守护页，即未映射的页面），因此一旦某个线程触碰到 guard page，
    // 就会触发本处理函数。我们希望检测出这些情况，并打印出一条有帮助的错误信息，
    // 说明栈已经溢出。不过，所有其他信号都应当回到它们原本应该执行的行为。
    //
    // 本处理函数目前的存在纯粹是为了在某个线程栈溢出时打印一条提示信息。随后我们
    // 通过 abort 来退出并表明发生了崩溃；但这样做也是为了避免一个具误导性的
    // SIGSEGV——它可能让用户误以为是某段 unsafe 代码访问了无效指针；而栈溢出时遇到
    // 的 SIGSEGV 是预期之中、且行为良定义的。
    //
    // 如果这并非栈溢出，本处理函数会注销自身、然后返回（以便让原本的信号能再次被
    // 投递）。严格按照 POSIX 规范来说，从这类信号处理函数中返回在技术上并非有定义
    // 的行为，但实践证明许多大型系统、以及所有的实现都允许从信号处理函数返回并
    // 正常工作。更详细的解释参见 #26458 上的注释。
    /// SIGSEGV/SIGBUS 的入口点
    /// # 安全性(Safety）
    /// Rust 不会调用它，而是它*被（系统）调用*。
    #[forbid(unsafe_op_in_unsafe_fn)]
    unsafe extern "C" fn signal_handler(
        signum: libc::c_int,
        info: *mut libc::siginfo_t,
        _data: *mut libc::c_void,
    ) {
        // SAFETY: 该指针由系统提供，并将始终指向一个有效的 `siginfo_t`。
        let fault_addr = unsafe { (*info).si_addr().addr() };

        // `with_current_info` 期望在它被调用之后进程会 abort。如果该信号并非由内存
        // 访问引起，那么这一点可能并不成立。我们通过注意到“当信号是合成的
        // （synthetic）时 `si_addr` 字段为零”来检测这种情况。
        if fault_addr != 0 {
            with_current_info(|thread_info| {
                // 如果出错的地址落在 guard page 范围内，那么我们就打印一条提示信息
                // 并 abort。
                if let Some(thread_info) = thread_info
                    && thread_info.guard_page_range.contains(&fault_addr)
                {
                    // 嘿，说你呢！没错，就是正在修改这条栈溢出信息的你！
                    // 请务必确保这里调用的所有函数都确实是 async-signal-safe（异步
                    // 信号安全）的。如果它们不是，请尝试事先把所需信息取出来、并
                    // 存进 `ThreadInfo`。
                    // 谢谢你！
                    // —— Jonas 在不得不眼看着自己精心编写的代码再次变得不健全
                    //    （unsound）之后如是说。
                    let tid = thread_info.tid;
                    let name = thread_info.name.as_deref().unwrap_or("<unknown>");
                    rtprintpanic!("\nthread '{name}' ({tid}) has overflowed its stack\n");
                    rtabort!("stack overflow");
                }
            })
        }

        // 通过恢复为默认行为来注销我们自己。
        // SAFETY: 假定所有平台都把 struct sigaction 定义为“可零初始化”的
        let mut action: sigaction = unsafe { mem::zeroed() };
        action.sa_sigaction = SIG_DFL;
        // SAFETY: 但愿这是一个行为良好的、对 fn sigaction 的 POSIX 实现
        unsafe { sigaction(signum, &action, ptr::null_mut()) };

        // 关于本函数为何会返回，参见上面的注释。
    }

    static PAGE_SIZE: Atomic<usize> = AtomicUsize::new(0);
    static MAIN_ALTSTACK: Atomic<*mut libc::c_void> = AtomicPtr::new(ptr::null_mut());
    static NEED_ALTSTACK: Atomic<bool> = AtomicBool::new(false);

    /// # 安全性(Safety）
    /// 只能被调用一次
    #[forbid(unsafe_op_in_unsafe_fn)]
    pub unsafe fn init() {
        PAGE_SIZE.store(os::page_size(), Ordering::Relaxed);

        let mut guard_page_range = unsafe { install_main_guard() };

        // 即便对于 panic=immediate-abort，安装 guard page 对于健全性
        // （soundness）来说也很重要。话虽如此，我们并不在意是否要通过自定义信号
        // 处理函数给出漂亮的栈溢出信息，直接提前退出、让用户尽情享受这次 segfault
        // 即可。
        if cfg!(panic = "immediate-abort") {
            return;
        }

        // SAFETY: 假定所有平台都把 struct sigaction 定义为“可零初始化”的
        let mut action: sigaction = unsafe { mem::zeroed() };
        for &signal in &[SIGSEGV, SIGBUS] {
            // SAFETY: 仅仅是把当前的信号处理函数取出到 action 中
            unsafe { sigaction(signal, ptr::null_mut(), &mut action) };
            // 如果尚未设置信号处理函数，就配置我们自己的。
            if action.sa_sigaction == SIG_DFL {
                if !NEED_ALTSTACK.load(Ordering::Relaxed) {
                    // 我们还没有设置好自己的 sigaltstack
                    NEED_ALTSTACK.store(true, Ordering::Release);
                    let handler = unsafe { make_handler(true) };
                    MAIN_ALTSTACK.store(handler.data, Ordering::Relaxed);
                    mem::forget(handler);

                    if let Some(guard_page_range) = guard_page_range.take() {
                        set_current_info(guard_page_range);
                    }
                }

                action.sa_flags = SA_SIGINFO | SA_ONSTACK;
                action.sa_sigaction = signal_handler
                    as unsafe extern "C" fn(i32, *mut libc::siginfo_t, *mut libc::c_void)
                    as sighandler_t;
                // SAFETY: 仅在默认处理函数已设置的情况下才覆盖信号
                unsafe { sigaction(signal, &action, ptr::null_mut()) };
            }
        }
    }

    /// # 安全性(Safety）
    /// 只能被调用一次
    #[forbid(unsafe_op_in_unsafe_fn)]
    pub unsafe fn cleanup() {
        if cfg!(panic = "immediate-abort") {
            return;
        }
        // FIXME: 我引入的 bug 大概比我自身的价值还多！
        // 参见 https://github.com/rust-lang/rust/issues/111272
        unsafe { drop_handler(MAIN_ALTSTACK.load(Ordering::Relaxed)) };
    }

    unsafe fn get_stack() -> libc::stack_t {
        // OpenBSD 进行栈映射时需要这个 flag，
        // 否则在大多数系统上该映射会作为空操作（no-op）失败，
        // 而在 FreeBSD 上它则有着不同的含义
        #[cfg(any(
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "linux",
            target_os = "dragonfly",
        ))]
        let flags = MAP_PRIVATE | MAP_ANON | libc::MAP_STACK;
        #[cfg(not(any(
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "linux",
            target_os = "dragonfly",
        )))]
        let flags = MAP_PRIVATE | MAP_ANON;

        let sigstack_size = sigstack_size();
        let page_size = PAGE_SIZE.load(Ordering::Relaxed);

        let stackp = mmap64(
            ptr::null_mut(),
            sigstack_size + page_size,
            PROT_READ | PROT_WRITE,
            flags,
            -1,
            0,
        );
        if stackp == MAP_FAILED {
            panic!("failed to allocate an alternative stack: {}", io::Error::last_os_error());
        }
        let guard_result = libc::mprotect(stackp, page_size, PROT_NONE);
        if guard_result != 0 {
            panic!("failed to set up alternative stack guard page: {}", io::Error::last_os_error());
        }
        let stackp = stackp.add(page_size);

        libc::stack_t { ss_sp: stackp, ss_flags: 0, ss_size: sigstack_size }
    }

    /// # 安全性(Safety）
    /// 会改动备用信号栈（alternate signal stack）
    #[forbid(unsafe_op_in_unsafe_fn)]
    pub unsafe fn make_handler(main_thread: bool) -> Handler {
        if cfg!(panic = "immediate-abort") || !NEED_ALTSTACK.load(Ordering::Acquire) {
            return Handler::null();
        }

        if !main_thread {
            if let Some(guard_page_range) = unsafe { current_guard() } {
                set_current_info(guard_page_range);
            }
        }

        // SAFETY: 假定 stack_t 是可零初始化的
        let mut stack = unsafe { mem::zeroed() };
        // SAFETY: 把当前的 stack_t 读入 stack
        unsafe { sigaltstack(ptr::null(), &mut stack) };
        // 如果尚未设置备用信号栈，就配置一个。
        if stack.ss_flags & SS_DISABLE != 0 {
            // SAFETY: 我们已经警告过调用方这一点会发生！
            unsafe {
                stack = get_stack();
                sigaltstack(&stack, ptr::null_mut());
            }
            Handler { data: stack.ss_sp as *mut libc::c_void }
        } else {
            Handler::null()
        }
    }

    /// # 安全性(Safety）
    /// 必须满足
    /// - 仅以我们自己的 handler 或 nullptr 来调用
    /// - 仅在用完我们自己的 altstack 之后才调用
    /// 该函数会禁用备用信号栈（alternate signal stack）！
    #[forbid(unsafe_op_in_unsafe_fn)]
    pub unsafe fn drop_handler(data: *mut libc::c_void) {
        if !data.is_null() {
            let sigstack_size = sigstack_size();
            let page_size = PAGE_SIZE.load(Ordering::Relaxed);
            let disabling_stack = libc::stack_t {
                ss_sp: ptr::null_mut(),
                ss_flags: SS_DISABLE,
                // 针对 macOS 的 sigaltstack 实现（UNIX2003）中一个 bug 的变通做
                // 法：当在禁用某个栈时传入小于 MINSIGSTKSZ 的 ss_size，它会返回
                // ENOMEM。按照 POSIX，在这种情况下 ss_sp 和 ss_size 都应当被忽略。
                ss_size: sigstack_size,
            };
            // SAFETY: 我们已经警告过调用方，这会禁用备用信号栈！
            unsafe { sigaltstack(&disabling_stack, ptr::null_mut()) };
            // SAFETY: 根据 `get_stackp`，我们安装的备用栈是某个映射的一部分，而该
            // 映射起始于（其）前一页，因此往回退一页、并从那里开始 unmap。
            unsafe { munmap(data.sub(page_size), sigstack_size + page_size) };
        }

        delete_current_info();
    }

    /// 在现代硬件上的现代内核可以拥有动态大小的信号栈。
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn sigstack_size() -> usize {
        let dynamic_sigstksz = unsafe { libc::getauxval(libc::AT_MINSIGSTKSZ) };
        // 如果 getauxval 找不到该条目，它会返回 0，
        // 所以取“常量值”与 auxval 二者中较大的那个。
        // 这透明地支持了那些不提供 AT_MINSIGSTKSZ 的较旧内核
        libc::SIGSTKSZ.max(dynamic_sigstksz as _)
    }

    /// 并非所有操作系统都支持那种需要用到这一点的硬件。
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    fn sigstack_size() -> usize {
        libc::SIGSTKSZ
    }

    #[cfg(any(target_os = "solaris", target_os = "illumos"))]
    unsafe fn get_stack_start() -> Option<*mut libc::c_void> {
        let mut current_stack: libc::stack_t = crate::mem::zeroed();
        assert_eq!(libc::stack_getbounds(&mut current_stack), 0);
        Some(current_stack.ss_sp)
    }

    #[cfg(target_os = "macos")]
    unsafe fn get_stack_start() -> Option<*mut libc::c_void> {
        let th = libc::pthread_self();
        let stackptr = libc::pthread_get_stackaddr_np(th);
        Some(stackptr.map_addr(|addr| addr - libc::pthread_get_stacksize_np(th)))
    }

    #[cfg(target_os = "openbsd")]
    unsafe fn get_stack_start() -> Option<*mut libc::c_void> {
        let mut current_stack: libc::stack_t = crate::mem::zeroed();
        assert_eq!(libc::pthread_stackseg_np(libc::pthread_self(), &mut current_stack), 0);

        let stack_ptr = current_stack.ss_sp;
        let stackaddr = if libc::pthread_main_np() == 1 {
            // 主线程
            stack_ptr.addr() - current_stack.ss_size + PAGE_SIZE.load(Ordering::Relaxed)
        } else {
            // 新线程
            stack_ptr.addr() - current_stack.ss_size
        };
        Some(stack_ptr.with_addr(stackaddr))
    }

    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "hurd",
        target_os = "linux",
        target_os = "l4re"
    ))]
    unsafe fn get_stack_start() -> Option<*mut libc::c_void> {
        let mut ret = None;
        let mut attr: mem::MaybeUninit<libc::pthread_attr_t> = mem::MaybeUninit::uninit();
        if !cfg!(target_os = "freebsd") {
            attr = mem::MaybeUninit::zeroed();
        }
        #[cfg(target_os = "freebsd")]
        assert_eq!(libc::pthread_attr_init(attr.as_mut_ptr()), 0);
        #[cfg(target_os = "freebsd")]
        let e = libc::pthread_attr_get_np(libc::pthread_self(), attr.as_mut_ptr());
        #[cfg(not(target_os = "freebsd"))]
        let e = libc::pthread_getattr_np(libc::pthread_self(), attr.as_mut_ptr());
        if e == 0 {
            let mut stackaddr = crate::ptr::null_mut();
            let mut stacksize = 0;
            assert_eq!(
                libc::pthread_attr_getstack(attr.as_ptr(), &mut stackaddr, &mut stacksize),
                0
            );
            ret = Some(stackaddr);
        }
        if e == 0 || cfg!(target_os = "freebsd") {
            assert_eq!(libc::pthread_attr_destroy(attr.as_mut_ptr()), 0);
        }
        ret
    }

    fn stack_start_aligned(page_size: usize) -> Option<*mut libc::c_void> {
        let stackptr = unsafe { get_stack_start()? };
        let stackaddr = stackptr.addr();

        // 确保 stackaddr 是按页对齐的！父进程可能把 RLIMIT_STACK 重设成了非按页
        // 对齐的值。pthread_attr_getstack() 报告的可用栈区间是
        // stackaddr < stackaddr + stacksize，因此若 stackaddr 未按页对齐，就计算
        // 出修正量，使得 stackaddr < new_page_aligned_stackaddr < stackaddr + stacksize
        let remainder = stackaddr % page_size;
        Some(if remainder == 0 {
            stackptr
        } else {
            stackptr.with_addr(stackaddr + page_size - remainder)
        })
    }

    #[forbid(unsafe_op_in_unsafe_fn)]
    unsafe fn install_main_guard() -> Option<Range<usize>> {
        let page_size = PAGE_SIZE.load(Ordering::Relaxed);

        unsafe {
            // 这样一来，任何 unix 类操作系统上的人都能检查这些代码是否都能编译
            if cfg!(all(target_os = "linux", not(target_env = "musl"))) {
                install_main_guard_linux(page_size)
            } else if cfg!(all(target_os = "linux", target_env = "musl")) {
                install_main_guard_linux_musl(page_size)
            } else if cfg!(target_os = "freebsd") {
                #[cfg(not(target_os = "freebsd"))]
                return None;
                // FreeBSD 代码无法在非 BSD 系统上做检查。
                #[cfg(target_os = "freebsd")]
                install_main_guard_freebsd(page_size)
            } else if cfg!(any(target_os = "netbsd", target_os = "openbsd")) {
                install_main_guard_bsds(page_size)
            } else {
                install_main_guard_default(page_size)
            }
        }
    }

    #[forbid(unsafe_op_in_unsafe_fn)]
    unsafe fn install_main_guard_linux(page_size: usize) -> Option<Range<usize>> {
        // Linux 不会一上来就分配整个栈，而且内核有它自己的 stack-guard 机制：当栈
        // 增长到过于接近某个已有映射时便触发缺页错误（fault）。如果我们映射自己的
        // guard，那么内核就会在其上方开始强制留出一段相当大的间隙（gap），从而使
        // 大量本可使用的栈空间变得无用。参见 #43052。
        //
        // 因此，我们只是记录下我们预期 rlimit 会从哪里开始触发缺页错误，以便我们的
        // 处理函数能够报告“stack overflow”，并信赖内核自己的 stack guard 会正常
        // 工作。
        let stackptr = stack_start_aligned(page_size)?;
        let stackaddr = stackptr.addr();
        Some(stackaddr - page_size..stackaddr)
    }

    #[forbid(unsafe_op_in_unsafe_fn)]
    unsafe fn install_main_guard_linux_musl(_page_size: usize) -> Option<Range<usize>> {
        // 对于主线程，musl 的 pthread_attr_getstack 返回的是当前栈大小，而非它
        // 最终能增长到的最大大小。因此它无法用来确定内核 stack guard 的位置。
        None
    }

    #[forbid(unsafe_op_in_unsafe_fn)]
    #[cfg(target_os = "freebsd")]
    unsafe fn install_main_guard_freebsd(page_size: usize) -> Option<Range<usize>> {
        // FreeBSD 的栈会自动增长，并且可选地在底部包含一个 guard page。如果我们
        // 试图自己重新映射栈的底部，FreeBSD 的 guard page 就会往上移动。所以我们
        // 干脆直接使用其内建的 guard page。
        let stackptr = stack_start_aligned(page_size)?;
        let guardaddr = stackptr.addr();
        // 严格来说，guard page 的数量是可调的，由 security.bsd.stack_guard_page
        // 这个 sysctl 控制。
        // 它默认是 1；由于这是一个启动时（boot time）的配置值，检查一次就够了。
        static PAGES: crate::sync::OnceLock<usize> = crate::sync::OnceLock::new();

        let pages = PAGES.get_or_init(|| {
            let mut guard: usize = 0;
            let mut size = size_of_val(&guard);
            let oid = c"security.bsd.stack_guard_page";

            let r = unsafe {
                libc::sysctlbyname(
                    oid.as_ptr(),
                    (&raw mut guard).cast(),
                    &raw mut size,
                    ptr::null_mut(),
                    0,
                )
            };
            if r == 0 { guard } else { 1 }
        });
        Some(guardaddr..guardaddr + pages * page_size)
    }

    #[forbid(unsafe_op_in_unsafe_fn)]
    unsafe fn install_main_guard_bsds(page_size: usize) -> Option<Range<usize>> {
        // OpenBSD 的栈已经包含一个 guard page，且栈是不可变的。
        // NetBSD 的栈也包含 guard page。
        //
        // 我们只是记录下我们预期 rlimit 会从哪里开始触发缺页错误，以便我们的处理
        // 函数能够报告“stack overflow”，并信赖内核自己的 stack guard 会正常工作。
        let stackptr = stack_start_aligned(page_size)?;
        let stackaddr = stackptr.addr();
        Some(stackaddr - page_size..stackaddr)
    }

    #[forbid(unsafe_op_in_unsafe_fn)]
    unsafe fn install_main_guard_default(page_size: usize) -> Option<Range<usize>> {
        // 重新分配栈的最后一页。
        // 这确保了栈溢出时会触发 SIGBUS。
        // 强制执行严格 PAX MPROTECT 的系统不允许用比最初 mmap() 时更宽松的权限去
        // mprotect() 一个映射，所以我们在这里用读/写权限来 mmap()，然后才把它
        // mprotect() 成完全没有任何权限。参见 issue #50313。
        let stackptr = stack_start_aligned(page_size)?;
        let result = unsafe {
            mmap64(
                stackptr,
                page_size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANON | MAP_FIXED,
                -1,
                0,
            )
        };
        if result != stackptr || result == MAP_FAILED {
            panic!("failed to allocate a guard page: {}", io::Error::last_os_error());
        }

        let result = unsafe { mprotect(stackptr, page_size, PROT_NONE) };
        if result != 0 {
            panic!("failed to protect the guard page: {}", io::Error::last_os_error());
        }

        let guardaddr = stackptr.addr();

        Some(guardaddr..guardaddr + page_size)
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "illumos",
    ))]
    // FIXME: 我大概并不是 unsafe 的。
    unsafe fn current_guard() -> Option<Range<usize>> {
        let stackptr = get_stack_start()?;
        let stackaddr = stackptr.addr();
        Some(stackaddr - PAGE_SIZE.load(Ordering::Relaxed)..stackaddr)
    }

    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "l4re"
    ))]
    // FIXME: 我大概并不是 unsafe 的。
    unsafe fn current_guard() -> Option<Range<usize>> {
        let mut ret = None;

        let mut attr: mem::MaybeUninit<libc::pthread_attr_t> = mem::MaybeUninit::uninit();
        if !cfg!(target_os = "freebsd") {
            attr = mem::MaybeUninit::zeroed();
        }
        #[cfg(target_os = "freebsd")]
        assert_eq!(libc::pthread_attr_init(attr.as_mut_ptr()), 0);
        #[cfg(target_os = "freebsd")]
        let e = libc::pthread_attr_get_np(libc::pthread_self(), attr.as_mut_ptr());
        #[cfg(not(target_os = "freebsd"))]
        let e = libc::pthread_getattr_np(libc::pthread_self(), attr.as_mut_ptr());
        if e == 0 {
            let mut guardsize = 0;
            assert_eq!(libc::pthread_attr_getguardsize(attr.as_ptr(), &mut guardsize), 0);
            if guardsize == 0 {
                if cfg!(all(target_os = "linux", target_env = "musl")) {
                    // 1.1.19 之前的 musl 版本，从 pthread_attr_get_np 取得的 guard
                    // size 总是报告为零。这里退而使用页大小作为兜底。
                    guardsize = PAGE_SIZE.load(Ordering::Relaxed);
                } else {
                    panic!("there is no guard page");
                }
            }
            let mut stackptr = crate::ptr::null_mut::<libc::c_void>();
            let mut size = 0;
            assert_eq!(libc::pthread_attr_getstack(attr.as_ptr(), &mut stackptr, &mut size), 0);

            let stackaddr = stackptr.addr();
            ret = if cfg!(any(target_os = "freebsd", target_os = "netbsd", target_os = "hurd")) {
                Some(stackaddr - guardsize..stackaddr)
            } else if cfg!(all(target_os = "linux", target_env = "musl")) {
                Some(stackaddr - guardsize..stackaddr)
            } else if cfg!(all(target_os = "linux", any(target_env = "gnu", target_env = "uclibc")))
            {
                // glibc 过去会把 guard 区域算在栈之内，正如 `man
                // pthread_attr_getguardsize` 的 BUGS 一节所指出的那样。从 glibc
                // 2.27 起、以及在某些发行版的反向移植（backport）中，这一点已被
                // 修正，因此现在 guard 被放在栈的末端（即下方）。我们在运行时没有
                // 简单的办法去判断自己面对的是哪一种情况，所以我们干脆把栈基址正
                // 上方或正下方范围内的任何缺页错误，都当作栈溢出来处理。
                Some(stackaddr - guardsize..stackaddr + guardsize)
            } else {
                Some(stackaddr..stackaddr + guardsize)
            };
        }
        if e == 0 || cfg!(target_os = "freebsd") {
            assert_eq!(libc::pthread_attr_destroy(attr.as_mut_ptr()), 0);
        }
        ret
    }
}

// 这里特意不在 iOS/tvOS/watchOS/visionOS 上启用，因为它用到了若干符号，可能导致被
// App Store 拒绝，即 `sigaction`、`sigaltstack`、`sysctlbyname`、`mmap`、`munmap`
// 和 `mprotect`。
//
// 这也许过于谨慎了，不过 Swift 也是这么做的（而他们通常对前向兼容性的顾虑更少，因为
// 其运行时是随操作系统一起发布的）：
// <https://github.com/apple/swift/blob/swift-5.10-RELEASE/stdlib/public/runtime/CrashHandlerMacOS.cpp>
#[cfg(any(
    miri,
    not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "hurd",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "cygwin",
    ))
))]
mod imp {
    pub unsafe fn init() {}

    pub unsafe fn cleanup() {}

    pub unsafe fn make_handler(_main_thread: bool) -> super::Handler {
        super::Handler::null()
    }

    pub unsafe fn drop_handler(_data: *mut libc::c_void) {}
}

#[cfg(target_os = "cygwin")]
mod imp {
    mod c {
        pub type PVECTORED_EXCEPTION_HANDLER =
            Option<unsafe extern "system" fn(exceptioninfo: *mut EXCEPTION_POINTERS) -> i32>;
        pub type NTSTATUS = i32;
        pub type BOOL = i32;

        unsafe extern "system" {
            pub fn AddVectoredExceptionHandler(
                first: u32,
                handler: PVECTORED_EXCEPTION_HANDLER,
            ) -> *mut core::ffi::c_void;
            pub fn SetThreadStackGuarantee(stacksizeinbytes: *mut u32) -> BOOL;
        }

        pub const EXCEPTION_STACK_OVERFLOW: NTSTATUS = 0xC00000FD_u32 as _;
        pub const EXCEPTION_CONTINUE_SEARCH: i32 = 1i32;

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct EXCEPTION_POINTERS {
            pub ExceptionRecord: *mut EXCEPTION_RECORD,
            // 这里我们不需要这个字段
            // pub Context: *mut CONTEXT,
        }
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct EXCEPTION_RECORD {
            pub ExceptionCode: NTSTATUS,
            pub ExceptionFlags: u32,
            pub ExceptionRecord: *mut EXCEPTION_RECORD,
            pub ExceptionAddress: *mut core::ffi::c_void,
            pub NumberParameters: u32,
            pub ExceptionInformation: [usize; 15],
        }
    }

    /// 预留一些栈空间，以供栈溢出异常时使用。
    fn reserve_stack() {
        let result = unsafe { c::SetThreadStackGuarantee(&mut 0x5000) };
        // 预留栈空间并非关键操作，所以我们允许它在 libstd 的发布版构建中失败。
        // 这里我们仍然使用 debug assert，以便 CI 会测试我们在调用该函数时没有出错。
        debug_assert_ne!(result, 0, "failed to reserve stack space for exception handling");
    }

    unsafe extern "system" fn vectored_handler(ExceptionInfo: *mut c::EXCEPTION_POINTERS) -> i32 {
        // SAFETY: 由调用方（在此情形下即操作系统）负责确保 `ExceptionInfo` 是有效的。
        unsafe {
            let rec = &(*(*ExceptionInfo).ExceptionRecord);
            let code = rec.ExceptionCode;

            if code == c::EXCEPTION_STACK_OVERFLOW {
                crate::thread::with_current_name(|name| {
                    let name = name.unwrap_or("<unknown>");
                    let tid = crate::thread::current_os_id();
                    rtprintpanic!("\nthread '{name}' ({tid}) has overflowed its stack\n");
                });
            }
            c::EXCEPTION_CONTINUE_SEARCH
        }
    }

    pub unsafe fn init() {
        // SAFETY: `vectored_handler` 具有正确的 ABI，且在异常处理期间调用它是安全的。
        unsafe {
            let result = c::AddVectoredExceptionHandler(0, Some(vectored_handler));
            // 与上面类似，添加栈溢出处理函数也允许失败，
            // 但这里用 debug assert，以便 CI 仍会测试它在通常情况下能正常工作。
            debug_assert!(!result.is_null(), "failed to install exception handler");
        }
        // 为主线程设置线程栈保证（thread stack guarantee）。
        reserve_stack();
    }

    pub unsafe fn cleanup() {}

    pub unsafe fn make_handler(main_thread: bool) -> super::Handler {
        if !main_thread {
            reserve_stack();
        }
        super::Handler::null()
    }

    pub unsafe fn drop_handler(_data: *mut libc::c_void) {}
}
