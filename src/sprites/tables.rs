//! 角色精灵数据表：Warrior/Rat/Snake/Crab 四套帧网格 + 动画剪辑，
//! 逐项照抄各 `sprites/*Sprite.java` 构造器并注明行号（表驱动原则见
//! `docs/plans/01`）。贴图像素尺寸 Java 在运行期读取，此处为常量，
//! 与 png 实际尺寸的一致性由测试解析 PNG 头对拍。
//!
//! 英雄图集两级布局（`HeroSprite.java`）：六职业各一张 256×128 同构图集
//! （`HeroClass.spritesheet()` L296-L311，正因同构，Java `tiers()` L170
//! 固定用 ROGUE 贴图切行条）；图集内先按"全宽 × 15px"切护甲阶行条
//! （`tiers()` L168-L175，行号 = `Hero.tier()` L454-L462：0 无甲、
//! 1-5 护甲阶、6 职业甲），再在行条内按 12×15 切帧（`updateArmor()` L73）。
//! 本波只取战士基础行（tier 0）；换甲/换职业 = 换行条/换贴图，网格结构不变。

use bevy::math::{URect, UVec2};

use super::{
    CharSpriteSheet,
    clip::{AnimClip, CharAnimSet},
    film::FrameGrid,
};
use crate::assets::SpriteType;

/// 英雄单帧像素尺寸（`HeroSprite.java` L42-L43 `FRAME_WIDTH`/`FRAME_HEIGHT`）。
pub const HERO_FRAME_SIZE: UVec2 = UVec2::new(12, 15);

/// 英雄图集像素尺寸（六职业同构，warrior.png 等实测 256×128）。
pub const HERO_TEXTURE_SIZE: UVec2 = UVec2::new(256, 128);

/// 英雄跑动帧率（`HeroSprite.java` L45 `RUN_FRAMERATE`）。
const HERO_RUN_FRAMERATE: u32 = 20;

/// 护甲阶 `tier` 的帧网格：行条 y ∈ `[tier*15, (tier+1)*15)` 全宽（`tiers()`
/// L168-L175），行条内 256/12 = 21 帧（`updateArmor()` L73；HeroSprite 实际
/// 使用帧 0-20：四剪辑用到 0-15，operate 16-17、fly 18、read 19-20）。
///
/// 六职业图集同构，本函数对任一职业贴图成立；tier 合法域 0-6
/// （`Hero.tier()` L454-L462），越界（行条超出 128px）编译期/运行期断言兜底。
#[must_use]
pub const fn hero_tier_grid(tier: u32) -> FrameGrid {
    FrameGrid::with_region(
        HERO_TEXTURE_SIZE,
        URect {
            min: UVec2::new(0, tier * HERO_FRAME_SIZE.y),
            max: UVec2::new(HERO_TEXTURE_SIZE.x, (tier + 1) * HERO_FRAME_SIZE.y),
        },
        HERO_FRAME_SIZE,
    )
}

/// 角色精灵种类（本波四套：英雄基线 + 下水道三怪，与 `actors/bestiary.rs`
/// 的 M1 范围对应；后续随图鉴逐层扩充）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharSpriteKind {
    /// 战士（`HeroSprite.java`，基础行 = 护甲阶 0）。
    Warrior,
    /// 老鼠（`RatSprite.java`）。
    Rat,
    /// 蛇（`SnakeSprite.java`）。
    Snake,
    /// 螃蟹（`CrabSprite.java`）。
    Crab,
}

impl CharSpriteKind {
    /// 图集资产枚举映射（Java `texture(...)` 调用处：`HeroSprite` 经
    /// `HeroClass.spritesheet()` L296-L299、`RatSprite` L32、
    /// `SnakeSprite` L32、`CrabSprite` L32）。
    #[must_use]
    pub const fn sprite_type(self) -> SpriteType {
        match self {
            Self::Warrior => SpriteType::Warrior,
            Self::Rat => SpriteType::Rat,
            Self::Snake => SpriteType::Snake,
            Self::Crab => SpriteType::Crab,
        }
    }

