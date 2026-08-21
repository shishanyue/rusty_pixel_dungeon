//! 角色战斗数值组件：SPD `Char.java` 数值字段的组件化
//! （`docs/plans/15-char-combat-core.md`，下文行号未注明文件者均指 `Char.java`）。
//!
//! 纯逻辑域（M4 前置）：只定义数据与纯方法，不含系统/AI/输入/渲染。
//! 战斗公式见同域 [`combat`](super::combat)，初始数值表见
//! [`bestiary`](super::bestiary)。
//!
//! Java 的 `attackSkill(Char)`/`defenseSkill(Char)`/`speed()` 是动态方法，
//! 受装备、Buff、偷袭状态影响（如 `Mob.defenseSkill` L698-705 被偷袭/麻痹时
//! 归 0，`Hero.defenseSkill` L604 的 `max(1, …)` 下限）；纯核只承载**已折算**
//! 的基础值，M4 的装备/Buff 域在其上做乘加修正。

use bevy::prelude::*;

/// 整数闭区间 `[min, max]`。SPD 伤害域与护甲域的通用形态：`damageRoll`/`drRoll`
/// 皆以 `Random.NormalIntRange(min, max)` 在闭区间上取三角分布值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatRange {
    /// 下界（含）。
    pub min: i32,
    /// 上界（含）。调用方保证 `min <= max`（与 Java 相同不设防）。
    pub max: i32,
}

impl StatRange {
    /// 构造闭区间。
    #[must_use]
    pub const fn new(min: i32, max: i32) -> Self {
        Self { min, max }
    }
}

/// 角色战斗数值（`Char.java` 实例字段 + 子类覆写的静态数值部分）。
///
/// 字段来源：`HP`/`HT`（L171-172）、`baseSpeed`（L174，默认 1）；
/// `attackSkill`/`defenseSkill` 默认 0（L694-700）、伤害与护甲域为子类覆写值，
/// 具体条目见 [`bestiary`](super::bestiary) 数值表。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CharStats {
    /// 当前生命（L172 `HP`）。
    pub hp: i32,
    /// 生命上限（L171 `HT`）。
    pub ht: i32,
    /// 命中技能（`attackSkill()`，L694-696 默认 0）。
    pub attack_skill: i32,
    /// 闪避技能（`defenseSkill()`，L698-700 默认 0）。
    pub defense_skill: i32,
    /// 伤害域：`damageRoll()` 的 `NormalIntRange` 闭区间参数。
    pub damage_range: StatRange,
    /// 护甲域：`drRoll()` 的 `NormalIntRange` 闭区间参数。
    pub armor_range: StatRange,
    /// 基础速度（L174）。`speed()`（L775-788）的 Cripple/Haste 等 Buff 乘子
    /// 与护甲铭文乘子皆属 M4，纯核阶段速度即本值。
    pub base_speed: f32,
    /// 攻击耗时（`attackDelay`：`Mob.java` L655-659、`Hero.java` L776-806，
    /// 基础皆 1；Adrenaline、武器 `delayFactor` 等修正属 M4）。
    pub attack_delay: f32,
}

impl CharStats {
    /// 存活判定（`isAlive` L1096-1098：`HP > 0`；`deathMarked` 属 M4 Buff 域）。
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        self.hp > 0
    }

    /// 承伤的纯核骨架（`damage()` L812-1034 中与 Buff/护盾/抗性无关的部分）：
    /// 已死或负伤害直接忽略（L814），`HP` 扣减（L951）后钳到 0（L1027）。
    /// 攻击流程的护甲减免（`attack()` L493 `max(damage - dr, 0)`）发生在调用
    /// 本方法**之前**，属 M4 攻击管线。
    pub const fn take_damage(&mut self, dmg: i32) {
        if !self.is_alive() || dmg < 0 {
            return;
        }
        self.hp -= dmg;
        if self.hp < 0 {
            self.hp = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(hp: i32) -> CharStats {
        CharStats {
            hp,
            ht: 8,
            attack_skill: 0,
            defense_skill: 0,
            damage_range: StatRange::new(1, 1),
            armor_range: StatRange::new(0, 0),
            base_speed: 1.0,
            attack_delay: 1.0,
        }
    }

    /// 对拍 `Char.java`：`isAlive` L1096-1098（`HP > 0`）；`damage()` L814
    /// （负伤害忽略）、L951（扣减）、L1027（钳 0）。
    #[test]
    fn take_damage_clamps_like_java() {
        let mut c = stats(8);
        c.take_damage(3);
        assert_eq!(c.hp, 5);
        assert!(c.is_alive());

        // 负伤害不生效（L814 `dmg < 0 → return`）
        c.take_damage(-100);
        assert_eq!(c.hp, 5);

        // 超杀钳到 0（L1027），随即判死（L1096-1098 严格 `> 0`）
        c.take_damage(999);
        assert_eq!(c.hp, 0);
        assert!(!c.is_alive());

        // 已死不再结算（L814 `!isAlive() → return`）
        c.take_damage(1);
        assert_eq!(c.hp, 0);
    }

    /// 恰好打到 0 即死：SPD 无"濒死 1 HP"保护（`deathMarked` 例外属 M4）。
    #[test]
    fn exactly_zero_hp_is_dead() {
        let mut c = stats(4);
        c.take_damage(4);
        assert_eq!(c.hp, 0);
        assert!(!c.is_alive());
    }
}
