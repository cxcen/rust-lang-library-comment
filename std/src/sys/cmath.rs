#![cfg(not(test))]

// 这些符号全部由 `libm` 定义，
// 在不受支持的平台上则由 `compiler-builtins` 定义。
unsafe extern "C" {
    pub safe fn acos(n: f64) -> f64;
    pub safe fn asin(n: f64) -> f64;
    pub safe fn atan(n: f64) -> f64;
    pub safe fn atan2(a: f64, b: f64) -> f64;
    pub safe fn cosh(n: f64) -> f64;
    pub safe fn expm1(n: f64) -> f64;
    pub safe fn expm1f(n: f32) -> f32;
    #[cfg_attr(target_env = "msvc", link_name = "_hypot")]
    pub safe fn hypot(x: f64, y: f64) -> f64;
    #[cfg_attr(target_env = "msvc", link_name = "_hypotf")]
    pub safe fn hypotf(x: f32, y: f32) -> f32;
    pub safe fn log1p(n: f64) -> f64;
    pub safe fn log1pf(n: f32) -> f32;
    pub safe fn sinh(n: f64) -> f64;
    pub safe fn tan(n: f64) -> f64;
    pub safe fn tanh(n: f64) -> f64;
    pub safe fn tgamma(n: f64) -> f64;
    pub safe fn tgammaf(n: f32) -> f32;
    pub safe fn lgamma_r(n: f64, s: &mut i32) -> f64;
    #[cfg(not(target_os = "aix"))]
    pub safe fn lgammaf_r(n: f32, s: &mut i32) -> f32;
    pub safe fn erf(n: f64) -> f64;
    pub safe fn erff(n: f32) -> f32;
    pub safe fn erfc(n: f64) -> f64;
    pub safe fn erfcf(n: f32) -> f32;

    pub safe fn acosf128(n: f128) -> f128;
    pub safe fn asinf128(n: f128) -> f128;
    pub safe fn atanf128(n: f128) -> f128;
    pub safe fn atan2f128(a: f128, b: f128) -> f128;
    pub safe fn cbrtf128(n: f128) -> f128;
    pub safe fn coshf128(n: f128) -> f128;
    pub safe fn expm1f128(n: f128) -> f128;
    pub safe fn hypotf128(x: f128, y: f128) -> f128;
    pub safe fn log1pf128(n: f128) -> f128;
    pub safe fn sinhf128(n: f128) -> f128;
    pub safe fn tanf128(n: f128) -> f128;
    pub safe fn tanhf128(n: f128) -> f128;
    pub safe fn tgammaf128(n: f128) -> f128;
    pub safe fn lgammaf128_r(n: f128, s: &mut i32) -> f128;
    pub safe fn erff128(n: f128) -> f128;
    pub safe fn erfcf128(n: f128) -> f128;
}

cfg_select! {
    all(target_os = "windows", target_env = "msvc", target_arch = "x86") => {
        // 在 32 位 x86 MSVC 上这些函数没有定义，因此我们直接定义一组桩函数（shim），
        // 把所有运算提升到 f64 进行计算，再降回 f32。虽然不是精确正确，
        // 但目前“够用”。
        #[inline]
        pub fn acosf(n: f32) -> f32 {
            f64::acos(n as f64) as f32
        }

        #[inline]
        pub fn asinf(n: f32) -> f32 {
            f64::asin(n as f64) as f32
        }

        #[inline]
        pub fn atan2f(n: f32, b: f32) -> f32 {
            f64::atan2(n as f64, b as f64) as f32
        }

        #[inline]
        pub fn atanf(n: f32) -> f32 {
            f64::atan(n as f64) as f32
        }

        #[inline]
        pub fn coshf(n: f32) -> f32 {
            f64::cosh(n as f64) as f32
        }

        #[inline]
        pub fn sinhf(n: f32) -> f32 {
            f64::sinh(n as f64) as f32
        }

        #[inline]
        pub fn tanf(n: f32) -> f32 {
            f64::tan(n as f64) as f32
        }

        #[inline]
        pub fn tanhf(n: f32) -> f32 {
            f64::tanh(n as f64) as f32
        }
    }
    _ => {
        unsafe extern "C" {
            pub safe fn acosf(n: f32) -> f32;
            pub safe fn asinf(n: f32) -> f32;
            pub safe fn atan2f(a: f32, b: f32) -> f32;
            pub safe fn atanf(n: f32) -> f32;
            pub safe fn coshf(n: f32) -> f32;
            pub safe fn sinhf(n: f32) -> f32;
            pub safe fn tanf(n: f32) -> f32;
            pub safe fn tanhf(n: f32) -> f32;
        }
    }
}

// 在 AIX 上没有 lgammaf_r，只有 f64 版本，因此我们
// 可以借用 f64 版本的 lgamma_r 来实现。
#[cfg(target_os = "aix")]
pub fn lgammaf_r(n: f32, s: &mut i32) -> f32 {
    lgamma_r(n.into(), s) as f32
}
