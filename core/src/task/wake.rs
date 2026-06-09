#![stable(feature = "futures_api", since = "1.36.0")]

use crate::any::Any;
use crate::marker::PhantomData;
use crate::mem::{ManuallyDrop, transmute};
use crate::panic::AssertUnwindSafe;
use crate::{fmt, ptr};

/// `RawWaker` 让任务执行器的实现者得以构造出一个 [`Waker`] 或 [`LocalWaker`],
/// 从而提供自定义的唤醒行为。
///
/// 它由一个 `data` 裸指针和一张[虚函数指针表(vtable)][vtable]构成,后者用来定制 `RawWaker`
/// 的具体行为。这正是 `core` 只定义“唤醒任务”这一**契约/接口**、而把唤醒的真正实现完全交给
/// 上层运行时的方式:`core` 不知道任务存在哪里、如何调度,只通过 vtable 中的函数指针回调到
/// 运行时提供的实现。
///
/// `RawWaker` 的使用是 unsafe 的——它背后是一组裸函数指针和裸数据指针,正确性完全由实现者保证。
/// vtable 中的 `clone`、`wake`、`wake_by_ref` 和 `drop` 必须共同维护 `data` 所代表任务句柄的
/// 所有权、引用计数和线程安全边界。实现 [`Wake`] trait 是一种安全的替代方案,代价是需要进行
/// 堆内存分配。
///
/// [vtable]: https://en.wikipedia.org/wiki/Virtual_method_table
/// [`Wake`]: ../../alloc/task/trait.Wake.html
#[derive(PartialEq, Debug)]
#[stable(feature = "futures_api", since = "1.36.0")]
pub struct RawWaker {
    /// 一个数据指针,可用来存放执行器所需的任意数据。例如,它可以是一个指向与该任务关联的
    /// `Arc` 的类型擦除指针。该字段的值会作为第一个参数被传给 vtable 中的每一个函数。
    /// 这块数据的所有权与生命周期完全由实现者通过 vtable(尤其是 `clone`/`drop`)来管理。
    data: *const (),
    /// 用来定制本 waker 行为的虚函数指针表。
    vtable: &'static RawWakerVTable,
}

impl RawWaker {
    /// 从给定的 `data` 指针和 `vtable` 创建一个新的 `RawWaker`。
    ///
    /// `data` 指针可用来存放执行器所需的任意数据。例如,它可以是一个指向与该任务关联的
    /// `Arc` 的类型擦除指针。这个指针的值会作为第一个参数被传给 `vtable` 中的每一个函数。
    ///
    /// 需要特别注意:当用它来构造 [`Waker`] 时,`data` 指针必须指向一个线程安全的类型,
    /// 例如 `Arc<T: Send + Sync>`。而在构造 [`LocalWaker`] 时这一限制被解除——可以使用像
    /// `Rc<T>` 这样不实现 <code>[Send] + [Sync]</code> 的类型。
    ///
    /// `vtable` 用来定制由该 `RawWaker` 创建出的 `Waker` 的行为。对 `Waker` 的每一项操作,
    /// 都会调用底层 `RawWaker` 的 `vtable` 中对应的那个函数。
    #[inline]
    #[rustc_promotable]
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[rustc_const_stable(feature = "futures_api", since = "1.36.0")]
    #[must_use]
    pub const fn new(data: *const (), vtable: &'static RawWakerVTable) -> RawWaker {
        RawWaker { data, vtable }
    }

    #[stable(feature = "noop_waker", since = "1.85.0")]
    const NOOP: RawWaker = {
        const VTABLE: RawWakerVTable = RawWakerVTable::new(
            // clone 只是返回一个新的“什么都不做”的 `RawWaker`
            |_| RawWaker::NOOP,
            // wake 什么都不做
            |_| {},
            // wake_by_ref 什么都不做
            |_| {},
            // 由于我们没有分配任何资源,drop 也什么都不做
            |_| {},
        );
        RawWaker::new(ptr::null(), &VTABLE)
    };
}

/// 一张虚函数指针表(vtable),用来指定某个 [`RawWaker`] 的行为。
///
/// 传给表中所有函数的指针,都是其外层 [`RawWaker`] 对象里的 `data` 指针。
///
/// 表中的这些函数只应在 [`RawWaker`] 实现内部、针对一个正确构造的 [`RawWaker`] 对象的
/// `data` 指针来调用。这个 `data` 指针必须正是为同一张 vtable 准备的值,并且必须满足该 vtable
/// 各函数约定的有效性、所有权和线程安全要求。用任何其它 `data` 指针去调用其中某个函数,
/// 都会导致未定义行为。
///
/// 注意:尽管本类型实现了 `PartialEq`,但比较函数指针(以及由此比较像本结构这样含有函数指针的
/// 结构体)是不可靠的:指向同一函数的指针可能比较为不相等(因为函数会在多个 codegen unit 中被
/// 复制),而指向*不同*函数的指针也可能比较为相等(因为相同的函数可能在一个 codegen unit 内被
/// 去重合并)。
///
/// # 线程安全
/// 如果该 [`RawWaker`] 将被用来构造一个 [`Waker`],那么这些函数全部**必须是线程安全的**
/// (即便 [`RawWaker`] 本身是 <code>\![Send] + \![Sync]</code>)。原因在于 [`Waker`] 是
/// <code>[Send] + [Sync]</code> 的,它可能被移动到任意线程,或通过 `&` 引用被调用。举例来说,
/// 这意味着如果 `clone` 与 `drop` 函数维护着一个引用计数,它们就必须以原子方式来增减该计数,
/// 否则会发生数据竞争,进而导致内存不安全。
///
/// 然而,如果该 [`RawWaker`] 将被用来构造一个 [`LocalWaker`],这些函数就不必线程安全。
/// 这意味着 `data` 指针中可以存放 <code>\![Send] + \![Sync]</code> 的数据,引用计数也不需要任何
/// 原子同步。因为 [`LocalWaker`] 自身就不是线程安全的,无法被跨线程发送。
#[stable(feature = "futures_api", since = "1.36.0")]
#[allow(unpredictable_function_pointer_comparisons)]
#[derive(PartialEq, Copy, Clone, Debug)]
pub struct RawWakerVTable {
    /// 当 [`RawWaker`] 被克隆时(例如存放着该 [`RawWaker`] 的 [`Waker`] 被克隆时)调用此函数。
    ///
    /// 此函数的实现必须保留这一新增的 [`RawWaker`] 实例及其关联任务所需的全部资源(通常意味着
    /// 把引用计数加一)。返回的 [`RawWaker`] 必须使用与其 `data` 匹配的 vtable,并且在返回的那个
    /// [`RawWaker`] 上调用 `wake`,所唤醒的任务必须与在原始 [`RawWaker`] 上调用 `wake` 时唤醒的是
    /// 同一个任务。
    clone: unsafe fn(*const ()) -> RawWaker,

