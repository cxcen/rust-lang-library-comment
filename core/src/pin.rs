//! 把数据固定（pin）在内存中某个位置上的各种类型。
//!
//! 有时，能够依赖某个特定值无法被*移动（move）*（也就是它在内存中的地址不能改变）是很有用的。
//! 当有一个或多个[*指针*][pointer]指向那个值时，这尤其有用。能够依赖以下保证——即[指针][pointer]
//! 所指向的那个值（它的 **pointee**，即被指对象）将
//!
//! 1. 不会从它所在的内存位置被*移动*出去
//! 2. 更一般地说，会在同一个内存位置上保持*有效（valid）*
//!
//! ——这种能力被称为“固定（pinning）”。我们会说一个满足这些保证的值已被“固定”，意思是它已经被
//! 永久地（直到其生命周期结束为止）附着在它的内存位置上，仿佛被钉子钉在了一块图钉板上一样。固定
//! 一个值，对于让 [`unsafe`] 代码能够推断“指向被固定值的裸指针是否仍然有效”，是一个极其有用的
//! 构建块。[正如我们稍后将看到的][drop-guarantee]，一旦一个值被固定，它就必然会在其内存位置上
//! 保持有效，直到其生命周期结束。这种“固定”概念，对于在自引用类型（self-referential type）以及
//! 侵入式数据结构（intrusive data structure）之上实现安全接口是必需的——这类东西目前在完全安全的
//! Rust 中无法仅用受借用检查的[引用][reference]来建模。
//!
//! “固定”使我们能够把一个存在于某内存位置上的*值*置于这样一种状态：安全代码无法把那个值*移动*到
//! 内存中的另一个位置，也无法以其他方式让它在当前位置上失效（除非它实现了 [`Unpin`]，我们将
//! [在下文][self#unpin]讨论它）。任何想要以可能违反这些保证的方式与被固定值交互的东西，都必须
//! 承诺它不会真的违反这些保证，并使用 [`unsafe`] 关键字来标记这样一个承诺是由用户而非编译器来
//! 维护的。通过这种方式，我们就可以允许其他 [`unsafe`] 代码依赖“指向被固定值的任何指针在其被固定
//! 期间都可以安全解引用”这一点。
//!
//! 注意，只要你不使用 [`unsafe`]，就不可能以一种不健全的方式创建或误用一个被固定的值。关于如何
//! 固定一个值、以及如何从用户视角（不使用 [`unsafe`]）使用那个被固定值的实践细节，参见
//! [`Pin<Ptr>`] 的文档。
//!
//! 本文档的其余部分意在成为这样一类用户的权威依据（source of truth）：他们正在实现某个依赖固定来
//! 保证有效性的接口中那些 [`unsafe`] 的部分；在安全代码中使用 [`Pin<Ptr>`] 的用户无需详读它。
//!
//! 本文档分为以下几个部分：
//!
//! * [什么是“*移动*”？][what-is-moving]
//! * [什么是“固定”？][what-is-pinning]
//! * [地址敏感性，亦即“我们何时需要固定？”][address-sensitive-values]
//! * [带有地址敏感状态的类型示例][address-sensitive-examples]
//!   * [自引用结构体][self-ref]
//!   * [侵入式双向链表][linked-list]
//! * [细微之处与 `Drop` 保证][subtle-details]
//!
//! # 什么是“*移动*”？
//! [what-is-moving]: self#what-is-moving
//!
//! 当我们说一个值被*移动*时，我们指的是编译器逐字节地把该值从一个位置复制到另一个位置。在纯机械
//! 意义上，这与把一个值从内存中某处 [`Copy`] 到另一处是完全相同的。在 Rust 中，“move”还附带了
//! 所有权从一个变量转移到另一个变量的语义，这正是 [`Copy`] 与 move 之间的关键区别。不过，就本模块
//! 文档而言，当我们用斜体写下*move*时，我们*特指*该值在“位于内存中一个新位置”这一机械意义上发生了
//! *移动*。
//!
//! Rust 中所有的值都是可以平凡地*移动（moveable）*的。这意味着一个值所在的地址在两次借用之间不
//! 一定稳定。编译器被允许把一个值*移动*到一个新地址，而不运行任何代码来通知该值它的地址已经改变。
//! 尽管编译器不会在没有发生语义移动的地方插入内存*移动*，但有许多地方值*可能*被移动。例如，在做
//! 赋值或把一个值传入函数时。
//!
//! ```
//! #[derive(Default)]
//! struct AddrTracker(Option<usize>);
//!
//! impl AddrTracker {
//!     // 如果我们还没检查过 self 的地址，就存下当前地址。
//!     // 如果检查过了，就确认当前地址与上次相同，否则就 panic。
//!     fn check_for_move(&mut self) {
//!         let current_addr = self as *mut Self as usize;
//!         match self.0 {
//!             None => self.0 = Some(current_addr),
//!             Some(prev_addr) => assert_eq!(prev_addr, current_addr),
//!         }
//!     }
//! }
//!
//! // 创建一个 tracker 并存下初始地址
//! let mut tracker = AddrTracker::default();
//! tracker.check_for_move();
//!
//! // 这里我们对变量进行 shadow（遮蔽）。这带有一次语义移动，因此也可能
//! // 伴随一次机械的内存*移动*
//! let mut tracker = tracker;
//!
//! // 可能会 panic！
//! // tracker.check_for_move();
//! ```
//!
//! 在这个意义上，Rust 并不保证 `check_for_move()` 永远不会 panic，因为编译器在许多情况下都被允许
//! *移动* `tracker`。
//!
//! 像 [`Box<T>`] 和 [`&mut T`] 这样常见的智能指针类型也允许*移动*它们所指向的底层*值*：你可以从
//! [`Box<T>`] 中 move 出值，或者你可以用 [`mem::replace`] 把一个 `T` 从 [`&mut T`] 中 move 出来。
//! 因此，仅仅把一个值（例如上面的 `tracker`）放到一个指针后面，本身并不足以确保它的地址不会改变。
//!
//! # 什么是“固定”？
//! [what-is-pinning]: self#what-is-pinning
//!
//! 我们说一个值已被*固定*，是指它被置于这样一种状态：保证它从被固定起、直到它的 [`drop`] 被调用
//! 为止，都*位于内存中的同一个位置*。
//!
//! ## 地址敏感的值，亦即“我们何时需要固定”
//! [address-sensitive-values]: self#address-sensitive-values-aka-when-we-need-pinning
//!
//! Rust 中大多数值都完全可以随意地被*移动*。如果一个类型*始终*满足“该类型的*任何*值都可以随意
//! 被*移动*”，那么它就应当实现 [`Unpin`]，我们将在[下文][self#unpin]进一步讨论它。
//!
//! [`Pin`] 专门针对这样一种需求：围绕那些“在某些状态下会变得‘地址敏感（address-sensitive）’”的
//! 类型，实现*安全接口*。处于这种“地址敏感”状态的值*不*可以随意被*移动*。这样的值在其生命周期中
//! 地址敏感的那一段期间内，必须保持*未被移动*且有效，因为某个接口正依赖这些不变量为真，才能让其
//! 实现是健全的。
//!
//! 作为一个会变得地址敏感的类型的引导性例子，考虑一个类型，它含有一个指向它自身另一部分数据的
//! 指针，*亦即*一个“自引用”类型。为了让这样一个类型被健全地实现，那个指向 `self` 数据的指针在每次
//! 被访问时都必须被证明是有效的。但如果那个值被*移动*了，那个指针仍然会指向该值原先所在的旧地址，
//! 而不是指向 `self` 的新位置，从而变得无效。这类自引用类型的一个关键例子，就是编译器为 `async fn`
//! 实现 [`Future`] 而生成的状态机（state machine）。
//!
//! 这种带有*地址敏感*状态的类型，通常遵循一个大致如下的生命周期：
//!
//! 1. 创建出一个可以被随意移动的值。
//!     * 例如调用一个返回“实现了 [`Future`] 的状态机”的异步函数
//! 2. 某个操作导致该值开始依赖于它自己的地址不发生变化
//!     * 例如首次对所产生的 [`Future`] 调用 [`poll`]
//! 3. 该类型安全接口的进一步部分使用了内部的 [`unsafe`] 操作，这些操作假定该值的地址是稳定的
//!     * 例如后续对 [`poll`] 的调用
//! 4. 在该值失效（例如被释放）之前，它会被*drop*，从而有机会通知任何持有指向它自身指针的东西：
//!    那些指针即将失效
//!     * 例如 [`drop`] 那个 [`Future`] [^pin-drop-future]
//!
//! 要确保上面第 2 点和第 3 点所要求的不变量（它们适用于任何地址敏感的类型，而不只是自引用类型）
//! 不被破坏，有两种可能的方式。
//!
//! 1. 让该值检测到自己何时被移动，并更新所有指向它自身的指针。
//! 2. 保证在“指向该值的指针被期望可安全解引用”的那段时间内，该值的地址不发生变化（且其内存不会被
//!    重新用于其他任何用途）。
//!
//! 既然正如我们所讨论的，Rust 可以在不通知值的情况下移动它们，那么第一种方式就被排除了。
//!
//! 为了实现第二种方式，我们必须以某种方式强制其关键不变量，*亦即*阻止该值被*移动*或以其他方式
//! 失效（你可能注意到，这听起来非常像*固定*一个值的定义）。在 Rust 中，人们可能有几种方式来强制
//! 这个不变量：
//!
//! 1. 提供一个完全 `unsafe` 的 API 来与该对象交互，从而要求每个调用方自行维护该不变量
//! 2. 把那个绝不能被移动的值，存放在对象内部一个被精心管理的指针后面
//! 3. 利用类型系统，通过为“与*任何*需要这些不变量的对象交互”提供一个受限的 API 表面，来编码并
//!    强制这个不变量
//!
//! 第一种方式显然很不可取，因为该接口的 [`unsafe`] 性会像病毒一样蔓延到所有与该对象交互的代码中。
//!
//! 第二种方式对某些用例（尤其是自引用类型）来说是一个可行的解决方案。在这种模式下，任何带有地址
//! 敏感状态的类型，最终都会把它的数据存放在类似 [`Box<T>`] 的东西里，精心管理对那块数据的内部
//! 访问以确保不发生任何*移动*或其他失效，最后在其上提供一个安全接口。
//!
//! 使用这种模式有几个相互关联的缺点。最显著的一个是：每个单独的对象都必须假定它是*靠自己*来确保
//! 其数据不被*移动*或以其他方式失效的。由于不同类型的值之间没有共享的约定（contract），一个对象
//! 无法假定其他与它交互的东西会恰当地尊重“与它的数据交互”相关的不变量，因此它必须防范所有人。
//! 正因如此，地址敏感类型的*组合（composition）*每加入一个新对象，至少就需要一层指针间接
//!（indirection）（并且在实践中还需要一次堆分配）。
//!
//! 尽管还有其他原因，但这种“代价高昂的组合”问题正是促使 Rust 转向采用一种不同模式的关键所在。
//! 当人们考虑到——比如说——把那些最终将构成一个异步任务的各个 [`Future`]（包括地址敏感的
//! `async fn` 状态机）组合在一起会有什么影响时，这一点尤其成问题。完全有可能存在许多层相互组合的
//! [`Future`]，包括处理一个任务不同部分的多层 `async fn`。在这种情况下，强制每一层组合都进行间接
//! 和分配被认为是不可接受的。
//!
//! [`Pin<Ptr>`] 是第三种方式的一个实现。它通过围绕“固定”数据的保证构建一套*共享的契约性语言*
//!（shared contractual language），让我们能够解决上面讨论过的第二种方式所存在的问题。
//!
//! [^pin-drop-future]: Future 自身从不需要通知其他代码片段它正在被 drop，但像基于栈的侵入式链表
//! 这样的数据结构则确实需要。
//!
//! ## 使用 [`Pin<Ptr>`] 来固定值
//!
//! 为了固定一个值，我们把一个*指向该值的指针*（类型为某个 `Ptr`）包装进一个 [`Pin<Ptr>`]。
//! [`Pin<Ptr>`] 可以包装任意指针类型，从而形成一个承诺：那个 **pointee**（被指对象）不会被
//! *移动*或[以其他方式失效][subtle-details]。
//!
//! 我们把这样一个被 [`Pin`] 包装的指针称为**固定指针（pinning pointer）**（或固定引用、固定
//! `Box` 等），因为正是它的存在，才在概念上把底层的被指对象固定在原处：它就是那枚把数据牢牢钉在
//! 图钉板（即内存）上的隐喻意义的“图钉”。
//!
//! 注意，被 [`Pin`] 包装的并不是我们想要固定的那个值本身，而是一个指向那个值的指针！一个
//! [`Pin<Ptr>`] 并不固定那个 `Ptr`；相反，它固定的是该指针的***pointee**（被指对象）值*。
//!
//! ### 把固定作为一种库契约
//!
//! 固定既不需要、也不使用任何编译器“魔法”[^noalias]，它仅仅依赖于一个库 API 中 [`unsafe`] 的
//! 部分与其用户之间的特定契约。
//!
//! 作为 [`Pin`] API 中 [`unsafe`] 部分的使用者，强调这一点很重要。实际上，这意味着：通过创建一个
//! 指向某个值的 [`Pin<Ptr>`] 来执行“固定”一个值的机械操作，*并不*真的改变编译器对待内部值的方式！
//! 完全有可能用不正确的 [`unsafe`] 代码，去创建一个指向“实际上并不满足被固定值所必须满足的不变量”
//! 的值的 [`Pin<Ptr>`]，并以这种方式导致未定义行为——哪怕（从那一点起）是完全安全的代码。类似地，
//! 使用 [`unsafe`]，人们可以从一个 [`Pin<Ptr>`] 获取到一个裸的 [`&mut T`]，并用它非法地把被固定值
//! *移动*出去。确保这些不变量不被违反，正是 [`Pin`] API 中 [`unsafe`] 部分使用者的职责。
//!
//! 这一点不同于例如 [`UnsafeCell`]，后者会改变程序编译输出的语义。一个 [`Pin<Ptr>`] 是指向某个值
//! 的句柄（handle），我们已承诺不会把该值 move 出去，但 Rust 仍然认为所有值本身从根本上都是可以
//! 移动的，*例如*通过赋值或 [`mem::replace`]。
//!
//! [^noalias]: 关于 `Pin<&mut T>` 的别名（aliasing）语义究竟应当如何，这里还有一些细微之处仍在
//! 决定之中，不过截至今天，上文所述是成立的。
//!
//! ### [`Pin`] 如何在安全代码中防止误用
//!
//! 为了达成固定被指对象值这一目标，[`Pin<Ptr>`] 在安全代码中限制了对所包装 `Ptr` 类型的访问。
//! 具体来说，[`Pin`] 禁止那些“会让用户在不使用 [`unsafe`] 的情况下*移动*底层被指对象值、或以其他
//! 方式把那块内存重新用于别处”的访问方式。例如，一个 [`Pin<&mut T>`] 使得无法安全地获取到所包装的
//! <code>[&mut] T</code>，因为通过那个 <code>[&mut] T</code> 就有可能用 [`mem::replace`] 等手段
//! 把底层值从指针中*移动*出去。
//!
//! 正如上文所讨论的，这个承诺必须由与 [`Pin<Ptr>`] 交互的 [`unsafe`] 代码手动维护，这样其他
//! [`unsafe`] 代码才能依赖被指对象值保持*未被移动*且有效。那些操作“处于地址敏感状态的值”的接口，
//! 会接受形如 <code>[Pin]<[&mut] T></code> 或 <code>[Pin]<[Box]\<T>></code> 的参数，以向调用方
//! 表明这个契约。
//!
//! [正如下文所讨论的][drop-guarantee]，在一个地址敏感类型的接口中选择采用固定保证，对于在该类型
//! 上实现某些安全 trait 也会有相应的影响。
//!
//! ## [`Deref`] 与 [`Pin<Ptr>`] 之间的交互
//!
//! 既然 [`Pin<Ptr>`] 可以包装任意指针类型，它就利用 [`Deref`] 和 [`DerefMut`] 来识别被固定的
//! 被指对象数据的类型，并提供对它（受限的）访问。
//!
//! 一个满足 [`Ptr: Deref`][Deref] 的 [`Pin<Ptr>`] 是一个指向被固定的 [`Ptr::Target`][Target] 的
//! “`Ptr` 风格固定指针”——所以，一个 <code>[Pin]<[Box]\<T>></code> 是指向被固定 `T` 的、拥有所有权
//! 的固定指针，而一个 <code>[Pin]<[Rc]\<T>></code> 则是指向被固定 `T` 的、引用计数的固定指针。
//!
//! [`Pin<Ptr>`] 还会利用 [`<Ptr as Deref>::Target`][Target] 类型信息，来修改它被允许为“与那块
//! 数据交互”所提供的接口（例如，当一个固定指针指向的被固定数据实现了 [`Unpin`] 时，如
//! [下文所讨论的][self#unpin]）。
//!
//! [`Pin<Ptr>`] 要求 `Ptr` 上 [`Deref`] 和 [`DerefMut`] 的实现直接返回一个指向被固定数据的指针，
//! 并且在其 [`DerefMut::deref_mut`] 的实现过程中不从 `self` 参数中*移动*出值。让 [`unsafe`] 代码
//! 去包装带有这种“恶意”[`Deref`] 实现的指针类型是不健全的；详见 [`Pin<Ptr>::new_unchecked`]。
//!
//! ## 修复 `AddrTracker`
//!
//! 稳定地址的保证，对于让我们的 `AddrTracker` 例子能正常工作是必需的。当 `check_for_move` 看到一个
//! <code>[Pin]<&mut AddrTracker></code> 时，它可以安全地假定那个值会一直存在于那个相同的地址上，
//! 直到该值离开作用域为止，因此对它的多次调用*不可能* panic。
//!
//! ```
//! use std::marker::PhantomPinned;
//! use std::pin::Pin;
//! use std::pin::pin;
//!
//! #[derive(Default)]
//! struct AddrTracker {
//!     prev_addr: Option<usize>,
//!     // 移除自动实现的 `Unpin` 约束，把这个类型标记为带有某种
//!     // 地址敏感状态。这对于让我们期望的固定保证生效至关重要，
//!     // 下文会进一步讨论。
//!     _pin: PhantomPinned,
//! }
//!
//! impl AddrTracker {
//!     fn check_for_move(self: Pin<&mut Self>) {
//!         let current_addr = &*self as *const Self as usize;
//!         match self.prev_addr {
//!             None => {
//!                 // SAFETY: 我们不会从 self 中 move 出值
//!                 let self_data_mut = unsafe { self.get_unchecked_mut() };
//!                 self_data_mut.prev_addr = Some(current_addr);
//!             },
//!             Some(prev_addr) => assert_eq!(prev_addr, current_addr),
//!         }
//!     }
//! }
//!
//! // 1. 创建该值，此时还不处于地址敏感状态
//! let tracker = AddrTracker::default();
//!
//! // 2. 通过把该值放到一个固定指针后面来固定它，从而把
//! // 它置于地址敏感状态
//! let mut ptr_to_pinned_tracker: Pin<&mut AddrTracker> = pin!(tracker);
//! ptr_to_pinned_tracker.as_mut().check_for_move();
//!
//! // 尝试访问 `tracker`、或把 `ptr_to_pinned_tracker` 传给任何需要
//! // 对它的非固定版本进行可变访问的东西，都将不再能通过编译
//!
//! // 3. 我们现在可以假定该 tracker 值永远不会被移动，因此
//! // 这一句永远不会 panic！
//! ptr_to_pinned_tracker.as_mut().check_for_move();
//! ```
//!
//! 注意，这个不变量仅仅是通过“让那些会对被固定值执行移动的代码无法被调用”来强制的。之所以如此，
//! 是因为访问那个被固定值的唯一途径就是通过那个固定的 <code>[Pin]<[&mut] T></code>，而它反过来
//! 又限制了我们的访问。
//!
//! ## [`Unpin`]
//!
//! Rust 中绝大多数类型都没有地址敏感状态。这类类型实现 [`Unpin`] 这个 auto-trait（自动 trait），
//! 当*被指对象*类型 `T` 是 [`Unpin`] 时，它会取消 [`Pin`] 的限制性效果。当 [`T: Unpin`][Unpin]
//! 时，<code>[Pin]<[Box]\<T>></code> 的行为与一个不固定的 [`Box<T>`] 完全相同；类似地，
//! <code>[Pin]<[&mut] T></code> 也不会在普通的 [`&mut T`] 之上施加任何额外限制。
//!
//! 这个 trait 的设计意图，是缓解这样一类 API 在人体工程学上的退化：它们对某些类型出于健全性需要
//! 使用 [`Pin`]，但同时又希望被其他并不关心固定的类型所使用。这类 API 的典型例子就是
//! [`Future::poll`]。有许多 [`Future`] 类型并不关心固定。这些 future 可以实现 [`Unpin`]，从而
//! 绕开该 API 中与固定相关的限制，同时仍然允许那些*确实*需要固定的 [`Future`] 子集被健全地实现。
//!
//! 注意，[`Pin<Ptr>`] 与 [`Unpin`] 之间的交互是通过 **pointee**（被指对象）值的类型
//! [`<Ptr as Deref>::Target`][Target] 来发生的。`Ptr` 类型本身是否实现 [`Unpin`] 并不影响一个
//! [`Pin<Ptr>`] 的行为。例如，[`Box`] 是否为 [`Unpin`] 对 <code>[Pin]<[Box]\<T>></code> 的行为
//! 没有影响，因为被指对象值的类型是 `T` 而不是 [`Box`]。所以，影响
//! <code>[Pin]<[Box]\<T>></code> 行为的，是 `T` 是否实现 [`Unpin`]。
//!
//! 属于 [`Unpin`] 的内建类型包括所有原生类型，例如 [`bool`]、[`i32`]、[`f32`]、引用
//!（<code>[&]T</code> 和 <code>[&mut] T</code>）等，以及许多 core 和标准库类型，例如
//! [`Box<T>`]、[`String`] 等。这些类型之所以被标记为 [`Unpin`]，是因为它们不像我们上面讨论的
//! 那些类型一样带有地址敏感状态。如果它们确实带有这样的状态，那么其接口的那些部分若不通过固定
//! 来表达就会是不健全的，于是它们就需要不实现 [`Unpin`]。
//!
//! 只要一个类型的所有字段所组成的类型也都是 [`Unpin`]，编译器就可以采取保守立场，把该类型标记为
//! [`Unpin`]。这是因为：如果一个类型实现了 [`Unpin`]，那么让该类型的实现为了健全性而依赖与固定
//! 相关的保证就是不健全的——*即便*是通过一个“固定”指针来看待它时也是如此！确保一个“为健全性而依赖
//! 固定”的类型*不*被标记为 [`Unpin`]（通过添加一个 [`PhantomPinned`] 字段），是该类型实现者的
//! 责任。这正是我们在上面的 `AddrTracker` 例子中所做的。不这样做的话，你*绝不能*依赖与固定相关的
//! 保证适用于你的类型！
//!
//! 如果你确实需要固定一个“实现了 [`Unpin`] 的外部类型或内建类型”的值，你就需要围绕你想固定的那个
//! [`Unpin`] 类型创建你自己的包装类型，然后用 [`PhantomPinned`] 来 opt-out（退出）[`Unpin`]。
//!
//! 此时，对“你希望保持被固定的那个内部字段”暴露访问，也必须被谨慎考虑！记住，暴露一个能给出
//! <code>[Pin]<[&mut] InnerT></code>（其中 <code>InnerT: [Unpin]</code>）访问权的方法，会允许安全
//! 代码平凡地把那个内部值从那个固定指针中 move 出去，而这恰恰正是你试图阻止的！通过一个固定指针来
//! 暴露被固定值的某个字段，被称为“投影（projecting）”一个 pin，而“在哪些情况下一个 pin 应当能被
//! 投影、哪些情况下不能”这个更一般的问题，则被称为“结构化固定（structural pinning）”。我们将在
//! [下文][structural-pinning]更详细地讨论这一点。
//!
//! # 地址敏感类型的示例
//! [address-sensitive-examples]: #examples-of-address-sensitive-types
//!
//! ## 一个自引用结构体
//! [self-ref]: #a-self-referential-struct
//! [`Unmovable`]: #a-self-referential-struct
//!
//! 自引用结构体是最简单的一类地址敏感类型。
//!
//! 让一个结构体持有一个指回它自身的指针，常常是很有用的，这能让程序高效地追踪该结构体的各个子部分。
//! 在下面，`slice` 字段是一个指向 `data` 字段的指针，我们可以设想它在解析器代码中被用来追踪
//! `data` 上的一个滑动窗口（sliding window）。
//!
//! 如前所述，这种模式也被编译器生成的 [`Future`] 大量使用。
//!
//! ```rust
//! use std::pin::Pin;
//! use std::marker::PhantomPinned;
//! use std::ptr::NonNull;
//!
//! /// 这是一个自引用结构体，因为 `self.slice` 指向 `self.data`。
//! struct Unmovable {
//!     /// 后备缓冲区（backing buffer）。
//!     data: [u8; 64],
//!     /// 指向 `self.data`，我们知道它本身是非空的。这里用裸指针是因为我们无法
//!     /// 用普通引用做到这一点。
//!     slice: NonNull<[u8]>,
//!     /// 抑制 `Unpin`，使其一旦构造完成就无法从 `Pin` 中被 move 出去。
//!     _pin: PhantomPinned,
//! }
//!
//! impl Unmovable {
//!     /// 创建一个新的 `Unmovable`。
//!     ///
//!     /// 为了确保数据不会移动，我们把它放在堆上、一个固定 Box 的后面。
//!     /// 注意，数据被固定了，但固定它的那个 `Pin<Box<Self>>` 本身仍然可以被移动。
//!     /// 这一点很重要，因为它意味着我们可以从函数中返回那个固定指针，而返回本身
//!     /// 就是一种 move！
//!     fn new() -> Pin<Box<Self>> {
//!         let res = Unmovable {
//!             data: [0; 64],
//!             // 我们只在数据就位之后才创建这个指针，
//!             // 否则在我们还没开始之前它就已经移动过了。
//!             slice: NonNull::from(&[]),
//!             _pin: PhantomPinned,
//!         };
//!         // 首先我们把数据放进一个 box 里，这将是它最终的安息之所
//!         let mut boxed = Box::new(res);
//!
//!         // 然后我们让 slice 字段指向那块 boxed 数据的恰当部分。
//!         // 从现在起我们需要确保不会移动这块 boxed 数据。
//!         boxed.slice = NonNull::from(&boxed.data);
//!
//!         // 为此，我们用一个固定（被 `Pin` 包装的）指针指向这块数据，从而把它就地固定。
//!         //
//!         // `Box::into_pin` 让现有的 `Box` 就地固定数据而不移动它，
//!         // 所以我们现在可以安全地在上面插入 slice 指针*之后*这样做，但我们必须
//!         // 留意在此期间没有对 `res` 执行任何其他语义移动。
//!         let pin = Box::into_pin(boxed);
//!
//!         // 现在我们可以返回这块（通过一个固定 Box）被固定的数据了
//!         pin
//!     }
//! }
//!
//! let unmovable: Pin<Box<Unmovable>> = Unmovable::new();
//!
//! // 内部的被指对象 `Unmovable` 结构体现在将永远不被允许移动。
//! // 与此同时，我们可以自由地把那个指针移来移去。
//! # #[allow(unused_mut)]
//! let mut still_unmoved = unmovable;
//! assert_eq!(still_unmoved.slice, NonNull::from(&still_unmoved.data));
//!
//! // 我们无法可变地解引用一个 `Pin<Ptr>`，除非被指对象是 `Unpin` 或者我们使用 unsafe。
//! // 由于我们的类型没有实现 `Unpin`，下面这行将无法通过编译。
//! // let mut new_unmoved = Unmovable::new();
//! // std::mem::swap(&mut *still_unmoved, &mut *new_unmoved);
//! ```
//!
//! ## 一个侵入式双向链表
//! [linked-list]: #an-intrusive-doubly-linked-list
//!
//! 在一个侵入式双向链表中，集合本身并不拥有存放其各个元素的内存。相反，每个客户端（client）都可以
//! 用它喜欢的任意方式为它添加到链表中的元素分配空间，包括在栈上！只要存放在某个给定栈帧中的元素
//! 在离开作用域之前先从链表中被移除，元素就可以存活于一个比集合本身寿命更短的栈帧上。
//!
//! 为了让这样一个侵入式数据结构正常工作，每个元素都把指向其前驱和后继的指针存储在它自己的数据中，
//! 而不是由链表结构本身来管理这些指针。正是在这个意义上，这个结构是“侵入式（intrusive）”的：一个
//! 元素如何被存放在更大的结构中的种种细节，“侵入”到了元素类型本身的实现里！
//!
//! 这样一个数据结构的完整实现细节超出了本文档的范围，但我们将讨论 [`Pin`] 如何能在其中提供帮助。
//!
//! 使用这种侵入式模式，元素只有在被固定时才可以被添加。如果我们思考一下把未固定的值添加到这样一个
//! 链表中会有什么后果，这一点就清楚了：
//!
//! *移动*或以其他方式让一个元素的数据失效，会使存储在它前后元素中的、指回它的指针失效。因此，为了
//! 健全地解引用所存储的指向下一个和上一个元素的指针，我们必须满足这一保证：没有任何东西让那些指针
//!（它们指向我们并不拥有的数据）失效。
//!
//! 此外，每个元素的 [`Drop`][Drop] 实现都必须以某种方式通知它的前驱和后继元素：在它被完全销毁之前
//! 应当把它从链表中移除，否则那些指回它的指针又会失效。
//!
//! 至关重要的是，这意味着我们必须能够依赖 [`drop`] 总会在一个元素失效之前被调用。如果一个元素能够
//! 在不调用 [`drop`] 的情况下被释放或以其他方式失效，那么存储在它相邻元素中的、指向它的指针就会
//! 变得无效，从而破坏这个数据结构。
//!
//! 因此，固定数据还附带[“`Drop` 保证”][drop-guarantee]。
//!
//! # 细微之处与 `Drop` 保证
//! [subtle-details]: self#subtle-details-and-the-drop-guarantee
//! [drop-guarantee]: self#subtle-details-and-the-drop-guarantee
//!
//! 固定的目的不*仅仅*是阻止一个值被*移动*，更一般地说，是为了能够依赖被固定值在内存中***某个特定
//! 位置上*保持有效**。
//!
//! 为此，固定一个值会添加一个*额外的*不变量，要使用被固定数据是有效的，这个不变量就必须被维护——
//! 它叠加在“同类型的非固定值要有效所必须维护的那些不变量”之上：
//!
//! 从一个值通过构造一个指向它的 [`Pin`] 固定指针而被固定的那一刻起，那个值就必须*保持，**有效***，
//! 位于内存中的那同一个地址上，*直到它的 [`drop`] 处理函数被调用为止。*
//!
//! 这里有一些我们尚未详细谈及的细微之处。上面描述的那个不变量意味着：是的，
//!
//! 1. 该值不能从它在内存中的位置被 move 出去
//!
//! 但它还蕴含着，
//!
//! 2. 在被固定值的生命周期内，存放该值的那个内存位置不能被失效或以其他方式挪作他用，直到它的
//!    [`drop`] 返回或 panic 为止
//!
//! 这一点很微妙，但对于健全地实现侵入式数据结构是必需的。
//!
//! ## `Drop` 保证
//!
//! 需要有一种途径，让一个被固定的值能够通知任何依赖其被固定状态的代码：它即将被销毁。通过这种
//! 方式，那些依赖它的代码就可以把这个被固定值的地址从它们的数据结构中移除，或者在知道“再也不能依赖
//! 该值存在于它被固定到的那个位置”的前提下，改变它们的行为。
//!
//! 因此，在任何我们可能想要覆写一个被固定值的情形下，该值的 [`drop`] 都必须事先被调用（除非那个被
//! 固定值实现了 [`Unpin`]，在那种情况下，我们可以像往常一样忽略 [`Pin`] 的所有保证）。
//!
//! 最常见的“存储复用”情形发生在：栈上的一个值作为函数返回的一部分被销毁时，以及堆存储被释放时。
//! 在这两种情况下，当使用标准的安全代码时，[`drop`] 都会由 Rust 替我们运行。然而，对于手动的堆
//! 分配或其他自定义分配的存储，[`unsafe`] 代码必须确保在释放并复用该存储之前调用
//! [`ptr::drop_in_place`]。
//!
//! 此外，即便没有任何存储被（分配/释放），存储的“复用”/失效也可能发生。例如，如果我们有一个含有
//! `Some(v)`（其中 `v` 被固定）的 [`Option`]，那么把那个 option 设置为 `None` 就会使 `v` 失效。
//!
//! 类似地，如果用一个 [`Vec`] 来存储被固定的值，并用 [`Vec::set_len`] 手动“杀死”该 vector 的某些
//! 元素，那么所有被“杀死”的项都会失效——如果那些项是被固定的，这就会是*未定义行为*。
//!
//! 这两种情形都多少有些刻意构造，但至关重要的是要记住：[`Pin`] 固定的数据*必须*在它失效之前被
//! [`drop`]；这不仅是为了防止内存泄漏，更是健全性的要求。作为一个推论，下面这段代码*永远*无法被
//! 做成安全的：
//!
//! ```rust
//! # use std::mem::ManuallyDrop;
//! # use std::pin::Pin;
//! # struct Type;
//! // 把某个东西固定在一个 `ManuallyDrop` 内部。这本身没问题。
//! let mut pin: Pin<Box<ManuallyDrop<Type>>> = Box::pin(ManuallyDrop::new(Type));
//!
//! // 然而，创建一个指向 `ManuallyDrop` *内部*那个类型的固定可变引用，
//! // 就不行了！
//! let inner: Pin<&mut Type> = unsafe {
//!     Pin::map_unchecked_mut(pin.as_mut(), |x| &mut **x)
//! };
//! ```
//!
//! 由于 [`mem::ManuallyDrop`] 抑制了 `Type` 的析构逻辑，当那个
//! <code>[Box]<[ManuallyDrop]\<Type>></code> 被 drop 时，它不会被运行，从而违反了那个
//! <code>[Pin]<[&mut] Type>></code> 的 drop 保证。
//!
//! 当然，以一种“使底层存储永远不会失效或被复用”的方式*泄漏*内存仍然没问题：[`mem::forget`] 掉一个
//! [`Box<T>`] 会阻止它的存储被复用，因此 [`drop`] 保证仍然得到满足。
//!
//! # 实现一个地址敏感类型。
//!
//! 本节将详细讨论“实现你自己的地址敏感类型”的重要考量，它不同于仅仅以一种泛型的方式*使用*
//! [`Pin<Ptr>`]。
//!
//! ## 为带有地址敏感状态的类型实现 [`Drop`]
//! [drop-impl]: self#implementing-drop-for-types-with-address-sensitive-states
//!
//! [`drop`] 函数接受 [`&mut self`]，但*即便那个 `self` 已被固定*，它也会被调用！为带有地址敏感
//! 状态的类型实现 [`Drop`] 需要一些小心，因为如果在 [`drop`] 被调用之前 `self` 确实处于地址敏感
//! 状态，那么这就如同编译器自动调用了 [`Pin::get_unchecked_mut`]。
//!
//! 在纯安全代码中这永远不会引发问题，因为创建一个指向“带有地址敏感状态（因此没有实现 `Unpin`）的
//! 类型”的固定指针需要 `unsafe`；但重要的是要注意：选择利用与固定相关的保证来论证你类型实现的
//! 有效性，对该类型的 [`Drop`][Drop] 实现也有相应的影响：如果你类型的某个元素本可能被固定过，那么
//! 你就必须把 [`Drop`][Drop] 当作隐式地接受 <code>self: [Pin]<[&mut] Self></code> 来对待。
//!
//! 你应当按如下方式实现 [`Drop`]：
//!
//! ```rust,no_run
//! # use std::pin::Pin;
//! # struct Type;
//! impl Drop for Type {
//!     fn drop(&mut self) {
//!         // `new_unchecked` 没问题，因为我们知道这个值在被 drop 之后
//!         // 永远不会再被使用。
//!         inner_drop(unsafe { Pin::new_unchecked(self)});
//!         fn inner_drop(this: Pin<&mut Type>) {
//!             // 真正的 drop 代码放在这里。
//!         }
//!     }
//! }
//! ```
//!
//! 函数 `inner_drop` 拥有 [`drop`] 在这种情形下*应当*拥有的签名。这能确保你不会意外地以一种与
//! 固定不变量相冲突的方式使用 `self`/`this`。
//!
//! 此外，如果你的类型是 [`#[repr(packed)]`][packed]，编译器为了能够 drop 它们，会自动把字段挪来
//! 挪去。对于碰巧被充分对齐的字段，它甚至也可能这么做。因此，你不能对一个
//! [`#[repr(packed)]`][packed] 类型使用固定。
//!
//! ### 为将被用作 [`Pin`] 固定指针的指针类型实现 [`Drop`]
//!
//! 还应进一步注意：创建一个类型 `Ptr` 的固定指针*也*会对 `Ptr` 类型必须如何实现 [`Drop`]
//!（以及 [`Deref`] 和 [`DerefMut`]）带来影响！在实现一个可能被用作固定指针的指针类型时，你同样
//! 必须采取上文所述的相同小心，在 [`Drop`]、[`Deref`] 或 [`DerefMut`] 的实现期间，不要从被指对象
//! 中*移动*出值或以其他方式让它失效。
//!
//! ## “赋值（Assigning）”被固定的数据
//!
//! 尽管出于“复用被固定对象的内存是无效的”这同一个原因，一般来说通过一个 [`Pin<Ptr>`] 交换数据或
//! 赋值是无效的，但如果针对正在被修改的那个确切数据结构的需要、加以特别小心地实现，这是可以有效
//! 完成的。例如，那个赋值函数必须知道如何更新对被固定地址的所有使用（以及满足该类型有效性所需的
//! 任何其他不变量）。对于 [`Unmovable`]（来自上面的例子），我们可以写一个这样的赋值函数：
//!
//! ```
//! # use std::pin::Pin;
//! # use std::marker::PhantomPinned;
//! # use std::ptr::NonNull;
//! # struct Unmovable {
//! #     data: [u8; 64],
//! #     slice: NonNull<[u8]>,
//! #     _pin: PhantomPinned,
//! # }
//! #
//! impl Unmovable {
//!     // 把 `src` 的内容复制进 `self`，并在此过程中修正自指针（self-pointer）。
//!     fn assign(self: Pin<&mut Self>, src: Pin<&mut Self>) {
//!         unsafe {
//!             let unpinned_self = Pin::into_inner_unchecked(self);
//!             let unpinned_src = Pin::into_inner_unchecked(src);
//!             *unpinned_self = Self {
//!                 data: unpinned_src.data,
//!                 slice: NonNull::from(&mut []),
//!                 _pin: PhantomPinned,
//!             };
//!
//!             let data_ptr = unpinned_src.data.as_ptr() as *const u8;
//!             let slice_ptr = unpinned_src.slice.as_ptr() as *const u8;
//!             let offset = slice_ptr.offset_from(data_ptr) as usize;
//!             let len = unpinned_src.slice.as_ptr().len();
//!
//!             unpinned_self.slice = NonNull::from(&mut unpinned_self.data[offset..offset+len]);
//!         }
//!     }
//! }
//! ```
//!
//! 尽管我们无法让编译器替我们完成这种赋值，但为可能需要它的类型编写这种专门的函数是可能的。
//!
//! 注意，通过 [`Pin::set()`] 以泛型方式经由一个 [`Pin<Ptr>`] 进行赋值*是*可能的。这不会违反任何
//! 保证，因为它会在赋新值之前先对被指对象值运行 [`drop`]。因此，[`drop`] 实现仍然有机会在原先那个
//! 被固定值的内存位置被覆写之前，对依赖它的那些值执行必要的通知。
//!
//! ## 投影（Projection）与结构化固定（Structural Pinning）
//! [structural-pinning]: self#projections-and-structural-pinning
//!
//! 对于普通的结构体，当调用方持有对整个结构体的借用时，我们很自然地想添加一些*投影*方法，让其能够
//! 借用该结构体的一个或多个内部字段：
//!
//! ```
//! # struct Field;
//! struct Struct {
//!     field: Field,
//!     // ...
//! }
//!
//! impl Struct {
//!     fn field(&mut self) -> &mut Field { &mut self.field }
//! }
//! ```
//!
//! 在处理地址敏感类型时，这些函数的签名应当是什么样并不显而易见。如果 `field` 接受
//! <code>self: [Pin]<[&mut Struct][&mut]></code>，它应当返回 [`&mut Field`] 还是
//! <code>[Pin]<[`&mut Field`]></code>？这个问题在 `enum` 以及像 [`Vec<T>`]、[`Box<T>`] 和
//! [`RefCell<T>`] 这样的包装类型中也会出现。（这个问题对共享引用同样适用，但为便于说明，我们将
//! 考察更常见的可变引用情形。）
//!
//! 事实证明，“投影”应当产出哪种类型，是由 `Struct` 的作者来决定的。不过这个选择必须是*一致的*：
//! 如果一个 pin 在某处被投影到了某个字段，那么在别处很可能就不应该在不投影 pin 的情况下暴露它。
//!
//! 作为一个数据结构的作者，你可以为每个字段决定固定是否“传播（propagate）”到该字段。会传播的固定
//! 也被称为“结构化的（structural）”，因为它跟随该类型的结构。
//!
//! 这个选择取决于：为了让你的 [`unsafe`] 代码工作，你需要从该字段获得什么保证。如果该字段本身是
//! 地址敏感的，或者参与了父结构体的地址敏感性，那么它就需要被结构化固定。
//!
//! 一个有用的判断方法是：如果消费 <code>[Pin]\<[&mut Struct][&mut]></code> 的 [`unsafe`] 代码还
//! 需要留意该字段本身的地址，这或许就是该字段被结构化固定的证据。遗憾的是，并没有一成不变的硬性
//! 规则。
//!
//! ### 选择固定对 `field` *不是*结构化的……
//!
//! 尽管有违直觉，但这往往是更轻松的选择：如果你不暴露一个 <code>[Pin]<[&mut] Field></code>，你就
//! 无需小心防范其他代码从那个字段中 move 出值，你只需确保自己永远不创建指向那个字段的固定引用即可。
//! 当然，这也意味着如果你判定某个字段没有结构化固定，那么你就绝不能编写（无效地）假定该字段*确实*
//! 被结构化固定的 [`unsafe`] 代码！
//!
//! 没有结构化固定的字段，可以拥有一个把 <code>[Pin]<[&mut] Struct></code> 转换为 [`&mut Field`]
//! 的投影方法：
//!
//! ```rust,no_run
//! # use std::pin::Pin;
//! # type Field = i32;
//! # struct Struct { field: Field }
//! impl Struct {
//!     fn field(self: Pin<&mut Self>) -> &mut Field {
//!         // 这没问题，因为 `field` 从不被视为被固定，因此我们无需
//!         // 为这个字段单独维护任何固定保证。当然，如果我们选择暴露
//!         // 这样的方法，就绝不能在其他地方假定这个字段*已经*被固定！
//!         unsafe { &mut self.get_unchecked_mut().field }
//!     }
//! }
//! ```
//!
//! 在这种情况下，即便 `field` 的类型并未实现 [`Unpin`]，你也可以为 `Struct` 编写
//! <code>impl [Unpin] for Struct {}</code>。原因是：我们已经明确选择不依赖 `field` 的固定保证，
//! 因此 `field` 自身的类型如何与固定交互，在它作为 `Struct` 字段被使用的语境中已经不再相关。
//!
//! ### 选择固定对 `field` *是*结构化的……
//!
//! 另一种选择，是决定固定对 `field` 是“结构化的”：也就是说，只要整个结构体被固定，
//! 这个字段也随之被固定。
//!
//! 这样就可以编写一个投影方法来创建 <code>[Pin]<[`&mut Field`]></code>，用这个返回类型见证
//! 该字段确实处于固定状态：
//!
//! ```rust,no_run
//! # use std::pin::Pin;
//! # type Field = i32;
//! # struct Struct { field: Field }
//! impl Struct {
//!     fn field(self: Pin<&mut Self>) -> Pin<&mut Field> {
//!         // 这没问题，因为当 `self` 被固定时 `field` 也被固定。
//!         unsafe { self.map_unchecked_mut(|s| &mut s.field) }
//!     }
//! }
//! ```
//!
//! 结构化固定附带了一些额外要求：
//!
//! 1.  *结构化 [`Unpin`]。* 一个结构体只有在它所有被结构化固定的字段也都是 [`Unpin`] 时，才可以是
//!     [`Unpin`]。这是 [`Unpin`] 的默认行为。然而，作为库作者，你有责任不去写出诸如
//!     <code>impl\<T> [Unpin] for Struct\<T> {}</code> 这样的东西、然后又提供一个把结构化固定提供
//!     给 `T` 某个内部字段（而它可能不是 [`Unpin`]）的方法！（添加*任何*投影操作都需要 unsafe
//!     代码，因此“[`Unpin`] 是一个安全 trait”这一事实并不破坏“只有当你使用 [`unsafe`] 时才需要
//!     担心这一切”的原则。）
//!
//! 2.  *固定析构（Pinned Destruction）。* 正如[上文][drop-impl]所讨论的，[`drop`] 接受
//!     [`&mut self`]，但该结构体（以及它的字段）可能此前已被固定。析构逻辑必须写成仿佛它的参数是
//!     <code>self: [Pin]\<[`&mut Self`]></code> 一样。
//!
//!     因此，该结构体*绝不能*是 [`#[repr(packed)]`][packed]。
//!
//! 3.  *结构化的销毁通知（Structural Notice of Destruction）。* 你必须维护
//!     [`Drop` 保证][drop-guarantee]：一旦你的结构体被固定，那么不调用那些被结构化固定字段的析构
//!     逻辑，就不能复用该结构体的存储。
//!
//!     这可能很棘手，正如 [`VecDeque<T>`] 所示：如果某个析构逻辑发生 panic，[`VecDeque<T>`] 的
//!     析构逻辑就可能没能对所有元素调用 [`drop`]。这违反了 [`Drop` 保证][drop-guarantee]，因为它
//!     会导致元素在其析构逻辑未被调用的情况下就被释放。
//!
//!     [`VecDeque<T>`] 没有固定投影，所以它的析构逻辑是健全的。如果它想提供这种结构化固定，它的
//!     析构逻辑就需要在任何一个析构逻辑发生 panic 时中止（abort）进程。
//!
//! 4.  当你的类型被固定时，你绝不能提供任何其他可能导致数据从那些结构化字段中被*移动*出去的操作。
//!     例如，如果该结构体含有一个 [`Option<T>`]，且存在一个类型为
//!     <code>fn([Pin]<[&mut Struct\<T>][&mut]>) -> [`Option<T>`]</code> 的、类似
//!     [`take`][Option::take] 的操作，那么这个操作就可以被用来把一个 `T` 从被固定的 `Struct<T>`
//!     中 move 出去——这意味着对于持有这块数据的那个字段，固定不能是结构化的。
//!
//!     作为一个“从被固定类型中移出数据”的更复杂例子，设想如果 [`RefCell<T>`] 有一个方法
//!     <code>fn get_pin_mut(self: [Pin]<[`&mut Self`]>) -> [Pin]<[`&mut T`]></code>。
//!     那么我们就可以做下面的事：
//!     ```compile_fail
//!     # use std::cell::RefCell;
//!     # use std::pin::Pin;
//!     fn exploit_ref_cell<T>(rc: Pin<&mut RefCell<T>>) {
//!         // 这里我们获得了对 `T` 的固定访问。
//!         let _: Pin<&mut T> = rc.as_mut().get_pin_mut();
//!
//!         // 而这里我们对同一份数据持有 `&mut T`。
//!         let shared: &RefCell<T> = rc.into_ref().get_ref();
//!         let borrow = shared.borrow_mut();
//!         let content = &mut *borrow;
//!     }
//!     ```
//!     这是灾难性的：它意味着我们可以先固定 [`RefCell<T>`] 的内容（用
//!     <code>[RefCell]::get_pin_mut</code>），然后再用我们随后得到的可变引用把那块内容移走。
//!
//! ### 结构化固定的示例
//!
//! 对于像 [`Vec<T>`] 这样的类型，两种可能性（结构化固定或不固定）都说得通。一个带有结构化固定的
//! [`Vec<T>`] 可以提供 `get_pin`/`get_pin_mut` 方法来获取指向元素的固定引用。然而，它就*不能*允许
//! 对一个被固定的 [`Vec<T>`] 调用 [`pop`][Vec::pop]，因为那会移动那些（被结构化固定的）内容！它也
//! 不能允许 [`push`][Vec::push]，因为那可能重新分配（reallocate），从而也移动那些内容。
//!
//! 一个不带结构化固定的 [`Vec<T>`] 可以 <code>impl\<T> [Unpin] for [`Vec<T>`]</code>，因为其内容
//! 从不被固定，而且 [`Vec<T>`] 本身也可以随意被移动。这时固定对该 vector 就根本没有任何影响了。
//!
//! 在标准库中，指针类型通常没有结构化固定，因此它们不提供固定投影。这正是为什么对所有 `T` 都有
//! <code>[`Box<T>`]: [Unpin]</code> 成立。对指针类型这样做是合理的，因为移动那个 [`Box<T>`] 实际上
//! 并不移动那个 `T`：即便 `T` 不是 [`Unpin`]，[`Box<T>`] 也可以随意移动（亦即 [`Unpin`]）。事实上，
//! 出于同样的原因，即便是 <code>[Pin]<[`Box<T>`]></code> 和 <code>[Pin]<[`&mut T`]></code> 它们
//! 自身也始终是 [`Unpin`]：它们的内容（那个 `T`）是被固定的，但指针本身可以在不移动被固定数据的
//! 前提下被移动。对于 [`Box<T>`] 和 <code>[Pin]<[`Box<T>`]></code> 来说，内容是否被固定与指针是否
//! 被固定是完全无关的，这意味着固定*不是*结构化的。
//!
//! 在实现一个 [`Future`] 组合子（combinator）时，对于嵌套的 future，你通常会需要结构化固定，因为
//! 你需要获取指向它们的固定（被 [`Pin`] 包装的）引用以调用 [`poll`]。但如果你的组合子含有任何其他
//! 无需被固定的数据，你可以让那些字段不结构化，从而即便你只持有 <code>[Pin]<[`&mut Self`]></code>
//!（例如在你自己的 [`poll`] 实现中），也能用可变引用自由地访问它们。
//!
//! [`&mut T`]: &mut
//! [`&mut self`]: &mut
//! [`&mut Self`]: &mut
//! [`&mut Field`]: &mut
//! [Deref]: crate::ops::Deref "ops::Deref"
//! [`Deref`]: crate::ops::Deref "ops::Deref"
//! [Target]: crate::ops::Deref::Target "ops::Deref::Target"
//! [`DerefMut`]: crate::ops::DerefMut "ops::DerefMut"
//! [`mem::swap`]: crate::mem::swap "mem::swap"
//! [`mem::forget`]: crate::mem::forget "mem::forget"
//! [ManuallyDrop]: crate::mem::ManuallyDrop "ManuallyDrop"
//! [RefCell]: crate::cell::RefCell "cell::RefCell"
//! [`drop`]: Drop::drop
//! [`ptr::write`]: crate::ptr::write "ptr::write"
//! [`Future`]: crate::future::Future "future::Future"
//! [drop-impl]: #drop-implementation
//! [drop-guarantee]: #drop-guarantee
//! [`poll`]: crate::future::Future::poll "future::Future::poll"
//! [&]: reference "shared reference"
//! [&mut]: reference "mutable reference"
//! [`unsafe`]: ../../std/keyword.unsafe.html "keyword unsafe"
//! [packed]: https://doc.rust-lang.org/nomicon/other-reprs.html#reprpacked
//! [`std::alloc`]: ../../std/alloc/index.html
//! [`Box<T>`]: ../../std/boxed/struct.Box.html
//! [Box]: ../../std/boxed/struct.Box.html "Box"
//! [`Box`]: ../../std/boxed/struct.Box.html "Box"
//! [`Rc<T>`]: ../../std/rc/struct.Rc.html
//! [Rc]: ../../std/rc/struct.Rc.html "rc::Rc"
//! [`Vec<T>`]: ../../std/vec/struct.Vec.html
//! [Vec]: ../../std/vec/struct.Vec.html "Vec"
//! [`Vec`]: ../../std/vec/struct.Vec.html "Vec"
//! [`Vec::set_len`]: ../../std/vec/struct.Vec.html#method.set_len "Vec::set_len"
//! [Vec::pop]: ../../std/vec/struct.Vec.html#method.pop "Vec::pop"
//! [Vec::push]: ../../std/vec/struct.Vec.html#method.push "Vec::push"
//! [`Vec::set_len`]: ../../std/vec/struct.Vec.html#method.set_len
//! [`VecDeque<T>`]: ../../std/collections/struct.VecDeque.html
//! [VecDeque]: ../../std/collections/struct.VecDeque.html "collections::VecDeque"
//! [`String`]: ../../std/string/struct.String.html "String"

