pub fn fill_bytes(bytes: &mut [u8]) {
    // 处理零字节请求
    if bytes.is_empty() {
        return;
    }

    // 尝试 EFI_RNG_PROTOCOL
    if rng_protocol::fill_bytes(bytes) {
        return;
    }

    // 如果缺少 rng protocol，则回退到 rdrand。
    //
    // 真实世界中的示例参见 [issue-13825](https://github.com/rust-lang/rust/issues/138252#issuecomment-2891270323)
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    if rdrand::fill_bytes(bytes) {
        return;
    }

    panic!("failed to generate random data");
}

mod rng_protocol {
    use r_efi::protocols::rng;

    use crate::sys::pal::helpers;

    pub(crate) fn fill_bytes(bytes: &mut [u8]) -> bool {
        if let Ok(handles) = helpers::locate_handles(rng::PROTOCOL_GUID) {
            for handle in handles {
                if let Ok(protocol) =
                    helpers::open_protocol::<rng::Protocol>(handle, rng::PROTOCOL_GUID)
                {
                    let r = unsafe {
                        ((*protocol.as_ptr()).get_rng)(
                            protocol.as_ptr(),
                            crate::ptr::null_mut(),
                            bytes.len(),
                            bytes.as_mut_ptr(),
                        )
                    };
                    if r.is_error() {
                        continue;
                    } else {
                        return true;
                    }
                }
            }
        }

        false
    }
}

/// 移植自 [getrandom](https://github.com/rust-random/getrandom/blob/master/src/backends/rdrand.rs)
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
mod rdrand {
    cfg_select! {
        target_arch = "x86_64" => {
            use crate::arch::x86_64 as arch;
            use arch::_rdrand64_step as rdrand_step;
            type Word = u64;
        }
        target_arch = "x86" => {
            use crate::arch::x86 as arch;
            use arch::_rdrand32_step as rdrand_step;
            type Word = u32;
        }
    }

    static RDRAND_GOOD: crate::sync::LazyLock<bool> = crate::sync::LazyLock::new(is_rdrand_good);

    // 推荐值来自《Intel® Digital Random Number Generator (DRNG) Software
    // Implementation Guide》第 5.2.1 节，以及《Intel® 64 and IA-32 Architectures
    // Software Developer’s Manual》第 1 卷第 7.3.17.1 节。
    const RETRY_LIMIT: usize = 10;

    unsafe fn rdrand() -> Option<Word> {
        for _ in 0..RETRY_LIMIT {
            let mut val = 0;
            if unsafe { rdrand_step(&mut val) } == 1 {
                return Some(val);
            }
        }
        None
    }

    // 运行一个小型自检（self-test），确保我们没有重复产生相同的值
    // 改编自 Linux 在 arch/x86/kernel/cpu/rdrand.c 中的测试
    // 在 32 位系统上，失败的概率 < 2^(-90)
    unsafe fn self_test() -> bool {
        // 在 AMD 上，RDRAND 失败时会返回 0xFF...FF，把它当作一次碰撞（collision）计入。
        let mut prev = Word::MAX;
        let mut fails = 0;
        for _ in 0..8 {
            match unsafe { rdrand() } {
                Some(val) if val == prev => fails += 1,
                Some(val) => prev = val,
                None => return false,
            };
        }
        fails <= 2
    }

    fn is_rdrand_good() -> bool {
        #[cfg(not(target_feature = "rdrand"))]
        {
            // SAFETY: 所有 Rust x86 目标平台都足够新、都带有 CPUID，
            // 而且我们在使用 leaf 1 之前会先检查它是否受支持。
            let cpuid0 = arch::__cpuid(0);
            if cpuid0.eax < 1 {
                return false;
            }
            let cpuid1 = arch::__cpuid(1);

            let vendor_id =
                [cpuid0.ebx.to_le_bytes(), cpuid0.edx.to_le_bytes(), cpuid0.ecx.to_le_bytes()];
            if vendor_id == [*b"Auth", *b"enti", *b"cAMD"] {
                let mut family = (cpuid1.eax >> 8) & 0xF;
                if family == 0xF {
                    family += (cpuid1.eax >> 20) & 0xFF;
                }
                // 17h（Zen）之前的 AMD CPU 家族有时会在挂起（suspend）后
                // RDRAND 失败时未能正确设置 CF 标志。不要在这些家族上使用 RDRAND。
                // 参见 https://bugzilla.redhat.com/show_bug.cgi?id=1150286
                if family < 0x17 {
                    return false;
                }
            }

            const RDRAND_FLAG: u32 = 1 << 30;
            if cpuid1.ecx & RDRAND_FLAG == 0 {
                return false;
            }
        }

        // SAFETY: 我们已经检查过 rdrand 可用。
        unsafe { self_test() }
    }

    unsafe fn rdrand_exact(dest: &mut [u8]) -> Option<()> {
        let (chunks, tail) = dest.as_chunks_mut();
        for chunk in chunks {
            *chunk = unsafe { rdrand() }?.to_ne_bytes();
        }

        let n = tail.len();
        if n > 0 {
            let src = unsafe { rdrand() }?.to_ne_bytes();
            tail.copy_from_slice(&src[..n]);
        }
        Some(())
    }

    pub(crate) fn fill_bytes(bytes: &mut [u8]) -> bool {
        if *RDRAND_GOOD { unsafe { rdrand_exact(bytes).is_some() } } else { false }
    }
}