    /// 当 [`Waker`] 上的 `wake` 被调用时调用此函数。它必须唤醒与该 [`RawWaker`] 关联的任务。
    ///
    /// 此函数的实现必须确保释放与该 [`RawWaker`] 实例及其关联任务相关的所有资源——因为 `wake`
    /// 会**消费**这个 waker(把所有权交给本函数),通常意味着在唤醒后把引用计数减一。调用方不会再
    /// 对同一个 [`RawWaker`] 实例调用 `drop`,所以释放责任必须在这里完成。
    wake: unsafe fn(*const ()),

    /// 当 [`Waker`] 上的 `wake_by_ref` 被调用时调用此函数。它必须唤醒与该 [`RawWaker`] 关联的任务。
    ///
    /// 此函数与 `wake` 类似,但**不得消费**所提供的 `data` 指针(即不获取其所有权、不释放资源),
    /// 因为调用方仍持有该 waker。
    wake_by_ref: unsafe fn(*const ()),

    /// 当一个 [`Waker`] 被丢弃(drop)时调用此函数。
    ///
    /// 此函数的实现必须确保释放与该 [`RawWaker`] 实例及其关联任务相关的所有资源(通常意味着把
    /// 引用计数减一,并在计数归零时回收底层分配)。它至少要释放这一份 waker 实例拥有的资源;
    /// 调用方不能依赖丢弃 waker 来安排任务再次运行。
    drop: unsafe fn(*const ()),
}

impl RawWakerVTable {
    /// 从给定的 `clone`、`wake`、`wake_by_ref` 和 `drop` 函数创建一个新的 `RawWakerVTable`。
    ///
    /// 如果该 [`RawWaker`] 将被用来构造一个 [`Waker`],那么这些函数全部**必须是线程安全的**
    /// (即便 [`RawWaker`] 本身是 <code>\![Send] + \![Sync]</code>)。原因在于 [`Waker`] 是
    /// <code>[Send] + [Sync]</code> 的,它可能被移动到任意线程,或通过 `&` 引用被调用。举例来说,
    /// 这意味着如果 `clone` 与 `drop` 函数维护着一个引用计数,它们就必须以原子方式来增减该计数。
    ///
    /// 然而,如果该 [`RawWaker`] 将被用来构造一个 [`LocalWaker`],这些函数就不必线程安全。
    /// 这意味着 `data` 指针中可以存放 <code>\![Send] + \![Sync]</code> 的数据,引用计数也不需要任何
    /// 原子同步。因为 [`LocalWaker`] 自身就不是线程安全的,无法被跨线程发送。
    /// # `clone`
    ///
    /// 当 [`RawWaker`] 被克隆时(例如存放着该 [`RawWaker`] 的 [`Waker`]/[`LocalWaker`] 被克隆时)
    /// 调用此函数。
    ///
    /// 此函数的实现必须保留这一新增的 [`RawWaker`] 实例及其关联任务所需的全部资源。在返回的那个
    /// [`RawWaker`] 上调用 `wake`,所唤醒的任务必须与在原始 [`RawWaker`] 上调用 `wake` 时唤醒的是
    /// 同一个任务。若 `data` 中包含引用计数,这里通常要增加计数,并且在线程安全 `Waker` 场景下
    /// 必须使用原子操作。
    ///
    /// # `wake`
    ///
    /// 当 [`Waker`] 上的 `wake` 被调用时调用此函数。它必须唤醒与该 [`RawWaker`] 关联的任务。
    ///
    /// 此函数的实现必须确保释放与该 [`RawWaker`] 实例及其关联任务相关的所有资源。因为 `wake`
    /// 消费 waker,这里必须完成与 `drop` 等价的释放责任,同时把任务重新提交给执行器。
    ///
    /// # `wake_by_ref`
    ///
    /// 当 [`Waker`] 上的 `wake_by_ref` 被调用时调用此函数。它必须唤醒与该 [`RawWaker`] 关联的任务。
    ///
    /// 此函数与 `wake` 类似,但**不得消费**所提供的 `data` 指针;它只能借用任务句柄并安排唤醒,
    /// 不能减少属于当前 waker 实例的引用计数。
    ///
    /// # `drop`
    ///
    /// 当一个 [`Waker`]/[`LocalWaker`] 被丢弃(drop)时调用此函数。
    ///
    /// 此函数的实现必须确保释放与该 [`RawWaker`] 实例及其关联任务相关的所有资源。它代表正常
    /// 丢弃 waker;调用方不能把 drop 当作一次唤醒来依赖。
    #[rustc_promotable]
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[rustc_const_stable(feature = "futures_api", since = "1.36.0")]
    pub const fn new(
        clone: unsafe fn(*const ()) -> RawWaker,
        wake: unsafe fn(*const ()),
        wake_by_ref: unsafe fn(*const ()),
        drop: unsafe fn(*const ()),
    ) -> Self {
        Self { clone, wake, wake_by_ref, drop }
    }
}

