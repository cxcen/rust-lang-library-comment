use super::weak;

pub(crate) macro syscall {
    (
        fn $name:ident($($param:ident : $t:ty),* $(,)?) -> $ret:ty;
    ) => (
        unsafe fn $name($($param: $t),*) -> $ret {
            weak!(fn $name($($param: $t),*) -> $ret;);

            // 尽可能使用 libc 中的弱符号，从而允许 `LD_PRELOAD`
            // 拦截（interposition）；若该符号未找到，则退回到直接发起裸系统调用。
            if let Some(fun) = $name.get() {
                unsafe { fun($($param),*) }
            } else {
                unsafe { libc::syscall(libc::${concat(SYS_, $name)}, $($param),*) as $ret }
            }
        }
    )
}