#![stable(feature = "pin", since = "1.33.0")]

use crate::hash::{Hash, Hasher};
use crate::ops::{CoerceUnsized, Deref, DerefMut, DerefPure, DispatchFromDyn, LegacyReceiver};
#[allow(unused_imports)]
use crate::{
    cell::{RefCell, UnsafeCell},
    future::Future,
    marker::PhantomPinned,
    mem, ptr,
};
use crate::{cmp, fmt};

mod unsafe_pinned;

#[unstable(feature = "unsafe_pinned", issue = "125735")]
pub use self::unsafe_pinned::UnsafePinned;

/// 一个把它的被指对象就地固定的指针。
///
/// [`Pin`] 是对某种指针 `Ptr` 的包装，它让那个指针把它的被指对象值就地“固定”，从而阻止该指针所
/// 引用的值在内存中那个位置上被移动或以其他方式失效——除非它实现了 [`Unpin`]。
///
/// *关于固定的更深入探讨，参见 [`pin` 模块][`pin` module] 文档。*
///
/// ## 用 [`Pin<Ptr>`] 固定值
///
/// 为了固定一个值，我们把一个*指向该值的指针*（类型为某个 `Ptr`）包装进一个 [`Pin<Ptr>`]。
/// [`Pin<Ptr>`] 可以包装任意指针类型，从而形成一个承诺：那个 **pointee**（被指对象）不会被
/// *移动*或[以其他方式失效][subtle-details]。如果被指对象值的类型实现了 [`Unpin`]，我们就可以
/// 完全无视这些要求，并通过 [`Pin::new`] 直接把指向那个值的任意指针包装进 [`Pin`]。如果被指对象
/// 值的类型没有实现 [`Unpin`]，那么 Rust 就不会让我们直接使用 [`Pin::new`] 函数，我们将需要以
/// 下文讨论的某种更专门的方式来构造一个被 [`Pin`] 包装的指针。
///
/// 我们把这样一个被 [`Pin`] 包装的指针称为**固定指针（pinning pointer）**（或固定引用、固定
/// [`Box`] 等），因为正是它的存在，才把底层的被指对象就地固定：它就是那枚把数据牢牢钉在图钉板
///（即内存）上的隐喻意义的“图钉”。
///
/// 需要强调的是，[`Pin`] 中的东西并不是我们想要固定的那个值本身，而是一个指向那个值的指针！一个
/// [`Pin<Ptr>`] 并不固定那个 `Ptr`，而是固定该指针的***pointee**（被指对象）值*。
///
/// 最常见的、为健全性而需要与固定相关保证的一组类型，就是编译器为 `async fn` 返回值实现 [`Future`]
/// 而生成的状态机。这些编译器生成的 [`Future`] 可能含有自引用指针，这是 [`Pin`] 最常见的用例之一。
/// 关于这一点的更多细节见 [`pin` 模块][`pin` module] 文档，但这里只需说：它们需要固定所提供的保证
/// 才能被健全地实现。
///
/// 这一对 `async fn` 实现的要求意味着：[`Future`] trait 要求所有对 [`poll`] 的调用都使用一个
/// <code>self: [Pin]\<&mut Self></code> 参数，而不是通常的 `&mut self`。因此，当手动 poll 一个
/// future 时，你需要先固定它。
///
/// 你可能注意到，源自 `async fn` 的 [`Future`] 只占所有存在的 [`Future`] 中的一小部分，然而我们却
/// 不得不为了适配它们而修改*所有* [`Future`] 的 [`poll`] 签名。这很遗憾，但语言提供了一种途径来
/// 缓解这一 API 选择所带来的额外摩擦：[`Unpin`] trait。
///
/// Rust 中绝大多数类型都没有任何理由去关心自己是否被固定。这些类型实现 [`Unpin`] trait，它让该
/// 类型的所有值完全 opt-out（退出）与固定相关的保证。对于这些类型的值，通过用一个 [`Pin<Ptr>`]
/// 指向它来固定它，将没有任何实际效果。
///
/// 之所以存在这种区分，恰恰是为了让 [`Future::poll`] 这样的 API 能够对所有类型都接受一个
/// [`Pin<Ptr>`] 作为参数，同时只让那些真正关心固定保证的 [`Future`] 类型来承担这份人体工程学成本。
/// 对于大多数没有理由关心被固定、因而实现了 [`Unpin`] 的 [`Future`] 类型来说，那个
/// <code>[Pin]\<&mut Self></code> 的行为将与一个普通的 `&mut Self` 完全相同，允许直接访问底层值。
/// 只有那些*没有*实现 [`Unpin`] 的类型才会受到限制。
///
/// ### 固定一个实现了 [`Unpin`] 的类型的值
///
/// 如果你需要“固定”的那个值的类型实现了 [`Unpin`]，你可以通过调用 [`Pin::new`] 平凡地把任意
/// 指向该值的指针包装进一个 [`Pin`]。
///
/// ```
/// use std::pin::Pin;
///
/// // 创建一个实现了 `Unpin` 的类型的值
/// let mut unpin_future = std::future::ready(5);
///
/// // 通过创建一个指向它的固定可变引用来固定它（可以拿去 `poll` 了！）
/// let my_pinned_unpin_future: Pin<&mut _> = Pin::new(&mut unpin_future);
/// ```
///
/// ### 在一个 [`Box`] 内部固定一个值
///
/// 固定一个未实现 [`Unpin`] 的值，最简单也最灵活的方式，是把那个值放进一个 [`Box`]，然后通过把它
/// 包装进一个 [`Pin`] 来把那个 [`Box`] 变成一个“固定 [`Box`]”。你可以用 [`Box::pin`] 一步完成这
/// 两件事。让我们来看一个使用这一流程来固定“调用 `async fn` 所返回的 [`Future`]”的例子，正如上文
/// 所述，这是一个常见用例。
///
/// ```
/// use std::pin::Pin;
///
/// async fn add_one(x: u32) -> u32 {
///     x + 1
/// }
///
/// // 调用这个异步函数以取回一个 future
/// let fut = add_one(42);
///
/// // 把这个 future 固定在一个固定 box 内部
/// let pinned_fut: Pin<Box<_>> = Box::pin(fut);
/// ```
///
/// 如果你有一个已经被 box 起来的值，例如一个 [`Box<dyn Future>`][Box]，你可以用 [`Box::into_pin`]
/// 把那个值就地固定在它当前的内存地址上。
///
/// ```
/// use std::pin::Pin;
/// use std::future::Future;
///
/// async fn add_one(x: u32) -> u32 {
///     x + 1
/// }
///
/// fn boxed_add_one(x: u32) -> Box<dyn Future<Output = u32>> {
///     Box::new(add_one(x))
/// }
///
/// let boxed_fut = boxed_add_one(42);
///
/// // 把这个 future 固定在已有的 box 内部
/// let pinned_fut: Pin<Box<_>> = Box::into_pin(boxed_fut);
/// ```
///
/// 在其他标准库智能指针类型上也提供了类似的固定方法，例如 [`Rc`] 和 [`Arc`]。
///
/// ### 用 [`pin!`] 在栈上固定一个值
///
/// 在某些情形下，把一个未实现 [`Unpin`] 的值固定到它在栈上的位置是可取的、甚至是必需的（例如，在
/// 一个无法访问标准库或一般意义上的分配的 `#[no_std]` 上下文中）。使用 [`pin!`] 宏可以做到这一点。
/// 更多内容参见它的文档。
///
/// ## 布局与 ABI
///
/// [`Pin<Ptr>`] 保证拥有与 `Ptr` 相同的内存布局和 ABI[^noalias]。
///
/// [^noalias]: 关于 `Pin<&mut T>` 的别名（aliasing）语义是否应当不同于 `&mut T`，这里还有一些细微
/// 之处仍在决定之中，不过截至今天，上文所述是成立的。
///
/// [`pin!`]: crate::pin::pin "pin!"
/// [`Future`]: crate::future::Future "Future"
/// [`poll`]: crate::future::Future::poll "Future::poll"
/// [`Future::poll`]: crate::future::Future::poll "Future::poll"
/// [`pin` module]: self "pin module"
/// [`Rc`]: ../../std/rc/struct.Rc.html "Rc"
/// [`Arc`]: ../../std/sync/struct.Arc.html "Arc"
/// [Box]: ../../std/boxed/struct.Box.html "Box"
/// [`Box`]: ../../std/boxed/struct.Box.html "Box"
/// [`Box::pin`]: ../../std/boxed/struct.Box.html#method.pin "Box::pin"
/// [`Box::into_pin`]: ../../std/boxed/struct.Box.html#method.into_pin "Box::into_pin"
/// [subtle-details]: self#subtle-details-and-the-drop-guarantee "pin subtle details"
/// [`unsafe`]: ../../std/keyword.unsafe.html "keyword unsafe"
//
// 注意：下面的 `Clone` derive 会导致不健全，因为为可变引用实现 `Clone` 是可能的。
// 更多细节见 <https://internals.rust-lang.org/t/unsoundness-in-pin/11311>。
#[stable(feature = "pin", since = "1.33.0")]
#[lang = "pin"]
#[fundamental]
#[repr(transparent)]
#[rustc_pub_transparent]
#[derive(Copy, Clone)]
pub struct Pin<Ptr> {
    pointer: Ptr,
}

