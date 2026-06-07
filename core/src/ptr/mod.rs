//! 通过裸指针（raw pointer）手动管理内存。
//!
//! *[另见指针原生类型的文档](pointer)。*
//!
//! 裸指针 `*const T`/`*mut T` 与引用 `&T`/`&mut T` 有本质区别，使用前务必理解：
//!
//! * 裸指针**不参与借用检查**：编译器不会追踪它的别名（aliasing）与生命周期，
//!   保证内存安全的责任完全落在调用方身上。
//! * 裸指针**可空（nullable）**：可以持有地址 0（即 [`null`]），而引用永远非空。
//! * 裸指针**可悬垂（dangling）**：可以指向已释放或从未分配的内存，而引用在其生命周期内
//!   必须始终指向有效内存。
//! * **解引用裸指针需要 `unsafe`**：`*ptr` 必须写在 `unsafe` 块里，因为编译器无法替你
//!   证明这次访问是安全的。
//!
//! # 安全性（Safety）
//!
//! 本模块中的许多函数接收裸指针作为参数，并对其进行读取或写入。要让这些操作安全，
//! 这些指针必须对所要进行的访问是 *valid*（有效的）。一个指针是否有效，取决于它被
//! 用于何种操作（读还是写），以及被访问内存的范围（即读/写多少字节）——单独问“这个
//! 指针有效吗”是没有意义的，必须问“这个指针对某个特定访问是否有效”。大多数函数使用
//! `*mut T` 和 `*const T` 只访问单个值，此时文档省略了访问大小，并隐式地假设它是
//! `size_of::<T>()` 字节。
//!
//! validity（有效性）的精确规则尚未最终确定。目前能提供的保证非常有限：
//!
//! * 对于大小为零的访问（[零大小访问][zst]），*任何*指针都是有效的，包括 [null]
//!   指针。以下各点只针对非零大小的访问。
//! * [null] 指针*永远*无效。
//! * 一个指针要有效，它“可解引用”（*dereferenceable*）是必要条件，但并不总是充分条件。
//!   指针的 [provenance]（来源）用于确定它派生自哪一块 [allocation]（分配）；当从该指针
//!   起、给定大小的内存范围完全落在那块分配的边界之内时，该指针就是可解引用的。注意，
//!   在 Rust 中，每个（栈上分配的）变量都被视为一块独立的分配。
//! * 本模块中各函数执行的所有访问，在线程间用于同步的 [atomic operations]（原子操作）
//!   意义上都是*非原子的*。这意味着：从不同线程并发地对同一位置执行两次访问是未定义
//!   行为，除非这两次访问都只读取内存。注意这显式地包括 [`read_volatile`] 和
//!   [`write_volatile`]：volatile 访问不能用于线程间同步，无论它们操作的是不是 Rust 内存。
//! * 把引用转换为指针所得到的结果，在底层 allocation 仍存活、且没有任何引用（只有裸指针）
//!   被用来访问同一块内存的期间内一直有效。也就是说，引用访问和指针访问不能交错进行。
//!
//! 这些公理，加上谨慎地用 [`offset`] 做指针算术，足以在 unsafe 代码中正确实现许多
//! 有用的东西。随着 [aliasing] 规则逐步确定，将来会提供更强的保证。更多信息见
//! [the book][book]，以及参考手册中专门讲 [undefined behavior][ub]（未定义行为）的章节。
//!
//! 我们称一个指针是 "dangling"（悬垂）的，如果它对任何非零大小的访问都无效。这意味着
//! 越界的指针、指向已释放内存的指针、null 指针，以及用 [`NonNull::dangling`] 创建的指针，
//! 都是悬垂指针。
//!
//! ## 对齐（Alignment）
//!
//! 按上文定义为 valid 的裸指针，未必是“正确对齐”的（这里“正确”对齐由所指类型定义，
//! 即 `*const T` 必须对齐到 `align_of::<T>()`）。然而，大多数函数都要求其参数正确对齐，
//! 并会在文档中明确写出这一要求。[`read_unaligned`] 和 [`write_unaligned`] 是值得注意的例外。
//!
//! 当一个函数要求正确对齐时，即使访问大小为 0（即实际并不触碰内存），它也要求对齐。
//! 这种情况下可考虑使用 [`NonNull::dangling`]。
//!
//! ## 指针到引用的转换（Pointer to reference conversion）
//!
//! 当把指针转换为引用时（例如通过 `&*ptr` 或 `&mut *ptr`），必须遵守以下若干规则：
//!
//! * 指针必须正确对齐。
//!
//! * 它必须非空（non-null）。
//!
//! * 它必须在上文定义的意义上“可解引用”（dereferenceable）。
//!
//! * 指针必须指向一个类型 `T` 的 [valid value]（有效值）。
//!
//! * 你必须遵守 Rust 的 aliasing 规则。确切的 aliasing 规则尚未确定，所以这里只给一个
//!   粗略概述。规则还取决于创建的是可变引用还是共享引用。
//!   * 创建可变引用时，在该引用存在期间，它所指向的内存不得通过任何其他不是从该引用派生
//!     而来的指针或引用被访问（读或写）。
//!   * 创建共享引用时，在该引用存在期间，它所指向的内存不得被改动（在 `UnsafeCell` 内部
//!     除外）。
//!
//! 如果一个指针遵守以上所有规则，就称它*可转换为（可变或共享）引用*。
// ^ 我们使用这个术语，而不是说“产生的引用必须 valid”，因为引用的有效性容易和它所引用之物
// 的有效性相混淆；这两个概念虽然密切相关，但并不相同。
//!
//! 这些规则即使在结果没有被使用时也照样适用！
//!（关于“必须已初始化”的那部分尚未完全定论，但在定论之前，唯一安全的做法就是确保它们
//! 确实已被初始化。）
//!
//! 上述规则的一个推论是：诸如 `unsafe { &*(0 as *const u8) }` 这样的表达式是立即（Immediate）
//! 未定义行为。
//!
//! [valid value]: ../../reference/behavior-considered-undefined.html#invalid-values
//!
//! ## 分配（Allocation）
//!
//! <a id="allocated-object"></a> <!-- 保持旧 URL 仍可用 -->
//!
//! 一块 *allocation*（分配）是程序内存的一个子集，它可从 Rust 中寻址，且在其内部可以进行
//! 指针算术。allocation 的例子包括堆分配、栈上分配的变量、`static` 和 `const`。某些 Rust
//! 操作的安全前置条件——例如 `offset` 和字段投影（`expr.field`）——就是以它们所操作的
//! allocation 来定义的。
//!
//! 一块 allocation 有一个基地址、一个大小，以及一组内存地址。allocation 的大小可以为零，
//! 但这样的 allocation 仍然有一个基地址。allocation 的基地址不一定唯一。虽然目前 allocation
//! 总是拥有一组完全连续的内存地址（即没有“空洞”），但不保证将来不会改变。
//!
//! allocation 的行为必须像“正常”内存：特别是，读取不得有副作用，写入必须能通过通常的同步
//! 原语对其他线程变得可见。
//!
//! 对于任意一块基地址为 `base`、大小为 `size`、地址集合为 `addresses` 的 allocation，保证如下：
//! - 对 `addresses` 中所有地址 `a`，`a` 都落在 `base .. (base + size)` 范围内
//!   （注意这要求 `a < base + size`，而不是 `a <= base + size`）
//! - `base` 不等于 [`null()`]（即数值为 0 的地址）
//! - `base + size <= usize::MAX`
//! - `size <= isize::MAX`
//!
//! 作为这些保证的推论，对于某块 allocation 地址集合中的任意地址 `a`：
//! - 保证 `a - base` 不会溢出 `isize`
//! - 保证 `a - base` 非负
//! - 保证：给定 `o = a - base`（即 `a` 在该 allocation 内的偏移），`base + o` 不会绕过
//!   地址空间（换言之，不会溢出 `usize`）
//!
//! [`null()`]: null
//!
//! # Provenance（来源 / 可证溯性）
//!
//! 指针不*仅仅*是一个“整数”或“地址”。举例来说，大家都认同 Use After Free（释放后使用，
//! 简称 UAF）显然是未定义行为，即便你“运气好”，被释放的内存在你读/写之前又被重新分配了
//! （事实上这才是最坏的情形——如果这种重分配不会发生，UAF 反倒不那么令人担忧了！）。
//! 再举一例：[`wrapping_offset`] 的文档说它会“记住”原始指针所指向的 allocation，即便它被
//! 偏移到远在该 allocation 所占内存范围之外的地方。要使这类说法成立，指针就必须比单纯的
//! 地址*更多*一些东西：它们必须带有 **provenance**（来源信息）。
//!
//! 在 Rust 语义中，一个指针值包含以下信息：
//!
//! * 它所指向的 **address**（地址），可用一个 `usize` 表示。
//! * 它所拥有的 **provenance**，定义了它有权访问哪块内存。provenance 也可以缺失，此时
//!   该指针没有访问任何内存的权限。
//!
//! provenance 的确切结构尚未规定，但一个指针的 provenance 所定义的权限具有一个*空间*分量、
//! 一个*时间*分量，以及一个*可变性*分量：
//!
//! * 空间（Spatial）：该指针被允许访问的内存地址的集合。
//! * 时间（Temporal）：该指针被允许访问那些内存地址的时间跨度。
//! * 可变性（Mutability）：该指针是只能用于读取这块内存，还是也可以用于写入。注意这一分量
//!   可与其他分量交互，例如一个指针可能只在地址的某个子集上允许改动，或只在其最大时间
//!   跨度的某个子段内允许改动。
//!
//! 当一块 [allocation] 被创建时，它有一个唯一的 Original Pointer（原始指针）。对 alloc 类
//! API 来说，它就是该调用所返回的那个指针；对局部变量和 `static` 来说，它就是该变量/static
//! 的名字。（为了简洁/便于阐述，这里稍微宽泛地借用了“指针”一词。）
//!
//! 一块 allocation 的 Original Pointer 所带的 provenance，将该指针的*空间*权限约束在该
//! allocation 的内存范围内，将其*时间*权限约束在该 allocation 的生命周期内。所有从 Original
//! Pointer 经由 [`offset`]、借用、指针强制转换等操作传递性地派生出来的指针，都会隐式地继承
//! 这份 provenance。某些操作可能*缩小*所派生 provenance 的权限，限制它能访问多少内存或能在
//! 多长时间内有效（例如借用一个子字段、对切片再切片，会缩小 provenance 的空间分量；而所有
//! 借用都可能缩小 provenance 的时间分量）。然而，任何操作都*永远不能扩大*所派生 provenance
//! 的权限：即便你“知道”存在一块更大的 allocation，你也无法派生出 provenance 更大的指针。
//! 同理，你也不能把两段连续的 provenance“重新合并”成一段（即写出形如
//! `fn merge(&[T], &[T]) -> &[T]` 的东西）。
//!
//! 指向某个位置（place）的引用，其 provenance 至少覆盖该位置所占的内存。指向某个切片的
//! 引用，其 provenance 至少覆盖该切片所描述的范围。引用的 provenance 是否、以及究竟何时会被
//! “缩小”到*恰好*贴合它所指向的内存，目前尚未确定。
//!
//! *共享*引用所带的 provenance 永远只允许读取内存，从不允许写入，[`UnsafeCell`] 内部除外。
//!
//! provenance 会影响一个程序是否具有未定义行为：
//!
//! * 通过一个对某块内存不持有 provenance 的指针去访问该内存，是未定义行为。注意，一个处于其
//!   provenance“末端”的指针实际上并未越出其 provenance，它只是没有任何字节可供 load/store。
//!   零大小访问不需要任何 provenance，因为它访问的是一段空内存范围。
//!
//! * 把一个指针 [`offset`] 跨越一段不包含在它所派生自的 allocation 内的内存范围，或者对两个
//!   并非派生自同一 allocation 的指针调用 [`offset_from`]，都是未定义行为。provenance 正是用来
//!   说明“派生自”究竟意味着什么的：一个指针的血统被追溯回它所源出的 Original Pointer，由此
//!   确定相关的 allocation。特别地，对一个派生自现已被释放之物的指针做 offset 总是 UB，除非
//!   偏移量为 0。
//!
//! 但以下操作*仍然*是合理（sound）的：
//!
//! * 仅凭一个地址创建一个不带 provenance 的指针（见 [`without_provenance`]）。这样的指针不能
//!   用于内存访问（零大小访问除外）。它仍然可用于哨兵值（如 `null`），*或者*用来表示一个永远
//!   不会可解引用的带标记指针（tagged pointer）。一般来说，让一个整数“假装是指针玩玩”总是
//!   合理的，只要你不对它做需要它有效的操作（非零大小的 offset、读、写等）。
//!
//! * 在任意一个对齐充分的非空地址处“伪造”一块大小为零的 allocation。即通常的“ZST 是虚的，
//!   随你怎么搞”规则适用。
//!
//! * 把一个指针 [`wrapping_offset`] 到其 provenance 之外。这包括那些“没有”provenance 的指针。
//!   特别地，这使得做指针打标记的各种小技巧成为合理操作。
//!
//! * 按地址比较任意指针。指针比较忽略 provenance，而地址*就是*整数，所以总有一个连贯的答案，
//!   即便这些指针是悬垂的或来自不同的 provenance。注意，如果你“走运”地发现某 allocation 末尾
//!   的指针与另一 allocation 起始处“同一个”地址相等，那么你基于这一事实所做的任何事情*多半*
//!   都会是一堆胡话。这堆胡话的影响范围受到如下事实的限制：这两个指针*仍然*不被允许访问对方
//!   的 allocation（字节），因为它们仍然具有不同的 provenance。
//!
//! 注意，provenance 在 Rust 中的完整定义尚未确定，因为它与至今未定的 [aliasing] 规则相互交织。
//!
//! ## 指针 vs 整数（Pointers Vs Integers）
//!
//! 经过以上讨论可以很清楚地看出：一个 `usize` *无法*准确表示一个指针，而从指针转换为 `usize`
//! 通常是一个*仅仅*提取地址的操作。把这个地址再转换回指针，则需要以某种方式回答这个问题：
//! 所得到的指针应该带有哪一份 provenance？
//!
//! Rust 提供了两种处理这种情况的方式：*Strict Provenance*（严格来源）和 *Exposed Provenance*
//!（暴露来源）。
//!
//! 注意，一个指针*可以*表示一个 `usize`（通过 [`without_provenance`]），所以在“有时是指针、
//! 有时是裸 `usize`”的场合，应当选用的正确类型是指针类型。
//!
//! ## Strict Provenance（严格来源）
//!
//! "Strict Provenance" 指的是一组旨在使 provenance 的使用更加显式的 API。它们意在替代“把指针
//! 转成整数再转回来”的做法。
//!
//! 完全避免 integer-to-pointer（整数到指针）转换，可以成功地绕开该操作固有的歧义。这有利于
//! 编译器优化，而且对于使用 [Miri] 这类工具、以及 [CHERI] 这类旨在检测和诊断指针误用的体系
//! 结构而言，几乎是一项硬性要求。
//!
//! 要让“*完全*不做 integer-to-pointer 转换”的编程方式变得可行，关键的洞见就是 [`with_addr`]
//! 方法：
//!
//! ```text
//!     /// 创建一个带有给定地址的新指针。
//!     ///
//!     /// 这执行的操作与 `addr as ptr` 转换相同，但会把 `self` 的 *provenance*
//!     /// 复制到新指针上。
//!     /// 这使我们能够动态地保留并传播这一重要信息，而这是用一元转换
//!     ///（unary cast）所无法做到的。
//!     ///
//!     /// 这等价于用 `wrapping_offset` 把 `self` 偏移到给定地址，
//!     /// 因此具有与之完全相同的能力与限制。
//!     pub fn with_addr(self, addr: usize) -> Self;
//! ```
//!
//! 于是你仍然能够下降到地址表示层、做任何你想做的巧妙位运算技巧，*只要*你能保留一个指向你
//! 关心的那块 allocation 的指针，用以“重建”provenance。通常这非常容易，因为你往往只是取一个
//! 指针、摆弄它的地址、然后立刻转换回指针。为让这一用例更顺手，我们提供了 [`map_addr`] 方法。
//!
//! 为了让代码“遵循”Strict Provenance 语义这件事更清晰，我们还提供了一个 [`addr`] 方法，它承诺
//! 返回的地址不属于某次 指针-整数-指针 往返过程的一部分。将来我们可能为 指针<->整数 转换提供
//! 一个 lint，帮你审查代码是否符合 strict provenance。
//!
//! ### 使用 Strict Provenance
//!
//! 大多数代码无需改动即可符合 strict provenance，因为唯一真正令人担忧的操作是从 `usize` 到
//! 指针的转换。对于*确实*要把 `usize` 转成指针的代码，改动的范围取决于你具体在做什么。
//!
//! 一般来说，你只需确保：如果你想把一个 `usize` 地址转换成指针、然后用该指针读/写内存，你就
//! 需要保留一个带有足够 provenance、能自行执行该读/写的指针。这样一来，你所有“从地址到指针”
//! 的转换本质上都只是在施加偏移/索引而已。
//!
//! 对于像带标记指针这样的简单情形，做到这一点通常很轻松，*只要你把带标记指针表示为一个真正的
//! 指针、而不是一个 `usize`*。例如：
//!
//! ```
//! unsafe {
//!     // 我们想打包进指针里的一个标志位
//!     static HAS_DATA: usize = 0x1;
//!     static FLAG_MASK: usize = !HAS_DATA;
//!
//!     // 我们的值，它必须有足够的对齐，才能留出可用的最低有效位。
//!     let my_precious_data: u32 = 17;
//!     assert!(align_of::<u32>() > 1);
//!
//!     // 创建一个带标记指针
//!     let ptr = &my_precious_data as *const u32;
//!     let tagged = ptr.map_addr(|addr| addr | HAS_DATA);
//!
//!     // 检查标志位：
//!     if tagged.addr() & HAS_DATA != 0 {
//!         // 去掉标记并读取指针
//!         let data = *tagged.map_addr(|addr| addr & FLAG_MASK);
//!         assert_eq!(data, 17);
//!     } else {
//!         unreachable!()
//!     }
//! }
//! ```
//!
//!（没错，如果你在并发数据结构里一直用 [`AtomicUsize`] 来存指针，那你应该改用 [`AtomicPtr`]。
//! 如果这打乱了你原子地操控指针的方式，我们很想知道为什么，以及需要做什么来修复它。）
//!
//! 在那些*必须*仅凭地址创建一个有效指针的场合——例如裸机（baremetal）代码访问位于固定地址的
//! 内存映射接口——目前无法用 strict provenance API 处理，应改用[暴露来源（exposed
//! provenance）](#exposed-provenance)。
//!
//! ## Exposed Provenance（暴露来源）
//!
//! 如上所述，integer-to-pointer 转换无法用 Strict Provenance API 完成。这是有意为之的：Strict
//! Provenance 的目标是提供一个清晰的规范，使我们有信心可以无歧义地形式化它，并对其进行精确的
//! 形式化推理。而 integer-to-pointer 转换（目前）没有这样清晰的规范。
//!
//! 然而，确实存在一些无法避免 integer-to-pointer 转换、或避免它就要大规模重构的场合。遗留的
//! 平台 API 也常常假设 `usize` 能够捕获构成一个指针的全部信息。裸机平台还可能要求“凭空”合成
//! 一个指针，而无处获取恰当的 provenance。
//!
//! Rust 用来处理 integer-to-pointer 转换的模型叫做 *Exposed Provenance*。然而，Exposed
//! Provenance 的语义所立足的根基远不如 Strict Provenance 牢靠，眼下尚不清楚能否为 Exposed
//! Provenance 定义出令人满意的、无歧义的语义。（如果这听起来很糟，请放心：其他提供
//! integer-to-pointer 转换的流行语言也好不到哪去。）此外，Exposed Provenance 与 [Miri] 和
//! [CHERI] 这类工具配合得（并）不好。
//!
//! Exposed Provenance 由 [`expose_provenance`] 和 [`with_exposed_provenance`] 这两个方法提供，
//! 它们等价于指针与整数之间的 `as` 转换。
//! - [`expose_provenance`] 很像 [`addr`]，但额外把该指针的 provenance 加入一个全局的“已暴露”
//!   provenance 列表。（这个列表纯属概念性的，它只为规范 Rust 而存在，在实际执行中并不会被
//!   物化，[Miri] 之类的工具除外。）位于 Rust 抽象机控制之外的内存（例如 MMIO 寄存器）总是
//!   被视为已暴露的，只要这块内存与抽象机将要使用的内存（如栈、堆、static）不相交。
//! - [`with_exposed_provenance`] 可用于构造一个带有某个先前“已暴露”provenance 的指针。
//!   [`with_exposed_provenance`] 只接受 `addr: usize` 作为参数，所以与 [`with_addr`] 不同，它
//!   没有任何线索说明返回指针的正确 provenance 应该是什么——而这正是 integer-to-pointer 转换
//!   如此难以严格规范的根源所在！编译器会尽力为你挑选“正确”的 provenance，但目前我们无法对
//!   结果指针将带有哪一份 provenance 提供任何保证。唯一明确的一点是：如果*没有*任何先前“已
//!   暴露”的 provenance 能够正当化返回指针被使用的方式，那么该程序就具有未定义行为。
//!
//! 如果可能的话，我们鼓励把代码移植到 [Strict Provenance] API，从而无需 Exposed Provenance。
//! 让这类代码的占比最大化，对于避免规范复杂度、以及推动采用 [CHERI] 和 [Miri] 这类能大幅提升
//!（unsafe）Rust 代码可信度的工具，都是一项重大利好。不过我们承认这并非总是可行，因此提供
//! Exposed Provenance 作为一种途径，让你显式地“退出”Strict Provenance 那套良定义的语义、
//! 并“接受”integer-to-pointer 转换那套不清晰的语义。
//!
//! [aliasing]: ../../nomicon/aliasing.html
//! [allocation]: #allocation
//! [provenance]: #provenance
//! [book]: ../../book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer
//! [ub]: ../../reference/behavior-considered-undefined.html
//! [zst]: ../../nomicon/exotic-sizes.html#zero-sized-types-zsts
//! [atomic operations]: crate::sync::atomic
//! [`offset`]: pointer::offset
//! [`offset_from`]: pointer::offset_from
//! [`wrapping_offset`]: pointer::wrapping_offset
//! [`with_addr`]: pointer::with_addr
//! [`map_addr`]: pointer::map_addr
//! [`addr`]: pointer::addr
//! [`AtomicUsize`]: crate::sync::atomic::AtomicUsize
//! [`AtomicPtr`]: crate::sync::atomic::AtomicPtr
//! [`expose_provenance`]: pointer::expose_provenance
//! [`with_exposed_provenance`]: with_exposed_provenance
//! [Miri]: https://github.com/rust-lang/miri
//! [CHERI]: https://www.cl.cam.ac.uk/research/security/ctsrd/cheri/
//! [Strict Provenance]: #strict-provenance
//! [`UnsafeCell`]: core::cell::UnsafeCell