#[derive(Debug)]
enum ExtData<'a> {
    Some(&'a mut dyn Any),
    None(()),
}

/// 一个异步任务的上下文。
///
/// 目前,`Context` 唯一的作用就是提供对一个 [`&Waker`](Waker) 的访问,该 waker 可用于唤醒
/// 当前任务。执行器在每次调用 [`Future::poll`](crate::future::Future::poll) 前构造或复用一个
/// `Context`,并把“如果本次返回 `Pending`,之后该如何重新调度这个任务”的能力放进其中。
/// future 实现若需要挂起,就从这个 `Context` 克隆 waker 并交给 I/O 资源、计时器或其它事件源。
/// 预留这一层封装是为了未来可扩展(例如携带更多上下文信息),而不破坏现有 API。
///
/// 注意:`Context` 在不同的 `poll` 调用之间**不保证**是同一个,其中携带的 [`Waker`] 也可能不同。
/// 因此实现者在每次 `poll` 时都应使用本次传入的、最新的那个 waker,而不要假定它与上次相同。
/// 这条规则允许执行器迁移任务、替换调度队列或压缩唤醒状态,同时仍保持 `poll`/`wake` 协议正确。
#[stable(feature = "futures_api", since = "1.36.0")]
#[lang = "Context"]
pub struct Context<'a> {
    waker: &'a Waker,
    local_waker: &'a LocalWaker,
    ext: AssertUnwindSafe<ExtData<'a>>,
    // 通过强制让生命周期为不变,来对未来可能的型变改动做前向兼容
    // 防护(参数位置的生命周期是逆变的,返回位置的生命周期是协变的)。
    _marker: PhantomData<fn(&'a ()) -> &'a ()>,
    // 确保 `Context` 是 `!Send` 且 `!Sync` 的,以便将来可以加入 `!Send` 和/或 `!Sync` 的字段。
    _marker2: PhantomData<*mut ()>,
}

impl<'a> Context<'a> {
    /// 从一个 [`&Waker`](Waker) 创建一个新的 `Context`。
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_waker", since = "1.82.0")]
    #[must_use]
    #[inline]
    pub const fn from_waker(waker: &'a Waker) -> Self {
        ContextBuilder::from_waker(waker).build()
    }

    /// 返回当前任务的 [`Waker`] 的引用。
    #[inline]
    #[must_use]
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_waker", since = "1.82.0")]
    pub const fn waker(&self) -> &'a Waker {
        &self.waker
    }

    /// 返回当前任务的 [`LocalWaker`] 的引用。
    #[inline]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub const fn local_waker(&self) -> &'a LocalWaker {
        &self.local_waker
    }

    /// 返回当前任务的扩展数据的引用。
    #[inline]
    #[unstable(feature = "context_ext", issue = "123392")]
    pub const fn ext(&mut self) -> &mut dyn Any {
        // FIXME: 这个字段让 Context 在 unwind 安全性方面变得格外别扭;
        // 若要稳定它,需要重新评估这里使用 AssertUnwindSafe 是否正当,以及调用方是否会观察到影响。
        match &mut self.ext.0 {
            ExtData::Some(data) => *data,
            ExtData::None(unit) => unit,
        }
    }
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context").field("waker", &self.waker).finish()
    }
}

/// 一个用于构造 `Context` 实例的构造器,支持设置 `LocalWaker`。
///
/// # 示例
/// ```
/// #![feature(local_waker)]
/// use std::task::{ContextBuilder, LocalWaker, Waker, Poll};
/// use std::future::Future;
///
/// let local_waker = LocalWaker::noop();
/// let waker = Waker::noop();
///
/// let mut cx = ContextBuilder::from_waker(&waker)
///     .local_waker(&local_waker)
///     .build();
///
/// let mut future = std::pin::pin!(async { 20 });
/// let poll = future.as_mut().poll(&mut cx);
/// assert_eq!(poll, Poll::Ready(20));
///
/// ```
#[unstable(feature = "local_waker", issue = "118959")]
#[derive(Debug)]
pub struct ContextBuilder<'a> {
    waker: &'a Waker,
    local_waker: &'a LocalWaker,
    ext: ExtData<'a>,
    // 通过强制让生命周期为不变,来对未来可能的型变改动做前向兼容
    // 防护(参数位置的生命周期是逆变的,返回位置的生命周期是协变的)。
    _marker: PhantomData<fn(&'a ()) -> &'a ()>,
    // 确保 `Context` 是 `!Send` 且 `!Sync` 的,以便将来可以加入 `!Send` 和/或 `!Sync` 的字段。
    _marker2: PhantomData<*mut ()>,
}