// 下面这些实现没有用 derive，是为了避免健全性问题。`&self.pointer` 不应当能被不受信任的 trait
// 实现所访问。
//
// 更多细节见 <https://internals.rust-lang.org/t/unsoundness-in-pin/11311/73>。

#[stable(feature = "pin_trait_impls", since = "1.41.0")]
impl<Ptr: Deref, Q: Deref> PartialEq<Pin<Q>> for Pin<Ptr>
where
    Ptr::Target: PartialEq<Q::Target>,
{
    fn eq(&self, other: &Pin<Q>) -> bool {
        Ptr::Target::eq(self, other)
    }

    fn ne(&self, other: &Pin<Q>) -> bool {
        Ptr::Target::ne(self, other)
    }
}

#[stable(feature = "pin_trait_impls", since = "1.41.0")]
impl<Ptr: Deref<Target: Eq>> Eq for Pin<Ptr> {}

#[stable(feature = "pin_trait_impls", since = "1.41.0")]
impl<Ptr: Deref, Q: Deref> PartialOrd<Pin<Q>> for Pin<Ptr>
where
    Ptr::Target: PartialOrd<Q::Target>,
{
    fn partial_cmp(&self, other: &Pin<Q>) -> Option<cmp::Ordering> {
        Ptr::Target::partial_cmp(self, other)
    }

    fn lt(&self, other: &Pin<Q>) -> bool {
        Ptr::Target::lt(self, other)
    }

    fn le(&self, other: &Pin<Q>) -> bool {
        Ptr::Target::le(self, other)
    }

    fn gt(&self, other: &Pin<Q>) -> bool {
        Ptr::Target::gt(self, other)
    }

    fn ge(&self, other: &Pin<Q>) -> bool {
        Ptr::Target::ge(self, other)
    }
}

