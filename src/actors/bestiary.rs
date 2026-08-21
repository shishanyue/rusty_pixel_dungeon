//! 初始数值表：Hero 四经典职业 + 下水道三怪（Rat/Snake/Crab），逐项照抄 Java
//! 并注明来源行号（表驱动原则见 `docs/plans/01`）。
//!
//! SPD 现版本另有 DUELIST/CLERIC 两职业（`HeroClass.java` L91-92），裸数值与
//! 四经典职业完全相同——`initHero`（L100-154）及各职业 init（L174-261）只发
//! 初始装备与天赋，不改 HP/STR/攻防技能；待 M4 物品域接入后再开放枚举项。

use super::char_stats::{CharStats, StatRange};

/// 英雄等级上限（`Hero.java` L199 `MAX_LEVEL`）。
pub const HERO_MAX_LEVEL: i32 = 30;

/// 英雄初始力量（`Hero.java` L201 `STARTING_STR`；L247 构造时赋给 `STR`）。
pub const HERO_STARTING_STR: i32 = 10;

/// 从 `lvl` 级升下一级所需经验（`Hero.java` L2067-2073 `maxExp`）。
/// 升级循环 `while (exp >= maxExp())`（L2015-2024）：扣除后 `lvl++`；
/// 满级后每满一管改发 Bless 并清零经验（L2035-2037，Buff 属 M4）。
#[must_use]
pub const fn hero_max_exp(lvl: i32) -> i32 {
    5 + lvl * 5
}

/// `lvl` 级生命上限（`Hero.java` L254-269 `updateHT` 的 L257：
/// `HT = 20 + 5*(lvl-1) + HTBoost`）。`HTBoost` 与力量之戒乘数在裸基线下为
/// 0/1（物品域 M4）；升级时 `updateHT(true)` 把增量补进当前 HP（L265-267）。
#[must_use]
pub const fn hero_max_ht(lvl: i32) -> i32 {
    20 + 5 * (lvl - 1)
}

/// `lvl` 级命中技能：初值 10（`Hero.java` L213），每级 +1（L2032
/// `attackSkill++`）。Java 折算入口 `Hero.attackSkill(Char)`（L504-559）在
/// 裸手基线下为 `max(1, round(attackSkill × 1))`（L557）即本值；武器精度、
/// 天赋乘子属 M4。
#[must_use]
pub const fn hero_attack_skill(lvl: i32) -> i32 {
    10 + (lvl - 1)
}

/// `lvl` 级闪避技能：初值 5（`Hero.java` L214），每级 +1（L2033
/// `defenseSkill++`）。Java 折算入口 `Hero.defenseSkill(Char)`（L561-605）在
/// 裸身无麻痹基线下为 `max(1, round(evasion))`（L604）即本值；护甲闪避系数、
/// 麻痹减半（L591-593）属 M4。
#[must_use]
pub const fn hero_defense_skill(lvl: i32) -> i32 {
    5 + (lvl - 1)
}

/// 徒手伤害域（`RingOfForce.java` L105：无戒指时
/// `heroDamageIntRange(1, max(STR - 8, 1))`，掷法见
/// [`combat::damage_roll`](super::combat::damage_roll)）。
#[must_use]
pub const fn hero_unarmed_damage(str: i32) -> StatRange {
    let max = str - 8;
    StatRange::new(1, if max > 1 { max } else { 1 })
}

/// 英雄职业：四经典职业（`HeroClass.java` L87-90 枚举常量）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeroClass {
    /// 战士（L87）。
    Warrior,
    /// 法师（L88）。
    Mage,
    /// 盗贼（L89）。
    Rogue,
    /// 女猎手（L90）。
    Huntress,
}