impl<'a> ContextBuilder<'a> {
    /// 从一个 `Waker` 创建一个 `ContextBuilder`。
    ///
    /// 这是执行器准备调用 `poll` 时的入口:普通 `Waker` 总是存在,`LocalWaker` 默认通过同一份
    /// `RawWaker` 派生出来,除非调用方随后显式设置。
    #[inline]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub const fn from_waker(waker: &'a Waker) -> Self {
        // SAFETY: LocalWaker 就是去掉了线程安全性的 Waker,两者内存布局一致,
        // 因此把 `&Waker` transmute 成 `&LocalWaker` 是合法的。
        let local_waker = unsafe { transmute(waker) };
        Self {
            waker,
            local_waker,
            ext: ExtData::None(()),
            _marker: PhantomData,
            _marker2: PhantomData,
        }
    }

    /// 从一个已有的 `Context` 创建一个 `ContextBuilder`。
    #[inline]
    #[unstable(feature = "context_ext", issue = "123392")]
    pub const fn from(cx: &'a mut Context<'_>) -> Self {
        let ext = match &mut cx.ext.0 {
            ExtData::Some(ext) => ExtData::Some(*ext),
            ExtData::None(()) => ExtData::None(()),
        };
        Self {
            waker: cx.waker,
            local_waker: cx.local_waker,
            ext,
            _marker: PhantomData,
            _marker2: PhantomData,
        }
    }

    /// 设置 `Context` 上 waker 的取值。
    #[inline]
    #[unstable(feature = "context_ext", issue = "123392")]
    pub const fn waker(self, waker: &'a Waker) -> Self {
        Self { waker, ..self }
    }

    /// 设置 `Context` 上 `LocalWaker` 的取值。
    #[inline]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub const fn local_waker(self, local_waker: &'a LocalWaker) -> Self {
        Self { local_waker, ..self }
    }

    /// 设置 `Context` 上扩展数据的取值。
    #[inline]
    #[unstable(feature = "context_ext", issue = "123392")]
    pub const fn ext(self, data: &'a mut dyn Any) -> Self {
        Self { ext: ExtData::Some(data), ..self }
    }

    /// 构建出 `Context`。
    #[inline]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub const fn build(self) -> Context<'a> {
        let ContextBuilder { waker, local_waker, ext, _marker, _marker2 } = self;
        Context { waker, local_waker, ext: AssertUnwindSafe(ext), _marker, _marker2 }
    }
}

/// `Waker` 是一个用于唤醒任务的句柄:它通过通知任务所属的执行器“该任务已可以再次运行”来完成
/// 唤醒。
///
/// 该句柄封装了一个 [`RawWaker`] 实例,后者定义了与具体执行器相关的唤醒行为。
///
/// 一个 `Waker` 的典型生命周期是:由执行器构造出来,包进一个 [`Context`],再传给
/// [`Future::poll()`]。随后,如果该 future 选择返回 [`Poll::Pending`],它就必须以某种方式把这个
/// waker 保存下来,并在该 future 应当被再次 poll 时调用 [`Waker::wake()`]。这正是 [`Future::poll`]
/// “返回 `Pending` 前必须已安排好唤醒”这一契约得以落实的机制。
///
/// `Waker` 实现了 [`Clone`]、[`Send`] 和 [`Sync`];因此一个 waker 可以从任意线程被调用,包括完全
/// 不受执行器管理的线程。举例来说,当某个阻塞式函数调用在另一个线程上完成时,就可以借此唤醒一个
/// future。
///
/// 注意:相比 `*waker = new_waker.clone()`,更推荐使用 `waker.clone_from(&new_waker)`,因为当两个
/// waker [唤醒的是同一个任务](Self::will_wake)时,后者可以避免不必要的克隆。
///
/// 从一个 [`RawWaker`] 构造 `Waker` 是 unsafe 的。实现 [`Wake`] trait 是一种安全的替代方案,
/// 代价是需要进行堆内存分配。
///
/// [`Future::poll()`]: core::future::Future::poll
/// [`Future::poll`]: core::future::Future::poll
/// [`Poll::Pending`]: core::task::Poll::Pending
/// [`Wake`]: ../../alloc/task/trait.Wake.html
#[repr(transparent)]
#[stable(feature = "futures_api", since = "1.36.0")]
#[rustc_diagnostic_item = "Waker"]
pub struct Waker {
    waker: RawWaker,
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl Unpin for Waker {}
#[stable(feature = "futures_api", since = "1.36.0")]
unsafe impl Send for Waker {}
#[stable(feature = "futures_api", since = "1.36.0")]
unsafe impl Sync for Waker {}

impl Waker {
    /// 唤醒与此 `Waker` 关联的任务。
    ///
    /// 只要执行器仍在运行且任务尚未结束,就保证:每一次 [`wake()`](Self::wake)(或
    /// [`wake_by_ref()`](Self::wake_by_ref))调用之后,该 `Waker` 所属的任务至少会被
    /// [`poll()`] 一次。这使得在运行可能无界的处理循环时,能够临时让出给其它任务。
    ///
    /// 注意,上面这一点意味着:运行时可能会把多次唤醒合并为对 [`poll()`] 的一次调用。
    ///
    /// 还要注意,让出给相互竞争的任务并不被保证:具体运行哪个任务由执行器决定,执行器也可能选择
    /// 再次运行当前任务。
    ///
    /// [`poll()`]: crate::future::Future::poll
    #[inline]
    #[stable(feature = "futures_api", since = "1.36.0")]
    pub fn wake(self) {
        // 真正的唤醒调用通过一次虚函数调用,委派给由执行器定义的那份实现。

        // 不要调用 `drop`——这个 waker 会被 `wake` 消费掉(其资源由 vtable 的 wake 负责释放)。
        let this = ManuallyDrop::new(self);

        // SAFETY: 这是安全的,因为初始化 `wake` 与 `data` 的唯一途径是 `Waker::from_raw`
        // 或 `Waker::new`,这要求使用者已确认 `RawWaker` 的契约得到了遵守
        // (即 data 指针对该 vtable 有效,且 wake 会消费这一份 waker 资源)。
        unsafe { (this.waker.vtable.wake)(this.waker.data) };
    }

