#![unstable(feature = "core_io_borrowed_buf", issue = "117693")]

use crate::fmt::{self, Debug, Formatter};
use crate::mem::{self, MaybeUninit};
use crate::{cmp, ptr};

/// 一个借用的字节缓冲区，会逐步被填充和初始化。
///
/// 此类型是一种“双游标”。它跟踪缓冲区中的三个区域：开头已经在逻辑上填入数据的区域、
/// 曾经被初始化但尚未在逻辑上填充的区域，以及末尾完全未初始化的区域。
/// filled 区域保证始终是 initialized 区域的子集。
///
/// 概括地说，缓冲区内容可以表示为：
/// ```not_rust
/// [             capacity              ]
/// [ filled |         unfilled         ]
/// [    initialized    | uninitialized ]
/// ```
///
/// `BorrowedBuf` 通过唯一引用（`&mut`）围绕已有数据（或用于放置数据的容量）创建。
/// 可以配置 `BorrowedBuf`（例如使用 `clear` 或 `set_init`），但不能直接写入它。
/// 若要写入缓冲区，请使用 `unfilled` 创建 `BorrowedCursor`。该 cursor 对缓冲区的
/// unfilled 部分具有只写访问权（可以把它理解成只写迭代器）。
///
/// 生命周期 `'data` 是底层数据生命周期的上界。
pub struct BorrowedBuf<'data> {
    /// 缓冲区的底层数据。
    buf: &'data mut [MaybeUninit<u8>],
    /// `self.buf` 中已知已填充部分的长度。
    filled: usize,
    /// `self.buf` 中已知已初始化部分的长度。
    init: usize,
}

impl Debug for BorrowedBuf<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedBuf")
            .field("init", &self.init)
            .field("filled", &self.filled)
            .field("capacity", &self.capacity())
            .finish()
    }
}

/// 从完全初始化的切片创建新的 `BorrowedBuf`。
impl<'data> From<&'data mut [u8]> for BorrowedBuf<'data> {
    #[inline]
    fn from(slice: &'data mut [u8]) -> BorrowedBuf<'data> {
        let len = slice.len();

        BorrowedBuf {
            // SAFETY: 输入是已初始化的 `[u8]` 切片；`BorrowedBuf` 的不变量保证
            // initialized 字节之后不会再变为 uninitialized，因此可将其视为
            // `MaybeUninit<u8>` 底层缓冲区并把 `init` 设为整个长度。
            buf: unsafe { (slice as *mut [u8]).as_uninit_slice_mut().unwrap() },
            filled: 0,
            init: len,
        }
    }
}

/// 从未初始化缓冲区创建新的 `BorrowedBuf`。
///
/// 如果已知缓冲区的一部分已经初始化，请使用 `set_init`。
impl<'data> From<&'data mut [MaybeUninit<u8>]> for BorrowedBuf<'data> {
    #[inline]
    fn from(buf: &'data mut [MaybeUninit<u8>]) -> BorrowedBuf<'data> {
        BorrowedBuf { buf, filled: 0, init: 0 }
    }
}

/// 从 cursor 创建新的 `BorrowedBuf`。
///
/// 更安全的替代方案是使用 `BorrowedCursor::with_unfilled_buf`。
impl<'data> From<BorrowedCursor<'data>> for BorrowedBuf<'data> {
    #[inline]
    fn from(mut buf: BorrowedCursor<'data>) -> BorrowedBuf<'data> {
        let init = buf.init_mut().len();
        BorrowedBuf {
            // SAFETY: 根据 `BorrowedBuf` 的不变量，已初始化字节不会再变为未初始化。
            // 这里取得的是原缓冲区的 unfilled 后缀，`init` 已用 `init_mut().len()`
            // 计算为该后缀中已初始化的长度。
            buf: unsafe { buf.buf.buf.get_unchecked_mut(buf.buf.filled..) },
            filled: 0,
            init,
        }
    }
}

impl<'data> BorrowedBuf<'data> {
    /// 返回缓冲区的总容量。
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// 返回缓冲区 filled 部分的长度。
    #[inline]
    pub fn len(&self) -> usize {
        self.filled
    }

