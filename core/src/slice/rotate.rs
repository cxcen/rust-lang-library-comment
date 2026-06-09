use crate::mem::{MaybeUninit, SizedTypeProperties};
use crate::ptr;

type BufType = [usize; 32];

/// 旋转范围 `[mid-left, mid+right)`，使 `mid` 处元素成为第一个元素。
/// 等价地说，把该范围向左旋转 `left` 个元素，或向右旋转 `right` 个元素。
///
/// # 安全性(Safety）
///
/// 指定范围必须对读取和写入都有效，并且全部位于同一 allocation 内。
#[inline]
pub(super) const unsafe fn ptr_rotate<T>(left: usize, mid: *mut T, right: usize) {
    if T::IS_ZST {
        return;
    }
    // 如果旋转为空操作，尽早返回。
    if (left == 0) || (right == 0) {
        return;
    }
    // `T` 不是 zero-sized type，因此可以用它的大小做除法。
    if !cfg!(feature = "optimize_for_size")
        // FIXME(const-hack): const 中可用 cmp::min 后改用它。
        && const_min(left, right) <= size_of::<BufType>() / size_of::<T>()
    {
        // SAFETY: 调用方保证旋转范围有效。
        unsafe { ptr_rotate_memmove(left, mid, right) };
    } else if !cfg!(feature = "optimize_for_size")
        && ((left + right < 24) || (size_of::<T>() > size_of::<[usize; 4]>()))
    {
        // SAFETY: 调用方保证旋转范围有效。
        unsafe { ptr_rotate_gcd(left, mid, right) }
    } else {
        // SAFETY: 调用方保证旋转范围有效。
        unsafe { ptr_rotate_swap(left, mid, right) }
    }
}

/// 算法 1 用于 `min(left, right)` 足够小、能放入栈缓冲区的情况。
/// 先把较短一侧的元素复制到缓冲区，对另一侧执行 `memmove`，再把缓冲区中的元素移回
/// 原位置相对侧留下的空洞。
///
/// # 安全性(Safety）
///
/// 指定范围必须对读取和写入都有效。
#[inline]
const unsafe fn ptr_rotate_memmove<T>(left: usize, mid: *mut T, right: usize) {
    // 这里的 `[T; 0]` 用来确保缓冲区对 `T` 正确对齐。
    let mut rawarray = MaybeUninit::<(BufType, [T; 0])>::uninit();
    let buf = rawarray.as_mut_ptr() as *mut T;
    // SAFETY: `mid-left <= mid-left+right < mid+right`，指针仍在旋转范围内。
    let dim = unsafe { mid.sub(left).add(right) };
    if left <= right {
        // SAFETY:
        //
        // 1) 大小判断保证 `[mid-left; left]` 能放进 `buf` 而不会溢出；`buf` 刚在栈上创建，
        //    不会与 `[mid-left; left]` 中任何元素重叠。
        // 2) `[mid-left, mid+right)` 对读写都有效；这里使用允许重叠的移动。
        // 3) `left <= right` 保证向 `dim = mid-left+right` 写入 `left` 个元素有效：
        //    - `buf` 有效，并且 1) 已经写入 `left` 个元素；
        //    - `dim+left = mid-left+right+left = mid+right`，写入范围是 `[dim, dim+left)`。
        unsafe {
            // 1)
            ptr::copy_nonoverlapping(mid.sub(left), buf, left);
            // 2)
            ptr::copy(mid, mid.sub(left), right);
            // 3)
            ptr::copy_nonoverlapping(buf, dim, left);
        }
    } else {
        // SAFETY: 与上面相同，只是 `left` 与 `right` 互换。
        unsafe {
            ptr::copy_nonoverlapping(mid, buf, right);
            ptr::copy(mid.sub(left), dim, left);
            ptr::copy_nonoverlapping(buf, mid.sub(left), right);
        }
    }
}