    /// 在不消费该 `Waker` 的前提下,唤醒与之关联的任务。
    ///
    /// 这与 [`wake()`](Self::wake) 类似,但在已持有一个拥有所有权的 `Waker` 的场景下可能略低效一些。
    /// 相比 `waker.clone().wake()`,应优先使用本方法。
    #[inline]
    #[stable(feature = "futures_api", since = "1.36.0")]
    pub fn wake_by_ref(&self) {
        // 真正的唤醒调用通过一次虚函数调用,委派给由执行器定义的那份实现。

        // SAFETY: 见 `wake`
        unsafe { (self.waker.vtable.wake_by_ref)(self.waker.data) }
    }

    /// 如果此 `Waker` 与另一个 `Waker` 会唤醒同一个任务,返回 `true`。
    ///
    /// 本函数以“尽力而为”为基础工作:即便两个 `Waker` 确实会唤醒同一个任务,它也
    /// 可能返回 `false`。但反过来,如果本函数返回了 `true`,就保证这两个 `Waker` 唤醒的是同一个
    /// 任务。
    ///
    /// 本函数主要用于优化目的——例如本类型的 [`clone_from`](Self::clone_from) 实现就借助它,
    /// 在两个 waker 反正会唤醒同一任务时避免克隆。
    #[inline]
    #[must_use]
    #[stable(feature = "futures_api", since = "1.36.0")]
    pub fn will_wake(&self, other: &Waker) -> bool {
        // 我们通过比较 vtable 的地址(而非 vtable 的内容)来做优化。
        // 由于本函数被文档声明为“尽力而为”,这样做是允许的。
        let RawWaker { data: a_data, vtable: a_vtable } = self.waker;
        let RawWaker { data: b_data, vtable: b_vtable } = other.waker;
        a_data == b_data && ptr::eq(a_vtable, b_vtable)
    }

    /// 从给定的 `data` 指针和 `vtable` 创建一个新的 `Waker`。
    ///
    /// `data` 指针可用来存放执行器所需的任意数据。例如,它可以是一个指向与该任务关联的
    /// `Arc` 的类型擦除指针。这个指针的值会作为第一个参数被传给 `vtable` 中的每一个函数。
    ///
    /// 需要特别注意:`data` 指针必须指向一个线程安全的类型,例如 `Arc`。
    ///
    /// `vtable` 用来定制 `Waker` 的行为。对 `Waker` 的每一项操作,都会调用 `vtable` 中对应的
    /// 那个函数。
    ///
    /// # 安全性(Safety）
    ///
    /// `data` 必须是为这张 `vtable` 准备的指针,并且 `vtable` 中的 `clone`、`wake`、
    /// `wake_by_ref` 和 `drop` 必须满足 [`RawWakerVTable`] 文档中定义的全部契约。因为返回的是
    /// 线程安全的 [`Waker`],这些函数还必须能被任意线程调用;例如引用计数必须用原子操作维护。
    /// 若这些条件不成立,返回的 `Waker` 在克隆、唤醒或丢弃时可能触发未定义行为。
    ///
    /// (希望避免使用 unsafe 代码的作者可以改为实现 [`Wake`] trait,代价是需要一次堆分配。)
    ///
    /// [`Wake`]: ../../alloc/task/trait.Wake.html
    #[inline]
    #[must_use]
    #[stable(feature = "waker_getters", since = "1.83.0")]
    #[rustc_const_stable(feature = "waker_getters", since = "1.83.0")]
    pub const unsafe fn new(data: *const (), vtable: &'static RawWakerVTable) -> Self {
        Waker { waker: RawWaker { data, vtable } }
    }

    /// 从一个 [`RawWaker`] 创建一个新的 `Waker`。
    ///
    /// # 安全性(Safety）
    ///
    /// 传入的 [`RawWaker`] 必须拥有一个与其 `data` 匹配的 [`RawWakerVTable`],并且该 vtable
    /// 必须满足 [`Waker`] 所需的线程安全契约。`from_raw` 会接管这一份 `RawWaker` 的所有权;
    /// 之后 `Waker` 的 clone/wake/drop 都会直接调用该 vtable。若这些契约未被遵守,返回的 `Waker`
    /// 的行为是未定义的。
    ///
    /// (希望避免使用 unsafe 代码的作者可以改为实现 [`Wake`] trait,代价是需要一次堆分配。)
    ///
    /// [`Wake`]: ../../alloc/task/trait.Wake.html
    #[inline]
    #[must_use]
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_waker", since = "1.82.0")]
    pub const unsafe fn from_raw(waker: RawWaker) -> Waker {
        Waker { waker }
    }

