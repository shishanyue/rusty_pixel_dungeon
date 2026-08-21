//! SPD 整数矩形移植，对照 `SPD-classes/.../watabou/utils/Rect.java`。
//!
//! # 开闭区间语义（沉默陷阱）
//!
//! SPD 的 `right`/`bottom` 是**闭区间墙位**：
//!
//! - 基类 `Rect.width() == right - left`（**不含** `right` 列）；
//! - 但 `Room` 覆写 `width()` 为 `right - left + 1`（Room.java L134-L143）——
//!   房间的格子足迹是双闭区间 `[left, right] × [top, bottom]`，
//!   最外一圈即墙，相邻房间**共享墙位**（A.right == B.left）。
//!
//! 本工程约定：生成流水线内部保留 SPD 闭区间语义逐行对照移植；
//! 写入 [`crate::levels::Level`]（`bevy::math::IRect`，`max` 开区间）时统一经
//! [`SpdRect::to_irect`] 换算：足迹 = `IRect::new(left, top, right + 1, bottom + 1)`。

use bevy::math::{IRect, IVec2};

/// SPD `Rect`（闭区间坐标）。字段名与 Java 一致，便于对照。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpdRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl SpdRect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// SPD 单位宽（Rect.java L48-L50）：= 格数 - 1（当作房间足迹解释时）。
    pub const fn width(&self) -> i32 {
        self.right - self.left
    }

    /// SPD 单位高（Rect.java L52-L54）。
    pub const fn height(&self) -> i32 {
        self.bottom - self.top
    }

    /// Rect.java L56-L58。
    pub const fn square(&self) -> i32 {
        self.width() * self.height()
    }

    /// 平移到左上角 (x, y)，保持尺寸（Rect.java L72-L74）。
    pub fn set_pos(&mut self, x: i32, y: i32) {
        let w = self.width();
        let h = self.height();
        self.left = x;
        self.top = y;
        self.right = x + w;
        self.bottom = y + h;
    }

    /// Rect.java L76-L78。
    pub fn shift(&mut self, dx: i32, dy: i32) {
        self.left += dx;
        self.top += dy;
        self.right += dx;
        self.bottom += dy;
    }

    /// 以左上角为锚点重设 SPD 单位尺寸（Rect.java L80-L82）；
    /// 房间语义下实际格数 = `w + 1` × `h + 1`。
    pub fn resize(&mut self, w: i32, h: i32) {
        self.right = self.left + w;
        self.bottom = self.top + h;
    }

    /// Rect.java L84-L86。
    pub const fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    /// Rect.java L88-L91。
    pub fn set_empty(&mut self) {
        self.left = 0;
        self.top = 0;
        self.right = 0;
        self.bottom = 0;
    }

    /// 逐分量交集（Rect.java L93-L100）。结果可能"翻转"（left > right），
    /// 由调用方按 `width()/height()` 判定；共享墙列的交集宽为 0。
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            left: self.left.max(other.left),
            right: self.right.min(other.right),
            top: self.top.max(other.top),
            bottom: self.bottom.min(other.bottom),
        }
    }

    /// 双闭区间内全部格点，x 外层、y 内层（Rect.java `getPoints` L155-L161）。
    /// 翻转矩形产出空序列（与 Java `i <= right` 循环一致）。
    pub fn points(&self) -> impl Iterator<Item = IVec2> + use<> {
        let (top, bottom) = (self.top, self.bottom);
        (self.left..=self.right).flat_map(move |x| (top..=bottom).map(move |y| IVec2::new(x, y)))
    }

    /// 双闭区间足迹 → `IRect`（`max` 开区间）。要求 `!is_empty()` 语义下使用；
    /// SPD 单位宽 w 对应 `IRect` 尺寸 w + 1。
    pub fn to_irect(&self) -> IRect {
        IRect::new(self.left, self.top, self.right + 1, self.bottom + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验收用例：SPD 闭区间 ↔ `IRect` 开区间换算对拍（手算）。
    /// 房间 A(0,0,5,5) 与 B(5,0,10,4) 共享 x=5 墙列。
    #[test]
    fn spd_intersect_and_irect_conversion_hand_case() {
        let a = SpdRect::new(0, 0, 5, 5);
        let b = SpdRect::new(5, 0, 10, 4);

        // A 足迹 6×6 格（SPD 宽 5 → 格数 5+1）
        assert_eq!(a.width(), 5);
        assert_eq!(a.to_irect(), IRect::new(0, 0, 6, 6));
        assert_eq!(a.to_irect().size(), IVec2::new(6, 6));

        // 交集 = x=5 单列、y ∈ [0,4]：SPD 单位宽 0、高 4
        let i = a.intersect(&b);
        assert_eq!(i, SpdRect::new(5, 0, 5, 4));
        assert_eq!(i.width(), 0);
        assert_eq!(i.height(), 4);

        // 同一交叠在 IRect（开区间）下是 1×5 格 —— 与 SPD 高度恰差 1
        let ii = a.to_irect().intersect(b.to_irect());
        assert_eq!(ii, IRect::new(5, 0, 6, 5));
        assert_eq!(ii.size(), IVec2::new(1, 5));
        assert_eq!(ii.size().y, i.height() + 1);

        // 不相交矩形 → 翻转交集，宽/高为负
        let c = SpdRect::new(20, 20, 24, 24);
        let empty = a.intersect(&c);
        assert!(empty.width() < 0 && empty.height() < 0);
        assert_eq!(empty.points().count(), 0);
    }

    #[test]
    fn resize_and_pos_follow_spd_semantics() {
        let mut r = SpdRect::default();
        assert!(r.is_empty());
        // resize(4, 4) → SPD 宽 4 = 5 格足迹
        r.resize(4, 4);
        assert_eq!(r, SpdRect::new(0, 0, 4, 4));
        assert!(!r.is_empty());
        r.set_pos(3, 7);
        assert_eq!(r, SpdRect::new(3, 7, 7, 11));
        r.shift(-3, -7);
        assert_eq!(r, SpdRect::new(0, 0, 4, 4));
        r.set_empty();
        assert!(r.is_empty());
    }

    #[test]
    fn points_iterate_closed_intervals_x_outer() {
        let r = SpdRect::new(1, 2, 2, 3);
        let pts: Vec<IVec2> = r.points().collect();
        assert_eq!(
            pts,
            vec![
                IVec2::new(1, 2),
                IVec2::new(1, 3),
                IVec2::new(2, 2),
                IVec2::new(2, 3),
            ]
        );
    }
}