#[stable(feature = "pin_trait_impls", since = "1.41.0")]
impl<Ptr: Deref<Target: Ord>> Ord for Pin<Ptr> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        Ptr::Target::cmp(self, other)
    }
}

#[stable(feature = "pin_trait_impls", since = "1.41.0")]
impl<Ptr: Deref<Target: Hash>> Hash for Pin<Ptr> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Ptr::Target::hash(self, state);
    }
}

impl<Ptr: Deref<Target: Unpin>> Pin<Ptr> {
    /// 围绕一个“指向某种实现了 [`Unpin`] 的类型的数据”的指针，构造一个新的 `Pin<Ptr>`。
    ///
    /// 与 `Pin::new_unchecked` 不同，此方法是安全的，因为指针 `Ptr` 解引用得到的是一个 [`Unpin`]
    /// 类型，这会取消固定保证。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::pin::Pin;
    ///
    /// let mut val: u8 = 5;
    ///
    /// // 既然 `val` 不在乎被移动，我们就可以安全地创建一个“外观（facade）” `Pin`，
    /// // 它将允许 `val` 参与到那些以 `Pin` 为约束的 API 中，而无需检查
    /// // 固定保证是否真的得到维护。
    /// let mut pinned: Pin<&mut u8> = Pin::new(&mut val);
    /// ```
    #[inline(always)]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    #[stable(feature = "pin", since = "1.33.0")]
    pub const fn new(pointer: Ptr) -> Pin<Ptr> {
        // SAFETY: 被指向的值是 `Unpin`，因此在固定方面没有任何要求。
        unsafe { Pin::new_unchecked(pointer) }
    }