    /// 返回一个引用,指向一个被使用时什么都不做的 `Waker`。
    ///
    // 注意!本方法的文档大部分与 `LocalWaker::noop` 的文档重复。
    // 如果你修改了这里,请考虑同时修改那一份。
    //
    /// 它主要用于编写测试:这些测试需要一个 [`Context`] 来 poll 某些 future,但并不期待这些
    /// future 会唤醒该 waker,或者即便唤醒了也无需做任何特定处理。
    ///
    /// 更一般地说,用 `Waker::noop()` 去 poll 一个 future,意味着丢弃“该 future 何时应被再次
    /// poll”的通知。所以只有当这种通知对推进进度并非必需时,才应使用它。
    ///
    /// 如果需要一个拥有所有权的 `Waker`,对它 `clone()` 即可。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::future::Future;
    /// use std::task;
    ///
    /// let mut cx = task::Context::from_waker(task::Waker::noop());
    ///
    /// let mut future = Box::pin(async { 10 });
    /// assert_eq!(future.as_mut().poll(&mut cx), task::Poll::Ready(10));
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "noop_waker", since = "1.85.0")]
    #[rustc_const_stable(feature = "noop_waker", since = "1.85.0")]
    pub const fn noop() -> &'static Waker {
        const WAKER: &Waker = &Waker { waker: RawWaker::NOOP };
        WAKER
    }

    /// 取出用于创建此 `Waker` 的 `data` 指针。
    #[inline]
    #[must_use]
    #[stable(feature = "waker_getters", since = "1.83.0")]
    pub fn data(&self) -> *const () {
        self.waker.data
    }

    /// 取出用于创建此 `Waker` 的 `vtable` 指针。
    #[inline]
    #[must_use]
    #[stable(feature = "waker_getters", since = "1.83.0")]
    pub fn vtable(&self) -> &'static RawWakerVTable {
        self.waker.vtable
    }

    /// 从一个函数指针构造一个 `Waker`。
    #[inline]
    #[must_use]
    #[unstable(feature = "waker_from_fn_ptr", issue = "148457")]
    pub const fn from_fn_ptr(f: fn()) -> Self {
        // SAFETY: 这里的 unsafe 用于 transmute;`data` 指针只由下面的 `from_fn_ptr`
        // 从同一个 `fn()` 指针写入,所以 vtable 回调收到它时,把它还原成 `fn()` 并调用是可靠的。
        static VTABLE: RawWakerVTable = unsafe {
            RawWakerVTable::new(
                |this| RawWaker::new(this, &VTABLE),
                |this| transmute::<*const (), fn()>(this)(),
                |this| transmute::<*const (), fn()>(this)(),
                |_| {},
            )
        };
        let raw = RawWaker::new(f as *const (), &VTABLE);

        // SAFETY: 该 `RawWaker` 不拥有外部分配:`clone` 只是复制同一个函数指针,`drop` 是空操作,
        // 而 `wake` 与 `wake_by_ref` 只是调用该函数指针,不会消费或释放不属于自己的资源。
        unsafe { Self::from_raw(raw) }
    }
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl Clone for Waker {
    #[inline]
    fn clone(&self) -> Self {
        Waker {
            // SAFETY: 这是安全的,因为初始化 `clone` 与 `data` 的唯一途径是 `Waker::from_raw`
            // 或 `Waker::new`,这要求使用者已确认 [`RawWaker`] 的契约得到了遵守
            // (clone 会正确管理底层引用计数)。
            waker: unsafe { (self.waker.vtable.clone)(self.waker.data) },
        }
    }

    /// 把 `source` 的一份克隆赋给 `self`,除非 [`self.will_wake(source)`][Waker::will_wake]
    /// 本就成立(那样就跳过克隆)。
    ///
    /// 相比直接把 `source.clone()` 赋给 `self`,更推荐本方法,因为当 `self` 已经是同一个 waker
    /// 时,它可以避免克隆。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::future::Future;
    /// use std::pin::Pin;
    /// use std::sync::{Arc, Mutex};
    /// use std::task::{Context, Poll, Waker};
    ///
    /// struct Waiter {
    ///     shared: Arc<Mutex<Shared>>,
    /// }
    ///
    /// struct Shared {
    ///     waker: Waker,
    ///     // ...
    /// }
    ///
    /// impl Future for Waiter {
    ///     type Output = ();
    ///     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    ///         let mut shared = self.shared.lock().unwrap();
    ///
    ///         // 更新 waker
    ///         shared.waker.clone_from(cx.waker());
    ///
    ///         // 就绪判断逻辑 ...
    /// #       Poll::Ready(())
    ///     }
    /// }
    ///
    /// ```
    #[inline]
    fn clone_from(&mut self, source: &Self) {
        if !self.will_wake(source) {
            *self = source.clone();
        }
    }
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl Drop for Waker {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: 这是安全的,因为初始化 `drop` 与 `data` 的唯一途径是 `Waker::from_raw`
        // 或 `Waker::new`,这要求使用者已确认 `RawWaker` 的契约得到了遵守
        // (drop 会正确释放底层资源/减少引用计数)。
        unsafe { (self.waker.vtable.drop)(self.waker.data) }
    }
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vtable_ptr = self.waker.vtable as *const RawWakerVTable;
        f.debug_struct("Waker")
            .field("data", &self.waker.data)
            .field("vtable", &vtable_ptr)
            .finish()
    }
}

/// `LocalWaker` 类似于 [`Waker`],但它**不**实现 [`Send`] 或 [`Sync`]。
///
/// 该句柄封装了一个 [`RawWaker`] 实例,后者定义了与具体执行器相关的唤醒行为。
///
/// 可以通过 `Context` 的 [`local_waker`] 方法取得 `LocalWaker`。
///
/// 一个 `LocalWaker` 的典型生命周期是:由执行器构造出来,借助 [`ContextBuilder`] 包进一个
/// [`Context`],再传给 [`Future::poll()`]。随后,如果该 future 选择返回 [`Poll::Pending`],
/// 它就必须以某种方式把这个 waker 保存下来,并在该 future 应当被再次 poll 时调用
/// [`LocalWaker::wake()`]。
///
/// `LocalWaker` 实现了 [`Clone`],但既不是 [`Send`] 也不是 [`Sync`];因此一个 `LocalWaker` 不能
/// 被移动到其它线程。一般来说,在选择用 `Waker` 还是 `LocalWaker` 时,除非确实需要把 waker 跨线程
/// 发送,否则 `LocalWaker` 更可取——因为普通 `Waker` 可能带来与内存同步相关的额外开销
/// (例如引用计数必须用原子操作)。
///
/// 注意:相比 `*local_waker = new_waker.clone()`,更推荐使用 `local_waker.clone_from(&new_waker)`,
/// 因为当两个 waker [唤醒的是同一个任务](Self::will_wake)时,后者可以避免不必要的克隆。
///
/// # 示例
/// 用一个 `LocalWaker` 来实现一个类似于 `std::thread::yield_now()` 的 future。
/// ```
/// #![feature(local_waker)]
/// use std::future::{Future, poll_fn};
/// use std::task::Poll;
///
/// // 一个会先返回一次 `Pending` 的 future。
/// fn yield_now() -> impl Future<Output=()> + Unpin {
///     let mut yielded = false;
///     poll_fn(move |cx| {
///         if !yielded {
///             yielded = true;
///             cx.local_waker().wake_by_ref();
///             return Poll::Pending;
///         }
///         return Poll::Ready(())
///     })
/// }
///
/// # async fn __() {
/// yield_now().await;
/// # }
/// ```
///
/// [`Future::poll()`]: core::future::Future::poll
/// [`Poll::Pending`]: core::task::Poll::Pending
/// [`local_waker`]: core::task::Context::local_waker
#[unstable(feature = "local_waker", issue = "118959")]
#[repr(transparent)]
pub struct LocalWaker {
    waker: RawWaker,
}

