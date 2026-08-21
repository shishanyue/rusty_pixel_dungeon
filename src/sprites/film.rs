//! 帧网格切割：SPD `noosa/TextureFilm.java` 的数据层等价物。
//!
//! Java 版在运行期读贴图尺寸、把帧存成归一化 UV `RectF`；Rust 版改为编译期
//! 常量表，直接产出整数像素矩形（[`URect`]，min 含 / max 不含），归一化交给
//! `bevy_sprite` 内部完成——`TextureAtlasLayout::textures` 与 `Sprite.rect`
//! 消费的正是这种像素矩形。

use bevy::{
    image::TextureAtlasLayout,
    math::{Rect, URect, UVec2},
};

/// 帧网格：贴图像素尺寸 + 帧宽高 → 帧索引 → 像素矩形。
///
/// 等价 `TextureFilm(Object tx, int width, int height)`（Java L53-L71）：
/// `cols = texWidth / width`、`rows = texHeight / height`（整除截断，
/// 贴图尺寸非帧尺寸整倍数时右/下余量不成帧，与 Java 一致），
/// 帧索引行主序 `index = row * cols + col`（L65-L69 `add(i * cols + j, …)`）。
///
/// 另支持子区域切割（[`FrameGrid::with_region`]），等价
/// `TextureFilm(TextureFilm atlas, Object key, int width, int height)`
/// （Java L73-L92）：帧矩形相对子区域左上角平移（L88 `rect.shift`），
/// 供英雄图集"护甲阶行条内再切帧"的两级布局使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGrid {
    /// 整张贴图像素尺寸（Java L34-L35 `texWidth`/`texHeight`，运行期读取；
    /// Rust 侧为常量，与 png 实际尺寸的一致性由测试对拍）
    texture_size: UVec2,
    /// 单帧像素尺寸（构造参数 `width`/`height`）
    frame_size: UVec2,
    /// 网格区域左上角在贴图内的像素偏移（整图切割为零；子区域切割 = patch 左上角）
    origin: UVec2,
    /// 每行帧数（Java L62 `cols = texWidth / width`）
    columns: u32,
    /// 行数（Java L63 `rows = texHeight / height`）
    rows: u32,
}

impl FrameGrid {
    /// 整图网格切割（`TextureFilm(tx, width, height)`，Java L53-L71）。
    #[must_use]
    pub const fn new(texture_size: UVec2, frame_size: UVec2) -> Self {
        Self::with_region(
            texture_size,
            URect {
                min: UVec2::ZERO,
                max: texture_size,
            },
            frame_size,
        )
    }

    /// 贴图子区域内的网格切割（`TextureFilm(atlas, key, width, height)`，
    /// Java L73-L92）：`cols`/`rows` 按子区域尺寸整除（L82-L83），
    /// 帧矩形整体平移子区域左上角（L88 `rect.shift(patch.left, patch.top)`）。
    ///
    /// # Panics
    ///
    /// 帧尺寸为零（除零）或子区域越出贴图时 panic；
    /// 常量表构造发生在 const 上下文，违规直接编译失败。
    #[must_use]
    pub const fn with_region(texture_size: UVec2, region: URect, frame_size: UVec2) -> Self {
        assert!(
            frame_size.x > 0 && frame_size.y > 0,
            "帧尺寸必须为正（除零保护）"
        );
        assert!(
            region.max.x <= texture_size.x && region.max.y <= texture_size.y,
            "子区域越出贴图"
        );
        Self {
            texture_size,
            frame_size,
            origin: region.min,
            columns: (region.max.x - region.min.x) / frame_size.x,
            rows: (region.max.y - region.min.y) / frame_size.y,
        }
    }

    /// 整张贴图像素尺寸。
    #[must_use]
    pub const fn texture_size(&self) -> UVec2 {
        self.texture_size
    }

    /// 单帧像素尺寸。
    #[must_use]
    pub const fn frame_size(&self) -> UVec2 {
        self.frame_size
    }

    /// 每行帧数。
    #[must_use]
    pub const fn columns(&self) -> u32 {
        self.columns
    }

    /// 行数。
    #[must_use]
    pub const fn rows(&self) -> u32 {
        self.rows
    }

    /// 网格内总帧数（Java 只登记 `rows * cols` 个键，越界键取不到帧）。
    #[must_use]
    pub const fn frame_count(&self) -> u32 {
        self.columns * self.rows
    }

    /// 帧索引 → 贴图像素矩形（min 含、max 不含；原点 = 贴图左上角，y 向下，
    /// 与 `TextureAtlasLayout::textures` / `Sprite.rect` 的图像坐标一致）。
    ///
    /// 行主序反解 `col = index % cols`、`row = index / cols`，矩形即
    /// Java L67 归一化 UV 的像素原值，子区域切割时已含 L88 的平移。
    ///
    /// # Panics
    ///
    /// `index >= frame_count()` 时 panic（Java 对未登记键返回 null 继而
    /// NPE，Rust 侧显式断言，数据表越界由测试兜底）。
    #[must_use]
    pub const fn frame_rect(&self, index: u32) -> URect {
        assert!(index < self.frame_count(), "帧索引越出网格");
        let col = index % self.columns;
        let row = index / self.columns;
        let min = UVec2::new(
            self.origin.x + col * self.frame_size.x,
            self.origin.y + row * self.frame_size.y,
        );
        URect {
            min,
            max: UVec2::new(min.x + self.frame_size.x, min.y + self.frame_size.y),
        }
    }