    /// 解包（unwrap）这个 `Pin<Ptr>`，返回底层的指针。
    ///
    /// 安全地执行此操作要求这个固定指针所指向的数据实现 [`Unpin`]，这样我们在解包它时就可以忽略
    /// 固定不变量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::pin::Pin;
    ///
    /// let mut val: u8 = 5;
    /// let pinned: Pin<&mut u8> = Pin::new(&mut val);
    ///
    /// // 解包这个 pin，以取回指向该值的底层可变引用。我们之所以能这样做，
    /// // 是因为 `val` 不在乎被移动，所以那个 `Pin` 本来也只是一个“外观”而已。
    /// let r = Pin::into_inner(pinned);
    /// assert_eq!(*r, 5);
    /// ```
    #[inline(always)]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    #[stable(feature = "pin_into_inner", since = "1.39.0")]
    pub const fn into_inner(pin: Pin<Ptr>) -> Ptr {
        pin.pointer
    }
}

impl<Ptr: Deref> Pin<Ptr> {
    /// 围绕一个“指向某种可能实现、也可能未实现 [`Unpin`] 的类型的数据”的引用，构造一个新的
    /// `Pin<Ptr>`。
    ///
    /// 如果 `pointer` 解引用得到的是一个 [`Unpin`] 类型，则应改用 [`Pin::new`]。
    ///
    /// # 安全性(Safety）
    ///
    /// 此构造器是 unsafe 的，因为我们无法保证 `pointer` 所指向的数据是被固定的。固定一个值，其
    /// 核心含义就是作出这样的保证：该值的数据在它被 drop 之前，既不会被移动、其存储也不会失效。
    /// 关于固定的更深入解释，参见 [`pin` 模块文档][`pin` module docs]。
    ///
    /// 如果构造这个 `Pin<Ptr>` 的调用方没有确保 `Ptr` 所指向的数据是被固定的，那就是对 API
    /// 契约的违反，并可能在后续（甚至是安全的）操作中导致未定义行为。
    ///
    /// 通过使用此方法，你同时也在对 `Ptr` 的 [`Deref`]、[`DerefMut`] 和 [`Drop`] 实现（如果存在的
    /// 话）作出承诺。最重要的是，它们绝不能从它们的 `self` 参数中 move 出值：`Pin::as_mut` 和
    /// `Pin::as_ref` 会*在指针类型 `Ptr` 上*调用 `DerefMut::deref_mut` 和 `Deref::deref`，并期望
    /// 这些方法维护固定不变量。此外，通过调用此方法，你承诺 `Ptr` 解引用得到的那个引用不会再被
    /// move 出值；尤其是，绝不能有可能获取到一个 `&mut Ptr::Target` 然后从那个引用中 move 出值
    ///（例如使用 [`mem::swap`]）。
    ///
    /// 举例来说，对一个 `&'a mut T` 调用 `Pin::new_unchecked` 是 unsafe 的，因为：虽然你能在给定的
    /// 生命周期 `'a` 内把它固定，但一旦 `'a` 结束，你就无法控制它是否仍然保持被固定，因此无法维护
    /// “一个值一旦被固定，就保持被固定直到它被 drop”这一保证：
    ///
    /// ```
    /// use std::mem;
    /// use std::pin::Pin;
    ///
    /// fn move_pinned_ref<T>(mut a: T, mut b: T) {
    ///     unsafe {
    ///         let p: Pin<&mut T> = Pin::new_unchecked(&mut a);
    ///         // 这本应意味着被指对象 `a` 永远不能再移动了。
    ///     }
    ///     mem::swap(&mut a, &mut b); // 后续路径上潜在的 UB ⚠️
    ///     // `a` 的地址变成了 `b` 的栈槽位，所以尽管我们此前已经固定了 `a`，它还是被移动了！
    ///     // 我们违反了固定 API 契约。
    /// }
    /// ```
    /// 一个值一旦被固定，就必须保持被固定直到它被 drop（除非它的类型实现了 `Unpin`）。由于
    /// `Pin<&mut T>` 并不拥有该值，drop 这个 `Pin` 不会 drop 那个值，也不会终止固定契约。所以，
    /// 在 drop 那个 `Pin<&mut T>` 之后再移动该值，仍然是对 API 契约的违反。
    ///
    /// 类似地，对一个 `Rc<T>` 调用 `Pin::new_unchecked` 是 unsafe 的，因为可能存在指向同一份数据
    /// 的别名（alias），而它们并不受固定限制的约束：
    /// ```
    /// use std::rc::Rc;
    /// use std::pin::Pin;
    ///
    /// fn move_pinned_rc<T>(mut x: Rc<T>) {
    ///     // 这本应意味着被指对象永远不能再移动了。
    ///     let pin = unsafe { Pin::new_unchecked(Rc::clone(&x)) };
    ///     {
    ///         let p: Pin<&T> = pin.as_ref();
    ///         // ...
    ///     }
    ///     drop(pin);
    ///
    ///     let content = Rc::get_mut(&mut x).unwrap(); // 后续路径上潜在的 UB ⚠️
    ///     // 现在，如果 `x` 是唯一的引用，那么我们就持有了一个指向“我们上面固定过的数据”的
    ///     // 可变引用，我们可以像前一个例子里看到的那样用它来移动那份数据。
    ///     // 我们违反了固定 API 契约。
    /// }
    /// ```
    ///
    /// ## 闭包捕获（capture）的固定
    ///
    /// 在闭包中使用 `Pin::new_unchecked` 时需要格外小心：`Pin::new_unchecked(&mut var)`（其中
    /// `var` 是一个按值（move）捕获的闭包捕获变量）隐式地作出了这样的承诺——闭包本身是被固定的，
    /// 并且对这个闭包捕获变量的*所有*使用都尊重那个固定。
    /// ```
    /// use std::pin::Pin;
    /// use std::task::Context;
    /// use std::future::Future;
    ///
    /// fn move_pinned_closure(mut x: impl Future, cx: &mut Context<'_>) {
    ///     // 创建一个 move `x` 的闭包，然后在其内部以固定的方式使用它。
    ///     let mut closure = move || unsafe {
    ///         let _ignore = Pin::new_unchecked(&mut x).poll(cx);
    ///     };
    ///     // 调用这个闭包，于是那个 future 可以假定它已被固定。
    ///     closure();
    ///     // 把这个闭包移动到别处。这也移动了 `x`！
    ///     let mut moved = closure;
    ///     // 再次调用它意味着我们从两个不同的位置 poll 了这个 future，
    ///     // 从而违反了固定 API 契约。
    ///     moved(); // 潜在的 UB ⚠️
    /// }
    /// ```
    /// 当把一个闭包传给另一个 API 时，它可能在任意时刻移动这个闭包，因此只有当该 API 明确文档说明
    /// 该闭包是被固定的时，才可以对闭包捕获变量使用 `Pin::new_unchecked`。
    ///
    /// 更好的替代方案是避免所有这些麻烦，转而在外层函数中完成固定（这里使用
    /// [`pin!`][crate::pin::pin] 宏）：
    /// ```
    /// use std::pin::pin;
    /// use std::task::Context;
    /// use std::future::Future;
    ///
    /// fn move_pinned_closure(mut x: impl Future, cx: &mut Context<'_>) {
    ///     let mut x = pin!(x);
    ///     // 创建一个捕获 `x: Pin<&mut _>` 的闭包，它可以被安全地移动。
    ///     let mut closure = move || {
    ///         let _ignore = x.as_mut().poll(cx);
    ///     };
    ///     // 调用这个闭包，于是那个 future 可以假定它已被固定。
    ///     closure();
    ///     // 把这个闭包移动到别处。
    ///     let mut moved = closure;
    ///     // 在这里再次调用它没问题（除了我们可能在 poll 一个已经返回了
    ///     // `Poll::Ready` 的 future，但那是另一个单独的问题）。
    ///     moved();
    /// }
    /// ```
    ///
    /// [`mem::swap`]: crate::mem::swap
    /// [`pin` module docs]: self
    #[lang = "new_unchecked"]
    #[inline(always)]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    #[stable(feature = "pin", since = "1.33.0")]
    pub const unsafe fn new_unchecked(pointer: Ptr) -> Pin<Ptr> {
        Pin { pointer }
    }