#[unstable(feature = "local_waker", issue = "118959")]
impl Unpin for LocalWaker {}

impl LocalWaker {
    /// 唤醒与此 `LocalWaker` 关联的任务。
    ///
    /// 只要执行器仍在运行且任务尚未结束,就保证:每一次 [`wake()`](Self::wake)(或
    /// [`wake_by_ref()`](Self::wake_by_ref))调用之后,该 `LocalWaker` 所属的任务至少会被
    /// [`poll()`] 一次。这使得在运行可能无界的处理循环时,能够临时让出给其它任务。
    ///
    /// 注意,上面这一点意味着:运行时可能会把多次唤醒合并为对 [`poll()`] 的一次调用。
    ///
    /// 还要注意,让出给相互竞争的任务并不被保证:具体运行哪个任务由执行器决定,执行器也可能选择
    /// 再次运行当前任务。
    ///
    /// [`poll()`]: crate::future::Future::poll
    #[inline]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub fn wake(self) {
        // 真正的唤醒调用通过一次虚函数调用,委派给由执行器定义的那份实现。

        // 不要调用 `drop`——这个 waker 会被 `wake` 消费掉。
        let this = ManuallyDrop::new(self);

        // SAFETY: 这是安全的,因为初始化 `wake` 与 `data` 的唯一途径是 `LocalWaker::from_raw`
        // 或 `LocalWaker::new`,这要求使用者已确认 `RawWaker` 的契约得到了遵守。
        unsafe { (this.waker.vtable.wake)(this.waker.data) };
    }

    /// 在不消费该 `LocalWaker` 的前提下,唤醒与之关联的任务。
    ///
    /// 这与 [`wake()`](Self::wake) 类似,但在已持有一个拥有所有权的 `LocalWaker` 的场景下可能
    /// 略低效一些。相比 `local_waker.clone().wake()`,应优先使用本方法。
    #[inline]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub fn wake_by_ref(&self) {
        // 真正的唤醒调用通过一次虚函数调用,委派给由执行器定义的那份实现。

        // SAFETY: 见 `wake`
        unsafe { (self.waker.vtable.wake_by_ref)(self.waker.data) }
    }

    /// 如果此 `LocalWaker` 与另一个 `LocalWaker` 会唤醒同一个任务,返回 `true`。
    ///
    /// 本函数以“尽力而为”为基础工作:即便两个 `LocalWaker` 确实会唤醒同一个任务,它也可能返回
    /// `false`。但反过来,如果本函数返回了 `true`,就保证这两个 `LocalWaker` 唤醒的是同一个任务。
    ///
    /// 本函数主要用于优化目的——例如本类型的 [`clone_from`](Self::clone_from) 实现就借助它,
    /// 在两个 waker 反正会唤醒同一任务时避免克隆。
    #[inline]
    #[must_use]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub fn will_wake(&self, other: &LocalWaker) -> bool {
        // 我们通过比较 vtable 的地址(而非 vtable 的内容)来做优化。
        // 由于本函数被文档声明为“尽力而为”,这样做是允许的。
        let RawWaker { data: a_data, vtable: a_vtable } = self.waker;
        let RawWaker { data: b_data, vtable: b_vtable } = other.waker;
        a_data == b_data && ptr::eq(a_vtable, b_vtable)
    }

    /// 从给定的 `data` 指针和 `vtable` 创建一个新的 `LocalWaker`。
    ///
    /// `data` 指针可用来存放执行器所需的任意数据。例如,它可以是一个指向与该任务关联的
    /// `Arc` 的类型擦除指针。这个指针的值会作为第一个参数被传给 `vtable` 中的每一个函数。
    ///
    /// `vtable` 用来定制 `LocalWaker` 的行为。对 `LocalWaker` 的每一项操作,都会调用 `vtable`
    /// 中对应的那个函数。
    ///
    /// # 安全性(Safety）
    ///
    /// `data` 必须是为这张 `vtable` 准备的指针,并且 `vtable` 中的 `clone`、`wake`、
    /// `wake_by_ref` 和 `drop` 必须满足 [`RawWakerVTable`] 文档中定义的全部契约。与 [`Waker`]
    /// 不同,`LocalWaker` 不要求这些函数线程安全,但它们仍必须正确维护这份任务句柄的所有权和
    /// 引用计数。若这些条件不成立,返回的 `LocalWaker` 的行为是未定义的。
    ///
    #[inline]
    #[must_use]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub const unsafe fn new(data: *const (), vtable: &'static RawWakerVTable) -> Self {
        LocalWaker { waker: RawWaker { data, vtable } }
    }

    /// 从一个 [`RawWaker`] 创建一个新的 `LocalWaker`。
    ///
    /// # 安全性(Safety）
    ///
    /// 传入的 [`RawWaker`] 必须拥有一个与其 `data` 匹配的 [`RawWakerVTable`],并且该 vtable
    /// 必须满足 [`LocalWaker`] 的所有权与引用计数契约。`from_raw` 会接管这一份 `RawWaker` 的
    /// 所有权;之后 `LocalWaker` 的 clone/wake/drop 都会直接调用该 vtable。若这些契约未被遵守,
    /// 返回的 `LocalWaker` 的行为是未定义的。
    #[inline]
    #[must_use]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub const unsafe fn from_raw(waker: RawWaker) -> LocalWaker {
        Self { waker }
    }

    /// 返回一个引用,指向一个被使用时什么都不做的 `LocalWaker`。
    ///
    // 注意!本方法的文档大部分与 `Waker::noop` 的文档重复。
    // 如果你修改了这里,请考虑同时修改那一份。
    //
    /// 它主要用于编写测试:这些测试需要一个 [`Context`] 来 poll 某些 future,但并不期待这些
    /// future 会唤醒该 waker,或者即便唤醒了也无需做任何特定处理。
    ///
    /// 更一般地说,用 `LocalWaker::noop()` 去 poll 一个 future,意味着丢弃“该 future 何时应被
    /// 再次 poll”的通知。所以只有当这种通知对推进进度并非必需时,才应使用它。
    ///
    /// 如果需要一个拥有所有权的 `LocalWaker`,对它 `clone()` 即可。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(local_waker)]
    /// use std::future::Future;
    /// use std::task::{ContextBuilder, LocalWaker, Waker, Poll};
    ///
    /// let mut cx = ContextBuilder::from_waker(Waker::noop())
    ///     .local_waker(LocalWaker::noop())
    ///     .build();
    ///
    /// let mut future = Box::pin(async { 10 });
    /// assert_eq!(future.as_mut().poll(&mut cx), Poll::Ready(10));
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub const fn noop() -> &'static LocalWaker {
        const WAKER: &LocalWaker = &LocalWaker { waker: RawWaker::NOOP };
        WAKER
    }

    /// 取出用于创建此 `LocalWaker` 的 `data` 指针。
    #[inline]
    #[must_use]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub fn data(&self) -> *const () {
        self.waker.data
    }

    /// 取出用于创建此 `LocalWaker` 的 `vtable` 指针。
    #[inline]
    #[must_use]
    #[unstable(feature = "local_waker", issue = "118959")]
    pub fn vtable(&self) -> &'static RawWakerVTable {
        self.waker.vtable
    }

    /// 从一个函数指针构造一个 `LocalWaker`。
    #[inline]
    #[must_use]
    #[unstable(feature = "waker_from_fn_ptr", issue = "148457")]
    pub const fn from_fn_ptr(f: fn()) -> Self {
        // SAFETY: 这里的 unsafe 用于 transmute;`data` 指针只由下面的 `from_fn_ptr`
        // 从同一个 `fn()` 指针写入,所以 vtable 回调收到它时,把它还原成 `fn()` 并调用是可靠的。
        static VTABLE: RawWakerVTable = unsafe {
            RawWakerVTable::new(
                |this| RawWaker::new(this, &VTABLE),
                |this| transmute::<*const (), fn()>(this)(),
                |this| transmute::<*const (), fn()>(this)(),
                |_| {},
            )
        };
        let raw = RawWaker::new(f as *const (), &VTABLE);

        // SAFETY: 该 `RawWaker` 不拥有外部分配:`clone` 只是复制同一个函数指针,`drop` 是空操作,
        // 而 `wake` 与 `wake_by_ref` 只是调用该函数指针,不会消费或释放不属于自己的资源。
        unsafe { Self::from_raw(raw) }
    }
}
#[unstable(feature = "local_waker", issue = "118959")]
impl Clone for LocalWaker {
    #[inline]
    fn clone(&self) -> Self {
        LocalWaker {
            // SAFETY: 这是安全的,因为初始化 `clone` 与 `data` 的唯一途径是 `LocalWaker::from_raw`
            // 或 `LocalWaker::new`,这要求使用者已确认 [`RawWaker`] 的契约得到了遵守。
            waker: unsafe { (self.waker.vtable.clone)(self.waker.data) },
        }
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        if !self.will_wake(source) {
            *self = source.clone();
        }
    }
}