    /// 返回缓冲区 initialized 部分的长度。
    #[inline]
    pub fn init_len(&self) -> usize {
        self.init
    }

    /// 返回缓冲区 filled 部分的共享引用。
    #[inline]
    pub fn filled(&self) -> &[u8] {
        // SAFETY: filled 区域始终是 initialized 区域的子集；这里只截取 filled
        // 部分，因此该范围内所有字节都已初始化且范围在界内。
        unsafe {
            let buf = self.buf.get_unchecked(..self.filled);
            buf.assume_init_ref()
        }
    }

    /// 返回缓冲区 filled 部分的可变引用。
    #[inline]
    pub fn filled_mut(&mut self) -> &mut [u8] {
        // SAFETY: filled 区域始终是 initialized 区域的子集；这里只截取 filled
        // 部分，因此该范围内所有字节都已初始化且范围在界内。
        unsafe {
            let buf = self.buf.get_unchecked_mut(..self.filled);
            buf.assume_init_mut()
        }
    }

    /// 以原始生命周期返回缓冲区 filled 部分的共享引用。
    #[inline]
    pub fn into_filled(self) -> &'data [u8] {
        // SAFETY: filled 区域始终是 initialized 区域的子集；这里只截取 filled
        // 部分，因此该范围内所有字节都已初始化且范围在界内。
        unsafe {
            let buf = self.buf.get_unchecked(..self.filled);
            buf.assume_init_ref()
        }
    }

    /// 以原始生命周期返回缓冲区 filled 部分的可变引用。
    #[inline]
    pub fn into_filled_mut(self) -> &'data mut [u8] {
        // SAFETY: filled 区域始终是 initialized 区域的子集；这里只截取 filled
        // 部分，因此该范围内所有字节都已初始化且范围在界内。
        unsafe {
            let buf = self.buf.get_unchecked_mut(..self.filled);
            buf.assume_init_mut()
        }
    }

    /// 返回覆盖缓冲区 unfilled 部分的 cursor。
    #[inline]
    pub fn unfilled<'this>(&'this mut self) -> BorrowedCursor<'this> {
        BorrowedCursor {
            // SAFETY: `BorrowedCursor::buf` 创建后不会被重新赋值替换，
            // 因而不会把较短生命周期的 `BorrowedBuf` 写回较长生命周期位置；
            // 将其生命周期按协变方式缩短是安全的。
            buf: unsafe {
                mem::transmute::<&'this mut BorrowedBuf<'data>, &'this mut BorrowedBuf<'this>>(self)
            },
        }
    }

    /// 清空缓冲区，将 filled 区域重置为空。
    ///
    /// 已初始化字节数不会改变，缓冲区内容也不会被修改。
    #[inline]
    pub fn clear(&mut self) -> &mut Self {
        self.filled = 0;
        self
    }

    /// 断言缓冲区的前 `n` 个字节已经初始化。
    ///
    /// `BorrowedBuf` 假设字节不会被反初始化，因此当 `n` 小于已知初始化字节数时，
    /// 此方法不会做任何事。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用者必须确保缓冲区前 `n` 个字节已经初始化；否则后续把这些字节作为 `u8`
    /// 读取会读取未初始化内存。
    #[inline]
    pub unsafe fn set_init(&mut self, n: usize) -> &mut Self {
        self.init = cmp::max(self.init, n);
        self
    }
}

