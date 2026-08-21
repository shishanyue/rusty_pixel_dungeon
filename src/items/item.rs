//! `Item` 实例结构：对照 `Item.java` 的核心字段
//! （quantity L81 / level L84 / levelKnown L86 / cursed L88 / cursedKnown L89）
//! 与堆叠语义（`merge` L197-L204、`split` L286-L306）。
//!
//! 纯数据结构，不进 ECS；Java 的继承树行为（各子类覆写）折进
//! [`ItemKind`](super::kinds::ItemKind) 的数据表。

use super::kinds::ItemKind;

/// 一个物品实例（一摞可堆叠物或一件装备）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Item {
    /// 种类（Java 的具体 class）。
    pub kind: ItemKind,
    /// 数量（`Item.java` L81，默认 1；金币的数量即金额）。
    pub quantity: i32,
    /// 升级等级（L84，默认 0；诅咒转移等临时修正属效果域 TODO）。
    pub level: i32,
    /// 等级是否已鉴定（L86 `levelKnown`）。
    pub level_known: bool,
    /// 是否被诅咒（L88）。
    pub cursed: bool,
    /// 诅咒是否已鉴定（L89 `cursedKnown`）。
    pub cursed_known: bool,
}

impl Item {
    /// 新实例：数量取种类默认值（投掷武器 3/飞镖 2，其余 1），
    /// 等级 0、未鉴定、未诅咒（`Item.java` L81-L89 字段初值）。
    #[must_use]
    pub fn new(kind: ItemKind) -> Self {
        Self {
            kind,
            quantity: kind.default_quantity(),
            level: 0,
            level_known: false,
            cursed: false,
            cursed_known: false,
        }
    }

    /// 指定数量的新实例（`Gold(int value)` 构造、掉落堆常用）。
    #[must_use]
    pub fn with_quantity(kind: ItemKind, quantity: i32) -> Self {
        Self {
            quantity,
            ..Self::new(kind)
        }
    }

    /// 可堆叠位代理（见 [`ItemKind::stackable`]）。
    #[must_use]
    pub fn stackable(&self) -> bool {
        self.kind.stackable()
    }

    /// 堆叠相似判定：种类与等级相同（`Item.java` L367-L369 按 class 判等；
    /// `MissileWeapon.java` L185-L187 另比较 `trueLevel`——对其余可堆叠类
    /// level 恒 0，条件不改变语义。投掷武器的 `setID`/耐久共享属二期 TODO）。
    #[must_use]
    pub fn is_similar(&self, other: &Self) -> bool {
        self.kind == other.kind && self.level == other.level
    }

    /// 并摞（`Item.java` L197-L204 `merge`）：相似则数量并入 `self`，
    /// `other` 清零（Java 原样保留空壳，由调用方丢弃）。
    pub fn merge(&mut self, other: &mut Self) {
        if self.is_similar(other) {
            self.quantity += other.quantity;
            other.quantity = 0;
        }
    }

    /// 拆摞（`Item.java` L286-L306 `split`）：`amount` 不在 `(0, quantity)`
    /// 开区间内时返回 `None`；否则分出一摞 `amount` 个（其余字段复制），
    /// 自身数量扣减。
    #[must_use]
    pub fn split(&mut self, amount: i32) -> Option<Self> {
        if amount <= 0 || amount >= self.quantity {
            None
        } else {
            self.quantity -= amount;
            Some(Self {
                quantity: amount,
                ..self.clone()
            })
        }
    }

    /// 升 1 级（`Item.java` L401-L408）。
    pub fn upgrade(&mut self) {
        self.level += 1;
    }

    /// 降 1 级（L418-L423；可降为负，代表劣化）。
    pub fn degrade(&mut self) {
        self.level -= 1;
    }

    /// 完全鉴定（L457-L473 `identify`：两位同置 true）。
    pub fn identify(&mut self) {
        self.level_known = true;
        self.cursed_known = true;
    }

    /// 是否完全鉴定（L449-L451：`levelKnown && cursedKnown`）。
    /// 药水/卷轴等消耗品的"种类是否已知"由
    /// [`ItemStatusHandler`](super::identification::ItemStatusHandler) 查询
    /// （`Potion.java` L392-L395 覆写语义），本位只表达单件实例的鉴定态。
    #[must_use]
    pub fn is_identified(&self) -> bool {
        self.level_known && self.cursed_known
    }

    /// 可见升级数（L433-L435：未鉴定显示 0）。
    #[must_use]
    pub fn visibly_upgraded(&self) -> i32 {
        if self.level_known { self.level } else { 0 }
    }