#[unstable(feature = "local_waker", issue = "118959")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<LocalWaker> for Waker {
    fn as_ref(&self) -> &LocalWaker {
        // SAFETY: LocalWaker 就是去掉了线程安全性的 Waker,两者内存布局一致,转换合法。
        unsafe { transmute(self) }
    }
}

#[unstable(feature = "local_waker", issue = "118959")]
impl Drop for LocalWaker {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: 这是安全的,因为初始化 `drop` 与 `data` 的唯一途径是 `LocalWaker::from_raw`
        // 或 `LocalWaker::new`,这要求使用者已确认 `RawWaker` 的契约得到了遵守
        // (drop 会正确释放底层资源)。
        unsafe { (self.waker.vtable.drop)(self.waker.data) }
    }
}

#[unstable(feature = "local_waker", issue = "118959")]
impl fmt::Debug for LocalWaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vtable_ptr = self.waker.vtable as *const RawWakerVTable;
        f.debug_struct("LocalWaker")
            .field("data", &self.waker.data)
            .field("vtable", &vtable_ptr)
            .finish()
    }
}

#[unstable(feature = "local_waker", issue = "118959")]
impl !Send for LocalWaker {}
#[unstable(feature = "local_waker", issue = "118959")]
impl !Sync for LocalWaker {}
