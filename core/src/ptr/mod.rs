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
//! # 安全性(Safety）
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
/// # 安全性(Safety）
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
/// # 安全性(Safety）
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
/// /// # 安全性(Safety）
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
/// # 安全性(Safety）
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
/// # 安全性(Safety）
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
/// # 安全性(Safety）
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
/// *不得*重叠。
///
/// 该操作是“无类型的”（untyped），意思是数据可以是未初始化的、或以其他方式违反 `T` 的要求。
/// 初始化状态会被原样保留。
///
/// # 安全性(Safety）
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
/// # 安全性(Safety）
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
/// # 安全性(Safety）
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
/// # 安全性(Safety）
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
/// *未定义行为*。
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

/// 用给定值覆盖一个内存位置，不读取也不丢弃(drop)旧值。
///
/// `write` 不会 drop `dst` 原本指向的内容。这一语义本身不会读取旧值，因此适合裸指针
/// 写入；但如果旧值本应运行析构逻辑，它可能泄漏 allocation 或其他资源。调用方必须自己
/// 知道该位置是否未初始化，或者旧值是否已经通过 [`read`] 等方式被取走。
///
/// 另外，`write` 也不会 drop `src`。语义上，`src` 被移动到 `dst` 指向的位置；
/// `write` 取得第二个参数的所有权后，原来的 `src` 位置不会再被隐式析构。
///
/// 因此，本函数常用于初始化未初始化内存，或覆盖之前已经被 [`read`] 取走值的内存。
///
/// # 安全性(Safety）
///
/// 如果违反以下任一条件，行为即为未定义：
///
/// * `dst` 必须对写入 [valid]。它必须带有允许写入该内存的 provenance，并且覆盖的
///   字节范围必须位于同一个有效 allocation 内。
///
/// * `dst` 必须为 `T` 正确对齐。如果不能满足对齐要求，应使用 [`write_unaligned`]。
///
/// 注意：即便 `T` 的大小为 0，指针也必须正确对齐；零大小类型不读写字节，但引用和指针
/// API 的对齐不变量仍然会被编译器优化依赖。
///
/// [valid]: self#safety
///
/// # 示例
///
/// 基本用法：
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
///         // 如果此时退出（无论显式 return，还是调用会 panic 的函数），`tmp` 中的值会
///         // 被 drop，而同一个值仍由 `a` 引用。若 `T` 不是 `Copy`，这可能触发未定义行为。
///
///         // 在 `a` 中创建 `b` 处值的按位副本。
///         // 这里成立是因为可变引用不能互相 alias。
///         ptr::copy_nonoverlapping(b, a, 1);
///
///         // 同理，如果此时退出，同一个值会同时由 `a` 和 `b` 引用，可能触发未定义行为。
///
///         // 把 `tmp` 移动进 `b`。
///         ptr::write(b, tmp);
///
///         // `tmp` 已被移动（`write` 接管第二个参数的所有权），此处不会再隐式 drop。
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
    // 从语义上讲，这可以用 `copy_nonoverlapping` 加上适当抑制 `src` 的 drop 来实现。
    //
    // 但那样当前会产生偏多 MIR。直接使用 intrinsic 可以把 MIR 降到简单的
    // `*dst = move src`（撰写时少 11 条语句），并让 `src` 在 codegen_ssa 中保持
    // SSA 值，而不是落到内存对象。
    //
    // SAFETY: 调用方必须保证 `dst` 对写入有效且满足 `T` 的对齐要求。`dst` 不能与
    // `src` 重叠，因为调用方在传入时拥有对 `dst` 的可变访问，而 `src` 的所有权已经
    // 转移给本函数。
    unsafe {
        #[cfg(debug_assertions)] // 目前始终启用的开销过高。
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

/// 用给定值覆盖一个内存位置，不读取也不丢弃(drop)旧值。
///
/// 与 [`write()`] 不同，目标指针可以不对齐。
///
/// `write_unaligned` 不会 drop `dst` 原本指向的内容。这避免了读取旧值，但如果旧值
/// 本应析构，仍可能泄漏 allocation 或资源；调用方必须自行确保覆盖该位置是正确的。
///
/// 另外，它也不会 drop `src`。语义上，`src` 被移动到 `dst` 指向的位置。
///
/// 本函数适合初始化未初始化内存，或覆盖之前已经用 [`read_unaligned`] 取走值的内存。
///
/// # 安全性(Safety）
///
/// 如果违反以下任一条件，行为即为未定义：
///
/// * `dst` 必须对写入 [valid]。它必须带有允许写入该内存的 provenance，且写入的
///   字节范围必须落在同一个有效 allocation 内。
///
/// [valid]: self#safety
///
/// ## 关于 `packed` 结构体
///
/// 如果用 `&packed.unaligned as *const FieldType` 这类表达式为 `unaligned` 字段创建
/// 裸指针，会先创建一个中间的未对齐引用，然后再把它转换为裸指针。这个引用只是临时值并
/// 立即被 cast 并不重要：编译器始终假设引用已经正确对齐。因此，使用
/// `&packed.unaligned as *const FieldType` 会立刻在程序中造成 *undefined behavior*。
///
/// 应改用 `&raw mut` 语法创建指针。这样构造出的指针可以与本函数配合使用。
///
/// 下面示例展示这种写法以及它与 `write_unaligned` 的关系：
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
/// // 取得一个未对齐 32 位整数的地址。
/// // 与 `&packed.unaligned as *mut _` 不同，这不会造成未定义行为。
/// let unaligned = &raw mut packed.unaligned;
///
/// unsafe { std::ptr::write_unaligned(unaligned, 42) };
///
/// assert_eq!({packed.unaligned}, 42); // `{...}` 强制复制字段，而不是创建引用。
/// ```
///
/// 不过，像 `packed.unaligned` 这样直接访问未对齐字段本身是安全的（上面的 `assert_eq!`
/// 就展示了这一点），因为编译器会生成不经由未对齐引用的访问。
///
/// # 示例
///
/// 向字节缓冲区写入一个 `usize` 值：
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
    // SAFETY: 调用方必须保证 `dst` 对写入有效。`dst` 不能与 `src` 重叠，因为调用方
    // 在传入时拥有对 `dst` 的可变访问，而 `src` 的所有权已经转移给本函数。
    unsafe {
        copy_nonoverlapping((&raw const src) as *const u8, dst as *mut u8, size_of::<T>());
        // 直接调用 intrinsic，以避免生成代码中出现额外函数调用。
        intrinsics::forget(src);
    }
}