    /// 获取一个指向这个 [`Pin`] 所指向的被固定值的共享引用。
    ///
    /// 这是一个从 `&Pin<Pointer<T>>` 得到 `Pin<&T>` 的泛型方法。它是安全的，因为作为
    /// `Pin::new_unchecked` 契约的一部分，被指对象在 `Pin<Pointer<T>>` 被创建之后就不能再移动。
    /// `Pointer::Deref` 的“恶意”实现同样被 `Pin::new_unchecked` 的契约所排除。
    #[stable(feature = "pin", since = "1.33.0")]
    #[inline(always)]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn as_ref(&self) -> Pin<&Ptr::Target>
    where
        Ptr: [const] Deref,
    {
        // SAFETY: 见此函数的文档
        unsafe { Pin::new_unchecked(&*self.pointer) }
    }
}

// 这些方法之所以放在一个 `Ptr: DerefMut` 的 impl 块里，关乎 semver 稳定性。
// 目前，比如对一个 `Pin<&T>` 调用 `.set()` 时，编译器看到 `Ptr: DerefMut` 不成立，于是转而去
// 检查 `T` 上是否有 `.set()` 方法。但是，如果把 `where Ptr: DerefMut` 约束移到方法上，rustc 就会
// 把这个 impl 块视为一个有效候选，并在它看到方法上的那个约束（不成立）时，不再继续检查其他候选。
impl<Ptr: DerefMut> Pin<Ptr> {
    /// 获取一个指向这个 `Pin<Ptr>` 所指向的被固定值的可变引用。
    ///
    /// 这是一个从 `&mut Pin<Pointer<T>>` 得到 `Pin<&mut T>` 的泛型方法。它是安全的，因为作为
    /// `Pin::new_unchecked` 契约的一部分，被指对象在 `Pin<Pointer<T>>` 被创建之后就不能再移动。
    /// `Pointer::DerefMut` 的“恶意”实现同样被 `Pin::new_unchecked` 的契约所排除。
    ///
    /// 当对“会消耗固定指针的函数”进行多次调用时，此方法很有用。
    ///
    /// # 示例
    /// ```
    /// use std::pin::Pin;
    ///
    /// # struct Type {}
    /// impl Type {
    ///     fn method(self: Pin<&mut Self>) {
    ///         // 做点什么
    ///     }
    ///
    ///     fn call_method_twice(mut self: Pin<&mut Self>) {
    ///         // `method` 会消耗 `self`，所以通过 `as_mut` 对 `Pin<&mut Self>` 进行重借用。
    ///         self.as_mut().method();
    ///         self.as_mut().method();
    ///     }
    /// }
    /// ```
    #[stable(feature = "pin", since = "1.33.0")]
    #[inline(always)]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn as_mut(&mut self) -> Pin<&mut Ptr::Target>
    where
        Ptr: [const] DerefMut,
    {
        // SAFETY: 见此函数的文档
        unsafe { Pin::new_unchecked(&mut *self.pointer) }
    }