#![stable(feature = "rust1", since = "1.0.0")]
// 本模块中有许多接收指针、但并不解引用它们的 unsafe 函数。
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::cmp::Ordering;
use crate::intrinsics::const_eval_select;
use crate::marker::{Destruct, FnPtr, PointeeSized};
use crate::mem::{self, MaybeUninit, SizedTypeProperties};
use crate::num::NonZero;
use crate::{fmt, hash, intrinsics, ub_checks};

mod alignment;
#[unstable(feature = "ptr_alignment_type", issue = "102070")]
pub use alignment::Alignment;

mod metadata;
#[unstable(feature = "ptr_metadata", issue = "81513")]
pub use metadata::{DynMetadata, Pointee, Thin, from_raw_parts, from_raw_parts_mut, metadata};

mod non_null;
#[stable(feature = "nonnull", since = "1.25.0")]
pub use non_null::NonNull;

mod unique;
#[unstable(feature = "ptr_internals", issue = "none")]
pub use unique::Unique;

mod const_ptr;
mod mut_ptr;

// 有些函数定义在这里，是因为它们曾在 stable 中意外地暴露在了本模块里。
// 详见 <https://github.com/rust-lang/rust/issues/15702>。
//（`transmute` 也属于这一类，但由于存在“`T` 与 `U` 必须同样大小”的检查，它无法被包装。）

/// 从 `src` 处复制 `count * size_of::<T>()` 个字节到 `dst`。源区域与目标区域*不得*重叠。
///
/// 对于可能重叠的内存区域，请改用 [`copy`]。
///
/// `copy_nonoverlapping` 在语义上等价于 C 的 [`memcpy`]，只是源参数和目标参数互换了位置，
/// 且 `count` 计数的是 `T` 的个数而非字节数。
///
/// 这次复制是“无类型的”（untyped），意思是数据可以是未初始化的、或以其他方式违反 `T` 的要求。
/// 初始化状态会被原样保留。
///
/// [`memcpy`]: https://en.cppreference.com/w/c/string/byte/memcpy
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `src` 必须对读取 `count * size_of::<T>()` 个字节是 [valid]（有效）的：非空、所指内存
///   已分配且未释放、且这段范围完全落在同一块 allocation 内（不越界），并带有覆盖该范围的
///   provenance。
///
/// * `dst` 必须对写入 `count * size_of::<T>()` 个字节是 [valid] 的（同上，针对写入）。
///
/// * `src` 和 `dst` 都必须正确对齐到 `align_of::<T>()`。
///
/// * 从 `src` 起、大小为 `count * size_of::<T>()` 字节的内存区域，与从 `dst` 起、同样大小的
///   内存区域*不得*重叠（这是 `copy_nonoverlapping` 相较 `copy` 的关键约束，对应 memcpy）。
///
/// 与 [`read`] 一样，无论 `T` 是否为 [`Copy`]，`copy_nonoverlapping` 都会创建 `T` 的按位
///（bitwise）副本。如果 `T` 不是 [`Copy`]，那么*同时*使用从 `*src` 起和从 `*dst` 起这两段
/// 区域中的值，可能[违反内存安全][read-ownership]（因为同一个值会被析构两次）。
///
/// 注意：即使实际复制的大小（`count * size_of::<T>()`）为 `0`，这些指针也必须正确对齐。
///
/// [`read`]: crate::ptr::read
/// [read-ownership]: crate::ptr::read#ownership-of-the-returned-value
/// [valid]: crate::ptr#safety
///
/// # 示例
///
/// 手动实现 [`Vec::append`]：
///
/// ```
/// use std::ptr;
///
/// /// 把 `src` 的所有元素移动进 `dst`，使 `src` 变为空。
/// fn append<T>(dst: &mut Vec<T>, src: &mut Vec<T>) {
///     let src_len = src.len();
///     let dst_len = dst.len();
///
///     // 确保 `dst` 有足够容量容纳 `src` 的全部内容。
///     dst.reserve(src_len);
///
///     unsafe {
///         // 这次调用 add 总是安全的，因为 `Vec` 绝不会分配超过
///         // `isize::MAX` 个字节。
///         let dst_ptr = dst.as_mut_ptr().add(dst_len);
///         let src_ptr = src.as_ptr();
///
///         // 把 `src` 截断但不析构其内容。我们先做这一步，
///         // 以免后面的某步发生 panic 时出问题。
///         src.set_len(0);
///
///         // 这两块区域不可能重叠，因为可变引用不会别名（alias），
///         // 而且两个不同的 vector 不可能拥有同一块内存。
///         ptr::copy_nonoverlapping(src_ptr, dst_ptr, src_len);
///
///         // 通知 `dst` 它现在持有了 `src` 的内容。
///         dst.set_len(dst_len + src_len);
///     }
/// }
///
/// let mut a = vec!['r'];
/// let mut b = vec!['u', 's', 't'];
///
/// append(&mut a, &mut b);
///
/// assert_eq!(a, &['r', 'u', 's', 't']);
/// assert!(b.is_empty());
/// ```
///
/// [`Vec::append`]: ../../std/vec/struct.Vec.html#method.append
#[doc(alias = "memcpy")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
#[inline(always)]
#[cfg_attr(miri, track_caller)] // even without panics, this helps for Miri backtraces
#[rustc_diagnostic_item = "ptr_copy_nonoverlapping"]
pub const unsafe fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize) {
    ub_checks::assert_unsafe_precondition!(
        check_language_ub,
        "ptr::copy_nonoverlapping requires that both pointer arguments are aligned and non-null \
        and the specified memory ranges do not overlap",
        (
            src: *const () = src as *const (),
            dst: *mut () = dst as *mut (),
            size: usize = size_of::<T>(),
            align: usize = align_of::<T>(),
            count: usize = count,
        ) => {
            let zero_size = count == 0 || size == 0;
            ub_checks::maybe_is_aligned_and_not_null(src, align, zero_size)
                && ub_checks::maybe_is_aligned_and_not_null(dst, align, zero_size)
                && ub_checks::maybe_is_nonoverlapping(src, dst, size, count)
        }
    );

    // SAFETY: `copy_nonoverlapping` 的安全契约必须由调用方维护。
    unsafe { crate::intrinsics::copy_nonoverlapping(src, dst, count) }
}