/// [`BorrowedBuf`] 的 unfilled 部分的可写视图。
///
/// unfilled 部分由 initialized 子区域和 uninitialized 子区域组成；详见 [`BorrowedBuf`]。
///
/// 可以使用 [`append`](BorrowedCursor::append) 直接向 cursor 写入数据，也可以取得
/// cursor 的部分或全部切片并写入该切片来间接写入。采用间接方式时，调用者必须在写入后
/// 调用 [`advance`](BorrowedCursor::advance)，告知 cursor 已写入的字节数。
///
/// 数据一旦写入 cursor，就会成为底层 `BorrowedBuf` 的 filled 部分，不能再由该 cursor
/// 访问或重写。换言之，cursor 跟踪的是底层 `BorrowedBuf` 的 unfilled 部分。
///
/// 生命周期 `'a` 是底层缓冲区生命周期的上界（传递地也是该缓冲区中数据生命周期的上界）。
#[derive(Debug)]
pub struct BorrowedCursor<'a> {
    /// 底层缓冲区。
    // 安全不变量：创建 `BorrowedCursor` 时，会把 `buf` 的类型视为对 `BorrowedBuf`
    // 生命周期协变。只有在永不通过赋值替换 `buf` 时这才安全；否则可能把较短生命周期的
    // 缓冲区写入较长生命周期的位置，所以不要这样做。
    buf: &'a mut BorrowedBuf<'a>,
}