    /// 从这个嵌套的 `Pin` 指针获取一个指向底层被固定值的 `Pin<&mut T>`。
    ///
    /// 这是一个从 `Pin<&mut Pin<Pointer<T>>>` 得到 `Pin<&mut T>` 的泛型方法。它是安全的，因为
    /// 一个 `Pin<Pointer<T>>` 的存在确保了被指对象 `T` 在将来不能移动，而此方法也不会使该被指对象
    /// 能够移动。`Ptr::DerefMut` 的“恶意”实现同样被 `Pin::new_unchecked` 的契约所排除。
    #[stable(feature = "pin_deref_mut", since = "1.84.0")]
    #[must_use = "`self` will be dropped if the result is not used"]
    #[inline(always)]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn as_deref_mut(self: Pin<&mut Self>) -> Pin<&mut Ptr::Target>
    where
        Ptr: [const] DerefMut,
    {
        // SAFETY: 我们在这里所断言的是，从
        //
        //     Pin<&mut Pin<Ptr>>
        //
        // 转换到
        //
        //     Pin<&mut Ptr::Target>
        //
        // 是安全的。
        //
        // 为使其成立，我们需要确保两件事：
        //
        // 1) 一旦我们交出一个 `Pin<&mut Ptr::Target>`，就不会再交出一个 `&mut Ptr::Target`。
        // 2) 通过交出一个 `Pin<&mut Ptr::Target>`，我们不会冒着违反
        // `Pin<&mut Pin<Ptr>>` 的风险。
        //
        // `Pin<Ptr>` 的存在足以保证第 1 点：既然我们已经有了一个 `Pin<Ptr>`，它必定已经维护了固定
        // 保证，这必定意味着 `Pin<&mut Ptr::Target>` 也维护了固定保证，因为 `Pin::as_mut` 是安全
        // 的。我们无需依赖“`Ptr` *也*被固定”这一事实。
        //
        // 对于第 2 点，我们需要确保拿到 `Pin<&mut Ptr::Target>` 的代码不能导致那个 `Pin<Ptr>` 移动。
        // 这是不可能的，因为 `Pin<&mut Ptr::Target>` 不再保留对 `Ptr` 本身（更别说 `Pin<Ptr>`）的
        // 任何访问。
        unsafe { self.get_unchecked_mut() }.as_mut()
    }

    /// 为这个 `Pin<Ptr>` 所指向的内存位置赋一个新值。
    ///
    /// 这会覆写被固定的数据，但这没问题：原先那个被固定值的析构逻辑会在被覆写之前先运行，而新值
    /// 也是同一类型的一个有效值，因此没有任何固定不变量被违反。关于这如何维护固定不变量的更多信息，
    /// 参见 [`pin` 模块文档][subtle-details]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::pin::Pin;
    ///
    /// let mut val: u8 = 5;
    /// let mut pinned: Pin<&mut u8> = Pin::new(&mut val);
    /// println!("{}", pinned); // 5
    /// pinned.set(10);
    /// println!("{}", pinned); // 10
    /// ```
    ///
    /// [subtle-details]: self#subtle-details-and-the-drop-guarantee
    #[stable(feature = "pin", since = "1.33.0")]
    #[inline(always)]
    pub fn set(&mut self, value: Ptr::Target)
    where
        Ptr::Target: Sized,
    {
        *(self.pointer) = value;
    }
}

impl<Ptr: Deref> Pin<Ptr> {
    /// 解包（unwrap）这个 `Pin<Ptr>`，返回底层的 `Ptr`。
    ///
    /// # 安全性(Safety）
    ///
    /// 此函数是 unsafe 的。你必须保证在调用此函数之后，你会继续把指针 `Ptr` 当作被固定来对待，
    /// 以便 `Pin` 类型上的不变量能够得到维护。如果使用所得 `Ptr` 的代码没有继续维护固定不变量，
    /// 那就是对 API 契约的违反，并可能在后续（安全的）操作中导致未定义行为。
    ///
    /// 注意，你必须能够保证：`Ptr` 所指向的数据会一直被当作被固定来对待，直到它的 `drop` 处理函数
    /// 完成为止！
    ///
    /// *更多信息，参见 [`pin` 模块文档][self]*
    ///
    /// 如果底层数据是 [`Unpin`]，则应改用 [`Pin::into_inner`]。
    #[inline(always)]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    #[stable(feature = "pin_into_inner", since = "1.39.0")]
    pub const unsafe fn into_inner_unchecked(pin: Pin<Ptr>) -> Ptr {
        pin.pointer
    }
}

impl<'a, T: ?Sized> Pin<&'a T> {
    /// 通过映射（map）内部值来构造一个新的 pin。
    ///
    /// 例如，如果你想获取某个东西的某个字段的 `Pin`，你可以用它在一行代码里就获得对那个字段的
    /// 访问。然而，这些“固定投影（pinning projection）”有几个陷阱；关于这一话题的更多细节参见
    /// [`pin` 模块][`pin` module] 文档。
    ///
    /// # 安全性(Safety）
    ///
    /// 此函数是 unsafe 的。你必须保证：只要参数值不移动，你返回的数据就不会移动（例如，因为它是
    /// 那个值的某个字段），并且你也不会从你在内部函数中收到的那个参数中 move 出值。
    ///
    /// [`pin` module]: self#projections-and-structural-pinning
    #[stable(feature = "pin", since = "1.33.0")]
    pub unsafe fn map_unchecked<U, F>(self, func: F) -> Pin<&'a U>
    where
        U: ?Sized,
        F: FnOnce(&T) -> &U,
    {
        let pointer = &*self.pointer;
        let new_pointer = func(pointer);

        // SAFETY: `new_unchecked` 的安全契约必须由调用方维护。
        unsafe { Pin::new_unchecked(new_pointer) }
    }

    /// 从一个 pin 中取出一个共享引用。
    ///
    /// 这是安全的，因为从一个共享引用中 move 出值是不可能的。这里看起来似乎会有一个关于内部可变性
    /// 的问题：事实上，从一个 `&RefCell<T>` 中 move 出一个 `T` *确实*是可能的。然而，只要不同时存在
    /// 一个指向 `RefCell` 内部那个 `T` 的 `Pin<&T>`，这就不是问题，而 `RefCell<T>` 也不会让你获取
    /// 一个指向其内容的 `Pin<&T>` 指针。更多细节参见关于[“固定投影”]["pinning projections"]的讨论。
    ///
    /// 注意：`Pin` 也实现了到目标（target）的 `Deref`，可以用它来访问内部值。然而，`Deref` 只提供
    /// 一个其存活时间与对 `Pin` 的借用一样长（而非 `Pin` 中所含引用的生命周期）的引用。此方法允许
    /// 把 `Pin` 转换为一个生命周期与它所包装的引用相同的引用。
    ///
    /// ["pinning projections"]: self#projections-and-structural-pinning
    #[inline(always)]
    #[must_use]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    #[stable(feature = "pin", since = "1.33.0")]
    pub const fn get_ref(self) -> &'a T {
        self.pointer
    }
}

impl<'a, T: ?Sized> Pin<&'a mut T> {
    /// 把这个 `Pin<&mut T>` 转换为一个生命周期相同的 `Pin<&T>`。
    #[inline(always)]
    #[must_use = "`self` will be dropped if the result is not used"]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    #[stable(feature = "pin", since = "1.33.0")]
    pub const fn into_ref(self) -> Pin<&'a T> {
        Pin { pointer: self.pointer }
    }

    /// 获取一个指向这个 `Pin` 内部数据的可变引用。
    ///
    /// 这要求这个 `Pin` 内部的数据是 `Unpin`。
    ///
    /// 注意：`Pin` 也实现了到该数据的 `DerefMut`，可以用它来访问内部值。然而，`DerefMut` 只提供
    /// 一个其存活时间与对 `Pin` 的借用一样长（而非 `Pin` 自身的生命周期）的引用。此方法允许把
    /// `Pin` 转换为一个生命周期与原始 `Pin` 相同的引用。
    #[inline(always)]
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "pin", since = "1.33.0")]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    pub const fn get_mut(self) -> &'a mut T
    where
        T: Unpin,
    {
        self.pointer
    }

    /// 获取一个指向这个 `Pin` 内部数据的可变引用。
    ///
    /// # 安全性(Safety）
    ///
    /// 此函数是 unsafe 的。你必须保证你绝不会把数据从你调用此函数时收到的那个可变引用中 move 出去，
    /// 以便 `Pin` 类型上的不变量能够得到维护。
    ///
    /// 如果底层数据是 `Unpin`，则应改用 `Pin::get_mut`。
    #[inline(always)]
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "pin", since = "1.33.0")]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    pub const unsafe fn get_unchecked_mut(self) -> &'a mut T {
        self.pointer
    }

    /// 通过映射（map）内部值来构造一个新的 pin。
    ///
    /// 例如，如果你想获取某个东西的某个字段的 `Pin`，你可以用它在一行代码里就获得对那个字段的
    /// 访问。然而，这些“固定投影（pinning projection）”有几个陷阱；关于这一话题的更多细节参见
    /// [`pin` 模块][`pin` module] 文档。
    ///
    /// # 安全性(Safety）
    ///
    /// 此函数是 unsafe 的。你必须保证：只要参数值不移动，你返回的数据就不会移动（例如，因为它是
    /// 那个值的某个字段），并且你也不会从你在内部函数中收到的那个参数中 move 出值。
    ///
    /// [`pin` module]: self#projections-and-structural-pinning
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "pin", since = "1.33.0")]
    pub unsafe fn map_unchecked_mut<U, F>(self, func: F) -> Pin<&'a mut U>
    where
        U: ?Sized,
        F: FnOnce(&mut T) -> &mut U,
    {
        // SAFETY: 调用方有责任不把值从这个引用中 move 出去。
        let pointer = unsafe { Pin::get_unchecked_mut(self) };
        let new_pointer = func(pointer);
        // SAFETY: 既然 `this` 的值保证未被 move 出去，那么这次对 `new_unchecked` 的调用就是安全的。
        unsafe { Pin::new_unchecked(new_pointer) }
    }
}

impl<T: ?Sized> Pin<&'static T> {
    /// 从一个 `&'static` 引用获取一个固定引用。
    ///
    /// 这是安全的，因为 `T` 是在 `'static` 生命周期内被不可变借用的，而该生命周期永不结束。
    #[stable(feature = "pin_static_ref", since = "1.61.0")]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    pub const fn static_ref(r: &'static T) -> Pin<&'static T> {
        // SAFETY: 'static 借用保证了该数据在被 drop 之前（而它永远不会被 drop）既不会
        // 被 move 也不会失效。
        unsafe { Pin::new_unchecked(r) }
    }
}

impl<T: ?Sized> Pin<&'static mut T> {
    /// 从一个静态可变引用获取一个固定可变引用。
    ///
    /// 这是安全的，因为 `T` 是在 `'static` 生命周期内被借用的，而该生命周期永不结束。
    #[stable(feature = "pin_static_ref", since = "1.61.0")]
    #[rustc_const_stable(feature = "const_pin", since = "1.84.0")]
    pub const fn static_mut(r: &'static mut T) -> Pin<&'static mut T> {
        // SAFETY: 'static 借用保证了该数据在被 drop 之前（而它永远不会被 drop）既不会
        // 被 move 也不会失效。
        unsafe { Pin::new_unchecked(r) }
    }
}

#[stable(feature = "pin", since = "1.33.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<Ptr: [const] Deref> const Deref for Pin<Ptr> {
    type Target = Ptr::Target;
    fn deref(&self) -> &Ptr::Target {
        Pin::get_ref(Pin::as_ref(self))
    }
}

mod helper {
    /// 用于阻止下游 crate 为 `Pin` 实现 `DerefMut` 的辅助物。
    ///
    /// `Pin` 类型实现了 unsafe trait `PinCoerceUnsized`，它本质上要求该类型没有一个恶意的 `Deref`
    /// 或 `DerefMut` 实现。然而，如果没有这个辅助模块，下游 crate 就能够写出
    /// `impl DerefMut for Pin<LocalType>`，只要它不与 stdlib 提供的实现重叠即可。这是因为 `Pin` 是
    /// `#[fundamental]`，所以 stdlib 承诺永远不为 `Pin` 实现它今天还没实现的 trait。
    ///
    /// 然而，这是有问题的。下游 crate 可能为 `Pin<&LocalType>` 实现 `DerefMut`，而且它们可能怀着
    /// 恶意这么做。为了阻止这一点，`Pin` 的实现把任务委托给了这个辅助模块。由于 `helper::Pin` 不是
    /// `#[fundamental]`，孤儿规则（orphan rules）会假定 stdlib 将来可能为 `helper::Pin<&_>` 实现
    /// `helper::DerefMut`。正因如此，下游 crate 就再也无法为 `Pin<&_>` 提供 `DerefMut` 的实现了，
    /// 因为它可能与某个 trait 实现重叠——而根据孤儿规则，stdlib 在未来某个版本中引入那个实现并不算
    /// 破坏性变更。
    ///
    /// 此项所修复的 issue 见 <https://github.com/rust-lang/rust/issues/85099>。
    #[repr(transparent)]
    #[unstable(feature = "pin_derefmut_internals", issue = "none")]
    #[allow(missing_debug_implementations)]
    pub struct PinHelper<Ptr> {
        pointer: Ptr,
    }