/// 从 `src` 处复制 `count * size_of::<T>()` 个字节到 `dst`。源区域与目标区域可以重叠。
///
/// 如果源区域与目标区域*永远不会*重叠，可改用 [`copy_nonoverlapping`]。
///
/// `copy` 在语义上等价于 C 的 [`memmove`]，只是源参数和目标参数互换了位置，且 `count`
/// 计数的是 `T` 的个数而非字节数。复制的进行方式仿佛是：先把字节从 `src` 拷到一个临时数组，
/// 再从该数组拷到 `dst`（因此即便重叠也能给出确定结果，这正是 memmove 的语义）。
///
/// 这次复制是“无类型的”（untyped），意思是数据可以是未初始化的、或以其他方式违反 `T` 的要求。
/// 初始化状态会被原样保留。
///
/// [`memmove`]: https://en.cppreference.com/w/c/string/byte/memmove
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `src` 必须对读取 `count * size_of::<T>()` 个字节是 [valid]（有效）的：非空、已分配未释放、
///   不越界、并带有覆盖该范围的 provenance。
///
/// * `dst` 必须对写入 `count * size_of::<T>()` 个字节是 [valid] 的，并且即便在读取 `src` 的
///   `count * size_of::<T>()` 个字节期间也必须保持有效。（这意味着：如果两段内存范围重叠，
///   对 `src` 的读取不得使 `dst` 指针失效。）
///
/// * `src` 和 `dst` 都必须正确对齐到 `align_of::<T>()`。
///
/// 与 [`read`] 一样，无论 `T` 是否为 [`Copy`]，`copy` 都会创建 `T` 的按位副本。如果 `T` 不是
/// [`Copy`]，那么同时使用从 `*src` 起和从 `*dst` 起这两段区域中的值，可能[违反内存
/// 安全][read-ownership]（值会被析构两次）。
///
/// 注意：即使实际复制的大小（`count * size_of::<T>()`）为 `0`，这些指针也必须正确对齐。
///
/// [`read`]: crate::ptr::read
/// [read-ownership]: crate::ptr::read#ownership-of-the-returned-value
/// [valid]: crate::ptr#safety
///
/// # 示例
///
/// 从一个 unsafe 缓冲区高效地创建一个 Rust vector：
///
/// ```
/// use std::ptr;
///
/// /// # 安全性（Safety）
/// ///
/// /// * `ptr` 必须按其类型正确对齐且非零。
/// /// * `ptr` 必须对读取 `elts` 个连续的、类型为 `T` 的元素是 valid 的。
/// /// * 调用本函数之后，这些元素不得再被使用，除非 `T: Copy`。
/// # #[allow(dead_code)]
/// unsafe fn from_buf_raw<T>(ptr: *const T, elts: usize) -> Vec<T> {
///     let mut dst = Vec::with_capacity(elts);
///
///     // SAFETY: 我们的前置条件确保源是对齐且有效的，
///     // 而 `Vec::with_capacity` 确保我们有可写入它们的可用空间。
///     unsafe { ptr::copy(ptr, dst.as_mut_ptr(), elts); }
///
///     // SAFETY: 我们先前以这么多容量创建了它，
///     // 而上面的 `copy` 已经初始化了这些元素。
///     unsafe { dst.set_len(elts); }
///     dst
/// }
/// ```
#[doc(alias = "memmove")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
#[inline(always)]
#[cfg_attr(miri, track_caller)] // even without panics, this helps for Miri backtraces
#[rustc_diagnostic_item = "ptr_copy"]
pub const unsafe fn copy<T>(src: *const T, dst: *mut T, count: usize) {
    // SAFETY: `copy` 的安全契约必须由调用方维护。
    unsafe {
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::copy requires that both pointer arguments are aligned and non-null",
            (
                src: *const () = src as *const (),
                dst: *mut () = dst as *mut (),
                align: usize = align_of::<T>(),
                zero_size: bool = T::IS_ZST || count == 0,
            ) =>
            ub_checks::maybe_is_aligned_and_not_null(src, align, zero_size)
                && ub_checks::maybe_is_aligned_and_not_null(dst, align, zero_size)
        );
        crate::intrinsics::copy(src, dst, count)
    }
}

/// 把从 `dst` 起的 `count * size_of::<T>()` 个字节内存全部设置为 `val`。
///
/// `write_bytes` 类似于 C 的 [`memset`]，但设置的是 `count * size_of::<T>()` 个字节，每个字节
/// 都设为 `val`。
///
/// [`memset`]: https://en.cppreference.com/w/c/string/byte/memset
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `dst` 必须对写入 `count * size_of::<T>()` 个字节是 [valid]（有效）的：非空、已分配未释放、
///   不越界、并带有覆盖该范围的 provenance。
///
/// * `dst` 必须正确对齐到 `align_of::<T>()`。
///
/// 注意：即使实际写入的大小（`count * size_of::<T>()`）为 `0`，该指针也必须正确对齐。
///
/// 此外请注意：以这种方式改动 `*dst`，如果写入的字节并不构成某个 `T` 的有效表示，后续很容易
/// 导致未定义行为（UB）。例如，下面就是对本函数的一个**错误**用法：
///
/// ```rust,no_run
/// unsafe {
///     let mut value: u8 = 0;
///     let ptr: *mut bool = &mut value as *mut u8 as *mut bool;
///     let _bool = ptr.read(); // 这没问题，`ptr` 指向一个有效的 `bool`。
///     ptr.write_bytes(42u8, 1); // 这个函数本身不会造成 UB……
///     let _bool = ptr.read(); // ……但它使得这次操作成为 UB！⚠️
/// }
/// ```
///
/// [valid]: crate::ptr#safety
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// use std::ptr;
///
/// let mut vec = vec![0u32; 4];
/// unsafe {
///     let vec_ptr = vec.as_mut_ptr();
///     ptr::write_bytes(vec_ptr, 0xfe, 2);
/// }
/// assert_eq!(vec, [0xfefefefe, 0xfefefefe, 0, 0]);
/// ```
#[doc(alias = "memset")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
#[inline(always)]
#[cfg_attr(miri, track_caller)] // even without panics, this helps for Miri backtraces
#[rustc_diagnostic_item = "ptr_write_bytes"]
pub const unsafe fn write_bytes<T>(dst: *mut T, val: u8, count: usize) {
    // SAFETY: `write_bytes` 的安全契约必须由调用方维护。
    unsafe {
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::write_bytes requires that the destination pointer is aligned and non-null",
            (
                addr: *const () = dst as *const (),
                align: usize = align_of::<T>(),
                zero_size: bool = T::IS_ZST || count == 0,
            ) => ub_checks::maybe_is_aligned_and_not_null(addr, align, zero_size)
        );
        crate::intrinsics::write_bytes(dst, val, count)
    }
}

/// 就地（in place）执行所指向的值的析构函数（如果有的话）。
///
/// 这几乎等同于调用 [`ptr::read`] 然后丢弃其结果，但具有以下优点：
// FIXME: 说点比“几乎相同”更有用的？
// 这里有一些悬而未决的问题：`read` 要求值是完全 valid 的，例如若 `T` 是
// `bool` 则它必须是 0 或 1，若它是引用则必须可解引用。而 `drop_in_place`
// 只要求 `*to_drop` 是“可供析构（valid for dropping）”的，但我们尚未定义这究竟意味着什么。
// 在 Miri 中目前（2024 年 5 月）对于没有 drop glue 的类型不要求任何东西。
///
/// * 对于像 trait 对象这样的非定长（unsized）类型，*必须*使用 `drop_in_place` 来析构，因为
///   它们无法被读出到栈上再正常析构。
///
/// * 在析构手动分配的内存时（例如在 `Box`/`Rc`/`Vec` 的实现中），相比 [`ptr::read`]，
///   它对优化器更友好，因为编译器无需证明省略那次复制是合理的。
///
/// * 当 `T` 不是 `repr(packed)` 时，它可用于析构 [pinned] 数据（被 pin 住的数据在析构前
///   不得被移动）。
///
/// 未对齐（unaligned）的值不能就地析构，必须先用 [`ptr::read_unaligned`] 复制到一个对齐的
/// 位置。对于 packed 结构体，这次移动由编译器自动完成。这意味着 packed 结构体的字段不是
/// 就地析构的。
///
/// [`ptr::read`]: self::read
/// [`ptr::read_unaligned`]: self::read_unaligned
/// [pinned]: crate::pin
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `to_drop` 必须对读取和写入*两者*都是 [valid]（有效）的。
///
/// * `to_drop` 必须正确对齐，即使 `T` 的大小为 0。
///
/// * `to_drop` 必须非空（nonnull），即使 `T` 的大小为 0。
///
/// * `to_drop` 所指向的值必须是“可供析构”的，这可能意味着它要维护额外的不变量。这些不变量
///   取决于被析构的值的类型。例如，析构一个 Box 时，该 box 指向堆的指针必须有效。
///
/// * 在 `drop_in_place` 执行期间，访问 `to_drop` 各部分的唯一途径，是 `drop_in_place` 所调用
///   的各个 `Drop::drop` 方法被提供的那些 `&mut self` 引用。
///
/// 此外，如果 `T` 不是 [`Copy`]，那么在调用 `drop_in_place` 之后再使用所指向的值会导致未定义
/// 行为。注意 `*to_drop = foo` 也算作一次使用，因为它会导致该值被再次析构（双重 drop）。
/// 可用 [`write()`] 来覆盖数据而不触发它被析构。
///
/// [valid]: self#safety
///
/// # 示例
///
/// 手动移除一个 vector 的最后一个元素：
///
/// ```
/// use std::ptr;
/// use std::rc::Rc;
///
/// let last = Rc::new(1);
/// let weak = Rc::downgrade(&last);
///
/// let mut v = vec![Rc::new(0), last];
///
/// unsafe {
///     // 取得指向 `v` 中最后一个元素的裸指针。
///     let ptr = &mut v[1] as *mut _;
///     // 缩短 `v`，以防最后一个元素被析构。我们先做这一步，
///     // 以免下面的 `drop_in_place` 发生 panic 时出问题。
///     v.set_len(1);
///     // 若不调用 `drop_in_place`，最后一个元素将永远不会被析构，
///     // 它所管理的内存也会泄漏。
///     ptr::drop_in_place(ptr);
/// }
///
/// assert_eq!(v, &[0.into()]);
///
/// // 确认最后一个元素确实被析构了。
/// assert!(weak.upgrade().is_none());
/// ```
#[stable(feature = "drop_in_place", since = "1.8.0")]
#[lang = "drop_in_place"]
#[allow(unconditional_recursion)]
#[rustc_diagnostic_item = "ptr_drop_in_place"]
#[rustc_const_unstable(feature = "const_drop_in_place", issue = "109342")]
pub const unsafe fn drop_in_place<T: PointeeSized>(to_drop: *mut T)
where
    T: [const] Destruct,
{
    // 这里的代码无关紧要——它会被编译器替换为真正的 drop glue。

    // SAFETY: 见上方注释
    unsafe { drop_in_place(to_drop) }
}

/// 创建一个 null（空）裸指针。
///
/// 本函数等价于把指针零初始化：`MaybeUninit::<*const T>::zeroed().assume_init()`。
/// 所得指针的地址为 0。
///
/// # 示例
///
/// ```
/// use std::ptr;
///
/// let p: *const i32 = ptr::null();
/// assert!(p.is_null());
/// assert_eq!(p as usize, 0); // 这个指针的地址是 0
/// ```
#[inline(always)]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_promotable]
#[rustc_const_stable(feature = "const_ptr_null", since = "1.24.0")]
#[rustc_diagnostic_item = "ptr_null"]
pub const fn null<T: PointeeSized + Thin>() -> *const T {
    from_raw_parts(without_provenance::<()>(0), ())
}

/// 创建一个 null（空）可变裸指针。
///
/// 本函数等价于把指针零初始化：`MaybeUninit::<*mut T>::zeroed().assume_init()`。
/// 所得指针的地址为 0。
///
/// # 示例
///
/// ```
/// use std::ptr;
///
/// let p: *mut i32 = ptr::null_mut();
/// assert!(p.is_null());
/// assert_eq!(p as usize, 0); // 这个指针的地址是 0
/// ```
#[inline(always)]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_promotable]
#[rustc_const_stable(feature = "const_ptr_null", since = "1.24.0")]
#[rustc_diagnostic_item = "ptr_null_mut"]
pub const fn null_mut<T: PointeeSized + Thin>() -> *mut T {
    from_raw_parts_mut(without_provenance_mut::<()>(0), ())
}

/// 创建一个带有给定地址、且不带 [provenance][crate::ptr#provenance] 的指针。
///
/// 这等价于 `ptr::null().with_addr(addr)`。
///
/// 由于不带 provenance，这个指针不与任何实际的 allocation 关联。这样的无 provenance 指针可用于
/// 零大小的内存访问（如果对齐合适的话），但用无 provenance 指针做非零大小的内存访问是 UB。
/// 无 provenance 指针只不过是一个伪装成指针的 `usize` 地址而已。
///
/// 这与 `addr as *const T` 不同：后者创建的指针会拾取某个先前已暴露的 provenance。关于该操作的
/// 更多细节见 [`with_exposed_provenance`]。
///
/// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
#[inline(always)]
#[must_use]
#[stable(feature = "strict_provenance", since = "1.84.0")]
#[rustc_const_stable(feature = "strict_provenance", since = "1.84.0")]
#[rustc_diagnostic_item = "ptr_without_provenance"]
pub const fn without_provenance<T>(addr: usize) -> *const T {
    without_provenance_mut(addr)
}

/// 创建一个新指针：它是悬垂（dangling）的，但非空且良好对齐。
///
/// 这对初始化那些惰性分配（lazily allocate）的类型很有用，例如 `Vec::new` 就是这么做的。
///
/// 注意，返回指针的地址有可能恰好就是某个有效指针的地址，因此它*不能*被当作“尚未初始化”的
/// 哨兵值（sentinel value）使用。惰性分配的类型必须用其他手段来追踪初始化状态。
#[inline(always)]
#[must_use]
#[stable(feature = "strict_provenance", since = "1.84.0")]
#[rustc_const_stable(feature = "strict_provenance", since = "1.84.0")]
pub const fn dangling<T>() -> *const T {
    dangling_mut()
}

/// 创建一个带有给定地址、且不带 [provenance][crate::ptr#provenance] 的指针。
///
/// 这等价于 `ptr::null_mut().with_addr(addr)`。
///
/// 由于不带 provenance，这个指针不与任何实际的 allocation 关联。这样的无 provenance 指针可用于
/// 零大小的内存访问（如果对齐合适的话），但用无 provenance 指针做非零大小的内存访问是 UB。
/// 无 provenance 指针只不过是一个伪装成指针的 `usize` 地址而已。
///
/// 这与 `addr as *mut T` 不同：后者创建的指针会拾取某个先前已暴露的 provenance。关于该操作的
/// 更多细节见 [`with_exposed_provenance_mut`]。
///
/// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
#[inline(always)]
#[must_use]
#[stable(feature = "strict_provenance", since = "1.84.0")]
#[rustc_const_stable(feature = "strict_provenance", since = "1.84.0")]
#[rustc_diagnostic_item = "ptr_without_provenance_mut"]
#[allow(integer_to_ptr_transmutes)] // Expected semantics here.
pub const fn without_provenance_mut<T>(addr: usize) -> *mut T {
    // 一次 int-to-pointer 的 transmute 目前恰好具有我们想要的语义：它创建一个不带 provenance
    // 的指针。注意这*不是*关于 transmute 语义的稳定保证，它依赖于 sysroot crate 拥有特殊地位。
    // SAFETY: 每个有效的整数也都是一个有效的指针（只要你不去解引用那个指针）。
    unsafe { mem::transmute(addr) }
}