impl<'a> BorrowedCursor<'a> {
    /// 通过以更短生命周期克隆此 cursor 来重新借用它。
    ///
    /// 由于 cursor 保持对底层缓冲区的唯一访问权，在新 cursor 存在期间，
    /// 原被借用的 cursor 不可访问。
    #[inline]
    pub fn reborrow<'this>(&'this mut self) -> BorrowedCursor<'this> {
        BorrowedCursor {
            // SAFETY: `BorrowedCursor::buf` 创建后不会被重新赋值替换，
            // 因而不会把较短生命周期的 `BorrowedBuf` 写回较长生命周期位置；
            // 将其生命周期按协变方式缩短是安全的。
            buf: unsafe {
                mem::transmute::<&'this mut BorrowedBuf<'a>, &'this mut BorrowedBuf<'this>>(
                    self.buf,
                )
            },
        }
    }

    /// 返回 cursor 中的可用空间。
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.capacity() - self.buf.filled
    }

    /// 返回已写入创建此 cursor 所用 `BorrowedBuf` 的字节数。
    ///
    /// 尤其是，返回的计数会被该 cursor 的所有重新借用共享。
    #[inline]
    pub fn written(&self) -> usize {
        self.buf.filled
    }

    /// 返回 cursor initialized 部分的可变引用。
    #[inline]
    pub fn init_mut(&mut self) -> &mut [u8] {
        // SAFETY: 这里只截取 `filled..init`，该范围按不变量已初始化且在底层缓冲区内。
        unsafe {
            let buf = self.buf.buf.get_unchecked_mut(self.buf.filled..self.buf.init);
            buf.assume_init_mut()
        }
    }

    /// 返回整个 cursor 的可变引用。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用者不得把 cursor 的 initialized 部分中的任何字节变回未初始化状态；
    /// `BorrowedBuf` 依赖 initialized 字节不会被反初始化这一不变量。
    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        // SAFETY: `filled <= capacity` 是 `BorrowedBuf` 的不变量，因此后缀范围始终在界内。
        unsafe { self.buf.buf.get_unchecked_mut(self.buf.filled..) }
    }

    /// 通过断言已有 `n` 个字节被填充来推进 cursor。
    ///
    /// 推进后，这 `n` 个字节不能再通过 cursor 访问，只能通过底层缓冲区访问。
    /// 也就是说，缓冲区的 filled 部分增加 `n` 个元素，unfilled 部分
    /// （以及此 cursor 的容量）减少 `n` 个元素。
    ///
    /// 如果从 cursor 视角看已初始化字节少于 `n` 个，应先调用 `set_init`。
    ///
    /// # Panics
    ///
    /// 如果已初始化字节少于 `n` 个，会 panic。
    #[inline]
    pub fn advance(&mut self, n: usize) -> &mut Self {
        // 根据此类型的不变量，减法不会下溢。
        assert!(n <= self.buf.init - self.buf.filled);

        self.buf.filled += n;
        self
    }

    /// 通过断言已有 `n` 个字节被填充来推进 cursor。
    ///
    /// 推进后，这 `n` 个字节不能再通过 cursor 访问，只能通过底层缓冲区访问。
    /// 也就是说，缓冲区的 filled 部分增加 `n` 个元素，unfilled 部分
    /// （以及此 cursor 的容量）减少 `n` 个元素。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用者必须确保 cursor 的前 `n` 个字节已经被正确初始化；否则推进后底层
    /// `BorrowedBuf` 会把未初始化字节视为 filled 数据。
    #[inline]
    pub unsafe fn advance_unchecked(&mut self, n: usize) -> &mut Self {
        self.buf.filled += n;
        self.buf.init = cmp::max(self.buf.init, self.buf.filled);
        self
    }

    /// 初始化 cursor 中的所有字节。
    #[inline]
    pub fn ensure_init(&mut self) -> &mut Self {
        // SAFETY: `init <= capacity` 是不变量，因此该后缀范围在界内；
        // 写入零只会初始化这些字节，不会反初始化已有字节。
        let uninit = unsafe { self.buf.buf.get_unchecked_mut(self.buf.init..) };

        // SAFETY: 对 `MaybeUninit<u8>` 写入字节 0 会产生有效的已初始化 `u8` 值；
        // 长度来自切片引用，因而与分配匹配。
        unsafe {
            ptr::write_bytes(uninit.as_mut_ptr(), 0, uninit.len());
        }
        self.buf.init = self.buf.capacity();

        self
    }

    /// 断言 cursor 的前 `n` 个 unfilled 字节已经初始化。
    ///
    /// `BorrowedBuf` 假设字节不会被反初始化，因此当 `n` 小于已知初始化字节数时，
    /// 此方法不会做任何事。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用者必须确保 cursor 的前 `n` 个 unfilled 字节已经初始化；否则后续可能把
    /// 未初始化内存作为 `u8` 读取。
    #[inline]
    pub unsafe fn set_init(&mut self, n: usize) -> &mut Self {
        self.buf.init = cmp::max(self.buf.init, self.buf.filled + n);
        self
    }

    /// 向 cursor 追加数据，并推进其在缓冲区中的位置。
    ///
    /// # Panics
    ///
    /// 如果 `self.capacity()` 小于 `buf.len()`，会 panic。
    #[inline]
    pub fn append(&mut self, buf: &[u8]) {
        assert!(self.capacity() >= buf.len());

        // SAFETY: 写入 `buf` 只会初始化/覆盖目标字节，不会反初始化切片中的任何元素。
        unsafe {
            self.as_mut()[..buf.len()].write_copy_of_slice(buf);
        }

        // SAFETY: 刚刚已经把 `buf` 的全部内容写入 cursor，因此这些字节已初始化。
        unsafe {
            self.set_init(buf.len());
        }
        self.buf.filled += buf.len();
    }

    /// 用包含 cursor unfilled 部分的 `BorrowedBuf` 运行给定闭包。
    ///
    /// 这允许检查写入 cursor 的内容。
    ///
    /// # Panics
    ///
    /// 如果传给闭包的 `BorrowedBuf` 被替换成另一个，会 panic。
    pub fn with_unfilled_buf<T>(&mut self, f: impl FnOnce(&mut BorrowedBuf<'_>) -> T) -> T {
        let mut buf = BorrowedBuf::from(self.reborrow());
        let prev_ptr = buf.buf as *const _;
        let res = f(&mut buf);

        // 检查调用者没有替换 `BorrowedBuf`。这是下面代码安全性的必要条件：
        // 如果没有此检查，调用者可能把实际并不存在的字节标记为已初始化。
        assert!(core::ptr::addr_eq(prev_ptr, buf.buf));

        let filled = buf.filled;
        let init = buf.init;

        // 用写入缓冲区的内容更新 `init` 和 `filled` 字段。
        // `self.buf.filled` 是该 `BorrowedBuf` 的起始长度。
        //
        // SAFETY: 这些数量的字节已经在 `BorrowedBuf` 中初始化/填充；
        // 由于缓冲区没有被替换，它们在 cursor 中也同样已经初始化/填充。
        self.buf.init = self.buf.filled + init;
        self.buf.filled += filled;

        res
    }
}
