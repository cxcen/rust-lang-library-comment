use super::thread::Thread;
use crate::cell::Cell;
use crate::iter;
use crate::sync::Arc;

crate::thread_local! {
    /// 一个由 spawn hook 构成的线程局部链表。
    ///
    /// 它是一个由 Arc 构成的链表，因此可以极其廉价地被派生线程继承。
    ///
    ///（严格来说这使它成为一组共享尾部的链表，所以是一棵链树。）
    static SPAWN_HOOKS: Cell<SpawnHooks> = const { Cell::new(SpawnHooks { first: None }) };
}

#[derive(Default, Clone)]
struct SpawnHooks {
    first: Option<Arc<SpawnHook>>,
}

// 手动实现 drop，以防止在丢弃由 Arc 构成的链表时出现深度递归。
impl Drop for SpawnHooks {
    fn drop(&mut self) {
        let mut next = self.first.take();
        while let Some(SpawnHook { hook, next: n }) = next.and_then(|n| Arc::into_inner(n)) {
            drop(hook);
            next = n;
        }
    }
}

struct SpawnHook {
    hook: Box<dyn Send + Sync + Fn(&Thread) -> Box<dyn Send + FnOnce()>>,
    next: Option<Arc<SpawnHook>>,
}

/// 注册一个函数，使其为每个新派生的线程运行。
///
/// 这个 hook 在父线程中执行，并返回一个将在新线程中执行的函数。
///
/// 调用这个 hook 时会以新线程的 `Thread` 句柄作为参数。
///
/// 这个 hook 只会为当前线程添加，并由它所派生的线程继承。换句话说，添加一个
/// hook 对已经在运行的线程（当前线程除外）以及它们将来可能派生的线程没有影响。
///
/// hook 只能添加，不能移除。
///
/// 这些 hook 会以相反的顺序运行，从最近添加的那个开始。
///
/// # Usage
///
/// ```
/// #![feature(thread_spawn_hook)]
///
/// std::thread::add_spawn_hook(|_| {
///     ..; // 这会在父（派生方）线程中运行。
///     move || {
///         ..; // 这会在子（被派生）线程中运行。
///     }
/// });
/// ```
///
/// # Example
///
/// spawn hook 可用于从父线程“继承”一个线程局部变量：
///
/// ```
/// #![feature(thread_spawn_hook)]
///
/// use std::cell::Cell;
///
/// thread_local! {
///     static X: Cell<u32> = Cell::new(0);
/// }
///
/// // 这需要在主线程中、派生任何线程之前完成一次。
/// std::thread::add_spawn_hook(|_| {
///     // 获取派生方线程中 X 的值。
///     let value = X.get();
///     // 在新派生的线程中设置 X 的值。
///     move || X.set(value)
/// });
///
/// X.set(123);
///
/// std::thread::spawn(|| {
///     assert_eq!(X.get(), 123);
/// }).join().unwrap();
/// ```
#[unstable(feature = "thread_spawn_hook", issue = "132951")]
pub fn add_spawn_hook<F, G>(hook: F)
where
    F: 'static + Send + Sync + Fn(&Thread) -> G,
    G: 'static + Send + FnOnce(),
{
    SPAWN_HOOKS.with(|h| {
        let mut hooks = h.take();
        let next = hooks.first.take();
        hooks.first = Some(Arc::new(SpawnHook {
            hook: Box::new(move |thread| Box::new(hook(thread))),
            next,
        }));
        h.set(hooks);
    });
}

/// 运行所有的 spawn hook。
///
/// 在父线程上调用。
///
/// 返回那些将在新派生线程上调用的函数。
pub(super) fn run_spawn_hooks(thread: &Thread) -> ChildSpawnHooks {
    // 获取 spawn hook 的一份快照。
    //（这会递增对首节点的引用计数。）
    if let Ok(hooks) = SPAWN_HOOKS.try_with(|hooks| {
        let snapshot = hooks.take();
        hooks.set(snapshot.clone());
        snapshot
    }) {
        // 遍历这些 hook，运行它们，并把结果收集到一个 vector 中。
        let to_run: Vec<_> = iter::successors(hooks.first.as_deref(), |hook| hook.next.as_deref())
            .map(|hook| (hook.hook)(thread))
            .collect();
        // 把 hook 的快照和结果传递给新线程，新线程随后会运行
        // SpawnHookResults::run()。
        ChildSpawnHooks { hooks, to_run }
    } else {
        // TLS 已被销毁。跳过运行这些 hook。
        // 参见 https://github.com/rust-lang/rust/issues/138696
        ChildSpawnHooks::default()
    }
}

/// 运行 spawn hook 之后的结果。
///
/// 这个 struct 会被发送到新线程。
/// 它包含被继承的 hook 以及将要运行的闭包。
#[derive(Default)]
pub(super) struct ChildSpawnHooks {
    hooks: SpawnHooks,
    to_run: Vec<Box<dyn FnOnce() + Send>>,
}

impl ChildSpawnHooks {
    // 这会在新派生的线程上、于线程开始处直接运行。
    pub(super) fn run(self) {
        SPAWN_HOOKS.set(self.hooks);
        for run in self.to_run {
            run();
        }
    }
}