/// 创建一个新指针：它是悬垂（dangling）的，但非空且良好对齐。
///
/// 这对初始化那些惰性分配（lazily allocate）的类型很有用，例如 `Vec::new` 就是这么做的。
///
/// 注意，返回指针的地址有可能恰好就是某个有效指针的地址，因此它*不能*被当作“尚未初始化”的
/// 哨兵值（sentinel value）使用。惰性分配的类型必须用其他手段来追踪初始化状态。
#[inline(always)]
#[must_use]
#[stable(feature = "strict_provenance", since = "1.84.0")]
#[rustc_const_stable(feature = "strict_provenance", since = "1.84.0")]
pub const fn dangling_mut<T>() -> *mut T {
    NonNull::dangling().as_ptr()
}

/// 把一个地址转换回指针，并拾取某个先前“已暴露”的 [provenance][crate::ptr#provenance]。
///
/// 这完全等价于 `addr as *const T`。返回指针的 provenance，是*某个*先前通过传给
/// [`expose_provenance`][pointer::expose_provenance]（或 `ptr as usize` 转换）而被暴露的指针的
/// provenance。此外，位于 Rust 抽象机控制之外的内存（例如 MMIO 寄存器）总是被视为可用某个已
/// 暴露 provenance 访问，只要这块内存与抽象机将要使用的内存（如栈、堆、static）不相交。
///
/// 究竟会挑中哪一份 provenance 并未规定。编译器会尽力为你挑选“正确”的那份（不管那是什么），
/// 但目前我们无法对结果指针将带有哪一份 provenance 提供任何保证——因此也就没有关于结果指针可
/// 访问哪块内存的确定规范。
///
/// 如果*没有*任何先前“已暴露”的 provenance 能够正当化返回指针被使用的方式，那么该程序就具有
/// 未定义行为。特别地，aliasing 规则仍然适用：因别名访问而失效的指针和引用，即便它们曾被暴露，
/// 也不能再使用了！
///
/// 由于其固有的歧义性，那些帮助你遵守 Rust 内存模型的工具可能不支持此操作。建议尽可能改用
/// [Strict Provenance][self#strict-provenance] API，例如 [`with_addr`][pointer::with_addr]。
///
/// 在大多数平台上，这会产生一个字节与该地址相同的值。那些需要在指针中存储额外信息的平台可能
/// 不支持此操作，因为通常无法真正*计算*出返回指针应当拾取哪一份 provenance。
///
/// 这是一个 [Exposed Provenance][crate::ptr#exposed-provenance] API。
#[must_use]
#[inline(always)]
#[stable(feature = "exposed_provenance", since = "1.84.0")]
#[rustc_const_stable(feature = "const_exposed_provenance", since = "1.91.0")]
#[cfg_attr(miri, track_caller)] // 即便没有 panic，这对 Miri 的回溯（backtrace）也有帮助
#[allow(fuzzy_provenance_casts)] // 这*正是*应当替代使用的显式 provenance API
pub const fn with_exposed_provenance<T>(addr: usize) -> *const T {
    addr as *const T
}

/// 把一个地址转换回可变指针，并拾取某个先前“已暴露”的 [provenance][crate::ptr#provenance]。
///
/// 这完全等价于 `addr as *mut T`。返回指针的 provenance，是*某个*先前通过传给
/// [`expose_provenance`][pointer::expose_provenance]（或 `ptr as usize` 转换）而被暴露的指针的
/// provenance。此外，位于 Rust 抽象机控制之外的内存（例如 MMIO 寄存器）总是被视为可用某个已
/// 暴露 provenance 访问，只要这块内存与抽象机将要使用的内存（如栈、堆、static）不相交。
///
/// 究竟会挑中哪一份 provenance 并未规定。编译器会尽力为你挑选“正确”的那份（不管那是什么），
/// 但目前我们无法对结果指针将带有哪一份 provenance 提供任何保证——因此也就没有关于结果指针可
/// 访问哪块内存的确定规范。
///
/// 如果*没有*任何先前“已暴露”的 provenance 能够正当化返回指针被使用的方式，那么该程序就具有
/// 未定义行为。特别地，aliasing 规则仍然适用：因别名访问而失效的指针和引用，即便它们曾被暴露，
/// 也不能再使用了！
///
/// 由于其固有的歧义性，那些帮助你遵守 Rust 内存模型的工具可能不支持此操作。建议尽可能改用
/// [Strict Provenance][self#strict-provenance] API，例如 [`with_addr`][pointer::with_addr]。
///
/// 在大多数平台上，这会产生一个字节与该地址相同的值。那些需要在指针中存储额外信息的平台可能
/// 不支持此操作，因为通常无法真正*计算*出返回指针应当拾取哪一份 provenance。
///
/// 这是一个 [Exposed Provenance][crate::ptr#exposed-provenance] API。
#[must_use]
#[inline(always)]
#[stable(feature = "exposed_provenance", since = "1.84.0")]
#[rustc_const_stable(feature = "const_exposed_provenance", since = "1.91.0")]
#[cfg_attr(miri, track_caller)] // 即便没有 panic，这对 Miri 的回溯（backtrace）也有帮助
#[allow(fuzzy_provenance_casts)] // 这*正是*应当替代使用的显式 provenance API
pub const fn with_exposed_provenance_mut<T>(addr: usize) -> *mut T {
    addr as *mut T
}

/// 把一个引用转换为裸指针。
///
/// 对于 `r: &T`，`from_ref(r)` 等价于 `r as *const T`（下文提到的注意点除外），但更安全一些，
/// 因为它绝不会悄悄地改变类型或可变性，在代码被重构时尤其如此。
///
/// 调用方必须确保所指对象（pointee）的存活时间长于本函数返回的指针，否则该指针将变为悬垂。
///
/// 调用方还必须确保：指针（非传递地）所指向的内存，绝不会通过此指针或任何从它派生的指针被写入
///（在 `UnsafeCell` 内部除外）。如果你需要改动 pointee，请用 [`from_mut`]。具体来说，要把一个
/// 可变引用 `m: &mut T` 转成 `*const T`，更推荐用 `from_mut(m).cast_const()` 来得到一个之后
/// 仍可用于改动的指针。
///
/// ## 与生命周期延长（lifetime extension）的相互作用
///
/// 注意，这与尾表达式（tail expression）中临时量的生命周期延长规则有微妙的相互作用。下面这段
/// 代码是有效的，尽管原因并不显然：
/// ```rust
/// # type T = i32;
/// # fn foo() -> T { 42 }
/// // 持有 `foo` 返回值的那个临时量，其生命周期被延长了，
/// // 因为外围表达式不涉及函数调用。
/// let p = &foo() as *const T;
/// unsafe { p.read() };
/// ```
/// 天真地把这个转换换成 `from_ref` 则是无效的：
/// ```rust,no_run
/// # use std::ptr;
/// # type T = i32;
/// # fn foo() -> T { 42 }
/// // 持有 `foo` 返回值的那个临时量，其生命周期*没有*被延长，
/// // 因为外围表达式涉及了函数调用。
/// let p = ptr::from_ref(&foo());
/// unsafe { p.read() }; // UB！从悬垂指针读取 ⚠️
/// ```
/// 推荐的写法是：在涉及裸指针时，避免依赖生命周期延长。
/// ```rust
/// # use std::ptr;
/// # type T = i32;
/// # fn foo() -> T { 42 }
/// let x = foo();
/// let p = ptr::from_ref(&x);
/// unsafe { p.read() };
/// ```
#[inline(always)]
#[must_use]
#[stable(feature = "ptr_from_ref", since = "1.76.0")]
#[rustc_const_stable(feature = "ptr_from_ref", since = "1.76.0")]
#[rustc_never_returns_null_ptr]
#[rustc_diagnostic_item = "ptr_from_ref"]
pub const fn from_ref<T: PointeeSized>(r: &T) -> *const T {
    r
}

/// 把一个可变引用转换为裸指针。
///
/// 对于 `r: &mut T`，`from_mut(r)` 等价于 `r as *mut T`（下文提到的注意点除外），但更安全一些，
/// 因为它绝不会悄悄地改变类型或可变性，在代码被重构时尤其如此。
///
/// 调用方必须确保所指对象（pointee）的存活时间长于本函数返回的指针，否则该指针将变为悬垂。
///
/// ## 与生命周期延长（lifetime extension）的相互作用
///
/// 注意，这与尾表达式（tail expression）中临时量的生命周期延长规则有微妙的相互作用。下面这段
/// 代码是有效的，尽管原因并不显然：
/// ```rust
/// # type T = i32;
/// # fn foo() -> T { 42 }
/// // 持有 `foo` 返回值的那个临时量，其生命周期被延长了，
/// // 因为外围表达式不涉及函数调用。
/// let p = &mut foo() as *mut T;
/// unsafe { p.write(T::default()) };
/// ```
/// 天真地把这个转换换成 `from_mut` 则是无效的：
/// ```rust,no_run
/// # use std::ptr;
/// # type T = i32;
/// # fn foo() -> T { 42 }
/// // 持有 `foo` 返回值的那个临时量，其生命周期*没有*被延长，
/// // 因为外围表达式涉及了函数调用。
/// let p = ptr::from_mut(&mut foo());
/// unsafe { p.write(T::default()) }; // UB！向悬垂指针写入 ⚠️
/// ```
/// 推荐的写法是：在涉及裸指针时，避免依赖生命周期延长。
/// ```rust
/// # use std::ptr;
/// # type T = i32;
/// # fn foo() -> T { 42 }
/// let mut x = foo();
/// let p = ptr::from_mut(&mut x);
/// unsafe { p.write(T::default()) };
/// ```
#[inline(always)]
#[must_use]
#[stable(feature = "ptr_from_ref", since = "1.76.0")]
#[rustc_const_stable(feature = "ptr_from_ref", since = "1.76.0")]
#[rustc_never_returns_null_ptr]
pub const fn from_mut<T: PointeeSized>(r: &mut T) -> *mut T {
    r
}

/// 由一个指针和一个长度构造一个裸切片（raw slice）。
///
/// `len` 参数是**元素**个数，而不是字节数。
///
/// 本函数是安全的，但实际使用其返回值则是 unsafe 的。切片的安全要求见
/// [`slice::from_raw_parts`] 的文档。
///
/// [`slice::from_raw_parts`]: crate::slice::from_raw_parts
///
/// # 示例
///
/// ```rust
/// use std::ptr;
///
/// // 从指向首元素的指针出发，创建一个切片指针
/// let x = [5, 6, 7];
/// let raw_pointer = x.as_ptr();
/// let slice = ptr::slice_from_raw_parts(raw_pointer, 3);
/// assert_eq!(unsafe { &*slice }[2], 7);
/// ```
///
/// 在解引用这个裸切片之前，你必须确保指针是有效且非空的。切片引用绝不能持有 null 指针，
/// 即使它是空的。
///
/// ```rust,should_panic
/// use std::ptr;
/// let danger: *const [u8] = ptr::slice_from_raw_parts(ptr::null(), 0);
/// unsafe {
///     danger.as_ref().expect("references must not be null");
/// }
/// ```
#[inline]
#[stable(feature = "slice_from_raw_parts", since = "1.42.0")]
#[rustc_const_stable(feature = "const_slice_from_raw_parts", since = "1.64.0")]
#[rustc_diagnostic_item = "ptr_slice_from_raw_parts"]
pub const fn slice_from_raw_parts<T>(data: *const T, len: usize) -> *const [T] {
    from_raw_parts(data, len)
}

/// 由一个指针和一个长度构造一个可变裸切片（raw mutable slice）。
///
/// `len` 参数是**元素**个数，而不是字节数。
///
/// 功能与 [`slice_from_raw_parts`] 相同，区别在于返回的是可变裸切片，而非不可变裸切片。
///
/// 本函数是安全的，但实际使用其返回值则是 unsafe 的。切片的安全要求见
/// [`slice::from_raw_parts_mut`] 的文档。
///
/// [`slice::from_raw_parts_mut`]: crate::slice::from_raw_parts_mut
///
/// # 示例
///
/// ```rust
/// use std::ptr;
///
/// let x = &mut [5, 6, 7];
/// let raw_pointer = x.as_mut_ptr();
/// let slice = ptr::slice_from_raw_parts_mut(raw_pointer, 3);
///
/// unsafe {
///     (*slice)[2] = 99; // 给切片中某个下标处赋值
/// };
///
/// assert_eq!(unsafe { &*slice }[2], 99);
/// ```
///
/// 在解引用这个裸切片之前，你必须确保指针是有效且非空的。切片引用绝不能持有 null 指针，
/// 即使它是空的。
///
/// ```rust,should_panic
/// use std::ptr;
/// let danger: *mut [u8] = ptr::slice_from_raw_parts_mut(ptr::null_mut(), 0);
/// unsafe {
///     danger.as_mut().expect("references must not be null");
/// }
/// ```
#[inline]
#[stable(feature = "slice_from_raw_parts", since = "1.42.0")]
#[rustc_const_stable(feature = "const_slice_from_raw_parts_mut", since = "1.83.0")]
#[rustc_diagnostic_item = "ptr_slice_from_raw_parts_mut"]
pub const fn slice_from_raw_parts_mut<T>(data: *mut T, len: usize) -> *mut [T] {
    from_raw_parts_mut(data, len)
}