    /// 帧网格 + 四剪辑数据表（帧序列、fps、looped 逐值照抄 Java）。
    #[must_use]
    pub const fn sheet(self) -> CharSpriteSheet {
        match self {
            // HeroSprite.java（updateArmor L71-L102，战士取 tier 0 行条）：
            // idle 1fps 循环（L75-L76）、run 20fps 循环（L45/L78-L79）、
            // attack 15fps 单次（L84-L85）、die 20fps 单次（L81-L82）。
            // 注意 attack 末帧回 0（收势即待机首帧）、die 末帧 11（倒地回弹）。
            Self::Warrior => CharSpriteSheet {
                grid: hero_tier_grid(0),
                anims: CharAnimSet {
                    idle: AnimClip::new(1, &[0, 0, 0, 1, 0, 0, 1, 1], true), // L75-L76
                    run: AnimClip::new(HERO_RUN_FRAMERATE, &[2, 3, 4, 5, 6, 7], true), // L78-L79
                    attack: AnimClip::new(15, &[13, 14, 15, 0], false),      // L84-L85
                    die: AnimClip::new(20, &[8, 9, 10, 11, 12, 11], false),  // L81-L82
                },
            },
            // RatSprite.java：贴图 rat.png 256×64（L32），帧 16×15（L34）；
            // idle 2fps（L36-L37）、run 10fps（L39-L40）、
            // attack 15fps（L42-L43，末帧回 0）、die 10fps（L45-L46）。
            Self::Rat => CharSpriteSheet {
                grid: FrameGrid::new(UVec2::new(256, 64), UVec2::new(16, 15)),
                anims: CharAnimSet {
                    idle: AnimClip::new(2, &[0, 0, 0, 1], true), // L36-L37
                    run: AnimClip::new(10, &[6, 7, 8, 9, 10], true), // L39-L40
                    attack: AnimClip::new(15, &[2, 3, 4, 5, 0], false), // L42-L43
                    die: AnimClip::new(10, &[11, 12, 13, 14], false), // L45-L46
                },
            },
            // SnakeSprite.java：贴图 snake.png 256×16（L32），帧 12×11（L34）；
            // idle 10fps 30 帧——起伏慢、吐舌快，靠重复索引拼节奏
            // （L36 注释、L37-L39）；run 8fps（L41-L42）、
            // attack 15fps（L44-L45，末帧回 0）、die 10fps（L47-L48）。
            Self::Snake => CharSpriteSheet {
                grid: FrameGrid::new(UVec2::new(256, 16), UVec2::new(12, 11)),
                anims: CharAnimSet {
                    // L37-L39：15 帧 0 + 10 帧 1 + 2,3,2,1,1
                    idle: AnimClip::new(
                        10,
                        &[
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
                            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 3, 2, 1, 1,
                        ],
                        true,
                    ),
                    run: AnimClip::new(8, &[4, 5, 6, 7], true), // L41-L42
                    attack: AnimClip::new(15, &[8, 9, 10, 9, 0], false), // L44-L45
                    die: AnimClip::new(10, &[11, 12, 13], false), // L47-L48
                },
            },
            // CrabSprite.java：贴图 crab.png 256×64（L32），帧 16×16（L34）；
            // idle 5fps（L36-L37）、run 15fps（L39-L40）、
            // attack 12fps（L42-L43）、die 12fps（L45-L46）。
            Self::Crab => CharSpriteSheet {
                grid: FrameGrid::new(UVec2::new(256, 64), UVec2::new(16, 16)),
                anims: CharAnimSet {
                    idle: AnimClip::new(5, &[0, 1, 0, 2], true),  // L36-L37
                    run: AnimClip::new(15, &[3, 4, 5, 6], true),  // L39-L40
                    attack: AnimClip::new(12, &[7, 8, 9], false), // L42-L43
                    die: AnimClip::new(12, &[10, 11, 12, 13], false), // L45-L46
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::utils::PropertyPath;

    const ALL_KINDS: [CharSpriteKind; 4] = [
        CharSpriteKind::Warrior,
        CharSpriteKind::Rat,
        CharSpriteKind::Snake,
        CharSpriteKind::Crab,
    ];

    /// Warrior 逐值对拍（`HeroSprite.java` L42-L43/L45/L75-L85 +
    /// 战士基础行 = tier 0：`Hero.tier()` L461 无甲）。
    #[test]
    fn warrior_table_matches_java() {
        let sheet = CharSpriteKind::Warrior.sheet();

        assert_eq!(sheet.grid.texture_size(), UVec2::new(256, 128));
        assert_eq!(sheet.grid.frame_size(), UVec2::new(12, 15), "L42-L43");
        assert_eq!(sheet.grid.columns(), 21, "行条 256/12（updateArmor L73）");
        assert_eq!(sheet.grid.rows(), 1, "单行条切割");
        // tier 0 行条：帧 0 在图集左上角
        assert_eq!(sheet.grid.frame_rect(0), URect::new(0, 0, 12, 15));

        let a = sheet.anims;
        assert_eq!(a.idle.fps, 1, "L75");
        assert!(a.idle.looped, "L75");
        assert_eq!(a.idle.frames, &[0, 0, 0, 1, 0, 0, 1, 1], "L76");

        assert_eq!(a.run.fps, 20, "L45 RUN_FRAMERATE");
        assert!(a.run.looped, "L78");
        assert_eq!(a.run.frames, &[2, 3, 4, 5, 6, 7], "L79");

        assert_eq!(a.attack.fps, 15, "L84");
        assert!(!a.attack.looped, "L84");
        assert_eq!(a.attack.frames, &[13, 14, 15, 0], "L85");

        assert_eq!(a.die.fps, 20, "L81");
        assert!(!a.die.looped, "L81");
        assert_eq!(a.die.frames, &[8, 9, 10, 11, 12, 11], "L82");
    }

    /// 英雄两级布局：tier t 行条内帧矩形带 y = t*15 偏移（`tiers()`
    /// L168-L175 + `TextureFilm` patch 平移 L88）；tier 0-6 全部落在
    /// 128px 高度内（`Hero.tier()` L454-L462 合法域）。
    #[test]
    fn hero_tier_rows_shift_by_fifteen_pixels() {
        for tier in 0..=6 {
            let grid = hero_tier_grid(tier);
            assert_eq!(grid.columns(), 21);
            assert_eq!(grid.rows(), 1);
            assert_eq!(
                grid.frame_rect(0),
                URect::new(0, tier * 15, 12, (tier + 1) * 15),
                "tier {tier} 行条偏移"
            );
        }
        // 最高合法行条（tier 6 职业甲）底边 105 ≤ 128
        assert_eq!(
            hero_tier_grid(6).frame_rect(20),
            URect::new(240, 90, 252, 105)
        );
    }

    /// Rat 逐值对拍（`RatSprite.java` L32/L34/L36-L46）。
    #[test]
    fn rat_table_matches_java() {
        let sheet = CharSpriteKind::Rat.sheet();

        assert_eq!(
            sheet.grid.texture_size(),
            UVec2::new(256, 64),
            "L32 rat.png"
        );
        assert_eq!(sheet.grid.frame_size(), UVec2::new(16, 15), "L34");
        assert_eq!(sheet.grid.frame_count(), 64, "16 列 × 4 行");

        let a = sheet.anims;
        assert_eq!((a.idle.fps, a.idle.looped), (2, true), "L36");
        assert_eq!(a.idle.frames, &[0, 0, 0, 1], "L37");
        assert_eq!((a.run.fps, a.run.looped), (10, true), "L39");
        assert_eq!(a.run.frames, &[6, 7, 8, 9, 10], "L40");
        assert_eq!((a.attack.fps, a.attack.looped), (15, false), "L42");
        assert_eq!(a.attack.frames, &[2, 3, 4, 5, 0], "L43");
        assert_eq!((a.die.fps, a.die.looped), (10, false), "L45");
        assert_eq!(a.die.frames, &[11, 12, 13, 14], "L46");
    }

    /// Snake 逐值对拍（`SnakeSprite.java` L32/L34/L37-L48；
    /// idle 30 帧重复索引是身份特征，整条照抄断言）。
    #[test]
    fn snake_table_matches_java() {
        let sheet = CharSpriteKind::Snake.sheet();

        assert_eq!(
            sheet.grid.texture_size(),
            UVec2::new(256, 16),
            "L32 snake.png"
        );
        assert_eq!(sheet.grid.frame_size(), UVec2::new(12, 11), "L34");
        assert_eq!(sheet.grid.frame_count(), 21, "非满行：256 = 21×12 + 4");

        let a = sheet.anims;
        assert_eq!((a.idle.fps, a.idle.looped), (10, true), "L37");
        assert_eq!(
            a.idle.frames,
            &[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 3, 2, 1, 1,
            ],
            "L38-L39"
        );
        assert_eq!(a.idle.frames.len(), 30, "L38-L39 共 30 帧");
        assert_eq!((a.run.fps, a.run.looped), (8, true), "L41");
        assert_eq!(a.run.frames, &[4, 5, 6, 7], "L42");
        assert_eq!((a.attack.fps, a.attack.looped), (15, false), "L44");
        assert_eq!(a.attack.frames, &[8, 9, 10, 9, 0], "L45");
        assert_eq!((a.die.fps, a.die.looped), (10, false), "L47");
        assert_eq!(a.die.frames, &[11, 12, 13], "L48");
    }

    /// Crab 逐值对拍（`CrabSprite.java` L32/L34/L36-L46）。
    #[test]
    fn crab_table_matches_java() {
        let sheet = CharSpriteKind::Crab.sheet();

        assert_eq!(
            sheet.grid.texture_size(),
            UVec2::new(256, 64),
            "L32 crab.png"
        );
        assert_eq!(sheet.grid.frame_size(), UVec2::new(16, 16), "L34");
        assert_eq!(sheet.grid.frame_count(), 64, "16 列 × 4 行");

        let a = sheet.anims;
        assert_eq!((a.idle.fps, a.idle.looped), (5, true), "L36");
        assert_eq!(a.idle.frames, &[0, 1, 0, 2], "L37");
        assert_eq!((a.run.fps, a.run.looped), (15, true), "L39");
        assert_eq!(a.run.frames, &[3, 4, 5, 6], "L40");
        assert_eq!((a.attack.fps, a.attack.looped), (12, false), "L42");
        assert_eq!(a.attack.frames, &[7, 8, 9], "L43");
        assert_eq!((a.die.fps, a.die.looped), (12, false), "L45");
        assert_eq!(a.die.frames, &[10, 11, 12, 13], "L46");
    }

    /// 常量一致性：四套表所有剪辑的每个帧索引都在网格内，
    /// 且像素矩形不越贴图边界（min < max 由网格构造保证）。
    #[test]
    fn all_table_frames_stay_inside_texture() {
        for kind in ALL_KINDS {
            let sheet = kind.sheet();
            let texture = sheet.grid.texture_size();
            let a = sheet.anims;
            for (name, clip) in [
                ("idle", a.idle),
                ("run", a.run),
                ("attack", a.attack),
                ("die", a.die),
            ] {
                for &frame in clip.frames {
                    assert!(
                        frame < sheet.grid.frame_count(),
                        "{kind:?}.{name} 帧 {frame} 越出网格"
                    );
                    let rect = sheet.grid.frame_rect(frame);
                    assert!(
                        rect.max.x <= texture.x && rect.max.y <= texture.y,
                        "{kind:?}.{name} 帧 {frame} 矩形 {rect:?} 越出贴图 {texture}"
                    );
                }
            }
        }
    }

    /// 表内贴图尺寸常量与 png 实际尺寸对拍（解析 PNG IHDR 头：
    /// 8 字节签名 + 4 长度 + "IHDR" + 宽/高各 4 字节大端）。
    /// Java 运行期读贴图故天然一致，Rust 常量化后靠此测试防漂移。
    #[test]
    fn atlas_png_sizes_match_tables() {
        for kind in ALL_KINDS {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join(kind.sprite_type().get_property_path());
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("读取 {} 失败: {e}", path.display()));
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{kind:?} 非 PNG 文件");
            assert_eq!(&bytes[12..16], b"IHDR", "{kind:?} 首个块非 IHDR");
            let size = UVec2::new(
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
            );
            assert_eq!(
                kind.sheet().grid.texture_size(),
                size,
                "{kind:?} 表内贴图尺寸与 {} 不符",
                path.display()
            );
        }
    }

    /// 资产枚举映射（Java `texture(...)` 调用处，见 `sprite_type` 文档）。
    #[test]
    fn sprite_type_mapping_matches_java_texture_calls() {
        assert_eq!(CharSpriteKind::Warrior.sprite_type(), SpriteType::Warrior);
        assert_eq!(CharSpriteKind::Rat.sprite_type(), SpriteType::Rat);
        assert_eq!(CharSpriteKind::Snake.sprite_type(), SpriteType::Snake);
        assert_eq!(CharSpriteKind::Crab.sprite_type(), SpriteType::Crab);
    }
}
