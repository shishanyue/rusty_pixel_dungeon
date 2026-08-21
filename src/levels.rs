//! 关卡数据核心。生成流水线（房间/构建器/画师）见 `docs/plans/10-level-generation.md`。
//!
//! 坐标约定：`IVec2` 格子坐标，原点左上；矩形用 `bevy::math::IRect`（`max` 开区间）；
//! 线性索引 = `y * width + x`。

use bevy::{math::IRect, prelude::*};

use crate::levels::terrain::{Terrain, TerrainFlags};

pub mod builder;
pub mod generator;
pub mod painter;
pub mod patch;
pub mod random;
pub mod rect;
pub mod rooms;
pub mod special;
pub mod standard;
pub mod terrain;

pub use generator::generate_level;

/// 地牢层的氛围变体，对照 SPD `Level.Feeling`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Feeling {
    #[default]
    None,
    Chasm,
    Water,
    Grass,
    Dark,
    Large,
    Traps,
    Secrets,
}

/// 一层地牢的地图数据。生成完成后作为 Resource 插入。
#[derive(Debug, Resource)]
pub struct Level {
    map: Vec<Terrain>,
    width: usize,
    height: usize,

    pub depth: i32,
    pub feeling: Feeling,
    pub entrance: IVec2,
    pub exit: IVec2,

    // 地形标志缓存，随 set_terrain 增量维护（对照 SPD buildFlagMaps）
    pub passable: Vec<bool>,
    pub los_blocking: Vec<bool>,
    pub solid: Vec<bool>,
}