/// 交换两个同类型可变位置上的值，且不会让其中任何一个变为未初始化。
///
/// 除了以下若干例外，本函数在语义上等价于 [`mem::swap`]：
///
/// * 它操作的是裸指针而非引用。当引用可用时，应优先使用 [`mem::swap`]。
///
/// * 两个所指向的值可以重叠。如果它们确实重叠，那么将使用来自 `x` 的那段重叠内存区域。
///   下面第二个示例演示了这一点。
///
/// * 该操作是“无类型的”（untyped），意思是数据可以是未初始化的、或以其他方式违反 `T` 的要求。
///   初始化状态会被原样保留。
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `x` 和 `y` 都必须对读取和写入*两者*都是 [valid]（有效）的。即便在写入另一个指针时，它们
///   也必须保持有效。（这意味着：如果两段内存范围重叠，这两个指针相对彼此不得受到 aliasing
///   限制的约束。）
///
/// * `x` 和 `y` 都必须正确对齐。
///
/// 注意：即使 `T` 的大小为 `0`，这些指针也必须正确对齐。
///
/// [valid]: self#safety
///
/// # 示例
///
/// 交换两个不重叠的区域：
///
/// ```
/// use std::ptr;
///
/// let mut array = [0, 1, 2, 3];
///
/// let (x, y) = array.split_at_mut(2);
/// let x = x.as_mut_ptr().cast::<[u32; 2]>(); // 这是 `array[0..2]`
/// let y = y.as_mut_ptr().cast::<[u32; 2]>(); // 这是 `array[2..4]`
///
/// unsafe {
///     ptr::swap(x, y);
///     assert_eq!([2, 3, 0, 1], array);
/// }
/// ```
///
/// 交换两个重叠的区域：
///
/// ```
/// use std::ptr;
///
/// let mut array: [i32; 4] = [0, 1, 2, 3];
///
/// let array_ptr: *mut i32 = array.as_mut_ptr();
///
/// let x = array_ptr as *mut [i32; 3]; // 这是 `array[0..3]`
/// let y = unsafe { array_ptr.add(1) } as *mut [i32; 3]; // 这是 `array[1..4]`
///
/// unsafe {
///     ptr::swap(x, y);
///     // 切片的下标 `1..3` 在 `x` 与 `y` 之间重叠。
///     // 合理的结果可能是让它们为 `[2, 3]`，从而下标 `0..3` 为
///     // `[1, 2, 3]`（与 swap 之前的 `y` 相符）；或者让它们为 `[0, 1]`，
///     // 从而下标 `1..4` 为 `[0, 1, 2]`（与 swap 之前的 `x` 相符）。
///     // 本实现被定义为采取后一种选择。
///     assert_eq!([1, 0, 1, 2], array);
/// }
/// ```
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_swap", since = "1.85.0")]
#[rustc_diagnostic_item = "ptr_swap"]
pub const unsafe fn swap<T>(x: *mut T, y: *mut T) {
    // 给自己留一块临时的工作空间。
    // 我们无需担心 drop：`MaybeUninit` 被丢弃时什么也不做。
    let mut tmp = MaybeUninit::<T>::uninit();

    // 执行交换
    // SAFETY: 调用方必须保证 `x` 和 `y` 对写入有效且正确对齐。
    // `tmp` 不可能与 `x` 或 `y` 重叠，因为 `tmp` 刚刚作为一块独立的
    // allocation 分配在栈上。
    unsafe {
        copy_nonoverlapping(x, tmp.as_mut_ptr(), 1);
        copy(y, x, 1); // `x` 和 `y` 可能重叠
        copy_nonoverlapping(tmp.as_ptr(), y, 1);
    }
}

/// 在分别从 `x` 和 `y` 起的两段内存区域之间交换 `count * size_of::<T>()` 个字节。这两段区域
///*不得*重叠。
///
/// 该操作是“无类型的”（untyped），意思是数据可以是未初始化的、或以其他方式违反 `T` 的要求。
/// 初始化状态会被原样保留。
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `x` 和 `y` 都必须对读取和写入*两者*、各 `count * size_of::<T>()` 个字节是 [valid]
///（有效）的。
///
/// * `x` 和 `y` 都必须正确对齐。
///
/// * 从 `x` 起、大小为 `count * size_of::<T>()` 字节的内存区域，与从 `y` 起、同样大小的内存
///   区域*不得*重叠。
///
/// 注意：即使实际交换的大小（`count * size_of::<T>()`）为 `0`，这些指针也必须正确对齐。
///
/// [valid]: self#safety
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// use std::ptr;
///
/// let mut x = [1, 2, 3, 4];
/// let mut y = [7, 8, 9];
///
/// unsafe {
///     ptr::swap_nonoverlapping(x.as_mut_ptr(), y.as_mut_ptr(), 2);
/// }
///
/// assert_eq!(x, [7, 8, 3, 4]);
/// assert_eq!(y, [1, 2, 9]);
/// ```
#[inline]
#[stable(feature = "swap_nonoverlapping", since = "1.27.0")]
#[rustc_const_stable(feature = "const_swap_nonoverlapping", since = "1.88.0")]
#[rustc_diagnostic_item = "ptr_swap_nonoverlapping"]
#[rustc_allow_const_fn_unstable(const_eval_select)] // both implementations behave the same
#[track_caller]
pub const unsafe fn swap_nonoverlapping<T>(x: *mut T, y: *mut T, count: usize) {
    ub_checks::assert_unsafe_precondition!(
        check_library_ub,
        "ptr::swap_nonoverlapping requires that both pointer arguments are aligned and non-null \
        and the specified memory ranges do not overlap",
        (
            x: *mut () = x as *mut (),
            y: *mut () = y as *mut (),
            size: usize = size_of::<T>(),
            align: usize = align_of::<T>(),
            count: usize = count,
        ) => {
            let zero_size = size == 0 || count == 0;
            ub_checks::maybe_is_aligned_and_not_null(x, align, zero_size)
                && ub_checks::maybe_is_aligned_and_not_null(y, align, zero_size)
                && ub_checks::maybe_is_nonoverlapping(x, y, size, count)
        }
    );

    const_eval_select!(
        @capture[T] { x: *mut T, y: *mut T, count: usize }:
        if const {
            // 在编译期，我们不需要下面那些特殊代码。
            // SAFETY: 与本函数相同的前置条件
            unsafe { swap_nonoverlapping_const(x, y, count) }
        } else {
            // 这里经由一个切片，有助于让 codegen 知道大小可以放进 `isize`
            let slice = slice_from_raw_parts_mut(x, count);
            // SAFETY: 这一整块都可从该指针读取，意味着它属于同一块
            // allocation，因此不可能超过 isize::MAX 字节。
            let bytes = unsafe { mem::size_of_val_raw::<[T]>(slice) };
            if let Some(bytes) = NonZero::new(bytes) {
                // SAFETY: 这是同样的范围，只是用不同的类型来表达，
                // 所以它们仍然是不重叠的。
                unsafe { swap_nonoverlapping_bytes(x.cast(), y.cast(), bytes) };
            }
        }
    )
}

/// 行为与安全条件同 [`swap_nonoverlapping`]
#[inline]
const unsafe fn swap_nonoverlapping_const<T>(x: *mut T, y: *mut T, count: usize) {
    let mut i = 0;
    while i < count {
        // SAFETY: 依据前置条件，`i` 在界内，因为它小于 `n`
        let x = unsafe { x.add(i) };
        // SAFETY: 依据前置条件，`i` 在界内，因为它小于 `n`；
        // 又因为两段范围不重叠，它与 `x` 不同
        let y = unsafe { y.add(i) };

        // SAFETY: 我们拿到的指针总是可供读/写的（包括已对齐），
        // 而且这里没有任何东西会 panic，所以对 drop 是安全的。
        unsafe {
            // 注意，关键之处在于这些必须用 `copy_nonoverlapping`，
            // 而不是 `read`/`write`，以避免 T 有 padding 时触发 #134713。
            let mut temp = MaybeUninit::<T>::uninit();
            copy_nonoverlapping(x, temp.as_mut_ptr(), 1);
            copy_nonoverlapping(y, x, 1);
            copy_nonoverlapping(temp.as_ptr(), y, 1);
        }

        i += 1;
    }
}

// 不要让 MIR 内联这个函数，因为我们确实希望它保留其 noalias 元数据
#[rustc_no_mir_inline]
#[inline]
fn swap_chunk<const N: usize>(x: &mut MaybeUninit<[u8; N]>, y: &mut MaybeUninit<[u8; N]>) {
    let a = *x;
    let b = *y;
    *x = b;
    *y = a;
}

#[inline]
unsafe fn swap_nonoverlapping_bytes(x: *mut u8, y: *mut u8, bytes: NonZero<usize>) {
    // 与 `swap_nonoverlapping::<[u8; N]>` 相同。
    unsafe fn swap_nonoverlapping_chunks<const N: usize>(
        x: *mut MaybeUninit<[u8; N]>,
        y: *mut MaybeUninit<[u8; N]>,
        chunks: NonZero<usize>,
    ) {
        let chunks = chunks.get();
        for i in 0..chunks {
            // SAFETY: i 在 [0, chunks) 范围内，所以这些 add 和解引用都在界内。
            unsafe { swap_chunk(&mut *x.add(i), &mut *y.add(i)) };
        }
    }

    // 与 `swap_nonoverlapping_bytes` 相同，但最多接受 1+2+4=7 个字节
    #[inline]
    unsafe fn swap_nonoverlapping_short(x: *mut u8, y: *mut u8, bytes: NonZero<usize>) {
        // 自动向量化代码对尾部的处理有时会表现为逐元素的行为，
        // 详见 <https://github.com/rust-lang/rust/issues/134946>。
        // 通过按不同大小（而非按字节循环）来交换，
        // 我们确保不会落到比如说“连续七次逐字节复制”的局面。

        let bytes = bytes.get();
        let mut i = 0;
        macro_rules! swap_prefix {
            ($($n:literal)+) => {$(
                if (bytes & $n) != 0 {
                    // SAFETY: `i` 所置位的比特只可能是 bytes 中已置位的那些，
                    // 所以这些 `add` 都在 `bytes` 的界内。但 `$n` 对应的比特
                    // 尚未被置位，所以 `swap_chunk` 将读写的那 `$n` 个字节
                    // 落在可用范围内。
                    unsafe { swap_chunk::<$n>(&mut*x.add(i).cast(), &mut*y.add(i).cast()) };
                    i |= $n;
                }
            )+};
        }
        swap_prefix!(4 2 1);
        debug_assert_eq!(i, bytes);
    }

    const CHUNK_SIZE: usize = size_of::<*const ()>();
    let bytes = bytes.get();

    let chunks = bytes / CHUNK_SIZE;
    let tail = bytes % CHUNK_SIZE;
    if let Some(chunks) = NonZero::new(chunks) {
        // SAFETY: 这是 bytes/CHUNK_SIZE*CHUNK_SIZE 个字节，它 <= bytes，
        // 所以落在我们这段不重叠字节的范围内。
        unsafe { swap_nonoverlapping_chunks::<CHUNK_SIZE>(x.cast(), y.cast(), chunks) };
    }
    if let Some(tail) = NonZero::new(tail) {
        const { assert!(CHUNK_SIZE <= 8) };
        let delta = chunks * CHUNK_SIZE;
        // SAFETY: 因为取了余数，尾部长度小于 CHUNK_SIZE，
        // 而由 const 断言可知 CHUNK_SIZE 至多为 8，所以 tail <= 7
        unsafe { swap_nonoverlapping_short(x.add(delta), y.add(delta), tail) };
    }
}

/// 把 `src` 移动进 `dst` 所指向的位置，并返回 `dst` 先前的值。
///
/// 两个值都不会被析构。
///
/// 本函数在语义上等价于 [`mem::replace`]，区别仅在于它操作的是裸指针而非引用。当引用可用时，
/// 应优先使用 [`mem::replace`]。
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `dst` 必须对读取和写入*两者*都是 [valid]（有效）的。
///
/// * `dst` 必须正确对齐。
///
/// * `dst` 必须指向一个已正确初始化的、类型为 `T` 的值。
///
/// 注意：即使 `T` 的大小为 `0`，该指针也必须正确对齐。
///
/// [valid]: self#safety
///
/// # 示例
///
/// ```
/// use std::ptr;
///
/// let mut rust = vec!['b', 'u', 's', 't'];
///
/// // `mem::replace` 也能达到同样的效果，且无需 unsafe 块。
/// let b = unsafe {
///     ptr::replace(&mut rust[0], 'r')
/// };
///
/// assert_eq!(b, 'b');
/// assert_eq!(rust, &['r', 'u', 's', 't']);
/// ```
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_replace", since = "1.83.0")]
#[rustc_diagnostic_item = "ptr_replace"]
#[track_caller]
pub const unsafe fn replace<T>(dst: *mut T, src: T) -> T {
    // SAFETY: 调用方必须保证 `dst` 可被转为可变引用
    //（即对写入有效、已对齐、已初始化），且不可能与 `src` 重叠，
    // 因为 `dst` 必须指向一块不同的 allocation。
    unsafe {
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::replace requires that the pointer argument is aligned and non-null",
            (
                addr: *const () = dst as *const (),
                align: usize = align_of::<T>(),
                is_zst: bool = T::IS_ZST,
            ) => ub_checks::maybe_is_aligned_and_not_null(addr, align, is_zst)
        );
        mem::replace(&mut *dst, src)
    }
}