    /// 诅咒是否可见（L441-L443：`cursed && cursedKnown`）。
    #[must_use]
    pub fn visibly_cursed(&self) -> bool {
        self.cursed && self.cursed_known
    }

    /// 估价：基准价 × 数量（金币即数量本身）。诅咒减半/等级翻倍/附魔加成等
    /// 修正（`MeleeWeapon.java` L395-L407、`Armor.java` L706-L723 等）属
    /// 效果域 TODO。
    #[must_use]
    pub fn value(&self) -> i32 {
        match self.kind {
            ItemKind::Gold => self.quantity,
            kind => kind.base_value() * self.quantity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::kinds::{MissileWeaponKind, PotionKind, ScrollKind};

    #[test]
    fn new_item_has_java_defaults() {
        let item = Item::new(ItemKind::Potion(PotionKind::Healing));
        assert_eq!(item.quantity, 1, "Item.java L81");
        assert_eq!(item.level, 0, "Item.java L84");
        assert!(
            !item.level_known && !item.cursed && !item.cursed_known,
            "L86-L89"
        );
        // 投掷武器构造时数量取 defaultQuantity（MissileWeapon.java L67）
        assert_eq!(
            Item::new(ItemKind::Missile(MissileWeaponKind::Kunai)).quantity,
            3
        );
        assert_eq!(
            Item::new(ItemKind::Missile(MissileWeaponKind::Dart)).quantity,
            2
        );
    }

    /// merge 语义（`Item.java` L197-L204）：相似并摞、来源清零；不相似不动。
    #[test]
    fn merge_stacks_similar_and_zeroes_source() {
        let mut a = Item::with_quantity(ItemKind::Potion(PotionKind::Healing), 2);
        let mut b = Item::with_quantity(ItemKind::Potion(PotionKind::Healing), 3);
        a.merge(&mut b);
        assert_eq!(a.quantity, 5);
        assert_eq!(b.quantity, 0);

        let mut c = Item::with_quantity(ItemKind::Potion(PotionKind::Frost), 4);
        a.merge(&mut c);
        assert_eq!(a.quantity, 5, "不同种类不得并摞");
        assert_eq!(c.quantity, 4);
    }

    /// 投掷武器等级不同不算相似（`MissileWeapon.java` L185-L187 `trueLevel` 比较）。
    #[test]
    fn similar_requires_same_level() {
        let a = Item::new(ItemKind::Missile(MissileWeaponKind::Shuriken));
        let mut b = Item::new(ItemKind::Missile(MissileWeaponKind::Shuriken));
        assert!(a.is_similar(&b));
        b.upgrade();
        assert!(!a.is_similar(&b));
    }

    /// split 边界（`Item.java` L286-L306）：`amount <= 0` 或 `>= quantity` 返 `None`。
    #[test]
    fn split_respects_java_bounds() {
        let mut stack = Item::with_quantity(ItemKind::Scroll(ScrollKind::Identify), 5);
        assert_eq!(stack.split(0), None);
        assert_eq!(stack.split(-1), None);
        assert_eq!(stack.split(5), None, "拆全量返回 None（L288）");
        assert_eq!(stack.quantity, 5, "失败的拆分不得动数量");

        let part = stack.split(2).expect("合法拆分");
        assert_eq!(part.quantity, 2);
        assert_eq!(part.kind, stack.kind);
        assert_eq!(stack.quantity, 3);
    }

    /// 升降级与鉴定可见性（`Item.java` L401-L443、L449-L473）。
    #[test]
    fn upgrade_identify_visibility() {
        let mut item = Item::new(ItemKind::Weapon(
            crate::items::kinds::MeleeWeaponKind::Sword,
        ));
        item.upgrade();
        item.upgrade();
        item.cursed = true;
        assert_eq!(item.level, 2);
        assert_eq!(item.visibly_upgraded(), 0, "未鉴定显示 +0（L433-L435）");
        assert!(!item.visibly_cursed(), "诅咒未知不可见（L441-L443)");
        assert!(!item.is_identified());

        item.identify();
        assert!(item.is_identified());
        assert_eq!(item.visibly_upgraded(), 2);
        assert!(item.visibly_cursed());

        item.degrade();
        assert_eq!(item.level, 1, "degrade L418-L423");
    }

    /// 估价：基准价 × 数量；金币即数量（`Gold.java` 语义）。
    #[test]
    fn value_scales_with_quantity() {
        assert_eq!(Item::with_quantity(ItemKind::Gold, 137).value(), 137);
        assert_eq!(
            Item::with_quantity(ItemKind::Potion(PotionKind::Purity), 3).value(),
            90,
            "Potion.java L440-L442：30 × 数量"
        );
    }
}
