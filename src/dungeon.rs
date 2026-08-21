//! 一局游戏的全局状态，对照 SPD `Dungeon.java`（静态字段 → Resource）。

use bevy::{platform::collections::HashMap, prelude::*};
use num_enum::IntoPrimitive;
use strum::{AsRefStr, EnumIter, IntoEnumIterator};

use crate::setting::Settings;

/// 深度 + 分支决定的关卡类型，对照 `Dungeon.newLevel()`（v3.3.8, L297-L375）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LevelKind {
    Sewer,
    SewerBoss,
    Prison,
    PrisonBoss,
    Caves,
    CavesBoss,
    City,
    CityBoss,
    Halls,
    HallsBoss,
    Last,
    /// 矿洞支线（branch 1, depth 11-14）
    Mining,
    /// 金库支线（branch 1, depth 16-19）
    Vault,
    DeadEnd,
}

/// 主线 branch = 0，支线 branch = 1，其余一律死路。
pub fn level_kind(depth: i32, branch: i32) -> LevelKind {
    match branch {
        0 => match depth {
            1..=4 => LevelKind::Sewer,
            5 => LevelKind::SewerBoss,
            6..=9 => LevelKind::Prison,
            10 => LevelKind::PrisonBoss,
            11..=14 => LevelKind::Caves,
            15 => LevelKind::CavesBoss,
            16..=19 => LevelKind::City,
            20 => LevelKind::CityBoss,
            21..=24 => LevelKind::Halls,
            25 => LevelKind::HallsBoss,
            26 => LevelKind::Last,
            _ => LevelKind::DeadEnd,
        },
        1 => match depth {
            11..=14 => LevelKind::Mining,
            16..=19 => LevelKind::Vault,
            _ => LevelKind::DeadEnd,
        },
        _ => LevelKind::DeadEnd,
    }
}

/// 当前这局游戏的全局进度状态
#[derive(Debug, Default, Resource)]
pub struct Dungeon {
    pub challenges: i32,
    pub mobs_to_champion: f32,
    pub depth: i32,
    pub branch: i32,
    pub limited_drops: LimitedDrops,
}

impl Dungeon {
    pub fn init(&mut self, settings: &Settings) {
        self.challenges = settings.challenges;
        self.depth = 1;
        self.branch = 0;
        self.limited_drops.reset();
    }

    /// 当前深度应生成的关卡类型
    pub fn level_kind(&self) -> LevelKind {
        level_kind(self.depth, self.branch)
    }
}

/// 有生成上限的物品/房间类型，记录已生成数量。
/// 单独数字也行，但枚举可迭代，便于打包初始化（SPD 原注释同义）。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, IntoPrimitive, EnumIter, AsRefStr)]
#[repr(u8)]
pub enum LimitedDropType {
    // 世界限量掉落
    StrengthPotions,
    UpgradeScrolls,
    ArcaneStyli,
    EnchStone,
    TrinketCata,
    IntStone,
    /// 实际是个房间，但逻辑相同
    LabRoom,

    // 治疗药水来源：敌人
    SwarmHp,
    NecroHp,
    BatHp,
    WarlockHp,
    // 恶魔孵化器的刷率本身受限，无需限制其治疗掉落
    // 治疗药水来源：炼金
    CookingHp,
    BlandfruitSeed,

    // 其他限量敌人掉落
    SlimeWep,
    SkeleWep,
    TheifMisc,
    GuardArm,
    ShamanWand,
    Dm200Equip,
    GolemEquip,

    // 容器
    VelvetPouch,
    ScrollHolder,
    PotionBandolier,
    MagicalHolster,

    // 传说文书
    LoreSewers,
    LorePrison,
    LoreCaves,
    LoreCity,
    LoreHalls,
}

#[derive(Debug, Clone)]
pub struct LimitedDrops {
    counts: HashMap<LimitedDropType, u32>,
}

impl Default for LimitedDrops {
    fn default() -> Self {
        Self {
            counts: LimitedDropType::iter().map(|t| (t, 0)).collect(),
        }
    }
}

impl LimitedDrops {
    pub fn count(&self, drop_type: LimitedDropType) -> u32 {
        self.counts.get(&drop_type).copied().unwrap_or(0)
    }

    pub fn dropped(&self, drop_type: LimitedDropType) -> bool {
        self.count(drop_type) != 0
    }

    pub fn set_count(&mut self, drop_type: LimitedDropType, count: u32) {
        self.counts.insert(drop_type, count);
    }

    pub fn record_drop(&mut self, drop_type: LimitedDropType) {
        self.set_count(drop_type, 1);
    }

    pub fn reset(&mut self) {
        self.counts.values_mut().for_each(|count| *count = 0);
    }
}

pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dungeon>().add_systems(Startup, setup);
    }
}

fn setup(mut dungeon: ResMut<Dungeon>, settings: Res<Settings>) {
    dungeon.init(&settings);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 对照 `Dungeon.newLevel()` 的 switch 表
    #[test]
    fn level_kind_matches_spd_table() {
        assert_eq!(level_kind(1, 0), LevelKind::Sewer);
        assert_eq!(level_kind(4, 0), LevelKind::Sewer);
        assert_eq!(level_kind(5, 0), LevelKind::SewerBoss);
        assert_eq!(level_kind(10, 0), LevelKind::PrisonBoss);
        assert_eq!(level_kind(14, 0), LevelKind::Caves);
        assert_eq!(level_kind(20, 0), LevelKind::CityBoss);
        assert_eq!(level_kind(25, 0), LevelKind::HallsBoss);
        assert_eq!(level_kind(26, 0), LevelKind::Last);
        assert_eq!(level_kind(27, 0), LevelKind::DeadEnd);
        assert_eq!(level_kind(0, 0), LevelKind::DeadEnd);
        // 支线
        assert_eq!(level_kind(11, 1), LevelKind::Mining);
        assert_eq!(level_kind(19, 1), LevelKind::Vault);
        assert_eq!(level_kind(15, 1), LevelKind::DeadEnd);
        assert_eq!(level_kind(11, 2), LevelKind::DeadEnd);
    }

    #[test]
    fn limited_drops_lifecycle() {
        let mut drops = LimitedDrops::default();
        assert!(!drops.dropped(LimitedDropType::UpgradeScrolls));
        drops.record_drop(LimitedDropType::UpgradeScrolls);
        assert!(drops.dropped(LimitedDropType::UpgradeScrolls));
        drops.reset();
        assert_eq!(drops.count(LimitedDropType::UpgradeScrolls), 0);
    }
}