    /// 帧索引 → `Sprite.rect` 直接可用的 f32 像素矩形
    /// （`Sprite.rect: Option<Rect>` 语义：图像区域裁剪，像素坐标）。
    #[must_use]
    pub fn sprite_rect(&self, index: u32) -> Rect {
        self.frame_rect(index).as_rect()
    }

    /// 整网格 → [`TextureAtlasLayout`]（`size` = 整张贴图，`textures[i]` =
    /// [`Self::frame_rect`]），帧索引即 atlas 索引，供下波接线一行装配。
    #[must_use]
    pub fn atlas_layout(&self) -> TextureAtlasLayout {
        TextureAtlasLayout {
            size: self.texture_size,
            textures: (0..self.frame_count())
                .map(|i| self.frame_rect(i))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec2;

    use super::*;

    /// Rat 图集手算对拍（256×64、帧 16×15）：首帧/末帧/换行边界的像素矩形。
    /// 64/15 = 4 行（整除截断，底部 4px 余量不成帧，Java L63 语义）。
    #[test]
    fn rat_grid_hand_checked() {
        let grid = FrameGrid::new(UVec2::new(256, 64), UVec2::new(16, 15));
        assert_eq!(grid.columns(), 16, "256/16");
        assert_eq!(grid.rows(), 4, "64/15 整除截断");
        assert_eq!(grid.frame_count(), 64);

        // 首帧
        assert_eq!(
            grid.frame_rect(0),
            URect::new(0, 0, 16, 15),
            "帧 0 = 左上角"
        );
        // 首行末帧 → 次行首帧的换行边界
        assert_eq!(
            grid.frame_rect(15),
            URect::new(240, 0, 256, 15),
            "帧 15 = 首行最右"
        );
        assert_eq!(
            grid.frame_rect(16),
            URect::new(0, 15, 16, 30),
            "帧 16 换行回到 x=0"
        );
        // 末帧：col 15、row 3
        assert_eq!(
            grid.frame_rect(63),
            URect::new(240, 45, 256, 60),
            "末帧不吃底部 4px 余量"
        );
    }

    /// Snake 图集（256×16、帧 12×11）：非满行贴图（256 = 21×12 + 4）。
    #[test]
    fn snake_grid_drops_trailing_slack() {
        let grid = FrameGrid::new(UVec2::new(256, 16), UVec2::new(12, 11));
        assert_eq!(grid.columns(), 21, "256/12 截断，右侧 4px 不成帧");
        assert_eq!(grid.rows(), 1, "16/11 截断，底部 5px 不成帧");
        assert_eq!(grid.frame_count(), 21);
        assert_eq!(grid.frame_rect(0), URect::new(0, 0, 12, 11));
        let last = grid.frame_rect(20);
        assert_eq!(last, URect::new(240, 0, 252, 11));
        assert!(last.max.x <= 256 && last.max.y <= 16, "末帧不越贴图边界");
    }

    /// 子区域切割（TextureFilm patch 语义，Java L73-L92）：英雄图集
    /// 护甲阶行条内切帧，帧矩形带行条 y 偏移（L88 `rect.shift`）。
    #[test]
    fn sub_region_grid_shifts_frames() {
        let texture = UVec2::new(256, 128);
        // tier 1 行条：y ∈ [15, 30)
        let tier1 = FrameGrid::with_region(texture, URect::new(0, 15, 256, 30), UVec2::new(12, 15));
        assert_eq!(tier1.columns(), 21, "行条宽 256 / 帧宽 12");
        assert_eq!(tier1.rows(), 1, "行条高恰为一帧");
        assert_eq!(
            tier1.frame_rect(0),
            URect::new(0, 15, 12, 30),
            "帧 0 平移到行条内"
        );
        assert_eq!(tier1.frame_rect(20), URect::new(240, 15, 252, 30));
    }

    /// `sprite_rect` 与 `frame_rect` 数值一致（f32 化）；`atlas_layout`
    /// 尺寸 = 整贴图、逐帧矩形 = `frame_rect`。
    #[test]
    fn bevy_conversions_match_frame_rect() {
        let grid = FrameGrid::new(UVec2::new(256, 64), UVec2::new(16, 16));
        let rect = grid.sprite_rect(17);
        assert_eq!(rect.min, Vec2::new(16.0, 16.0));
        assert_eq!(rect.max, Vec2::new(32.0, 32.0));

        let layout = grid.atlas_layout();
        assert_eq!(layout.size, UVec2::new(256, 64));
        assert_eq!(layout.textures.len() as u32, grid.frame_count());
        for (i, tex) in layout.textures.iter().enumerate() {
            assert_eq!(*tex, grid.frame_rect(i as u32));
        }
    }

    /// 越界帧索引触发 panic（数据表兜底防线）。
    #[test]
    #[should_panic(expected = "帧索引越出网格")]
    fn out_of_range_index_panics() {
        let grid = FrameGrid::new(UVec2::new(256, 64), UVec2::new(16, 15));
        let _ = grid.frame_rect(64);
    }
}
