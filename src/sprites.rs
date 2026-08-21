//! 角色精灵动画数据域（25 号计划）：帧网格切割、动画剪辑与四套角色数据表。
//!
//! **纯数据层**——只提供 SPD `TextureFilm`/`MovieClip.Animation` 的数据等价物
//! 与 `bevy_sprite` 可直接消费的像素矩形换算（[`FrameGrid::frame_rect`] →
//! `TextureAtlasLayout::textures`；[`FrameGrid::sprite_rect`] → `Sprite.rect`）。
//! 播放系统（`MovieClip.updateAnimation` 的推帧循环、`CharSprite` 的状态切换）
//! 是下波接线，届时在此挂 Plugin。
//!
//! 对照源：
//! - `SPD-classes/.../noosa/TextureFilm.java`：帧网格 → [`film`]
//! - `SPD-classes/.../noosa/MovieClip.java`：动画剪辑 → [`clip`]
//! - `core/.../sprites/{CharSprite,HeroSprite,RatSprite,SnakeSprite,CrabSprite}.java`
//!   ：四剪辑约定与各角色帧表 → [`tables`]

pub mod clip;
pub mod film;
pub mod tables;

pub use clip::{AnimClip, CharAnimSet};
pub use film::FrameGrid;
pub use tables::{CharSpriteKind, HERO_FRAME_SIZE, HERO_TEXTURE_SIZE, hero_tier_grid};

/// 一套角色精灵数据：帧网格 + 四剪辑（Java 侧对应"`texture(...)` +
/// `TextureFilm` + 各 `Animation` 字段"在具体 `*Sprite` 构造器里的组合）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharSpriteSheet {
    /// 帧索引 → 像素矩形的换算网格。
    pub grid: FrameGrid,
    /// idle/run/attack/die 四剪辑。
    pub anims: CharAnimSet,
}