/// 从 `src` 处读取值，但不移动它。这会使 `src` 处的内存保持不变。
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `src` 必须对读取是 [valid]（有效）的：非空、已分配未释放、不越界、并带有覆盖该范围的
///   provenance。
///
/// * `src` 必须正确对齐。如果做不到这一点，请用 [`read_unaligned`]。
///
/// * `src` 必须指向一个已正确初始化的、类型为 `T` 的值（`read` 会按位复制出该值，要求它
///   已初始化）。
///
/// 注意：即使 `T` 的大小为 `0`，该指针也必须正确对齐。
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// let x = 12;
/// let y = &x as *const i32;
///
/// unsafe {
///     assert_eq!(std::ptr::read(y), 12);
/// }
/// ```
///
/// 手动实现 [`mem::swap`]：
///
/// ```
/// use std::ptr;
///
/// fn swap<T>(a: &mut T, b: &mut T) {
///     unsafe {
///         // 在 `tmp` 中创建 `a` 处值的按位副本。
///         let tmp = ptr::read(a);
///
///         // 在这一点上退出（无论是显式 return，还是调用一个会 panic 的
///         // 函数），都会导致 `tmp` 中的值被析构，而与此同时 `a` 仍引用着
///         // 同一个值。如果 `T` 不是 `Copy`，这可能触发未定义行为。
///
///         // 在 `a` 中创建 `b` 处值的按位副本。
///         // 这是安全的，因为可变引用不会别名（alias）。
///         ptr::copy_nonoverlapping(b, a, 1);
///
///         // 与上同理，在这里退出可能触发未定义行为，
///         // 因为同一个值同时被 `a` 和 `b` 引用。
///
///         // 把 `tmp` 移动进 `b`。
///         ptr::write(b, tmp);
///
///         // `tmp` 已被移走（`write` 取得其第二个参数的所有权），
///         // 所以这里不会隐式析构任何东西。
///     }
/// }
///
/// let mut foo = "foo".to_owned();
/// let mut bar = "bar".to_owned();
///
/// swap(&mut foo, &mut bar);
///
/// assert_eq!(foo, "bar");
/// assert_eq!(bar, "foo");
/// ```
///
/// ## 返回值的所有权（Ownership of the Returned Value）
///
/// 无论 `T` 是否为 [`Copy`]，`read` 都会创建 `T` 的按位副本。如果 `T` 不是 [`Copy`]，那么
/// 同时使用返回值和 `*src` 处的值会违反内存安全（即所有权问题：值会被析构两次）。注意，
/// 给 `*src` 赋值也算作一次使用，因为它会试图析构 `*src` 处的值。
///
/// 可用 [`write()`] 来覆盖数据而不触发它被析构。
///
/// ```
/// use std::ptr;
///
/// let mut s = String::from("foo");
/// unsafe {
///     // `s2` 现在指向与 `s` 相同的底层内存。
///     let mut s2: String = ptr::read(&s);
///
///     assert_eq!(s2, "foo");
///
///     // 给 `s2` 赋值会导致它原来的值被析构。在这一点之后，
///     // `s` 不得再被使用，因为其底层内存已被释放。
///     s2 = String::default();
///     assert_eq!(s2, "");
///
///     // 给 `s` 赋值会导致旧值被再次析构，
///     // 从而引发未定义行为。
///     // s = String::from("bar"); // 错误
///
///     // 可用 `ptr::write` 来覆盖一个值而不析构它。
///     ptr::write(&mut s, String::from("bar"));
/// }
///
/// assert_eq!(s, "bar");
/// ```
///
/// [valid]: self#safety
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_ptr_read", since = "1.71.0")]
#[track_caller]
#[rustc_diagnostic_item = "ptr_read"]
pub const unsafe fn read<T>(src: *const T) -> T {
    // 按语义来说，本可以经由 `copy_nonoverlapping` 和 `MaybeUninit` 实现，
    // 就像 PR #109035 之前那样。调用 `assume_init` 足以表明这是一次有类型的操作。

    // 然而，截至 2023 年 3 月，编译器还无法利用这一信息。于是这里的实现改用了
    // 一个 intrinsic，它在 MIR 中下降为 `_0 = *src`，以解决以下几个问题：
    //
    // - 在 `copy_nonoverlapping` 之后使用 `MaybeUninit::assume_init`，并不能把
    //   无类型复制变成有类型的 load。因此 LLVM 中生成的 `load` 拿不到诸如 `!range`
    //（#73258）、`!nonnull`、`!noundef` 之类的各种元数据，导致优化变差。
    // - 经由额外的局部变量会产生多次额外复制，即便在优化过的 MIR 中也是如此。
    //   （忽略 StorageLive/Dead，该 intrinsic 只是一条 MIR 语句，而此前的实现是八条。）
    //   LLVM 有时能把它们优化掉，但由于 `read` 处在众多东西的核心，一开始就不产生它们，
    //   能改善我们交给后端的东西。例如 `mem::replace::<Big>` 此前会发出 4 个 `alloca`
    //   和 6 个 `memcpy`，而现在是 1 个 `alloc` 和 3 个 `memcpy`。
    // - 总的来说，这一做法使我们不再招致更多形如“`read(p)` 比 `*p` 更差”的 bug
    //   （比如 #106369），因为它让二者在后端（或其他 MIR 消费者）看来完全一样。
    //
    // 将来 MIR 优化的增强，很可能允许它回到此前的实现，而不再使用 intrinsic。

    // SAFETY: 调用方必须保证 `src` 对读取有效。
    unsafe {
        #[cfg(debug_assertions)] // 总是启用代价太高（暂时如此？）
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::read requires that the pointer argument is aligned and non-null",
            (
                addr: *const () = src as *const (),
                align: usize = align_of::<T>(),
                is_zst: bool = T::IS_ZST,
            ) => ub_checks::maybe_is_aligned_and_not_null(addr, align, is_zst)
        );
        crate::intrinsics::read_via_copy(src)
    }
}

/// 从 `src` 处读取值，但不移动它。这会使 `src` 处的内存保持不变。
///
/// 与 [`read`] 不同，`read_unaligned` 可用于未对齐（unaligned）的指针。
///
/// # 安全性（Safety）
///
/// 若违反以下任一条件，行为即为未定义。调用方必须维护以下全部不变量：
///
/// * `src` 必须对读取是 [valid]（有效）的：非空、已分配未释放、不越界、并带有覆盖该范围的
///   provenance。（注意此处*不*要求对齐。）
///
/// * `src` 必须指向一个已正确初始化的、类型为 `T` 的值。
///
/// 与 [`read`] 一样，无论 `T` 是否为 [`Copy`]，`read_unaligned` 都会创建 `T` 的按位副本。
/// 如果 `T` 不是 [`Copy`]，那么同时使用返回值和 `*src` 处的值可能[违反内存安全][read-ownership]。
///
/// [read-ownership]: read#ownership-of-the-returned-value
/// [valid]: self#safety
///
/// ## 关于 `packed` 结构体
///
/// 试图用诸如 `&packed.unaligned as *const FieldType` 这样的表达式去创建指向 `unaligned`
///（未对齐）结构体字段的裸指针，会先创建一个中间的未对齐引用，然后再把它转换为裸指针。
/// 这个引用是临时的、且会被立即转换，这一点无关紧要——因为编译器始终期望引用是正确对齐的。
/// 结果就是：使用 `&packed.unaligned as *const FieldType` 会在你的程序中立即造成
///*未定义行为*。
///
/// 正确的做法是使用 `&raw const` 语法来创建指针。你可以把这样构造出的指针与本函数一起使用。
///
/// 一个“不该怎么做”、以及它与 `read_unaligned` 关系的示例：
///
/// ```
/// #[repr(packed, C)]
/// struct Packed {
///     _padding: u8,
///     unaligned: u32,
/// }
///
/// let packed = Packed {
///     _padding: 0x00,
///     unaligned: 0x01020304,
/// };
///
/// // 取一个未对齐的 32 位整数的地址。
/// // 与 `&packed.unaligned as *const _` 不同，这没有未定义行为。
/// let unaligned = &raw const packed.unaligned;
///
/// let v = unsafe { std::ptr::read_unaligned(unaligned) };
/// assert_eq!(v, 0x01020304);
/// ```
///
/// 不过，用例如 `packed.unaligned` 直接访问未对齐字段是安全的。
///
/// # 示例
///
/// 从一个字节缓冲区读取一个 `usize` 值：
///
/// ```
/// fn read_usize(x: &[u8]) -> usize {
///     assert!(x.len() >= size_of::<usize>());
///
///     let ptr = x.as_ptr() as *const usize;
///
///     unsafe { ptr.read_unaligned() }
/// }
/// ```
#[inline]
#[stable(feature = "ptr_unaligned", since = "1.17.0")]
#[rustc_const_stable(feature = "const_ptr_read", since = "1.71.0")]
#[track_caller]
#[rustc_diagnostic_item = "ptr_read_unaligned"]
pub const unsafe fn read_unaligned<T>(src: *const T) -> T {
    let mut tmp = MaybeUninit::<T>::uninit();
    // SAFETY: 调用方必须保证 `src` 对读取有效。
    // `src` 不可能与 `tmp` 重叠，因为 `tmp` 刚刚作为一块独立的
    // allocation 分配在栈上。
    //
    // 另外，由于我们刚刚往 `tmp` 写入了一个有效的值，它必定已正确初始化。
    unsafe {
        copy_nonoverlapping(src as *const u8, tmp.as_mut_ptr() as *mut u8, size_of::<T>());
        tmp.assume_init()
    }
}

/// Overwrites a memory location with the given value without reading or
/// dropping the old value.
///
/// `write` does not drop the contents of `dst`. This is safe, but it could leak
/// allocations or resources, so care should be taken not to overwrite an object
/// that should be dropped.
///
/// Additionally, it does not drop `src`. Semantically, `src` is moved into the
/// location pointed to by `dst`.
///
/// This is appropriate for initializing uninitialized memory, or overwriting
/// memory that has previously been [`read`] from.
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// * `dst` must be [valid] for writes.
///
/// * `dst` must be properly aligned. Use [`write_unaligned`] if this is not the
///   case.
///
/// Note that even if `T` has size `0`, the pointer must be properly aligned.
///
/// [valid]: self#safety
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let mut x = 0;
/// let y = &mut x as *mut i32;
/// let z = 12;
///
/// unsafe {
///     std::ptr::write(y, z);
///     assert_eq!(std::ptr::read(y), 12);
/// }
/// ```
///
/// Manually implement [`mem::swap`]:
///
/// ```
/// use std::ptr;
///
/// fn swap<T>(a: &mut T, b: &mut T) {
///     unsafe {
///         // Create a bitwise copy of the value at `a` in `tmp`.
///         let tmp = ptr::read(a);
///
///         // Exiting at this point (either by explicitly returning or by
///         // calling a function which panics) would cause the value in `tmp` to
///         // be dropped while the same value is still referenced by `a`. This
///         // could trigger undefined behavior if `T` is not `Copy`.
///
///         // Create a bitwise copy of the value at `b` in `a`.
///         // This is safe because mutable references cannot alias.
///         ptr::copy_nonoverlapping(b, a, 1);
///
///         // As above, exiting here could trigger undefined behavior because
///         // the same value is referenced by `a` and `b`.
///
///         // Move `tmp` into `b`.
///         ptr::write(b, tmp);
///
///         // `tmp` has been moved (`write` takes ownership of its second argument),
///         // so nothing is dropped implicitly here.
///     }
/// }
///
/// let mut foo = "foo".to_owned();
/// let mut bar = "bar".to_owned();
///
/// swap(&mut foo, &mut bar);
///
/// assert_eq!(foo, "bar");
/// assert_eq!(bar, "foo");
/// ```
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
#[rustc_diagnostic_item = "ptr_write"]
#[track_caller]
pub const unsafe fn write<T>(dst: *mut T, src: T) {
    // Semantically, it would be fine for this to be implemented as a
    // `copy_nonoverlapping` and appropriate drop suppression of `src`.

    // However, implementing via that currently produces more MIR than is ideal.
    // Using an intrinsic keeps it down to just the simple `*dst = move src` in
    // MIR (11 statements shorter, at the time of writing), and also allows
    // `src` to stay an SSA value in codegen_ssa, rather than a memory one.

    // SAFETY: the caller must guarantee that `dst` is valid for writes.
    // `dst` cannot overlap `src` because the caller has mutable access
    // to `dst` while `src` is owned by this function.
    unsafe {
        #[cfg(debug_assertions)] // Too expensive to always enable (for now?)
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::write requires that the pointer argument is aligned and non-null",
            (
                addr: *mut () = dst as *mut (),
                align: usize = align_of::<T>(),
                is_zst: bool = T::IS_ZST,
            ) => ub_checks::maybe_is_aligned_and_not_null(addr, align, is_zst)
        );
        intrinsics::write_via_move(dst, src)
    }
}

/// Overwrites a memory location with the given value without reading or
/// dropping the old value.
///
/// Unlike [`write()`], the pointer may be unaligned.
///
/// `write_unaligned` does not drop the contents of `dst`. This is safe, but it
/// could leak allocations or resources, so care should be taken not to overwrite
/// an object that should be dropped.
///
/// Additionally, it does not drop `src`. Semantically, `src` is moved into the
/// location pointed to by `dst`.
///
/// This is appropriate for initializing uninitialized memory, or overwriting
/// memory that has previously been read with [`read_unaligned`].
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// * `dst` must be [valid] for writes.
///
/// [valid]: self#safety
///
/// ## On `packed` structs
///
/// Attempting to create a raw pointer to an `unaligned` struct field with
/// an expression such as `&packed.unaligned as *const FieldType` creates an
/// intermediate unaligned reference before converting that to a raw pointer.
/// That this reference is temporary and immediately cast is inconsequential
/// as the compiler always expects references to be properly aligned.
/// As a result, using `&packed.unaligned as *const FieldType` causes immediate
/// *undefined behavior* in your program.
///
/// Instead, you must use the `&raw mut` syntax to create the pointer.
/// You may use that constructed pointer together with this function.
///
/// An example of how to do it and how this relates to `write_unaligned` is:
///
/// ```
/// #[repr(packed, C)]
/// struct Packed {
///     _padding: u8,
///     unaligned: u32,
/// }
///
/// let mut packed: Packed = unsafe { std::mem::zeroed() };
///
/// // Take the address of a 32-bit integer which is not aligned.
/// // In contrast to `&packed.unaligned as *mut _`, this has no undefined behavior.
/// let unaligned = &raw mut packed.unaligned;
///
/// unsafe { std::ptr::write_unaligned(unaligned, 42) };
///
/// assert_eq!({packed.unaligned}, 42); // `{...}` forces copying the field instead of creating a reference.
/// ```
///
/// Accessing unaligned fields directly with e.g. `packed.unaligned` is safe however
/// (as can be seen in the `assert_eq!` above).
///
/// # Examples
///
/// Write a `usize` value to a byte buffer:
///
/// ```
/// fn write_usize(x: &mut [u8], val: usize) {
///     assert!(x.len() >= size_of::<usize>());
///
///     let ptr = x.as_mut_ptr() as *mut usize;
///
///     unsafe { ptr.write_unaligned(val) }
/// }
/// ```
#[inline]
#[stable(feature = "ptr_unaligned", since = "1.17.0")]
#[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
#[rustc_diagnostic_item = "ptr_write_unaligned"]
#[track_caller]
pub const unsafe fn write_unaligned<T>(dst: *mut T, src: T) {
    // SAFETY: the caller must guarantee that `dst` is valid for writes.
    // `dst` cannot overlap `src` because the caller has mutable access
    // to `dst` while `src` is owned by this function.
    unsafe {
        copy_nonoverlapping((&raw const src) as *const u8, dst as *mut u8, size_of::<T>());
        // We are calling the intrinsic directly to avoid function calls in the generated code.
        intrinsics::forget(src);
    }
}

