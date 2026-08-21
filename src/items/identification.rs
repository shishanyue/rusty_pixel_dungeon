//! 鉴定系统底座：`ItemStatusHandler.java` 的等价物。
//!
//! 未鉴定消耗品的**外观 ↔ 种类**绑定每局洗牌一次（同种子可复现）：
//! 药水 12 色（`Potion.java` L90-L105 `colors` 表）、卷轴 12 符文
//! （`Scroll.java` L73-L88 `runes` 表）。绑定是双射——洗牌逐个从剩余外观中
//! 随机取一个并移除（`ItemStatusHandler.java` L50-L61）。
//!
//! "已鉴定种类"集合（`known`，L40）随 `know` 增长；`Item` 实例位
//! （`levelKnown`/`cursedKnown`）与此互补：消耗品的 `isIdentified` 以本表
//! 为准（`Potion.java` L392-L395 覆写）。外观 → 精灵图编号的映射属渲染域。

use rand::Rng;

use super::kinds::{PotionKind, ScrollKind};
use super::random::int;

/// 药水外观颜色（`Potion.java` L92-L103 `colors` 键序）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PotionColor {
    /// 绯红（L92）。
    Crimson,
    /// 琥珀（L93）。
    Amber,
    /// 鎏金（L94）。
    Golden,
    /// 翡翠（L95）。
    Jade,
    /// 松石绿（L96）。
    Turquoise,
    /// 天青（L97）。
    Azure,
    /// 靛蓝（L98）。
    Indigo,
    /// 品红（L99）。
    Magenta,
    /// 深褐（L100）。
    Bistre,
    /// 炭黑（L101）。
    Charcoal,
    /// 银白（L102）。
    Silver,
    /// 象牙白（L103）。
    Ivory,
}

impl PotionColor {
    /// 与 `colors` 表（L92-L103）同序。
    pub const ALL: [Self; 12] = [
        Self::Crimson,
        Self::Amber,
        Self::Golden,
        Self::Jade,
        Self::Turquoise,
        Self::Azure,
        Self::Indigo,
        Self::Magenta,
        Self::Bistre,
        Self::Charcoal,
        Self::Silver,
        Self::Ivory,
    ];
}

/// 卷轴符文（`Scroll.java` L75-L86 `runes` 键序，取自卢恩字母）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollRune {
    /// KAUNAN（L75）。
    Kaunan,
    /// SOWILO（L76）。
    Sowilo,
    /// LAGUZ（L77）。
    Laguz,
    /// YNGVI（L78）。
    Yngvi,
    /// GYFU（L79）。
    Gyfu,
    /// RAIDO（L80）。
    Raido,
    /// ISAZ（L81）。
    Isaz,
    /// MANNAZ（L82）。
    Mannaz,
    /// NAUDIZ（L83）。
    Naudiz,
    /// BERKANAN（L84）。
    Berkanan,
    /// ODAL（L85）。
    Odal,
    /// TIWAZ（L86）。
    Tiwaz,
}

impl ScrollRune {
    /// 与 `runes` 表（L75-L86）同序。
    pub const ALL: [Self; 12] = [
        Self::Kaunan,
        Self::Sowilo,
        Self::Laguz,
        Self::Yngvi,
        Self::Gyfu,
        Self::Raido,
        Self::Isaz,
        Self::Mannaz,
        Self::Naudiz,
        Self::Berkanan,
        Self::Odal,
        Self::Tiwaz,
    ];
}

/// `ItemStatusHandler<T>` 等价物：`K` = 种类枚举、`A` = 外观枚举。
///
/// 持有外观双射与已鉴定集合；种类表用 `&'static` 常量数组
/// （对应 Java 构造器的 `items` 数组 = `Generator.Category.*.classes`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStatusHandler<K: Copy + Eq + 'static, A: Copy + Eq + 'static> {
    /// 全部种类（与外观数组等长）。
    kinds: &'static [K],
    /// `kinds[i]` 洗牌后分到的外观（`itemLabels`，L38）。
    appearances: Vec<A>,
    /// `kinds[i]` 是否已鉴定（`known`，L40）。
    known: Vec<bool>,
}

impl<K: Copy + Eq + std::fmt::Debug + 'static, A: Copy + Eq + 'static> ItemStatusHandler<K, A> {
    /// 洗牌绑定（`ItemStatusHandler.java` L42-L62 构造器）：按种类序逐个
    /// `Random.Int(剩余外观数)` 取一个外观并从剩余表**保序移除**（L56-L59，
    /// `ArrayList.remove` 语义——移除方式影响洗牌结果，须照抄）。
    ///
    /// # Panics
    ///
    /// 种类数与外观数不等时 panic（Java 侧由 12↔12 的表长保证）。
    #[must_use]
    pub fn shuffled(kinds: &'static [K], appearances: &[A], rng: &mut impl Rng) -> Self {
        assert_eq!(kinds.len(), appearances.len(), "种类与外观必须等量成双射");
        let mut labels_left = appearances.to_vec();
        let mut assigned = Vec::with_capacity(kinds.len());
        for _ in kinds {
            let index = int(rng, labels_left.len() as i32) as usize;
            assigned.push(labels_left.remove(index));
        }
        Self {
            kinds,
            appearances: assigned,
            known: vec![false; kinds.len()],
        }
    }

    fn index_of(&self, kind: K) -> usize {
        self.kinds
            .iter()
            .position(|k| *k == kind)
            .unwrap_or_else(|| panic!("{kind:?} 不在本 handler 的种类表内"))
    }