/// 算法 2 用于 `left + right` 较小或 `T` 较大的情况。
/// 它从 `mid - left` 开始，每次按 `right` 步、模 `left + right` 前进，把元素逐个放到
/// 最终位置，因此只需要一个临时变量。最终会回到 `mid - left`。不过如果
/// `gcd(left + right, right) != 1`，上述步进会跳过部分元素。例如：
/// ```text
/// left = 10, right = 6
/// `^` 表示元素已经处在最终位置
/// 6 7 8 9 10 11 12 13 14 15 . 0 1 2 3 4 5
/// 使用上述算法一步之后（X 会在本轮末尾被覆盖，12 存在临时变量中）：
/// X 7 8 9 10 11 6 13 14 15 . 0 1 2 3 4 5
///               ^
/// 再执行一步（此时 2 在临时变量中）：
/// X 7 8 9 10 11 6 13 14 15 . 0 1 12 3 4 5
///               ^                 ^
/// 第三步之后（步进发生回绕，8 在临时变量中）：
/// X 7 2 9 10 11 6 13 14 15 . 0 1 12 3 4 5
///     ^         ^                 ^
/// 再执行 7 步后，本轮结束，临时变量中的 0 被放入 X：
/// 0 7 2 9 4 11 6 13 8 15 . 10 1 12 3 14 5
/// ^   ^   ^    ^    ^       ^    ^    ^
/// ```
/// 好在两个已定位元素之间被跳过的元素数总是相同，因此只需偏移起点并执行更多轮；
/// 总轮数就是 `gcd(left + right, right)`。最终每个元素都会且只会被放到最终位置一次。
///
/// 算法 2 可以通过分块并一次执行多轮来向量化，但在 `left + right` 非常大之前，
/// 平均轮数太少，而且单轮的最坏情况始终存在。
///
/// # 安全性(Safety）
///
/// 指定范围必须对读取和写入都有效。
#[inline]
const unsafe fn ptr_rotate_gcd<T>(left: usize, mid: *mut T, right: usize) {
    // 算法 2。微基准显示，对随机位移而言，平均性能在 `left + right == 32` 左右前都更好；
    // 但最坏性能在约 16 时持平，因此选择 24 作为折中。如果 `T` 大于 4 个 `usize`，
    // 该算法也优于其它算法。
    // SAFETY: 调用方必须保证 `mid - left` 对读写有效。
    let x = unsafe { mid.sub(left) };
    // 第一轮开始。
    // SAFETY: 见上一条注释。
    let mut tmp: T = unsafe { x.read() };
    let mut i = right;
    // 可以预先计算 `gcd(left + right, right)` 得到 `gcd`，但先执行一轮并顺带算出 gcd，
    // 再处理剩余轮会更快。
    let mut gcd = right;
    // 基准显示，一路交换临时值，比只读取一次临时值、向后复制、最后再写回临时值更快。
    // 可能原因是交换/替换临时值在循环中只使用一个内存地址，而不需要管理两个地址。
    loop {
        // [long-safety-expl]
        // SAFETY: 调用方必须保证 `[mid-left, mid+right)` 对读写全部有效。
        //
        // - `i` 从 `right` 开始，因此 `mid-left <= x+i = x+right = mid-left+right < mid+right`。
        // - `i <= left+right-1` 始终为真：
        //   - 如果 `i < left`，会加上 `right`，于是 `i < left+right`；下一轮会从 `i`
        //     减去 `left`，所以不会继续增大。
        //   - 如果 `i >= left`，会立刻减去 `left`，所以也不会继续增大。
        // - `i` 不会溢出，因为函数安全契约要求 `mid+right-1 = x+left+right` 可写。
        // - `i` 也不会下溢，因为只有 `i >= left` 时才会减去 `left`。
        //
        // 因此，只要调用方遵守契约，`x+i` 就对读写有效。
        tmp = unsafe { x.add(i).replace(tmp) };
        // 不先增加 `i` 再检查是否越界，而是提前检查下一次增加是否会越界，以避免指针或
        // `usize` 发生回绕。
        if i >= left {
            i -= left;
            if i == 0 {
                // 第一轮结束。
                // SAFETY: `tmp` 来自有效源位置，且调用方保证 `x` 可写。
                unsafe { x.write(tmp) };
                break;
            }
            // 当 `left + right >= 15` 时，这个条件必须保留在这里。
            if i < gcd {
                gcd = i;
            }
        } else {
            i += right;
        }
    }
    // 用更多轮完成这个 chunk。
    // FIXME(const-hack): const 中可用时改成 `for start in 1..gcd`。
    let mut start = 1;
    while start < gcd {
        // SAFETY: `gcd` 至多等于 `right`，所以 `1..gcd` 中的所有值按函数安全契约都可读写；
        // 见上方 [long-safety-expl]。
        tmp = unsafe { x.add(start).read() };
        // [safety-expl-addition]
        //
        // 这里 `start < gcd`，因此 `start < right`，所以 `i < right+right`。由于 `right`
        // 是 `(left+right, right)` 的最大公约数，`left` 也按同一周期前进，故 `i < left+right`；
        // 按函数安全契约，`x+i = mid-left+i` 始终可读写。
        i = start + right;
        loop {
            // SAFETY: 见 [long-safety-expl] 与 [safety-expl-addition]。
            tmp = unsafe { x.add(i).replace(tmp) };
            if i >= left {
                i -= left;
                if i == start {
                    // SAFETY: 见 [long-safety-expl] 与 [safety-expl-addition]。
                    unsafe { x.add(start).write(tmp) };
                    break;
                }
            } else {
                i += right;
            }
        }

        start += 1;
    }
}

