pub use super::common::Args;

/// 返回命令行参数
pub fn args() -> Args {
    Args::new(wasip2::cli::environment::get_arguments().into_iter().map(|arg| arg.into()).collect())
}