/// 对 `src` 执行 volatile 读取，按位复制出一个 `T`，但不移动原位置的值。
///
/// Volatile 操作主要用于 I/O 内存。它们会被视为外部可观察事件（类似系统调用，但更少
/// 不透明性），编译器保证不会删除它们，也不会让它们跨过其他外部可观察事件发生重排。
/// 因此必须区分两类使用场景：
///
/// - 当 volatile 操作用于 [allocation] 内的内存时，它除了“不会被删除或重排”这一额外保证外，
///   行为与 [`read`] 完全相同。这意味着操作会真的访问内存，而不是复用之前读取到的数据。
///   除此之外，普通内存访问的所有规则仍然适用，包括 provenance、对齐、初始化以及 aliasing。
///   特别是，和 C 中一样，`volatile` 与多线程并发访问没有任何同步关系；在这一点上，
///   volatile 访问仍然是非 atomic 访问。
///
/// - Volatile 操作也可用于访问任何 Rust allocation 之外的内存。在这种场景下，指针不必对读取
///   [valid]。典型用途是经由 I/O 内存映射访问 CPU 或外设寄存器，通常位于硬件保留的固定
///   地址。这些地址的读写常带有特殊硬件语义，不能当作通用内存使用。此时任意地址值都可能
///   合法，包括 0 和 [`usize::MAX`]，前提是目标硬件明确定义了这次读取的语义。指针的
///   provenance 与该访问无关，可以用 [`without_provenance`] 创建。访问不得 trap；它可以有
///   副作用，但这些副作用不得以任何方式修改 Rust allocation 内的内存。该访问仍不属于
///   [atomic] 访问，不能用于线程间同步。
///
/// 注意：当 `T` 是零大小类型时，volatile 内存操作是 no-op，可能被忽略。
///
/// [allocation]: crate::ptr#allocated-object
/// [atomic]: crate::sync::atomic#memory-model-for-atomic-accesses
///
/// # 安全性(Safety）
///
/// 与 [`read`] 一样，无论 `T` 是否实现 [`Copy`]，`read_volatile` 都会创建一个 `T` 的
/// 按位副本。如果 `T` 不是 [`Copy`]，同时使用返回值和 `*src` 处的原值可能
/// [破坏内存安全][read-ownership]。不过，把非 [`Copy`] 类型放在 volatile 内存中几乎一定
/// 是错误用法。
///
/// 如果违反以下任一条件，行为即为未定义：
///
/// * `src` 必须满足二者之一：要么对读取 [valid]；要么指向所有 Rust allocation 之外的
///   内存，并且读取该内存必须：
///   - 不会 trap；且
///   - 不会导致任何 Rust allocation 内的内存被修改。
///
/// * `src` 必须为 `T` 正确对齐。
///
/// * 从 `src` 读取必须产生一个已正确初始化的 `T` 值。
///
/// 注意：即便 `T` 的大小为 0，指针也必须正确对齐。
///
/// [valid]: self#safety
/// [read-ownership]: read#ownership-of-the-returned-value
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
///     assert_eq!(std::ptr::read_volatile(y), 12);
/// }
/// ```
#[inline]
#[stable(feature = "volatile", since = "1.9.0")]
#[track_caller]
#[rustc_diagnostic_item = "ptr_read_volatile"]
pub unsafe fn read_volatile<T>(src: *const T) -> T {
    // SAFETY: 调用方必须维护 `volatile_load` 的安全契约。
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

/// 对某个内存位置执行 volatile 写入，用给定值覆盖旧值，但不读取也不 drop 旧值。
///
/// Volatile 操作主要用于 I/O 内存。它们会被视为外部可观察事件（类似系统调用），编译器
/// 保证不会删除它们，也不会让它们跨过其他外部可观察事件发生重排。因此必须区分两类
/// 使用场景：
///
/// - 当 volatile 操作用于 [allocation] 内的内存时，它除了“不会被删除或重排”这一额外保证外，
///   行为与 [`write`][write()] 完全相同。这意味着操作会真的访问内存，而不是被降级成寄存器
///   写入等形式。除此之外，普通内存访问的所有规则仍然适用，包括 provenance、对齐和
///   aliasing。特别是，和 C 中一样，`volatile` 与多线程并发访问没有任何同步关系；在这一点上，
///   volatile 访问仍然是非 atomic 访问。
///
/// - Volatile 操作也可用于访问任何 Rust allocation 之外的内存。在这种场景下，指针不必对写入
///   [valid]。典型用途是经由 I/O 内存映射访问 CPU 或外设寄存器，通常位于硬件保留的固定
///   地址。这些地址的读写常带有特殊硬件语义，不能当作通用内存使用。此时任意地址值都可能
///   合法，包括 0 和 [`usize::MAX`]，前提是目标硬件明确定义了这次写入的语义。指针的
///   provenance 与该访问无关，可以用 [`without_provenance`] 创建。访问不得 trap；它可以有
///   副作用，但这些副作用不得以任何方式修改 Rust allocation 内的内存。该访问仍不属于
///   [atomic] 访问，不能用于线程间同步。
///
/// 注意：对零大小类型执行 volatile 内存操作（例如给 `write_volatile` 传入零大小类型）
/// 是 no-op，可能被忽略。
///
/// `write_volatile` 不会 drop `dst` 原本指向的内容。对 Rust 内存操作时，如果旧值本应
/// 析构，这可能泄漏 allocation 或资源。它也不会 drop `src`；语义上，`src` 被移动到
/// `dst` 指向的位置。
///
/// [allocation]: crate::ptr#allocated-object
/// [atomic]: crate::sync::atomic#memory-model-for-atomic-accesses
///
/// # 安全性(Safety）
///
/// 如果违反以下任一条件，行为即为未定义：
///
/// * `dst` 必须满足二者之一：要么对写入 [valid]；要么指向所有 Rust allocation 之外的
///   内存，并且写入该内存必须：
///   - 不会 trap；且
///   - 不会导致任何 Rust allocation 内的内存被修改。
///
/// * `dst` 必须为 `T` 正确对齐。
///
/// 注意：即便 `T` 的大小为 0，指针也必须正确对齐。
///
/// [valid]: self#safety
///
/// # 示例
///
/// 基本用法：
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
    // SAFETY: 调用方必须维护 `volatile_store` 的安全契约。
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

/// 计算一个以元素为单位的偏移量，用来提高指针的对齐程度。
///
/// 返回的偏移量不是字节偏移，而是 `T` 元素个数。把它加到给定指针 `p` 上后，
/// 结果指针的地址会至少满足给定对齐值 `a`；如果无法通过元素步长做到这一点，则返回
/// `usize::MAX`。
///
/// # 安全性(Safety）
/// `a` 必须是 2 的幂。调用方若传入其他值，会破坏本函数内部对 unchecked 算术和
/// 模逆运算的前置假设。
///
/// # 说明
///
/// 这个实现经过专门设计，不能 panic；如果它 panic，则调用该 intrinsic 的路径会产生 UB。
/// 这里真正适合调整的只有 `INV_TABLE_MOD_16` 及其相关常量。
///
/// 如果将来决定允许用非 2 的幂的 `a` 调用该 intrinsic，更稳妥的做法大概是改用朴素实现，
/// 而不是强行扩展当前这套为 2 的幂对齐定制的实现。
///
/// 如有疑问请联系 @nagisa。
#[allow(ptr_to_integer_transmute_in_consts)]
pub(crate) unsafe fn align_offset<T: Sized>(p: *const T, a: usize) -> usize {
    // FIXME(#75598): 在 opt-level <= 1 时，这些操作的方法版本不会被内联；直接使用这些
    // intrinsic 能显著改善 codegen。
    use intrinsics::{
        assume, cttz_nonzero, exact_div, mul_with_overflow, unchecked_rem, unchecked_shl,
        unchecked_shr, unchecked_sub, wrapping_add, wrapping_mul, wrapping_sub,
    };

    /// 计算 `x` 在模 `m` 意义下的乘法逆元。
    ///
    /// 这个实现专门服务于 `align_offset`，并依赖以下前置条件：
    ///
    /// * `m` 是 2 的幂；
    /// * `x < m`；如果 `x ≥ m`，调用方应传入 `x % m`。
    ///
    /// 本函数实现绝不能 panic。
    #[inline]
    const unsafe fn mod_inv(x: usize, m: usize) -> usize {
        /// 模 2⁴ = 16 意义下的乘法逆元表。
        ///
        /// 注意：这个表不包含逆元不存在的取值，例如 `0⁻¹ mod 16`、`2⁻¹ mod 16` 等。
        const INV_TABLE_MOD_16: [u8; 8] = [1, 11, 13, 7, 9, 3, 5, 15];
        /// `INV_TABLE_MOD_16` 所针对的模数。
        const INV_TABLE_MOD: usize = 16;

        // SAFETY: 前置要求 `m` 是 2 的幂，因此非零。
        let m_minus_one = unsafe { unchecked_sub(m, 1) };
        let mut inverse = INV_TABLE_MOD_16[(x & (INV_TABLE_MOD - 1)) >> 1] as usize;
        let mut mod_gate = INV_TABLE_MOD;
        // 使用下面的公式逐步“抬升”模数：
        //
        // $$ xy ≡ 1 (mod 2ⁿ) → xy (2 - xy) ≡ 1 (mod 2²ⁿ) $$
        //
        // 至少要应用到 `2²ⁿ ≥ m`，此时就可以通过取 `inverse mod m` 把结果约化到目标模数 `m`。
        //
        // 该计算复杂度为 `O(log log m)`；也就是说，在 64 位机器上这个循环最多 4 次迭代就会结束。
        loop {
            // y = y * (2 - xy) mod n
            //
            // 这里有意使用 wrapping 运算：原始公式使用的是例如模 `n` 意义下的减法。
            // 先在 `usize::MAX` 的回绕语义下计算也没问题，因为最后仍会把结果取 `mod n`。
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

    // SAFETY: `a` 是 2 的幂，因此非零。
    let a_minus_one = unsafe { unchecked_sub(a, 1) };

    if stride == 0 {
        // 特殊情况:处理零大小类型。无论前进多少步，地址都保持不变；因此除非指针
        // 本来已经对齐，否则不存在能让它对齐的偏移量。`stride` 在编译期已知，所以此分支会被优化掉。
        let p_mod_a = addr & a_minus_one;
        return if p_mod_a == 0 { 0 } else { usize::MAX };
    }

    // SAFETY: `stride == 0` 的情况已由上面的特殊分支处理。
    let a_mod_stride = unsafe { unchecked_rem(a, stride) };
    if a_mod_stride == 0 {
        // 特殊情况:当 `a` 能被 `stride` 整除时，用于对齐指针的字节偏移可以更简单地
        // 通过 `-p (mod a)` 计算。若这个字节偏移恰好不是 `stride` 的倍数，则输入指针
        // 本身相对 `T` 就是未对齐的，任何以元素为单位的偏移都无法得到满足指定 `a` 的 `p`。
        //
        // 朴素的 `-p (mod a)` 方程会妨碍 LLVM 选择 `lea` 等指令。这里改为计算
        // `(round_up_to_next_alignment(p, a) - p)`，把运算重新分布到关键但会悲观化的
        // `and` 指令周围，使 LLVM 仍能利用它已知的多种优化。
        //
        // LLVM 对这里的分支处理得很好。如果该分支需要在运行时求值，它会以无分支方式生成
        // `if addr_mod_stride == 0 { 0 } else { usize::MAX }` 这个 mask，然后把它与
        // `-p mod a` 计算得到的结果做 bitwise-OR。

        let aligned_address = wrapping_add(addr, a_minus_one) & wrapping_sub(0, a);
        let byte_offset = wrapping_sub(aligned_address, addr);
        // FIXME: <https://github.com/llvm/llvm-project/issues/62502> 解决后移除该 assume。
        // SAFETY: 用 `-a` 做掩码只会影响低位，因此最多把值减少 `a - 1`；所以即便中间值
        // 发生 wrapping，`byte_offset` 也始终位于 `[0, a)`。
        unsafe { assume(byte_offset < a) };

        // SAFETY: `stride == 0` 的情况已由上面的特殊分支处理。
        let addr_mod_stride = unsafe { unchecked_rem(addr, stride) };

        return if addr_mod_stride == 0 {
            // SAFETY: `stride` 非零。这里也保证能整除，因为 addr 已经被验证满足原始类型的对齐要求。
            unsafe { exact_div(byte_offset, stride) }
        } else {
            usize::MAX
        };
    }

    // 一般情况:从这里开始处理最一般的情况：`addr` 可能未对齐，`stride` 与 `a`
    // 之间没有明显可利用的关系，等等。相比上面的特殊分支，这里生成的机器码质量不会特别高；
    // 但考虑到它必须覆盖的场景，结果仍然相当不错。

    // SAFETY: `a` 是 2 的幂，因此非零；`stride == 0` 的情况已在上面处理。
    // FIXME(const-hack): 替换为 min。
    let gcdpow = unsafe {
        let x = cttz_nonzero(stride);
        let y = cttz_nonzero(a);
        if x < y { x } else { y }
    };
    // SAFETY: `gcdpow` 的上界最多是 `usize` 的位数。
    let gcd = unsafe { unchecked_shl(1usize, gcdpow) };
    // SAFETY: `gcd` 始终大于或等于 1。
    if addr & unsafe { unchecked_sub(gcd, 1) } == 0 {
        // 该分支求解下面的线性同余方程：
        //
        // ` p + so = 0 mod a `
        //
        // 这里 `p` 是指针值，`s` 是 `T` 的步长(stride)，`o` 是以 `T` 为单位的偏移量，
        // `a` 是请求的对齐值。
        //
        // 令 `g = gcd(a, s)`，且上面的条件已经断言 `p` 也能被 `g` 整除。记
        // `a' = a/g`、`s' = s/g`、`p' = p/g`，则该方程等价于：
        //
        // ` p' + s'o = 0 mod a' `
        // ` o = (a' - (p' mod a')) * (s'^-1 mod a') `
        //
        // 第一项表示“`p` 相对于 `a` 的相对对齐”（再除以 `g`）；第二项表示“把 `p`
        // 增加 `s` 字节会如何改变 `p` 的相对对齐”（同样除以 `g`）。当 `a` 与 `s`
        // 不互素时，必须先除以 `g`，逆元才是良定义的。
        //
        // 此外，该解产生的结果并不一定“最小”，因此需要取 `o mod lcm(s, a)`。
        // 这里的 `lcm(s, a)` 与 `a'` 相同。

        // SAFETY: `gcdpow` 的上界不超过 `a` 的尾随 0 位数。
        let a2 = unsafe { unchecked_shr(a, gcdpow) };
        // SAFETY: `a2` 非零。把 `a` 右移 `gcdpow` 不会移掉 `a` 中任何置位 bit；
        // `a` 是 2 的幂，只有一个置位 bit。
        let a2minus1 = unsafe { unchecked_sub(a2, 1) };
        // SAFETY: `gcdpow` 的上界不超过 `a` 的尾随 0 位数。
        let s2 = unsafe { unchecked_shr(stride & a_minus_one, gcdpow) };
        // SAFETY: `gcdpow` 的上界不超过 `a` 的尾随 0 位数。此外减法不会溢出，因为
        // `a2 = a >> gcdpow` 始终严格大于 `(p % a) >> gcdpow`。
        let minusp2 = unsafe { unchecked_sub(a2, unchecked_shr(addr & a_minus_one, gcdpow)) };
        // SAFETY: 如上所证，`a2` 是 2 的幂。`s2` 严格小于 `a2`，因为
        // `(s % a) >> gcdpow` 严格小于 `a >> gcdpow`。
        return wrapping_mul(minusp2, unsafe { mod_inv(s2, a2) }) & a2minus1;
    }

    // 完全无法对齐。
    usize::MAX
}

/// 比较两个裸指针是否相等。
///
/// 这与使用 `==` 运算符相同，但泛型程度更低：参数必须是 `*const T` 裸指针，
/// 而不是任意实现了 `PartialEq` 的值。
///
/// 这可用于按地址比较 `&T` 引用（它们会隐式强转为 `*const T`），而不是比较引用所指向的
/// 值；后者才是 `PartialEq for &T` 的行为。
///
/// 比较宽指针时，地址和 metadata 都会参与相等性判断。不过要注意：比较 trait object 指针
/// （`*const dyn Trait`）并不可靠。指向同一底层类型的值的指针可能比较为不相等，因为
/// vtable 可能在多个 codegen unit 中被复制；而指向*不同*底层类型的值的指针也可能比较为
/// 相等，因为相同 vtable 可能在同一个 codegen unit 内被去重。
///
/// # 示例
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
/// 切片也会按长度一起比较，因为切片指针是胖指针：
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
#[allow(ambiguous_wide_pointer_comparisons)] // 这里的语义实际是明确的。
pub fn eq<T: PointeeSized>(a: *const T, b: *const T) -> bool {
    a == b
}

/// 只比较两个指针的*地址*是否相等，忽略胖指针中的任何 metadata。
///
/// 如果参数是同一类型的瘦指针，则这与 [`eq`] 相同。
///
/// # 示例
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

/// 比较两个函数指针的*地址*是否相等。
///
/// 这与 `f == g` 相同，但使用此函数可以明确表达：这里涉及函数指针比较那套可能令人意外的语义。
///
/// Rust 对函数如何被编译只给出**极少保证**，函数也没有内在的“身份(identity)”。特别地，
/// 这种比较：
///
/// * 在函数等价的情况下，可能意外返回 `true`。
///
///   例如，下面的程序在开启优化后很可能（但不保证）打印 `(true, true)`：
///
///   ```
///   let f: fn(i32) -> i32 = |x| x;
///   let g: fn(i32) -> i32 = |x| x + 0;  // 不同 closure，不同函数体
///   let h: fn(u32) -> u32 = |x| x + 0;  // 签名也不同
///   dbg!(std::ptr::fn_addr_eq(f, g), std::ptr::fn_addr_eq(f, h)); // 不保证相等
///   ```
///
/// * 在任何情况下都可能返回 `false`。
///
///   这在泛型函数上尤其常见，但也可能发生在任何函数上。（从实现角度看，函数有时可能被
///   编译器处理多次，从而产生重复的机器码。）
///
/// 尽管可能出现这些假阳性和假阴性，该比较仍然有用。具体来说，如果：
///
/// * `T` 与 `U` 是同一类型，或 `T` 是 `U` 的[subtype]，或 `U` 是 `T` 的[subtype]；并且
/// * `ptr::fn_addr_eq(f, g)` 返回 true；
///
/// 那么调用 `f` 与调用 `g` 是等价的。
///
///
/// # 示例
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

/// 对裸指针进行 hash。
///
/// 这可用于按地址 hash 一个 `&T` 引用（它会隐式强转为 `*const T`），而不是 hash
/// 它所指向的值；后者才是 `Hash for &T` 的行为。
///
/// # 示例
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

/// 为一个 place 创建 `const` 裸指针，且不创建中间引用。
///
/// `addr_of!(expr)` 等价于 `&raw const expr`。该宏已被*软弃用(soft-deprecated)*；
/// 请改用 `&raw const`。
///
/// 通过 `addr_of!` 创建出的指针在什么条件下允许写入，目前仍是开放问题。如果 place
/// `expr` 的求值基于某个裸指针，那么 `addr_of!` 的结果会继承该裸指针的所有权限。
/// 但如果该 place 基于引用、局部变量或 `static`，在所有细节最终确定前，应按共享引用的
/// 规则处理：除位于 `UnsafeCell` 内的字节外，通过这种操作创建的指针写入是 UB。
/// 若需要明确允许 mutation 的裸指针，请使用 `&raw mut`（或 [`addr_of_mut`]）。
///
/// 只有在指针正确对齐并指向已初始化数据时，才允许用 `&`/`&mut` 创建引用。若这些要求不成立，
/// 应使用裸指针。不过，`&expr as *const _` 会先创建引用，再把它 cast 成裸指针；该引用必须
/// 遵守所有引用规则。本宏可以在*不先创建引用*的情况下创建裸指针。
///
/// 如何创建指向未初始化数据的指针，请见 [`addr_of_mut`]。用 `addr_of` 做这件事意义不大，
/// 因为它只能读取该数据，而读取未初始化数据会造成未定义行为(Undefined Behavior)。
///
/// # 安全性(Safety）
///
/// `addr_of!(expr)` 中的 `expr` 会作为 place expression 求值，但不会从该 place 读取，
/// 也不要求该 place 可解引用。这意味着 `addr_of!((*ptr).field)` 仍要求投影到 `field`
/// 的过程保持 in-bounds，规则与 [`offset`] 相同。不过，即便 `ptr` 为空、悬垂或未对齐，
/// `addr_of!(*ptr)` 本身也是定义行为。
///
/// 注意：`Deref`/`Index` 强转（以及它们的可变版本）在 `addr_of!` 内也会照常应用。
/// 此时会创建引用以调用 `Deref::deref` 或 `Index::index`。上面的说明只适用于没有发生
/// 这类强转的情况。
///
/// [`offset`]: pointer::offset
///
/// # 示例
///
/// **正确用法：创建指向未对齐数据的指针**
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
/// // `&packed.f2` 会创建未对齐引用，因此是未定义行为(Undefined Behavior)！
/// let raw_f2 = ptr::addr_of!(packed.f2);
/// assert_eq!(unsafe { raw_f2.read_unaligned() }, 2);
/// ```
///
/// **错误用法：越界字段投影**
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
/// let fieldptr = unsafe { ptr::addr_of!((*ptr).field2) }; // 未定义行为(Undefined Behavior) ⚠️
/// ```
///
/// 字段投影 `.field2` 会把指针偏移 4 字节，但该指针并不位于某个至少覆盖这 4 字节的
/// allocation 边界内，因此该偏移是未定义行为(Undefined Behavior)。关于 inbounds 指针算术的完整要求，
/// 见 [`offset`] 文档；同样的要求也适用于字段投影，即便投影发生在 `addr_of!` 内也是如此。
/// 特别是，指针为空还是悬垂并不会改变这一点。
#[stable(feature = "raw_ref_macros", since = "1.51.0")]
#[rustc_macro_transparency = "semiopaque"]
pub macro addr_of($place:expr) {
    &raw const $place
}

/// 为一个 place 创建 `mut` 裸指针，且不创建中间引用。
///
/// `addr_of_mut!(expr)` 等价于 `&raw mut expr`。该宏已被*软弃用(soft-deprecated)*；
/// 请改用 `&raw mut`。
///
/// 只有在指针正确对齐并指向已初始化数据时，才允许用 `&`/`&mut` 创建引用。若这些要求不成立，
/// 应使用裸指针。不过，`&mut expr as *mut _` 会先创建引用，再把它 cast 成裸指针；该引用
/// 必须遵守所有引用规则。本宏可以在*不先创建引用*的情况下创建裸指针。
///
/// # 安全性(Safety）
///
/// `addr_of_mut!(expr)` 中的 `expr` 会作为 place expression 求值，但不会从该 place 读取，
/// 也不要求该 place 可解引用。这意味着 `addr_of_mut!((*ptr).field)` 仍要求投影到 `field`
/// 的过程保持 in-bounds，规则与 [`offset`] 相同。不过，即便 `ptr` 为空、悬垂或未对齐，
/// `addr_of_mut!(*ptr)` 本身也是定义行为。
///
/// 注意：`Deref`/`Index` 强转（以及它们的可变版本）在 `addr_of_mut!` 内也会照常应用。
/// 此时会创建引用以调用 `Deref::deref` 或 `Index::index`。上面的说明只适用于没有发生
/// 这类强转的情况。
///
/// [`offset`]: pointer::offset
///
/// # 示例
///
/// **正确用法：创建指向未对齐数据的指针**
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
/// // `&mut packed.f2` 会创建未对齐引用，因此是未定义行为(Undefined Behavior)！
/// let raw_f2 = ptr::addr_of_mut!(packed.f2);
/// unsafe { raw_f2.write_unaligned(42); }
/// assert_eq!({packed.f2}, 42); // `{...}` 强制复制字段，而不是创建引用。
/// ```
///
/// **正确用法：创建指向未初始化数据的指针**
///
/// ```rust
/// use std::{ptr, mem::MaybeUninit};
///
/// struct Demo {
///     field: bool,
/// }
///
/// let mut uninit = MaybeUninit::<Demo>::uninit();
/// // `&uninit.as_mut().field` 会创建指向未初始化 `bool` 的引用，
/// // 因此是未定义行为(Undefined Behavior)！
/// let f1_ptr = unsafe { ptr::addr_of_mut!((*uninit.as_mut_ptr()).field) };
/// unsafe { f1_ptr.write(true); }
/// let init = unsafe { uninit.assume_init() };
/// ```
///
/// **错误用法：越界字段投影**
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
/// let fieldptr = unsafe { ptr::addr_of_mut!((*ptr).field2) }; // 未定义行为(Undefined Behavior) ⚠️
/// ```
///
/// 字段投影 `.field2` 会把指针偏移 4 字节，但该指针并不位于某个至少覆盖这 4 字节的
/// allocation 边界内，因此该偏移是未定义行为(Undefined Behavior)。关于 inbounds 指针算术的完整要求，
/// 见 [`offset`] 文档；同样的要求也适用于字段投影，即便投影发生在 `addr_of_mut!` 内也是如此。
/// 特别是，指针为空还是悬垂并不会改变这一点。
#[stable(feature = "raw_ref_macros", since = "1.51.0")]
#[rustc_macro_transparency = "semiopaque"]
pub macro addr_of_mut($place:expr) {
    &raw mut $place
}