    #[unstable(feature = "pin_derefmut_internals", issue = "none")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    #[rustc_diagnostic_item = "PinDerefMutHelper"]
    pub const trait PinDerefMutHelper {
        type Target: ?Sized;
        fn deref_mut(&mut self) -> &mut Self::Target;
    }

    #[unstable(feature = "pin_derefmut_internals", issue = "none")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    impl<Ptr: [const] super::DerefMut> const PinDerefMutHelper for PinHelper<Ptr>
    where
        Ptr::Target: crate::marker::Unpin,
    {
        type Target = Ptr::Target;

        #[inline(always)]
        fn deref_mut(&mut self) -> &mut Ptr::Target {
            &mut self.pointer
        }
    }
}

#[stable(feature = "pin", since = "1.33.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
#[cfg(not(doc))]
impl<Ptr> const DerefMut for Pin<Ptr>
where
    Ptr: [const] Deref,
    helper::PinHelper<Ptr>: [const] helper::PinDerefMutHelper<Target = Self::Target>,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Ptr::Target {
        // SAFETY: Pin 和 PinHelper 拥有相同的布局，所以这等价于 `&mut self.pointer`，
        // 而后者是安全的，因为 `Target: Unpin`。
        helper::PinDerefMutHelper::deref_mut(unsafe {
            &mut *(self as *mut Pin<Ptr> as *mut helper::PinHelper<Ptr>)
        })
    }
}

/// `Target` 类型被限制为 `Unpin` 类型，因为获取一个指向被固定值的可变引用是不安全的。
///
/// 出于健全性原因，即便 `T` 是一个未被此 impl 块覆盖的本地类型，为 `Pin<T>` 实现 `DerefMut` 也会
/// 被拒绝。（由于 `Pin` 是 [fundamental] 的，这类实现通常本应是可能的。）
///
/// [fundamental]: ../../reference/items/implementations.html#r-items.impl.trait.fundamental
#[stable(feature = "pin", since = "1.33.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
#[cfg(doc)]
impl<Ptr> const DerefMut for Pin<Ptr>
where
    Ptr: [const] DerefMut,
    <Ptr as Deref>::Target: Unpin,
{
    fn deref_mut(&mut self) -> &mut Ptr::Target {
        Pin::get_mut(Pin::as_mut(self))
    }
}

#[unstable(feature = "deref_pure_trait", issue = "87121")]
unsafe impl<Ptr: DerefPure> DerefPure for Pin<Ptr> {}

#[unstable(feature = "legacy_receiver_trait", issue = "none")]
impl<Ptr: LegacyReceiver> LegacyReceiver for Pin<Ptr> {}

#[stable(feature = "pin", since = "1.33.0")]
impl<Ptr: fmt::Debug> fmt::Debug for Pin<Ptr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.pointer, f)
    }
}

#[stable(feature = "pin", since = "1.33.0")]
impl<Ptr: fmt::Display> fmt::Display for Pin<Ptr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.pointer, f)
    }
}

#[stable(feature = "pin", since = "1.33.0")]
impl<Ptr: fmt::Pointer> fmt::Pointer for Pin<Ptr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.pointer, f)
    }
}

// 注意：这意味着任何允许“从一个实现了 `Deref<Target=impl !Unpin>` 的类型强转（coerce）到一个
// 实现了 `Deref<Target=Unpin>` 的类型”的 `CoerceUnsized` 实现都是不健全的。不过，任何这样的实现
// 多半出于其他原因也是不健全的，所以我们只需注意不要让这类实现进入 std。
#[stable(feature = "pin", since = "1.33.0")]
impl<Ptr, U> CoerceUnsized<Pin<U>> for Pin<Ptr>
where
    Ptr: CoerceUnsized<U> + PinCoerceUnsized,
    U: PinCoerceUnsized,
{
}

#[stable(feature = "pin", since = "1.33.0")]
impl<Ptr, U> DispatchFromDyn<Pin<U>> for Pin<Ptr>
where
    Ptr: DispatchFromDyn<U> + PinCoerceUnsized,
    U: PinCoerceUnsized,
{
}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
/// 表示“这是一个指针、或一个指针的包装器，且当其被指对象被固定时可以对它执行 unsizing（去定长
/// 化）”的 trait。
///
/// # 安全性(Safety）
///
/// 如果此类型实现了 `Deref`，那么 `deref` 和 `deref_mut` 所返回的具体类型（concrete type）在没有
/// 一次修改的情况下不得改变。以下操作不被视为修改：
///
/// * 移动该指针。
/// * 对该指针执行 unsizing 强转（coercion）。
/// * 用该指针执行动态分发（dynamic dispatch）。
/// * 对该指针调用 `deref` 或 `deref_mut`。
///
/// 一个 trait 对象的具体类型，就是其 vtable 所对应的类型。一个切片的具体类型，是一个元素类型相同、
/// 长度由元数据指定的数组。一个定长类型的具体类型，就是该类型本身。
pub unsafe trait PinCoerceUnsized {}

#[stable(feature = "pin", since = "1.33.0")]
unsafe impl<'a, T: ?Sized> PinCoerceUnsized for &'a T {}

#[stable(feature = "pin", since = "1.33.0")]
unsafe impl<'a, T: ?Sized> PinCoerceUnsized for &'a mut T {}

#[stable(feature = "pin", since = "1.33.0")]
unsafe impl<T: PinCoerceUnsized> PinCoerceUnsized for Pin<T> {}

#[stable(feature = "pin", since = "1.33.0")]
unsafe impl<T: ?Sized> PinCoerceUnsized for *const T {}

#[stable(feature = "pin", since = "1.33.0")]
unsafe impl<T: ?Sized> PinCoerceUnsized for *mut T {}

/// 通过把一个 `value: T` 局部地固定，构造一个 <code>[Pin]<[&mut] T></code>。
///
/// 与 [`Box::pin`] 不同，这不会创建新的堆分配。不过正如下文所述，该元素仍然可能最终落在堆上。
///
/// 此宏所执行的局部固定，通常被称为“栈（stack）”固定。在 `async` 上下文之外，局部变量确实会被
/// 存储在栈上。然而在 `async` 函数或代码块中，任何跨越 `.await` 点的局部变量都属于由 `Future` 所
/// 捕获的状态的一部分，并会使用那块存储。那块存储既可能在堆上，也可能在栈上。因此，“局部固定”是
/// 一个更准确的说法。
///
/// 如果给定值的类型没有实现 [`Unpin`]，那么此宏会以一种阻止移动的方式把该值固定在内存中。另一方面，
/// 如果该类型确实实现了 [`Unpin`]，那么 <code>[Pin]<[&mut] T></code> 的行为就与
/// <code>[&mut] T</code> 一样，而像 [`mem::replace()`][crate::mem::replace] 或
/// [`mem::take()`](crate::mem::take) 这样的操作将允许移动该值。详见
/// [`pin` 模块的 `Unpin` 一节][self#unpin]。
///
/// ## 示例
///
/// ### 基本用法
///
/// ```rust
/// # use core::marker::PhantomPinned as Foo;
/// use core::pin::{pin, Pin};
///
/// fn stuff(foo: Pin<&mut Foo>) {
///     // …
///     # let _ = foo;
/// }
///
/// let pinned_foo = pin!(Foo { /* … */ });
/// stuff(pinned_foo);
/// // 或者，直接地：
/// stuff(pin!(Foo { /* … */ }));
/// ```
///
/// ### 手动 poll 一个 `Future`（不带 `Unpin` 约束）
///
/// ```rust
/// use std::{
///     future::Future,
///     pin::pin,
///     task::{Context, Poll},
///     thread,
/// };
/// # use std::{sync::Arc, task::Wake, thread::Thread};
///
/// # /// 一个被调用时会唤醒当前线程的 waker。
/// # struct ThreadWaker(Thread);
/// #
/// # impl Wake for ThreadWaker {
/// #     fn wake(self: Arc<Self>) {
/// #         self.0.unpark();
/// #     }
/// # }
/// #
/// /// 把一个 future 运行至完成。
/// fn block_on<Fut: Future>(fut: Fut) -> Fut::Output {
///     let waker_that_unparks_thread = // …
///         # Arc::new(ThreadWaker(thread::current())).into();
///     let mut cx = Context::from_waker(&waker_that_unparks_thread);
///     // 固定这个 future，使其可以被 poll。
///     let mut pinned_fut = pin!(fut);
///     loop {
///         match pinned_fut.as_mut().poll(&mut cx) {
///             Poll::Pending => thread::park(),
///             Poll::Ready(res) => return res,
///         }
///     }
/// }
/// #
/// # assert_eq!(42, block_on(async { 42 }));
/// ```
///
/// ### 配合 `Coroutine`（协程）
///
/// ```rust
/// #![feature(coroutines)]
/// #![feature(coroutine_trait)]
/// use core::{
///     ops::{Coroutine, CoroutineState},
///     pin::pin,
/// };
///
/// fn coroutine_fn() -> impl Coroutine<Yield = usize, Return = ()> /* not Unpin */ {
///  // 允许该协程是自引用的（不是 `Unpin`）
///  // vvvvvv        以便局部变量可以跨越 yield 点。
///     #[coroutine] static || {
///         let foo = String::from("foo");
///         let foo_ref = &foo; // ------+
///         yield 0;                  // | <- 跨越了 yield 点！
///         println!("{foo_ref}"); // <--+
///         yield foo.len();
///     }
/// }
///
/// fn main() {
///     let mut coroutine = pin!(coroutine_fn());
///     match coroutine.as_mut().resume(()) {
///         CoroutineState::Yielded(0) => {},
///         _ => unreachable!(),
///     }
///     match coroutine.as_mut().resume(()) {
///         CoroutineState::Yielded(3) => {},
///         _ => unreachable!(),
///     }
///     match coroutine.resume(()) {
///         CoroutineState::Yielded(_) => unreachable!(),
///         CoroutineState::Complete(()) => {},
///     }
/// }
/// ```
///
/// ## 备注（Remarks）
///
/// 恰恰因为一个值被固定到了局部存储中，所得到的 <code>[Pin]<[&mut] T></code> 引用最终借用的是
/// 一个绑定到那个代码块的局部变量：它无法逃逸出该代码块。
///
/// 例如，下面这段代码无法通过编译：
///
/// ```rust,compile_fail
/// use core::pin::{pin, Pin};
/// # use core::{marker::PhantomPinned as Foo, mem::drop as stuff};
///
/// let x: Pin<&mut Foo> = {
///     let x: Pin<&mut Foo> = pin!(Foo { /* … */ });
///     x
/// }; // <- Foo 在此被 drop
/// stuff(x); // 错误：使用了已被 drop 的值
/// ```
///
/// <details><summary>错误信息</summary>
///
/// ```console
/// error[E0716]: temporary value dropped while borrowed
///   --> src/main.rs:9:28
///    |
/// 8  | let x: Pin<&mut Foo> = {
///    |     - borrow later stored here
/// 9  |     let x: Pin<&mut Foo> = pin!(Foo { /* … */ });
///    |                            ^^^^^^^^^^^^^^^^^^^^^ creates a temporary value which is freed while still in use
/// 10 |     x
/// 11 | }; // <- Foo is dropped
///    | - temporary value is freed at the end of this statement
///    |
///    = note: consider using a `let` binding to create a longer lived value
/// ```
///
/// </details>
///
/// 这使得 [`pin!`] **不适合在意图_返回_值时用来固定它们**。相反，期望的做法是：把该值以_未固定_的
/// 形式四处传递，直到它将被消耗（consume）的那一点，届时在那里用 [`pin!`] 把该值局部地固定才是
/// 有用、甚至合理的。
///
/// 如果你确实需要返回一个被固定的值，考虑改用 [`Box::pin`]。
///
/// 另一方面，使用 [`pin!`] 进行局部固定，很可能比用 [`Box::pin`] 固定到一块新的堆分配中更廉价。
/// 此外，由于无需分配器，[`pin!`] 是主要的非 `unsafe`、兼容 `#![no_std]` 的 [`Pin`] 构造器。
///
/// [`Box::pin`]: ../../std/boxed/struct.Box.html#method.pin
#[stable(feature = "pin_macro", since = "1.68.0")]
#[rustc_macro_transparency = "semiopaque"]
#[allow_internal_unstable(super_let)]
#[rustc_diagnostic_item = "pin_macro"]
// `super` 会被 rustfmt 移除
#[rustfmt::skip]
pub macro pin($value:expr $(,)?) {
    {
        super let mut pinned = $value;
        // SAFETY: 该值是被固定的：它就是上面那个局部变量，无法在此宏之外被命名。
        unsafe { $crate::pin::Pin::new_unchecked(&mut pinned) }
    }
}