impl HeroClass {
    /// 出生战斗数值（1 级裸身基线）：
    /// - `HP = HT = 20`、`STR = 10`：`Hero.java` 构造器 L243-252；
    /// - 命中 10 / 闪避 5：L213-214（经 [`hero_attack_skill`] /
    ///   [`hero_defense_skill`] 的 1 级值）；
    /// - 伤害域 (1, 2)：徒手 STR 10（`RingOfForce.java` L105）；
    /// - 护甲域 (0, 0)：无甲无武器（`Hero.drRoll` L637-660 仅剩 `super` 的
    ///   Barkskin 项，无 Buff 时为 0）；
    /// - 速度 1（`Char.java` L174）、出手耗时 1（`Hero.attackDelay`
    ///   L776-806 徒手且无疾风之戒）。
    ///
    /// 四职业裸数值相同，职业差异全在初始装备与天赋
    /// （`HeroClass.java` L100-154、L174-261），属 M4 物品域。
    #[must_use]
    pub const fn starting_stats(self) -> CharStats {
        CharStats {
            hp: 20,
            ht: 20,
            attack_skill: hero_attack_skill(1),
            defense_skill: hero_defense_skill(1),
            damage_range: hero_unarmed_damage(HERO_STARTING_STR),
            armor_range: StatRange::new(0, 0),
            base_speed: 1.0,
            attack_delay: 1.0,
        }
    }
}

/// 下水道三怪（M1 纯核范围；图鉴随关卡推进逐层补充）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobKind {
    /// 老鼠（`Rat.java`）。
    Rat,
    /// 蛇（`Snake.java`）。
    Snake,
    /// 螃蟹（`Crab.java`）。
    Crab,
}

/// 怪物数值表条目：战斗数值 + 经验字段（`Mob.java` L126-127）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MobStats {
    /// 战斗数值组件初值。
    pub char_stats: CharStats,
    /// 击杀经验（`EXP`，`Mob.java` L126 默认 1）。
    pub exp: i32,
    /// 经验上限等级（`maxLvl`，`Mob.java` L127 默认 `MAX_LEVEL - 1`）：
    /// 英雄超过该等级后击杀不再给经验。
    pub max_lvl: i32,
}