impl Level {
    /// 新建一层，全图填充墙（SPD 生成同样从全墙开始挖）。
    pub fn new(width: usize, height: usize, depth: i32) -> Self {
        assert!(width > 0 && height > 0, "关卡尺寸必须为正");
        let size = width * height;
        let wall_flags = Terrain::Wall.flags();
        Self {
            map: vec![Terrain::Wall; size],
            width,
            height,
            depth,
            feeling: Feeling::None,
            entrance: IVec2::ZERO,
            exit: IVec2::ZERO,
            passable: vec![wall_flags.contains(TerrainFlags::PASSABLE); size],
            los_blocking: vec![wall_flags.contains(TerrainFlags::LOS_BLOCKING); size],
            solid: vec![wall_flags.contains(TerrainFlags::SOLID); size],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn size(&self) -> usize {
        self.width * self.height
    }

    pub fn map(&self) -> &[Terrain] {
        &self.map
    }

    pub fn is_inside(&self, pos: IVec2) -> bool {
        pos.x >= 0 && pos.y >= 0 && (pos.x as usize) < self.width && (pos.y as usize) < self.height
    }

    /// 读地形；越界一律视为墙（与 SPD 边界语义一致）。
    pub fn terrain(&self, pos: IVec2) -> Terrain {
        if self.is_inside(pos) {
            self.map[self.index(pos)]
        } else {
            Terrain::Wall
        }
    }

    /// 写地形并同步标志缓存；越界写入静默忽略。
    pub fn set_terrain(&mut self, pos: IVec2, terrain: Terrain) {
        if self.is_inside(pos) {
            let index = self.index(pos);
            self.map[index] = terrain;
            let flags = terrain.flags();
            self.passable[index] = flags.contains(TerrainFlags::PASSABLE);
            self.los_blocking[index] = flags.contains(TerrainFlags::LOS_BLOCKING);
            self.solid[index] = flags.contains(TerrainFlags::SOLID);
        }
    }

    /// 填充矩形（`rect.max` 开区间，bevy `IRect` 约定）。
    pub fn fill(&mut self, rect: IRect, terrain: Terrain) {
        for y in rect.min.y..rect.max.y {
            for x in rect.min.x..rect.max.x {
                self.set_terrain(IVec2::new(x, y), terrain);
            }
        }
    }

    pub fn index(&self, pos: IVec2) -> usize {
        pos.y as usize * self.width + pos.x as usize
    }

    pub fn pos_of(&self, index: usize) -> IVec2 {
        IVec2::new((index % self.width) as i32, (index / self.width) as i32)
    }

    /// 调试字符画：`#` 墙、`.` 地板、`+` 门、`L` 锁门、`S` 密门（调试图有意
    /// 暴露，游戏内视觉是墙）、`E` 入口、`X` 出口、`~` 水、`"` 矮草、`!` 高草、
    /// `^` 明陷阱、`,` 暗陷阱（同为有意暴露）、`&` 雕像/区域装饰（实体）。
    /// 其余地形按类别归并（深渊留空等），行序自上而下。
    pub fn debug_ascii(&self) -> String {
        let mut out = String::with_capacity((self.width + 1) * self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let terrain = self.map[y * self.width + x];
                out.push(match terrain {
                    Terrain::Wall | Terrain::WallDeco => '#',
                    Terrain::Door | Terrain::OpenDoor => '+',
                    // 三期符号：锁门 L（含骷髅锁）、密门 S；水晶门并入 L
                    Terrain::LockedDoor | Terrain::HeroLockedDoor | Terrain::CrystalDoor => 'L',
                    Terrain::SecretDoor => 'S',
                    Terrain::Entrance | Terrain::EntranceSp => 'E',
                    Terrain::Exit | Terrain::LockedExit | Terrain::UnlockedExit => 'X',
                    Terrain::Water => '~',
                    Terrain::Grass | Terrain::FurrowedGrass => '"',
                    Terrain::HighGrass => '!',
                    Terrain::Trap => '^',
                    // 游戏内视觉是普通地板；调试图用 `,` 有意暴露以便核对落位
                    Terrain::SecretTrap => ',',
                    Terrain::Chasm => ' ',
                    Terrain::Barricade | Terrain::Bookshelf => 'B',
                    Terrain::Statue | Terrain::StatueSp | Terrain::RegionDeco | Terrain::RegionDecoAlt => {
                        '&'
                    }
                    t if t.is_passable() => '.',
                    _ => '?',
                });
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_level_is_all_wall() {
        let level = Level::new(4, 3, 1);
        assert_eq!(level.size(), 12);
        assert!(level.map().iter().all(|&t| t == Terrain::Wall));
        assert!(level.solid.iter().all(|&s| s));
        assert!(!level.passable.iter().any(|&p| p));
    }

    #[test]
    fn fill_respects_open_max_and_bounds() {
        let mut level = Level::new(5, 5, 1);
        // max 开区间：填 (1,1)-(4,4) 实际覆盖 x,y ∈ [1,3]
        level.fill(IRect::new(1, 1, 4, 4), Terrain::Empty);
        assert_eq!(level.terrain(IVec2::new(1, 1)), Terrain::Empty);
        assert_eq!(level.terrain(IVec2::new(3, 3)), Terrain::Empty);
        assert_eq!(level.terrain(IVec2::new(4, 4)), Terrain::Wall);
        // 越界填充不 panic、不写入
        level.fill(IRect::new(3, 3, 100, 100), Terrain::Water);
        assert_eq!(level.terrain(IVec2::new(4, 4)), Terrain::Water);
        assert_eq!(level.terrain(IVec2::new(100, 100)), Terrain::Wall);
    }

    #[test]
    fn flag_caches_follow_terrain() {
        let mut level = Level::new(3, 3, 1);
        let pos = IVec2::new(1, 1);
        level.set_terrain(pos, Terrain::Water);
        let idx = level.index(pos);
        assert!(level.passable[idx]);
        assert!(!level.solid[idx]);
        level.set_terrain(pos, Terrain::HighGrass);
        assert!(level.passable[idx]);
        assert!(level.los_blocking[idx]);
    }

    #[test]
    fn index_roundtrip() {
        let level = Level::new(7, 4, 1);
        for i in 0..level.size() {
            assert_eq!(level.index(level.pos_of(i)), i);
        }
    }

    #[test]
    fn out_of_bounds_reads_as_wall() {
        let level = Level::new(3, 3, 1);
        assert_eq!(level.terrain(IVec2::new(-1, 0)), Terrain::Wall);
        assert_eq!(level.terrain(IVec2::new(3, 0)), Terrain::Wall);
    }

    /// 二期符号集：水 `~`、矮草 `"`、高草 `!`、明陷阱 `^`、暗陷阱 `,`。
    #[test]
    fn debug_ascii_maps_phase2_terrains() {
        let mut level = Level::new(6, 1, 1);
        for (x, t) in [
            Terrain::Water,
            Terrain::Grass,
            Terrain::HighGrass,
            Terrain::Trap,
            Terrain::SecretTrap,
            Terrain::FurrowedGrass,
        ]
        .into_iter()
        .enumerate()
        {
            level.set_terrain(IVec2::new(x as i32, 0), t);
        }
        assert_eq!(level.debug_ascii(), "~\"!^,\"\n");
    }

    /// 三期符号集：锁门 `L`、密门 `S`、区域装饰 `&`、余烬/失效陷阱归地板。
    #[test]
    fn debug_ascii_maps_phase3_terrains() {
        let mut level = Level::new(7, 1, 1);
        for (x, t) in [
            Terrain::LockedDoor,
            Terrain::SecretDoor,
            Terrain::CrystalDoor,
            Terrain::RegionDecoAlt,
            Terrain::Embers,
            Terrain::InactiveTrap,
            Terrain::EmptySp,
        ]
        .into_iter()
        .enumerate()
        {
            level.set_terrain(IVec2::new(x as i32, 0), t);
        }
        assert_eq!(level.debug_ascii(), "LSL&...\n");
    }
}
