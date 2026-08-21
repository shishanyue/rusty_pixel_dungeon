//! 近战攻击结算纯核：把 [`combat`](super::combat) 的三个掷值函数按 SPD
//! `Char.attack()` 的顺序串起来（`Char.java` L364-L615，下文行号未注明文件者
//! 均指该文件）。英雄撞击攻击（`hero.rs`）与怪物邻格攻击（`ai.rs`）共用。
//!
//! 保真部分（基线值路径）：
//!
//! ```text
//! hit(attacker, defender, accMulti=1)      // L384，未中即返回（L590-L613）
//! dr  = drRoll()                           // L386（先掷护甲）
//! dmg = damageRoll()                       // L411（后掷伤害；Preparation 属 M4）
//! effective = max(dmg - dr, 0)             // L493
//! ```
//!
//! 掷值顺序钉死为 命中(2 掷) → 护甲(2 掷) → 伤害(2 掷)，未命中只消耗命中掷。
//!
//! 与 Java 的差异（皆为 M4 后续域，基线下值恒等）：
//! - `defenseProc`/`attackProc`（L490/L505）与 Berserk/Fury/Endure/Vulnerable
//!   等全部 Buff 乘子（L414-L503）未移植——裸基线下均为恒等变换；
//! - `dmgMulti`/`dmgBonus` 便捷参数（L368）恒为 1/0，未保留；
//! - 无敌判定（L374）与死亡插曲（L513-L586）由调用方处理。

use rand::Rng;

use super::char_stats::CharStats;
use super::combat;

/// 一次近战攻击的结算结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeleeOutcome {
    /// 未命中（`hit` L384 为假的分支，L590-L613）。
    Miss,
    /// 命中；`taken` 为护甲减免后的实伤（可为 0，即"全被甲弹开"）。
    Hit {
        /// 伤害掷值（`damageRoll()`，L411）。
        rolled: i32,
        /// 护甲减免掷值（`drRoll()`，L386）。
        blocked: i32,
        /// 实际承受 = `max(rolled - blocked, 0)`（L493）。
        taken: i32,
    },
}

/// 攻击结算（`Char.attack` 剥除 Buff/物品乘子的纯核）。只算不扣血：
/// 调用方拿 [`MeleeOutcome::Hit::taken`] 去 `CharStats::take_damage`，
/// 并处理死亡钩子（怪物 despawn + EXP / 英雄回 Title）。
#[must_use]
pub fn resolve_melee(
    attacker: &CharStats,
    defender: &CharStats,
    rng: &mut impl Rng,
) -> MeleeOutcome {
    if !combat::hit(attacker, defender, 1.0, rng) {
        return MeleeOutcome::Miss; // L590-L613：未中，无后续掷值
    }
    let blocked = combat::dr_roll(defender, rng); // L386：护甲先掷
    let rolled = combat::damage_roll(attacker, rng); // L411：伤害后掷
    MeleeOutcome::Hit {
        rolled,
        blocked,
        taken: (rolled - blocked).max(0), // L493
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::ChaCha12Rng;
    use rand::{RngExt, SeedableRng};

    use super::*;
    use crate::actors::bestiary::{HeroClass, MobKind};

    fn rng(seed: u64) -> ChaCha12Rng {
        ChaCha12Rng::seed_from_u64(seed)
    }

    /// 掷值顺序钉死（L384 → L386 → L411）：同一种子下，手工按
    /// hit → dr → damage 顺序复算应与 `resolve_melee` 逐位一致。
    #[test]
    fn roll_order_is_hit_then_dr_then_damage() {
        let hero = HeroClass::Warrior.starting_stats();
        let rat = MobKind::Rat.stats().char_stats;
        for seed in 0..32_u64 {
            let outcome = resolve_melee(&hero, &rat, &mut rng(seed));

            let mut manual = rng(seed);
            let expected = if combat::hit(&hero, &rat, 1.0, &mut manual) {
                let blocked = combat::dr_roll(&rat, &mut manual);
                let rolled = combat::damage_roll(&hero, &mut manual);
                MeleeOutcome::Hit {
                    rolled,
                    blocked,
                    taken: (rolled - blocked).max(0),
                }
            } else {
                MeleeOutcome::Miss
            };
            assert_eq!(outcome, expected, "种子 {seed}");
        }
    }

    /// 未命中不消耗护甲/伤害掷值（L590 分支直接返回）：无限闪避（零掷值判负，
    /// `combat` L643-L645）后随机流原封不动。
    #[test]
    fn miss_consumes_no_damage_rolls() {
        let hero = HeroClass::Warrior.starting_stats();
        let mut dodger = MobKind::Snake.stats().char_stats;
        dodger.defense_skill = 1_000_000; // INFINITE_EVASION：必闪且零掷值

        let mut used = rng(7);
        assert_eq!(resolve_melee(&hero, &dodger, &mut used), MeleeOutcome::Miss);
        let mut untouched = rng(7);
        assert_eq!(
            used.random::<u32>(),
            untouched.random::<u32>(),
            "Miss 路径不应动用随机流"
        );
    }

    /// 值域性质（多种子）：徒手英雄（伤害 1-2）打 Rat（护甲 0-1），实伤恒在
    /// `[0, 2]`；命中时 rolled ∈ [1,2]、blocked ∈ [0,1]；同种子重放逐位一致。
    #[test]
    fn damage_stays_in_expected_domain() {
        let hero = HeroClass::Warrior.starting_stats();
        let rat = MobKind::Rat.stats().char_stats;
        for seed in 0..16_u64 {
            let mut a = rng(seed);
            let mut b = rng(seed);
            for _ in 0..64 {
                let outcome = resolve_melee(&hero, &rat, &mut a);
                assert_eq!(outcome, resolve_melee(&hero, &rat, &mut b), "确定性");
                if let MeleeOutcome::Hit {
                    rolled,
                    blocked,
                    taken,
                } = outcome
                {
                    assert!((1..=2).contains(&rolled), "徒手伤害域 1-2：{rolled}");
                    assert!((0..=1).contains(&blocked), "Rat 护甲域 0-1：{blocked}");
                    assert!((0..=2).contains(&taken), "实伤域 0-2：{taken}");
                    assert_eq!(taken, (rolled - blocked).max(0));
                }
            }
        }
    }
}