impl MobKind {
    /// 数值表（`defense_skill` 为 `Mob.java` L124 字段值，即
    /// `Mob.defenseSkill(Char)` L698-705 在未被偷袭、未麻痹基线下的返回值；
    /// 偷袭/麻痹归 0 属 M4 AI 域。`attack_delay` = 1：`Mob.attackDelay`
    /// L655-659，Adrenaline 属 M4）。
    #[must_use]
    pub const fn stats(self) -> MobStats {
        match self {
            // Rat.java：HP/HT = 8（L36）、闪避 2（L37）、经验上限等级 5（L39）、
            // 伤害 1-4（L54-57）、命中 8（L59-62）、护甲 0-1（L64-67）；
            // EXP 未覆写 → 默认 1（Mob.java L126）；速度默认 1（Char.java L174）
            Self::Rat => MobStats {
                char_stats: CharStats {
                    hp: 8,
                    ht: 8,
                    attack_skill: 8,
                    defense_skill: 2,
                    damage_range: StatRange::new(1, 4),
                    armor_range: StatRange::new(0, 1),
                    base_speed: 1.0,
                    attack_delay: 1.0,
                },
                exp: 1,
                max_lvl: 5,
            },
            // Snake.java：HP/HT = 4（L38）、闪避 25（L39，下水道的高闪避教学怪）、
            // EXP 2（L41）、经验上限等级 7（L42）、伤害 1-4（L48-51）、
            // 命中 10（L53-56）；无 drRoll 覆写 → 护甲 0-0（Char.java L706-712）
            Self::Snake => MobStats {
                char_stats: CharStats {
                    hp: 4,
                    ht: 4,
                    attack_skill: 10,
                    defense_skill: 25,
                    damage_range: StatRange::new(1, 4),
                    armor_range: StatRange::new(0, 0),
                    base_speed: 1.0,
                    attack_delay: 1.0,
                },
                exp: 2,
                max_lvl: 7,
            },
            // Crab.java：HP/HT = 15（L34）、闪避 5（L35）、速度 2（L36）、
            // EXP 4（L38）、经验上限等级 9（L39）、伤害 1-7（L45-48）、
            // 命中 12（L50-53）、护甲 0-4（L55-58）
            Self::Crab => MobStats {
                char_stats: CharStats {
                    hp: 15,
                    ht: 15,
                    attack_skill: 12,
                    defense_skill: 5,
                    damage_range: StatRange::new(1, 7),
                    armor_range: StatRange::new(0, 4),
                    base_speed: 2.0,
                    attack_delay: 1.0,
                },
                exp: 4,
                max_lvl: 9,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rat 逐项对拍（`Rat.java` L36/L37/L39/L54-57/L59-62/L64-67；
    /// EXP 默认值 `Mob.java` L126；速度默认 `Char.java` L174）。
    #[test]
    fn rat_table_matches_java() {
        let rat = MobKind::Rat.stats();
        assert_eq!(rat.char_stats.hp, 8, "Rat.java L36");
        assert_eq!(rat.char_stats.ht, 8, "Rat.java L36");
        assert_eq!(rat.char_stats.defense_skill, 2, "Rat.java L37");
        assert_eq!(rat.char_stats.attack_skill, 8, "Rat.java L61");
        assert_eq!(
            rat.char_stats.damage_range,
            StatRange::new(1, 4),
            "Rat.java L56"
        );
        assert_eq!(
            rat.char_stats.armor_range,
            StatRange::new(0, 1),
            "Rat.java L66"
        );
        assert_eq!(rat.char_stats.base_speed, 1.0, "Char.java L174 默认");
        assert_eq!(rat.char_stats.attack_delay, 1.0, "Mob.java L655-659");
        assert_eq!(rat.exp, 1, "Mob.java L126 默认");
        assert_eq!(rat.max_lvl, 5, "Rat.java L39");
    }

    /// Snake 逐项对拍（`Snake.java` L38/L39/L41/L42/L48-51/L53-56；
    /// 高闪避 25 是其身份数值，务必与 Rat 的 2 区分）。
    #[test]
    fn snake_table_matches_java() {
        let snake = MobKind::Snake.stats();
        assert_eq!(snake.char_stats.hp, 4, "Snake.java L38");
        assert_eq!(snake.char_stats.ht, 4, "Snake.java L38");
        assert_eq!(snake.char_stats.defense_skill, 25, "Snake.java L39：高闪避");
        assert_eq!(snake.char_stats.attack_skill, 10, "Snake.java L55");
        assert_eq!(
            snake.char_stats.damage_range,
            StatRange::new(1, 4),
            "Snake.java L50"
        );
        assert_eq!(
            snake.char_stats.armor_range,
            StatRange::new(0, 0),
            "无 drRoll 覆写"
        );
        assert_eq!(snake.char_stats.base_speed, 1.0, "Char.java L174 默认");
        assert_eq!(snake.exp, 2, "Snake.java L41");
        assert_eq!(snake.max_lvl, 7, "Snake.java L42");
    }

    /// Crab 逐项对拍（`Crab.java` L34/L35/L36/L38/L39/L45-48/L50-53/L55-58）。
    #[test]
    fn crab_table_matches_java() {
        let crab = MobKind::Crab.stats();
        assert_eq!(crab.char_stats.hp, 15, "Crab.java L34");
        assert_eq!(crab.char_stats.ht, 15, "Crab.java L34");
        assert_eq!(crab.char_stats.defense_skill, 5, "Crab.java L35");
        assert_eq!(crab.char_stats.base_speed, 2.0, "Crab.java L36：双速");
        assert_eq!(crab.char_stats.attack_skill, 12, "Crab.java L52");
        assert_eq!(
            crab.char_stats.damage_range,
            StatRange::new(1, 7),
            "Crab.java L47"
        );
        assert_eq!(
            crab.char_stats.armor_range,
            StatRange::new(0, 4),
            "Crab.java L57"
        );
        assert_eq!(crab.exp, 4, "Crab.java L38");
        assert_eq!(crab.max_lvl, 9, "Crab.java L39");
    }

    /// 四职业出生数值逐项对拍（`Hero.java` L243-252 构造器 + L213-214），
    /// 且四职业裸数值一致（`HeroClass.java` init 只发装备）。
    #[test]
    fn hero_starting_stats_match_java() {
        let warrior = HeroClass::Warrior.starting_stats();
        assert_eq!(warrior.hp, 20, "Hero.java L246");
        assert_eq!(warrior.ht, 20, "Hero.java L246");
        assert_eq!(warrior.attack_skill, 10, "Hero.java L213");
        assert_eq!(warrior.defense_skill, 5, "Hero.java L214");
        assert_eq!(
            warrior.damage_range,
            StatRange::new(1, 2),
            "RingOfForce.java L105，STR 10"
        );
        assert_eq!(warrior.armor_range, StatRange::new(0, 0), "裸身无甲");
        assert_eq!(warrior.base_speed, 1.0, "Char.java L174");
        assert_eq!(warrior.attack_delay, 1.0, "Hero.java L776-806 徒手");

        for class in [HeroClass::Mage, HeroClass::Rogue, HeroClass::Huntress] {
            assert_eq!(
                class.starting_stats(),
                warrior,
                "{class:?} 裸数值应与战士一致"
            );
        }
        assert_eq!(HERO_STARTING_STR, 10, "Hero.java L201");
        assert_eq!(HERO_MAX_LEVEL, 30, "Hero.java L199");
    }

    /// 升级曲线对拍（`Hero.java` L213-214 初值、L2032-2033 每级 +1、
    /// L257 生命上限、L2071-2073 经验需求）。
    #[test]
    fn hero_growth_curves_match_java() {
        // 1 级基线
        assert_eq!(hero_attack_skill(1), 10);
        assert_eq!(hero_defense_skill(1), 5);
        assert_eq!(hero_max_ht(1), 20);
        assert_eq!(hero_max_exp(1), 10);
        // 每级增量：命中/闪避 +1、HT +5、升级经验 +5
        for lvl in 1..HERO_MAX_LEVEL {
            assert_eq!(
                hero_attack_skill(lvl + 1),
                hero_attack_skill(lvl) + 1,
                "L2032"
            );
            assert_eq!(
                hero_defense_skill(lvl + 1),
                hero_defense_skill(lvl) + 1,
                "L2033"
            );
            assert_eq!(hero_max_ht(lvl + 1), hero_max_ht(lvl) + 5, "L257");
            assert_eq!(hero_max_exp(lvl + 1), hero_max_exp(lvl) + 5, "L2071-2073");
        }
        // 满级抽查（30 级：命中 39、闪避 34、HT 165；升 11 级需 55 经验）
        assert_eq!(hero_attack_skill(HERO_MAX_LEVEL), 39);
        assert_eq!(hero_defense_skill(HERO_MAX_LEVEL), 34);
        assert_eq!(hero_max_ht(HERO_MAX_LEVEL), 165);
        assert_eq!(hero_max_exp(10), 55);
    }

    /// 徒手伤害域随 STR 变化（`RingOfForce.java` L105 `max(STR - 8, 1)`）：
    /// STR ≤ 9 时上界钳到 1，STR 10（出生值）为 (1, 2)。
    #[test]
    fn unarmed_damage_clamps_at_low_str() {
        assert_eq!(hero_unarmed_damage(8), StatRange::new(1, 1));
        assert_eq!(hero_unarmed_damage(9), StatRange::new(1, 1));
        assert_eq!(hero_unarmed_damage(10), StatRange::new(1, 2));
        assert_eq!(hero_unarmed_damage(19), StatRange::new(1, 11));
    }
}