/// 算法 3 通过反复交换 `min(left, right)` 个元素来旋转。
///
/// ///
/// ```text
/// left = 11, right = 4
/// [4 5 6 7 8 9 10 11 12 13 14 . 0 1 2 3]
///                  ^  ^  ^  ^   ^ ^ ^ ^ 将最右侧元素与左边元素交换
/// [4 5 6 7 8 9 10 . 0 1 2 3] 11 12 13 14
///        ^ ^ ^  ^   ^ ^ ^ ^ 交换这些元素
/// [4 5 6 . 0 1 2 3] 7 8 9 10 11 12 13 14
/// 已经无法继续交换，但还剩一个更小的旋转问题要解决
/// ```
/// 当 `left < right` 时，则从左侧开始交换。
///
/// # 安全性(Safety）
///
/// 指定范围必须对读取和写入都有效。
#[inline]
const unsafe fn ptr_rotate_swap<T>(mut left: usize, mut mid: *mut T, mut right: usize) {
    loop {
        if left >= right {
            // 算法 3。另一种交换方式会先找到本算法最后一次交换的位置，并使用最后那个
            // chunk 交换，而不是像这里这样交换相邻 chunk；但当前方式仍然更快。
            loop {
                // SAFETY:
                // `left >= right`，所以 `[mid-right, mid+right)` 对读写有效。
                // 每轮从 `mid` 减去 `right`，会被后续加法和检查配平。
                unsafe {
                    ptr::swap_nonoverlapping(mid.sub(right), mid, right);
                    mid = mid.sub(right);
                }
                left -= right;
                if left < right {
                    break;
                }
            }
        } else {
            // 算法 3，`left < right`。
            loop {
                // SAFETY: `[mid-left, mid+left)` 对读写有效，因为 `left < right`，
                // 所以 `mid+left < mid+right`。每轮给 `mid` 加上 `left`，会被后续减法和检查配平。
                unsafe {
                    ptr::swap_nonoverlapping(mid.sub(left), mid, left);
                    mid = mid.add(left);
                }
                right -= left;
                if right < left {
                    break;
                }
            }
        }
        if (right == 0) || (left == 0) {
            return;
        }
    }
}

// FIXME(const-hack): const 中可用 cmp::min 后改用它。
const fn const_min(left: usize, right: usize) -> usize {
    if right < left { right } else { left }
}