/// Performs a volatile read of the value from `src` without moving it.
///
/// Volatile operations are intended to act on I/O memory. As such, they are considered externally
/// observable events (just like syscalls, but less opaque), and are guaranteed to not be elided or
/// reordered by the compiler across other externally observable events. With this in mind, there
/// are two cases of usage that need to be distinguished:
///
/// - When a volatile operation is used for memory inside an [allocation], it behaves exactly like
///   [`read`], except for the additional guarantee that it won't be elided or reordered (see
///   above). This implies that the operation will actually access memory and not e.g. be lowered to
///   reusing data from a previous read. Other than that, all the usual rules for memory accesses
///   apply (including provenance).  In particular, just like in C, whether an operation is volatile
///   has no bearing whatsoever on questions involving concurrent accesses from multiple threads.
///   Volatile accesses behave exactly like non-atomic accesses in that regard.
///
/// - Volatile operations, however, may also be used to access memory that is _outside_ of any Rust
///   allocation. In this use-case, the pointer does *not* have to be [valid] for reads. This is
///   typically used for CPU and peripheral registers that must be accessed via an I/O memory
///   mapping, most commonly at fixed addresses reserved by the hardware. These often have special
///   semantics associated to their manipulation, and cannot be used as general purpose memory.
///   Here, any address value is possible, including 0 and [`usize::MAX`], so long as the semantics
///   of such a read are well-defined by the target hardware. The provenance of the pointer is
///   irrelevant, and it can be created with [`without_provenance`]. The access must not trap. It
///   can cause side-effects, but those must not affect Rust-allocated memory in any way. This
///   access is still not considered [atomic], and as such it cannot be used for inter-thread
///   synchronization.
///
/// Note that volatile memory operations where T is a zero-sized type are noops and may be ignored.
///
/// [allocation]: crate::ptr#allocated-object
/// [atomic]: crate::sync::atomic#memory-model-for-atomic-accesses
///
/// # Safety
///
/// Like [`read`], `read_volatile` creates a bitwise copy of `T`, regardless of whether `T` is
/// [`Copy`]. If `T` is not [`Copy`], using both the returned value and the value at `*src` can
/// [violate memory safety][read-ownership]. However, storing non-[`Copy`] types in volatile memory
/// is almost certainly incorrect.
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// * `src` must be either [valid] for reads, or it must point to memory outside of all Rust
///   allocations and reading from that memory must:
///   - not trap, and
///   - not cause any memory inside a Rust allocation to be modified.
///
/// * `src` must be properly aligned.
///
/// * Reading from `src` must produce a properly initialized value of type `T`.
///
/// Note that even if `T` has size `0`, the pointer must be properly aligned.
///
/// [valid]: self#safety
/// [read-ownership]: read#ownership-of-the-returned-value
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let x = 12;
/// let y = &x as *const i32;
///
/// unsafe {
///     assert_eq!(std::ptr::read_volatile(y), 12);
/// }
/// ```
#[inline]
#[stable(feature = "volatile", since = "1.9.0")]
#[track_caller]
#[rustc_diagnostic_item = "ptr_read_volatile"]
pub unsafe fn read_volatile<T>(src: *const T) -> T {
    // SAFETY: the caller must uphold the safety contract for `volatile_load`.
    unsafe {
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::read_volatile requires that the pointer argument is aligned",
            (
                addr: *const () = src as *const (),
                align: usize = align_of::<T>(),
            ) => ub_checks::maybe_is_aligned(addr, align)
        );
        intrinsics::volatile_load(src)
    }
}

/// Performs a volatile write of a memory location with the given value without reading or dropping
/// the old value.
///
/// Volatile operations are intended to act on I/O memory. As such, they are considered externally
/// observable events (just like syscalls), and are guaranteed to not be elided or reordered by the
/// compiler across other externally observable events. With this in mind, there are two cases of
/// usage that need to be distinguished:
///
/// - When a volatile operation is used for memory inside an [allocation], it behaves exactly like
///   [`write`][write()], except for the additional guarantee that it won't be elided or reordered
///   (see above). This implies that the operation will actually access memory and not e.g. be
///   lowered to a register access. Other than that, all the usual rules for memory accesses apply
///   (including provenance). In particular, just like in C, whether an operation is volatile has no
///   bearing whatsoever on questions involving concurrent access from multiple threads. Volatile
///   accesses behave exactly like non-atomic accesses in that regard.
///
/// - Volatile operations, however, may also be used to access memory that is _outside_ of any Rust
///   allocation. In this use-case, the pointer does *not* have to be [valid] for writes. This is
///   typically used for CPU and peripheral registers that must be accessed via an I/O memory
///   mapping, most commonly at fixed addresses reserved by the hardware. These often have special
///   semantics associated to their manipulation, and cannot be used as general purpose memory.
///   Here, any address value is possible, including 0 and [`usize::MAX`], so long as the semantics
///   of such a write are well-defined by the target hardware. The provenance of the pointer is
///   irrelevant, and it can be created with [`without_provenance`]. The access must not trap. It
///   can cause side-effects, but those must not affect Rust-allocated memory in any way. This
///   access is still not considered [atomic], and as such it cannot be used for inter-thread
///   synchronization.
///
/// Note that volatile memory operations on zero-sized types (e.g., if a zero-sized type is passed
/// to `write_volatile`) are noops and may be ignored.
///
/// `write_volatile` does not drop the contents of `dst`. This is safe, but it could leak
/// allocations or resources, so care should be taken not to overwrite an object that should be
/// dropped when operating on Rust memory. Additionally, it does not drop `src`. Semantically, `src`
/// is moved into the location pointed to by `dst`.
///
/// [allocation]: crate::ptr#allocated-object
/// [atomic]: crate::sync::atomic#memory-model-for-atomic-accesses
///
/// # Safety
///
/// Behavior is undefined if any of the following conditions are violated:
///
/// * `dst` must be either [valid] for writes, or it must point to memory outside of all Rust
///   allocations and writing to that memory must:
///   - not trap, and
///   - not cause any memory inside a Rust allocation to be modified.
///
/// * `dst` must be properly aligned.
///
/// Note that even if `T` has size `0`, the pointer must be properly aligned.
///
/// [valid]: self#safety
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// let mut x = 0;
/// let y = &mut x as *mut i32;
/// let z = 12;
///
/// unsafe {
///     std::ptr::write_volatile(y, z);
///     assert_eq!(std::ptr::read_volatile(y), 12);
/// }
/// ```
#[inline]
#[stable(feature = "volatile", since = "1.9.0")]
#[rustc_diagnostic_item = "ptr_write_volatile"]
#[track_caller]
pub unsafe fn write_volatile<T>(dst: *mut T, src: T) {
    // SAFETY: the caller must uphold the safety contract for `volatile_store`.
    unsafe {
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::write_volatile requires that the pointer argument is aligned",
            (
                addr: *mut () = dst as *mut (),
                align: usize = align_of::<T>(),
            ) => ub_checks::maybe_is_aligned(addr, align)
        );
        intrinsics::volatile_store(dst, src);
    }
}

/// Calculate an element-offset that increases a pointer's alignment.
///
/// Calculate an element-offset (not byte-offset) that when added to a given pointer `p`, increases `p`'s alignment to at least the given alignment `a`.
///
/// # Safety
/// `a` must be a power of two.
///
/// # Notes
/// This implementation has been carefully tailored to not panic. It is UB for this to panic.
/// The only real change that can be made here is change of `INV_TABLE_MOD_16` and associated
/// constants.
///
/// If we ever decide to make it possible to call the intrinsic with `a` that is not a
/// power-of-two, it will probably be more prudent to just change to a naive implementation rather
/// than trying to adapt this to accommodate that change.
///
/// Any questions go to @nagisa.
#[allow(ptr_to_integer_transmute_in_consts)]
pub(crate) unsafe fn align_offset<T: Sized>(p: *const T, a: usize) -> usize {
    // FIXME(#75598): Direct use of these intrinsics improves codegen significantly at opt-level <=
    // 1, where the method versions of these operations are not inlined.
    use intrinsics::{
        assume, cttz_nonzero, exact_div, mul_with_overflow, unchecked_rem, unchecked_shl,
        unchecked_shr, unchecked_sub, wrapping_add, wrapping_mul, wrapping_sub,
    };

    /// Calculate multiplicative modular inverse of `x` modulo `m`.
    ///
    /// This implementation is tailored for `align_offset` and has following preconditions:
    ///
    /// * `m` is a power-of-two;
    /// * `x < m`; (if `x ≥ m`, pass in `x % m` instead)
    ///
    /// Implementation of this function shall not panic. Ever.
    #[inline]
    const unsafe fn mod_inv(x: usize, m: usize) -> usize {
        /// Multiplicative modular inverse table modulo 2⁴ = 16.
        ///
        /// Note, that this table does not contain values where inverse does not exist (i.e., for
        /// `0⁻¹ mod 16`, `2⁻¹ mod 16`, etc.)
        const INV_TABLE_MOD_16: [u8; 8] = [1, 11, 13, 7, 9, 3, 5, 15];
        /// Modulo for which the `INV_TABLE_MOD_16` is intended.
        const INV_TABLE_MOD: usize = 16;

        // SAFETY: `m` is required to be a power-of-two, hence non-zero.
        let m_minus_one = unsafe { unchecked_sub(m, 1) };
        let mut inverse = INV_TABLE_MOD_16[(x & (INV_TABLE_MOD - 1)) >> 1] as usize;
        let mut mod_gate = INV_TABLE_MOD;
        // We iterate "up" using the following formula:
        //
        // $$ xy ≡ 1 (mod 2ⁿ) → xy (2 - xy) ≡ 1 (mod 2²ⁿ) $$
        //
        // This application needs to be applied at least until `2²ⁿ ≥ m`, at which point we can
        // finally reduce the computation to our desired `m` by taking `inverse mod m`.
        //
        // This computation is `O(log log m)`, which is to say, that on 64-bit machines this loop
        // will always finish in at most 4 iterations.
        loop {
            // y = y * (2 - xy) mod n
            //
            // Note, that we use wrapping operations here intentionally – the original formula
            // uses e.g., subtraction `mod n`. It is entirely fine to do them `mod
            // usize::MAX` instead, because we take the result `mod n` at the end
            // anyway.
            if mod_gate >= m {
                break;
            }
            inverse = wrapping_mul(inverse, wrapping_sub(2usize, wrapping_mul(x, inverse)));
            let (new_gate, overflow) = mul_with_overflow(mod_gate, mod_gate);
            if overflow {
                break;
            }
            mod_gate = new_gate;
        }
        inverse & m_minus_one
    }

    let stride = size_of::<T>();

    let addr: usize = p.addr();

    // SAFETY: `a` is a power-of-two, therefore non-zero.
    let a_minus_one = unsafe { unchecked_sub(a, 1) };

    if stride == 0 {
        // SPECIAL_CASE: handle 0-sized types. No matter how many times we step, the address will
        // stay the same, so no offset will be able to align the pointer unless it is already
        // aligned. This branch _will_ be optimized out as `stride` is known at compile-time.
        let p_mod_a = addr & a_minus_one;
        return if p_mod_a == 0 { 0 } else { usize::MAX };
    }

    // SAFETY: `stride == 0` case has been handled by the special case above.
    let a_mod_stride = unsafe { unchecked_rem(a, stride) };
    if a_mod_stride == 0 {
        // SPECIAL_CASE: In cases where the `a` is divisible by `stride`, byte offset to align a
        // pointer can be computed more simply through `-p (mod a)`. In the off-chance the byte
        // offset is not a multiple of `stride`, the input pointer was misaligned and no pointer
        // offset will be able to produce a `p` aligned to the specified `a`.
        //
        // The naive `-p (mod a)` equation inhibits LLVM's ability to select instructions
        // like `lea`. We compute `(round_up_to_next_alignment(p, a) - p)` instead. This
        // redistributes operations around the load-bearing, but pessimizing `and` instruction
        // sufficiently for LLVM to be able to utilize the various optimizations it knows about.
        //
        // LLVM handles the branch here particularly nicely. If this branch needs to be evaluated
        // at runtime, it will produce a mask `if addr_mod_stride == 0 { 0 } else { usize::MAX }`
        // in a branch-free way and then bitwise-OR it with whatever result the `-p mod a`
        // computation produces.

        let aligned_address = wrapping_add(addr, a_minus_one) & wrapping_sub(0, a);
        let byte_offset = wrapping_sub(aligned_address, addr);
        // FIXME: Remove the assume after <https://github.com/llvm/llvm-project/issues/62502>
        // SAFETY: Masking by `-a` can only affect the low bits, and thus cannot have reduced
        // the value by more than `a-1`, so even though the intermediate values might have
        // wrapped, the byte_offset is always in `[0, a)`.
        unsafe { assume(byte_offset < a) };

        // SAFETY: `stride == 0` case has been handled by the special case above.
        let addr_mod_stride = unsafe { unchecked_rem(addr, stride) };

        return if addr_mod_stride == 0 {
            // SAFETY: `stride` is non-zero. This is guaranteed to divide exactly as well, because
            // addr has been verified to be aligned to the original type’s alignment requirements.
            unsafe { exact_div(byte_offset, stride) }
        } else {
            usize::MAX
        };
    }

    // GENERAL_CASE: From here on we’re handling the very general case where `addr` may be
    // misaligned, there isn’t an obvious relationship between `stride` and `a` that we can take an
    // advantage of, etc. This case produces machine code that isn’t particularly high quality,
    // compared to the special cases above. The code produced here is still within the realm of
    // miracles, given the situations this case has to deal with.

    // SAFETY: a is power-of-two hence non-zero. stride == 0 case is handled above.
    // FIXME(const-hack) replace with min
    let gcdpow = unsafe {
        let x = cttz_nonzero(stride);
        let y = cttz_nonzero(a);
        if x < y { x } else { y }
    };
    // SAFETY: gcdpow has an upper-bound that’s at most the number of bits in a `usize`.
    let gcd = unsafe { unchecked_shl(1usize, gcdpow) };
    // SAFETY: gcd is always greater or equal to 1.
    if addr & unsafe { unchecked_sub(gcd, 1) } == 0 {
        // This branch solves for the following linear congruence equation:
        //
        // ` p + so = 0 mod a `
        //
        // `p` here is the pointer value, `s` - stride of `T`, `o` offset in `T`s, and `a` - the
        // requested alignment.
        //
        // With `g = gcd(a, s)`, and the above condition asserting that `p` is also divisible by
        // `g`, we can denote `a' = a/g`, `s' = s/g`, `p' = p/g`, then this becomes equivalent to:
        //
        // ` p' + s'o = 0 mod a' `
        // ` o = (a' - (p' mod a')) * (s'^-1 mod a') `
        //
        // The first term is "the relative alignment of `p` to `a`" (divided by the `g`), the
        // second term is "how does incrementing `p` by `s` bytes change the relative alignment of
        // `p`" (again divided by `g`). Division by `g` is necessary to make the inverse well
        // formed if `a` and `s` are not co-prime.
        //
        // Furthermore, the result produced by this solution is not "minimal", so it is necessary
        // to take the result `o mod lcm(s, a)`. This `lcm(s, a)` is the same as `a'`.

        // SAFETY: `gcdpow` has an upper-bound not greater than the number of trailing 0-bits in
        // `a`.
        let a2 = unsafe { unchecked_shr(a, gcdpow) };
        // SAFETY: `a2` is non-zero. Shifting `a` by `gcdpow` cannot shift out any of the set bits
        // in `a` (of which it has exactly one).
        let a2minus1 = unsafe { unchecked_sub(a2, 1) };
        // SAFETY: `gcdpow` has an upper-bound not greater than the number of trailing 0-bits in
        // `a`.
        let s2 = unsafe { unchecked_shr(stride & a_minus_one, gcdpow) };
        // SAFETY: `gcdpow` has an upper-bound not greater than the number of trailing 0-bits in
        // `a`. Furthermore, the subtraction cannot overflow, because `a2 = a >> gcdpow` will
        // always be strictly greater than `(p % a) >> gcdpow`.
        let minusp2 = unsafe { unchecked_sub(a2, unchecked_shr(addr & a_minus_one, gcdpow)) };
        // SAFETY: `a2` is a power-of-two, as proven above. `s2` is strictly less than `a2`
        // because `(s % a) >> gcdpow` is strictly less than `a >> gcdpow`.
        return wrapping_mul(minusp2, unsafe { mod_inv(s2, a2) }) & a2minus1;
    }

    // Cannot be aligned at all.
    usize::MAX
}

