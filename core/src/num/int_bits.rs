//! `uN::gather_bits` 和 `uN::scatter_bits` 的实现。
//!
//! 在这个实现中，可以把输入位看成从最低有效位开始的一串列表。Gathering 类似
//! `Vec::retain`：掩码为零的位置被删除，剩余位向低位压紧。Scattering 则做相反操作：
//! 在 gathering 会删除的位置重新插入零，把低位连续数据散布回稀疏掩码指定的位置。
//!
//! 关键观察是：每个被 gather/scatter 的位，需要移动的距离正好等于它对应掩码位之前
//! 出现过的零的数量。
//!
//! 因此整体思路是把操作分解成 `0..log2(BITS)` 的多个阶段。第 `stage` 阶段只把某些位
//! 移动 `n = 1 << stage` 位；每个阶段需要移动哪些位，由掩码中零位的前缀计数推导出的
//! stage mask 决定。这样可以用固定数量的移位和 XOR 完成压缩或展开，而不需要逐位循环。
//!
//! # Gathering
//!
//! 把输入看成若干段数据位串（A、B、C、...），中间由固定宽度的零组（'.'）分隔。
//! 初始宽度 `n = 1`。按零组计数时，每个阶段会把奇数位置的数据段右移 `n` 位，
//! 等价于把它们和前面的零组交换。进入下一阶段时，所有零组已经两两合并，因此 `n`
//! 翻倍。
//! ```text
//! .A.B.C.D.E.F.G.H
//! ..AB..CD..EF..GH
//! ....ABCD....EFGH
//! ........ABCDEFGH
//! ```
//! 难点在于数据位串的长度并不相同。若用小写字母表示单个位，上面的过程可能实际长成：
//! ```text
//! .a.bbb.ccccc.dd.e..g.hh
//! ..abbb..cccccdd..e..ghh
//! ....abbbcccccdd....eghh
//! ........abbbcccccddeghh
//! ```
//!
//! # Scattering
//!
//! 对 `scatter_bits`，阶段顺序正好反过来。初始时低位里只有一段连续数据；每个阶段会把
//! 每段数据的一部分左移 `n` 位，把该段拆成两段，而 `n` 在每个阶段减半。
//! ```text
//! ........ABCDEFGH
//! ....ABCD....EFGH
//! ..AB..CD..EF..GH
//! .A.B.C.D.E.F.G.H
//! ```
//!
//! # Stage masks
//!
//! 为了执行每个阶段的位移，会计算一个 mask，它同时覆盖“要被移动的数据位串”和
//! “这些数据位要移入的零位”。这样用 `x & mask` 取出候选位后，再通过 XOR 删除原位置、
//! 写入新位置。
//! ```text
//! .A.B.C.D.E.F.G.H
//!  ##  ##  ##  ##
//! ..AB..CD..EF..GH
//!   ####    ####
//! ....ABCD....EFGH
//!     ########
//! ........ABCDEFGH
//! ```

macro_rules! uint_impl {
    ($U:ident) => {
        pub(super) mod $U {
            const STAGES: usize = $U::BITS.ilog2() as usize;
            #[inline]
            const fn prepare(sparse: $U) -> [$U; STAGES] {
                // `zeros` 初始表示会被移除的位；随后把每个阶段需要移动的部分计算到
                // `masks` 中。
                let mut zeros = !sparse;
                let mut masks = [0; STAGES];
                let mut stage = 0;
                while stage < STAGES {
                    let n = 1 << stage;
                    // 假设 `zeros` 在 `{ a..a+n, b..b+n, ... }` 这些区间中置位。
                    // 那么 `parity` 会被计算为 `{ a.. } XOR { b.. } XOR ...`，
                    // 得到的正是 `{ a..b, c..d, e.. }` 这些区间。
                    let mut parity = zeros;
                    let mut len = n;
                    while len < $U::BITS {
                        parity ^= parity << len;
                        len <<= 1;
                    }
                    masks[stage] = parity;

                    // 关掉会被移入的那些位：
                    // { a..a+n, b..b+n, ... } & !{ a..b, c..d, e.. }
                    // == { b..b+n, d..d+n, ... }
                    zeros &= !parity;
                    // 把剩余区间向下扩展到作为移动来源的位：
                    // { b-n..b+n, d-n..d+n, ... }
                    zeros ^= zeros >> n;

                    stage += 1;
                }
                masks
            }

            #[inline(always)]
            pub(in super::super) const fn gather_impl(mut x: $U, sparse: $U) -> $U {
                let masks = prepare(sparse);
                x &= sparse;
                let mut stage = 0;
                while stage < STAGES {
                    let n = 1 << stage;
                    // 考察每两段带有前导 `n` 个零位的数据段。假设要右移的数据段长度为
                    // `a`，另一段长度为 `b`，并假设移入的只有零。
                    // ```text
                    // [0; n], [X; a], [0; n], [Y; b] // x
                    // [0; n], [X; a], [0; n], [0; b] // q
                    // [0; n], [0; a   +   n], [Y; b] // x ^= q
                    // [0; n   +   n], [X; a], [0; b] // q >> n
                    // [0; n], [0; n], [X; a], [Y; b] // x ^= q << n
                    // ```
                    // 被移出的只有零，因此下一组继续满足同一假设。

                    // 效果上，高位数据段会和它下面那组 `n` 个零位交换。
                    let q = x & masks[stage];
                    x ^= q;
                    x ^= q >> n;

                    stage += 1;
                }
                x
            }
            #[inline(always)]
            pub(in super::super) const fn scatter_impl(mut x: $U, sparse: $U) -> $U {
                let masks = prepare(sparse);
                let mut stage = STAGES;
                while stage > 0 {
                    stage -= 1;
                    let n = 1 << stage;
                    // 考察每段数据以及它上方 `2 * n` 个任意位。假设数据段长度为 `a + b`，
                    // 其中 `a` 是需要移动的部分长度，并假设移入的只有零。
                    // ```text
                    // [_; n], [_; n], [X; a], [Y; b] // x
                    // [0; n], [_; n], [X; a], [0; b] // q
                    // [_; n], [0; n   +   a], [Y; b] // x ^= q
                    // [_; n], [X; a], [0; b   +   n] // q << n
                    // [_; n], [X; a], [0; n], [Y; b] // x ^= q << n
                    // ```
                    // 被移出的只有零，因此下一组继续满足同一假设。

                    // 效果上，每段数据中会插入 `n` 个零位使其展开，而上方两组 `n` 位会
                    // 通过 XOR 合并。
                    let q = x & masks[stage];
                    x ^= q;
                    x ^= q << n;
                }
                x & sparse
            }
        }
    };
}

uint_impl!(u8);
uint_impl!(u16);
uint_impl!(u32);
uint_impl!(u64);
uint_impl!(u128);