    /// 种类 → 外观（`label(item)`，L179-L185）。
    #[must_use]
    pub fn appearance_of(&self, kind: K) -> A {
        self.appearances[self.index_of(kind)]
    }

    /// 外观 → 种类（双射反查；Java 无此接口，测试与"按外观提示"场景用）。
    #[must_use]
    pub fn kind_of(&self, appearance: A) -> K {
        let index = self
            .appearances
            .iter()
            .position(|a| *a == appearance)
            .expect("双射保证每个外观都有对应种类");
        self.kinds[index]
    }

    /// 是否已鉴定（`isKnown`，L187-L193）。
    #[must_use]
    pub fn is_known(&self, kind: K) -> bool {
        self.known[self.index_of(kind)]
    }

    /// 标记已鉴定（`know`，L195-L201）。
    pub fn know(&mut self, kind: K) {
        let index = self.index_of(kind);
        self.known[index] = true;
    }

    /// 已鉴定种类集合（`known()`，L203-L205；按种类表序）。
    pub fn known(&self) -> impl Iterator<Item = K> + '_ {
        self.kinds
            .iter()
            .enumerate()
            .filter(|(i, _)| self.known[*i])
            .map(|(_, k)| *k)
    }

    /// 未鉴定种类集合（`unknown()`，L207-L215；按种类表序）。
    pub fn unknown(&self) -> impl Iterator<Item = K> + '_ {
        self.kinds
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.known[*i])
            .map(|(_, k)| *k)
    }
}

/// 药水鉴定表（`Potion.java` L135 `handler`）。
pub type PotionStatusHandler = ItemStatusHandler<PotionKind, PotionColor>;

/// 卷轴鉴定表（`Scroll.java` L90 `handler`）。
pub type ScrollStatusHandler = ItemStatusHandler<ScrollKind, ScrollRune>;

impl PotionStatusHandler {
    /// `Potion.initColors()`（`Potion.java` L150-L152）：开局洗一次颜色。
    #[must_use]
    pub fn init_colors(rng: &mut impl Rng) -> Self {
        Self::shuffled(&PotionKind::ALL, &PotionColor::ALL, rng)
    }
}

impl ScrollStatusHandler {
    /// `Scroll.initLabels()`（`Scroll.java` L104-L107）：开局洗一次符文。
    #[must_use]
    pub fn init_labels(rng: &mut impl Rng) -> Self {
        Self::shuffled(&ScrollKind::ALL, &ScrollRune::ALL, rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::random::ItemRng;
    use rand::SeedableRng;
    use std::collections::HashSet;

    fn rng(seed: u64) -> ItemRng {
        ItemRng::seed_from_u64(seed)
    }

    /// 双射性：12 种药水/卷轴各分到互不相同的外观，反查还原。
    #[test]
    fn shuffle_is_bijective() {
        for seed in 0..20 {
            let potions = PotionStatusHandler::init_colors(&mut rng(seed));
            let colors: HashSet<PotionColor> = PotionKind::ALL
                .iter()
                .map(|&k| potions.appearance_of(k))
                .collect();
            assert_eq!(colors.len(), 12, "种子 {seed}：颜色必须互不重复");
            for kind in PotionKind::ALL {
                assert_eq!(potions.kind_of(potions.appearance_of(kind)), kind);
            }

            let scrolls = ScrollStatusHandler::init_labels(&mut rng(seed));
            let runes: HashSet<ScrollRune> = ScrollKind::ALL
                .iter()
                .map(|&k| scrolls.appearance_of(k))
                .collect();
            assert_eq!(runes.len(), 12, "种子 {seed}：符文必须互不重复");
        }
    }

    /// 同种子一致性：外观分配完全可复现；不同种子几乎必然不同。
    #[test]
    fn same_seed_reproduces_same_mapping() {
        let a = PotionStatusHandler::init_colors(&mut rng(2026));
        let b = PotionStatusHandler::init_colors(&mut rng(2026));
        assert_eq!(a, b, "同种子洗牌必须逐项一致");

        let c = PotionStatusHandler::init_colors(&mut rng(2027));
        let differs = PotionKind::ALL
            .iter()
            .any(|&k| a.appearance_of(k) != c.appearance_of(k));
        assert!(
            differs,
            "不同种子应产生不同分配（12! 种排列，撞车概率可忽略）"
        );

        let s1 = ScrollStatusHandler::init_labels(&mut rng(99));
        let s2 = ScrollStatusHandler::init_labels(&mut rng(99));
        assert_eq!(s1, s2);
    }

    /// 鉴定状态查询（`know`/`isKnown`/`known`/`unknown`，L187-L215）。
    #[test]
    fn known_set_tracks_identification() {
        let mut handler = PotionStatusHandler::init_colors(&mut rng(7));
        assert!(!handler.is_known(PotionKind::Healing));
        assert_eq!(handler.known().count(), 0);
        assert_eq!(handler.unknown().count(), 12);

        handler.know(PotionKind::Healing);
        handler.know(PotionKind::Strength);
        handler.know(PotionKind::Healing); // 重复 know 幂等
        assert!(handler.is_known(PotionKind::Healing));
        assert!(handler.is_known(PotionKind::Strength));
        assert!(!handler.is_known(PotionKind::Frost));
        assert_eq!(handler.known().count(), 2);
        assert_eq!(handler.unknown().count(), 10);
        assert!(handler.unknown().all(|k| k != PotionKind::Healing));

        // 鉴定不改变外观绑定
        let before = handler.appearance_of(PotionKind::Frost);
        handler.know(PotionKind::Frost);
        assert_eq!(handler.appearance_of(PotionKind::Frost), before);
    }
}