/// Compares raw pointers for equality.
///
/// This is the same as using the `==` operator, but less generic:
/// the arguments have to be `*const T` raw pointers,
/// not anything that implements `PartialEq`.
///
/// This can be used to compare `&T` references (which coerce to `*const T` implicitly)
/// by their address rather than comparing the values they point to
/// (which is what the `PartialEq for &T` implementation does).
///
/// When comparing wide pointers, both the address and the metadata are tested for equality.
/// However, note that comparing trait object pointers (`*const dyn Trait`) is unreliable: pointers
/// to values of the same underlying type can compare inequal (because vtables are duplicated in
/// multiple codegen units), and pointers to values of *different* underlying type can compare equal
/// (since identical vtables can be deduplicated within a codegen unit).
///
/// # Examples
///
/// ```
/// use std::ptr;
///
/// let five = 5;
/// let other_five = 5;
/// let five_ref = &five;
/// let same_five_ref = &five;
/// let other_five_ref = &other_five;
///
/// assert!(five_ref == same_five_ref);
/// assert!(ptr::eq(five_ref, same_five_ref));
///
/// assert!(five_ref == other_five_ref);
/// assert!(!ptr::eq(five_ref, other_five_ref));
/// ```
///
/// Slices are also compared by their length (fat pointers):
///
/// ```
/// let a = [1, 2, 3];
/// assert!(std::ptr::eq(&a[..3], &a[..3]));
/// assert!(!std::ptr::eq(&a[..2], &a[..3]));
/// assert!(!std::ptr::eq(&a[0..2], &a[1..3]));
/// ```
#[stable(feature = "ptr_eq", since = "1.17.0")]
#[inline(always)]
#[must_use = "pointer comparison produces a value"]
#[rustc_diagnostic_item = "ptr_eq"]
#[allow(ambiguous_wide_pointer_comparisons)] // it's actually clear here
pub fn eq<T: PointeeSized>(a: *const T, b: *const T) -> bool {
    a == b
}

/// Compares the *addresses* of the two pointers for equality,
/// ignoring any metadata in fat pointers.
///
/// If the arguments are thin pointers of the same type,
/// then this is the same as [`eq`].
///
/// # Examples
///
/// ```
/// use std::ptr;
///
/// let whole: &[i32; 3] = &[1, 2, 3];
/// let first: &i32 = &whole[0];
///
/// assert!(ptr::addr_eq(whole, first));
/// assert!(!ptr::eq::<dyn std::fmt::Debug>(whole, first));
/// ```
#[stable(feature = "ptr_addr_eq", since = "1.76.0")]
#[inline(always)]
#[must_use = "pointer comparison produces a value"]
pub fn addr_eq<T: PointeeSized, U: PointeeSized>(p: *const T, q: *const U) -> bool {
    (p as *const ()) == (q as *const ())
}

/// Compares the *addresses* of the two function pointers for equality.
///
/// This is the same as `f == g`, but using this function makes clear that the potentially
/// surprising semantics of function pointer comparison are involved.
///
/// There are **very few guarantees** about how functions are compiled and they have no intrinsic
/// “identity”; in particular, this comparison:
///
/// * May return `true` unexpectedly, in cases where functions are equivalent.
///
///   For example, the following program is likely (but not guaranteed) to print `(true, true)`
///   when compiled with optimization:
///
///   ```
///   let f: fn(i32) -> i32 = |x| x;
///   let g: fn(i32) -> i32 = |x| x + 0;  // different closure, different body
///   let h: fn(u32) -> u32 = |x| x + 0;  // different signature too
///   dbg!(std::ptr::fn_addr_eq(f, g), std::ptr::fn_addr_eq(f, h)); // not guaranteed to be equal
///   ```
///
/// * May return `false` in any case.
///
///   This is particularly likely with generic functions but may happen with any function.
///   (From an implementation perspective, this is possible because functions may sometimes be
///   processed more than once by the compiler, resulting in duplicate machine code.)
///
/// Despite these false positives and false negatives, this comparison can still be useful.
/// Specifically, if
///
/// * `T` is the same type as `U`, `T` is a [subtype] of `U`, or `U` is a [subtype] of `T`, and
/// * `ptr::fn_addr_eq(f, g)` returns true,
///
/// then calling `f` and calling `g` will be equivalent.
///
///
/// # Examples
///
/// ```
/// use std::ptr;
///
/// fn a() { println!("a"); }
/// fn b() { println!("b"); }
/// assert!(!ptr::fn_addr_eq(a as fn(), b as fn()));
/// ```
///
/// [subtype]: https://doc.rust-lang.org/reference/subtyping.html
#[stable(feature = "ptr_fn_addr_eq", since = "1.85.0")]
#[inline(always)]
#[must_use = "function pointer comparison produces a value"]
pub fn fn_addr_eq<T: FnPtr, U: FnPtr>(f: T, g: U) -> bool {
    f.addr() == g.addr()
}

/// Hash a raw pointer.
///
/// This can be used to hash a `&T` reference (which coerces to `*const T` implicitly)
/// by its address rather than the value it points to
/// (which is what the `Hash for &T` implementation does).
///
/// # Examples
///
/// ```
/// use std::hash::{DefaultHasher, Hash, Hasher};
/// use std::ptr;
///
/// let five = 5;
/// let five_ref = &five;
///
/// let mut hasher = DefaultHasher::new();
/// ptr::hash(five_ref, &mut hasher);
/// let actual = hasher.finish();
///
/// let mut hasher = DefaultHasher::new();
/// (five_ref as *const i32).hash(&mut hasher);
/// let expected = hasher.finish();
///
/// assert_eq!(actual, expected);
/// ```
#[stable(feature = "ptr_hash", since = "1.35.0")]
pub fn hash<T: PointeeSized, S: hash::Hasher>(hashee: *const T, into: &mut S) {
    use crate::hash::Hash;
    hashee.hash(into);
}

#[stable(feature = "fnptr_impls", since = "1.4.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<F: FnPtr> PartialEq for F {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.addr() == other.addr()
    }
}
#[stable(feature = "fnptr_impls", since = "1.4.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<F: FnPtr> Eq for F {}

#[stable(feature = "fnptr_impls", since = "1.4.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<F: FnPtr> PartialOrd for F {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.addr().partial_cmp(&other.addr())
    }
}
#[stable(feature = "fnptr_impls", since = "1.4.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<F: FnPtr> Ord for F {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.addr().cmp(&other.addr())
    }
}

#[stable(feature = "fnptr_impls", since = "1.4.0")]
impl<F: FnPtr> hash::Hash for F {
    fn hash<HH: hash::Hasher>(&self, state: &mut HH) {
        state.write_usize(self.addr() as _)
    }
}

#[stable(feature = "fnptr_impls", since = "1.4.0")]
impl<F: FnPtr> fmt::Pointer for F {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::pointer_fmt_inner(self.addr() as _, f)
    }
}

#[stable(feature = "fnptr_impls", since = "1.4.0")]
impl<F: FnPtr> fmt::Debug for F {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::pointer_fmt_inner(self.addr() as _, f)
    }
}

/// Creates a `const` raw pointer to a place, without creating an intermediate reference.
///
/// `addr_of!(expr)` is equivalent to `&raw const expr`. The macro is *soft-deprecated*;
/// use `&raw const` instead.
///
/// It is still an open question under which conditions writing through an `addr_of!`-created
/// pointer is permitted. If the place `expr` evaluates to is based on a raw pointer, then the
/// result of `addr_of!` inherits all permissions from that raw pointer. However, if the place is
/// based on a reference, local variable, or `static`, then until all details are decided, the same
/// rules as for shared references apply: it is UB to write through a pointer created with this
/// operation, except for bytes located inside an `UnsafeCell`. Use `&raw mut` (or [`addr_of_mut`])
/// to create a raw pointer that definitely permits mutation.
///
/// Creating a reference with `&`/`&mut` is only allowed if the pointer is properly aligned
/// and points to initialized data. For cases where those requirements do not hold,
/// raw pointers should be used instead. However, `&expr as *const _` creates a reference
/// before casting it to a raw pointer, and that reference is subject to the same rules
/// as all other references. This macro can create a raw pointer *without* creating
/// a reference first.
///
/// See [`addr_of_mut`] for how to create a pointer to uninitialized data.
/// Doing that with `addr_of` would not make much sense since one could only
/// read the data, and that would be Undefined Behavior.
///
/// # Safety
///
/// The `expr` in `addr_of!(expr)` is evaluated as a place expression, but never loads from the
/// place or requires the place to be dereferenceable. This means that `addr_of!((*ptr).field)`
/// still requires the projection to `field` to be in-bounds, using the same rules as [`offset`].
/// However, `addr_of!(*ptr)` is defined behavior even if `ptr` is null, dangling, or misaligned.
///
/// Note that `Deref`/`Index` coercions (and their mutable counterparts) are applied inside
/// `addr_of!` like everywhere else, in which case a reference is created to call `Deref::deref` or
/// `Index::index`, respectively. The statements above only apply when no such coercions are
/// applied.
///
/// [`offset`]: pointer::offset
///
/// # Example
///
/// **Correct usage: Creating a pointer to unaligned data**
///
/// ```
/// use std::ptr;
///
/// #[repr(packed)]
/// struct Packed {
///     f1: u8,
///     f2: u16,
/// }
///
/// let packed = Packed { f1: 1, f2: 2 };
/// // `&packed.f2` would create an unaligned reference, and thus be Undefined Behavior!
/// let raw_f2 = ptr::addr_of!(packed.f2);
/// assert_eq!(unsafe { raw_f2.read_unaligned() }, 2);
/// ```
///
/// **Incorrect usage: Out-of-bounds fields projection**
///
/// ```rust,no_run
/// use std::ptr;
///
/// #[repr(C)]
/// struct MyStruct {
///     field1: i32,
///     field2: i32,
/// }
///
/// let ptr: *const MyStruct = ptr::null();
/// let fieldptr = unsafe { ptr::addr_of!((*ptr).field2) }; // Undefined Behavior ⚠️
/// ```
///
/// The field projection `.field2` would offset the pointer by 4 bytes,
/// but the pointer is not in-bounds of an allocation for 4 bytes,
/// so this offset is Undefined Behavior.
/// See the [`offset`] docs for a full list of requirements for inbounds pointer arithmetic; the
/// same requirements apply to field projections, even inside `addr_of!`. (In particular, it makes
/// no difference whether the pointer is null or dangling.)
#[stable(feature = "raw_ref_macros", since = "1.51.0")]
#[rustc_macro_transparency = "semiopaque"]
pub macro addr_of($place:expr) {
    &raw const $place
}

/// Creates a `mut` raw pointer to a place, without creating an intermediate reference.
///
/// `addr_of_mut!(expr)` is equivalent to `&raw mut expr`. The macro is *soft-deprecated*;
/// use `&raw mut` instead.
///
/// Creating a reference with `&`/`&mut` is only allowed if the pointer is properly aligned
/// and points to initialized data. For cases where those requirements do not hold,
/// raw pointers should be used instead. However, `&mut expr as *mut _` creates a reference
/// before casting it to a raw pointer, and that reference is subject to the same rules
/// as all other references. This macro can create a raw pointer *without* creating
/// a reference first.
///
/// # Safety
///
/// The `expr` in `addr_of_mut!(expr)` is evaluated as a place expression, but never loads from the
/// place or requires the place to be dereferenceable. This means that `addr_of_mut!((*ptr).field)`
/// still requires the projection to `field` to be in-bounds, using the same rules as [`offset`].
/// However, `addr_of_mut!(*ptr)` is defined behavior even if `ptr` is null, dangling, or misaligned.
///
/// Note that `Deref`/`Index` coercions (and their mutable counterparts) are applied inside
/// `addr_of_mut!` like everywhere else, in which case a reference is created to call `Deref::deref`
/// or `Index::index`, respectively. The statements above only apply when no such coercions are
/// applied.
///
/// [`offset`]: pointer::offset
///
/// # Examples
///
/// **Correct usage: Creating a pointer to unaligned data**
///
/// ```
/// use std::ptr;
///
/// #[repr(packed)]
/// struct Packed {
///     f1: u8,
///     f2: u16,
/// }
///
/// let mut packed = Packed { f1: 1, f2: 2 };
/// // `&mut packed.f2` would create an unaligned reference, and thus be Undefined Behavior!
/// let raw_f2 = ptr::addr_of_mut!(packed.f2);
/// unsafe { raw_f2.write_unaligned(42); }
/// assert_eq!({packed.f2}, 42); // `{...}` forces copying the field instead of creating a reference.
/// ```
///
/// **Correct usage: Creating a pointer to uninitialized data**
///
/// ```rust
/// use std::{ptr, mem::MaybeUninit};
///
/// struct Demo {
///     field: bool,
/// }
///
/// let mut uninit = MaybeUninit::<Demo>::uninit();
/// // `&uninit.as_mut().field` would create a reference to an uninitialized `bool`,
/// // and thus be Undefined Behavior!
/// let f1_ptr = unsafe { ptr::addr_of_mut!((*uninit.as_mut_ptr()).field) };
/// unsafe { f1_ptr.write(true); }
/// let init = unsafe { uninit.assume_init() };
/// ```
///
/// **Incorrect usage: Out-of-bounds fields projection**
///
/// ```rust,no_run
/// use std::ptr;
///
/// #[repr(C)]
/// struct MyStruct {
///     field1: i32,
///     field2: i32,
/// }
///
/// let ptr: *mut MyStruct = ptr::null_mut();
/// let fieldptr = unsafe { ptr::addr_of_mut!((*ptr).field2) }; // Undefined Behavior ⚠️
/// ```
///
/// The field projection `.field2` would offset the pointer by 4 bytes,
/// but the pointer is not in-bounds of an allocation for 4 bytes,
/// so this offset is Undefined Behavior.
/// See the [`offset`] docs for a full list of requirements for inbounds pointer arithmetic; the
/// same requirements apply to field projections, even inside `addr_of_mut!`. (In particular, it
/// makes no difference whether the pointer is null or dangling.)
#[stable(feature = "raw_ref_macros", since = "1.51.0")]
#[rustc_macro_transparency = "semiopaque"]
pub macro addr_of_mut($place:expr) {
    &raw mut $place
}
