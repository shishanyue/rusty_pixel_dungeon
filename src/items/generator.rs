//! 掉落表：`Generator.java` 的逐行移植。
//!
//! 核心是两层"牌堆（deck）递减补充"机制：
//!
//! 1. **类目层**（`categoryProbs`，L621-L623）：两副各 35 张的类目牌
//!    （一副含戒指+双份护甲权重，另一副含神器+双份投掷权重，L275-L279）。
//!    每抽一类扣 1 权重（L682），全部抽空后换另一副（L677-L681）。
//! 2. **类目内层**（各 `Category.probs`，L256-L268）：带 `defaultProbs` 的
//!    类目按牌堆抽取，抽一件扣 1（L721），抽空重置补牌（L717-L720，
//!    药水/卷轴在两副牌间交替，L645-L654）；神器抽空**不重置**（全局唯一，
//!    L854-L879）。类目内抽取走**类目私有随机流**（`cat.seed` + `dropped`
//!    重放跳过，L711-L727），保证掉落序列与外部随机消耗的时序无关。
//!
//! 与 SPD 的静态字段不同，全部状态收进 [`Generator`] 结构；随机源显式传
//! `&mut impl Rng`（docs/plans/01 · 确定性），`Dungeon.depth` 改为显式
//! `depth` 参数（仅金币数额与武器/护甲/投掷的 `floor_set = depth / 5` 使用）。

use rand::{Rng, SeedableRng};

use super::item::Item;
use super::kinds::{
    ArmorKind, ArtifactKind, FoodKind, ItemKind, MeleeWeaponKind, MissileWeaponKind, PotionKind,
    RingKind, ScrollKind, SeedKind, StoneKind, TrinketKind, WandKind,
};
use super::random::{ItemRng, chances, float, int, int_range, next_seed};

/// 类目总数（`Generator.java` L221-L252 枚举常量数）。
pub const CATEGORY_COUNT: usize = 23;

/// 掉落类目（`Generator.java` L221-L252，**保持 Java 声明序**——
/// 类目牌堆 `categoryProbs` 按此序线扫抽取）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// TRINKET（L222）。
    Trinket,
    /// WEAPON（L224；空 `classes`，经 `randomWeapon` 路由到 tier 类目）。
    Weapon,
    /// `WEP_T1`（L225）。
    WepT1,
    /// `WEP_T2`（L226）。
    WepT2,
    /// `WEP_T3`（L227）。
    WepT3,
    /// `WEP_T4`（L228）。
    WepT4,
    /// `WEP_T5`（L229）。
    WepT5,
    /// ARMOR（L231；非 deck，直接按 `probs` 抽）。
    Armor,
    /// MISSILE（L233；空 `classes`，经 `randomMissile` 路由）。
    Missile,
    /// `MIS_T1`（L234）。
    MisT1,
    /// `MIS_T2`（L235）。
    MisT2,
    /// `MIS_T3`（L236）。
    MisT3,
    /// `MIS_T4`（L237）。
    MisT4,
    /// `MIS_T5`（L238）。
    MisT5,
    /// WAND（L240）。
    Wand,
    /// RING（L241）。
    Ring,
    /// ARTIFACT（L242）。
    Artifact,
    /// FOOD（L244）。
    Food,
    /// POTION（L246）。
    Potion,
    /// SEED（L247）。
    Seed,
    /// SCROLL（L249）。
    Scroll,
    /// STONE（L250）。
    Stone,
    /// GOLD（L252；非 deck）。
    Gold,
}

// ---- 类目内权重表（Generator.java 静态初始化块 L324-L610）----

/// `GOLD.probs`（L327）。
const GOLD_PROBS: [f32; 1] = [1.0];
/// `POTION.defaultProbs`（L342）。
const POTION_DEFAULT_PROBS: [f32; 12] =
    [0.0, 3.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
/// `POTION.defaultProbs2`（L343）。
const POTION_DEFAULT_PROBS_2: [f32; 12] =
    [0.0, 3.0, 2.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0];
/// `SEED.defaultProbs`（L359）。
const SEED_DEFAULT_PROBS: [f32; 12] = [0.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 1.0];
/// `SCROLL.defaultProbs`（L376）。
const SCROLL_DEFAULT_PROBS: [f32; 12] =
    [0.0, 3.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
/// `SCROLL.defaultProbs2`（L377）。
const SCROLL_DEFAULT_PROBS_2: [f32; 12] =
    [0.0, 3.0, 2.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0];
/// `STONE.defaultProbs`（L394）。
const STONE_DEFAULT_PROBS: [f32; 12] = [0.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0];
/// `WAND.defaultProbs`（L411）。
const WAND_DEFAULT_PROBS: [f32; 13] = [3.0; 13];
/// `WEP_T1.defaultProbs`（L426；法师之杖 0 权重）。
const WEP_T1_DEFAULT_PROBS: [f32; 6] = [2.0, 0.0, 2.0, 2.0, 2.0, 2.0];
/// `WEP_T2.defaultProbs`（L438；鹤嘴锄 0 权重）。
const WEP_T2_DEFAULT_PROBS: [f32; 7] = [2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0];
/// `WEP_T3.defaultProbs`（L449）。注意 L450 的上游笔误把 `WEP_T3.probs`
/// 初始化成了 `WEP_T1.defaultProbs` 的拷贝——但 `fullReset` 会按本表重置，
/// 游戏内不可观测，此处不复刻。
const WEP_T3_DEFAULT_PROBS: [f32; 6] = [2.0; 6];
/// `WEP_T4.defaultProbs`（L461）。
const WEP_T4_DEFAULT_PROBS: [f32; 7] = [2.0; 7];
/// `WEP_T5.defaultProbs`（L473）。
const WEP_T5_DEFAULT_PROBS: [f32; 7] = [2.0; 7];
/// `ARMOR.probs`（L490；非 deck，职业甲 0 权重）。
const ARMOR_PROBS: [f32; 11] = [1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
/// `MIS_T1.defaultProbs`（L502；飞镖 0 权重）。
const MIS_T1_DEFAULT_PROBS: [f32; 4] = [3.0, 3.0, 3.0, 0.0];
/// `MIS_T2.defaultProbs`（L510）。
const MIS_T2_DEFAULT_PROBS: [f32; 3] = [3.0; 3];
/// `MIS_T3.defaultProbs`（L518）。
const MIS_T3_DEFAULT_PROBS: [f32; 3] = [3.0; 3];
/// `MIS_T4.defaultProbs`（L526）。
const MIS_T4_DEFAULT_PROBS: [f32; 3] = [3.0; 3];
/// `MIS_T5.defaultProbs`（L534）。
const MIS_T5_DEFAULT_PROBS: [f32; 3] = [3.0; 3];
/// `FOOD.defaultProbs`（L541；神秘肉 0 权重）。
const FOOD_DEFAULT_PROBS: [f32; 3] = [4.0, 1.0, 0.0];
/// `RING.defaultProbs`（L557）。
const RING_DEFAULT_PROBS: [f32; 12] = [3.0; 12];
/// `ARTIFACT.defaultProbs`（L575；暗影披风/圣典 0 权重）。
const ARTIFACT_DEFAULT_PROBS: [f32; 13] = [
    1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
];
/// `TRINKET.defaultProbs`（L599）。
const TRINKET_DEFAULT_PROBS: [f32; 17] = [1.0; 17];

/// `POTION.defaultProbsTotal` = `defaultProbs + defaultProbs2` 逐项和
/// （静态块 L602-L609 计算）；`SCROLL` 两表同值故共用。
const POTION_SCROLL_PROBS_TOTAL: [f32; 12] =
    [0.0, 6.0, 4.0, 3.0, 3.0, 3.0, 2.0, 2.0, 2.0, 2.0, 2.0, 1.0];

/// `floorSetTierProbs`（L613-L619）：行 = `floor_set`（`depth / 5`），
/// 列 = tier-1..tier-5 权重。
pub const FLOOR_SET_TIER_PROBS: [[f32; 5]; 5] = [
    [0.0, 75.0, 20.0, 4.0, 1.0],
    [0.0, 25.0, 50.0, 20.0, 5.0],
    [0.0, 0.0, 40.0, 50.0, 10.0],
    [0.0, 0.0, 20.0, 40.0, 40.0],
    [0.0, 0.0, 0.0, 20.0, 80.0],
];

/// `wepTiers`（L788-L794）。
pub const WEP_TIERS: [Category; 5] = [
    Category::WepT1,
    Category::WepT2,
    Category::WepT3,
    Category::WepT4,
    Category::WepT5,
];

/// `misTiers`（L821-L827）。
pub const MIS_TIERS: [Category; 5] = [
    Category::MisT1,
    Category::MisT2,
    Category::MisT3,
    Category::MisT4,
    Category::MisT5,
];

impl Category {
    /// 全类目，Java 声明序（`values()` 语义）。
    pub const ALL: [Self; CATEGORY_COUNT] = [
        Self::Trinket,
        Self::Weapon,
        Self::WepT1,
        Self::WepT2,
        Self::WepT3,
        Self::WepT4,
        Self::WepT5,
        Self::Armor,
        Self::Missile,
        Self::MisT1,
        Self::MisT2,
        Self::MisT3,
        Self::MisT4,
        Self::MisT5,
        Self::Wand,
        Self::Ring,
        Self::Artifact,
        Self::Food,
        Self::Potion,
        Self::Seed,
        Self::Scroll,
        Self::Stone,
        Self::Gold,
    ];

    /// 第一副类目牌中的权重（`Generator.java` L222-L252 各常量第 1 参）。
    #[must_use]
    pub const fn first_prob(self) -> f32 {
        match self {
            Self::Weapon | Self::Armor => 2.0, // L224 / L231
            Self::Missile | Self::Wand | Self::Ring | Self::Seed | Self::Stone => 1.0, // L233/L240/L241/L247/L250
            Self::Potion | Self::Scroll => 8.0, // L246 / L249
            Self::Gold => 10.0,                 // L252
            _ => 0.0, // TRINKET、WEP_Tn、MIS_Tn、ARTIFACT、FOOD（L222-L244）
        }
    }

    /// 第二副类目牌中的权重（各常量第 2 参；与第一副的差异：
    /// 护甲 2→1、投掷 1→2、戒指 1→0、神器 0→1）。
    #[must_use]
    pub const fn second_prob(self) -> f32 {
        match self {
            Self::Weapon | Self::Missile => 2.0, // L224 / L233
            Self::Armor | Self::Wand | Self::Artifact | Self::Seed | Self::Stone => 1.0, // L231/L240/L242/L247/L250
            Self::Potion | Self::Scroll => 8.0, // L246 / L249
            Self::Gold => 10.0,                 // L252
            _ => 0.0,                           // TRINKET、WEP_Tn、MIS_Tn、RING、FOOD
        }
    }

    /// deck 类目的默认权重表（`defaultProbs`，L261）；
    /// `None` = 非 deck 类目（GOLD/ARMOR/WEAPON/MISSILE，L256-L258）。
    #[must_use]
    pub const fn default_probs(self) -> Option<&'static [f32]> {
        match self {
            Self::Potion => Some(&POTION_DEFAULT_PROBS),
            Self::Seed => Some(&SEED_DEFAULT_PROBS),
            Self::Scroll => Some(&SCROLL_DEFAULT_PROBS),
            Self::Stone => Some(&STONE_DEFAULT_PROBS),
            Self::Wand => Some(&WAND_DEFAULT_PROBS),
            Self::WepT1 => Some(&WEP_T1_DEFAULT_PROBS),
            Self::WepT2 => Some(&WEP_T2_DEFAULT_PROBS),
            Self::WepT3 => Some(&WEP_T3_DEFAULT_PROBS),
            Self::WepT4 => Some(&WEP_T4_DEFAULT_PROBS),
            Self::WepT5 => Some(&WEP_T5_DEFAULT_PROBS),
            Self::MisT1 => Some(&MIS_T1_DEFAULT_PROBS),
            Self::MisT2 => Some(&MIS_T2_DEFAULT_PROBS),
            Self::MisT3 => Some(&MIS_T3_DEFAULT_PROBS),
            Self::MisT4 => Some(&MIS_T4_DEFAULT_PROBS),
            Self::MisT5 => Some(&MIS_T5_DEFAULT_PROBS),
            Self::Food => Some(&FOOD_DEFAULT_PROBS),
            Self::Ring => Some(&RING_DEFAULT_PROBS),
            Self::Artifact => Some(&ARTIFACT_DEFAULT_PROBS),
            Self::Trinket => Some(&TRINKET_DEFAULT_PROBS),
            Self::Gold | Self::Armor | Self::Weapon | Self::Missile => None,
        }
    }

    /// 第二副类目内牌（`defaultProbs2`，L265）：仅药水/卷轴（L343/L377）。
    #[must_use]
    pub const fn default_probs2(self) -> Option<&'static [f32]> {
        match self {
            Self::Potion => Some(&POTION_DEFAULT_PROBS_2),
            Self::Scroll => Some(&SCROLL_DEFAULT_PROBS_2),
            _ => None,
        }
    }

    /// 两副牌逐项和（`defaultProbsTotal`，L268/L602-L609）：非 deck 抽取
    /// （`randomUsingDefaults`）时药水/卷轴用全量表。
    #[must_use]
    pub const fn default_probs_total(self) -> Option<&'static [f32]> {
        match self {
            Self::Potion | Self::Scroll => Some(&POTION_SCROLL_PROBS_TOTAL),
            _ => None,
        }
    }

    /// 静态初始化时的 `probs`（deck 类目取 `defaultProbs` 克隆；
    /// GOLD L327、ARMOR L490 为固定表；WEAPON/MISSILE 为空，L415-L416/L493-L494）。
    #[must_use]
    pub const fn initial_probs(self) -> &'static [f32] {
        match self {
            Self::Gold => &GOLD_PROBS,
            Self::Armor => &ARMOR_PROBS,
            Self::Weapon | Self::Missile => &[],
            _ => match self.default_probs() {
                Some(probs) => probs,
                // 除上列四类目外均为 deck 类目
                None => unreachable!(),
            },
        }
    }

    /// 类目 `classes` 数组长度。
    #[must_use]
    pub const fn class_count(self) -> usize {
        self.initial_probs().len()
    }

    /// 类目 `classes[index]` → [`ItemKind`]（各数组行号见
    /// [`kinds`](super::kinds) 模块的 `ALL`/`TIERn` 常量文档）。
    ///
    /// # Panics
    ///
    /// WEAPON/MISSILE 的 `classes` 为空（L415-L416/L493-L494），Java 侧同样
    /// 不可按下标取——必须经 `randomWeapon`/`randomMissile` 路由。
    #[must_use]
    pub fn kind_at(self, index: usize) -> ItemKind {
        match self {
            Self::Gold => ItemKind::Gold,
            Self::Food => ItemKind::Food(FoodKind::ALL[index]),
            Self::Potion => ItemKind::Potion(PotionKind::ALL[index]),
            Self::Seed => ItemKind::Seed(SeedKind::ALL[index]),
            Self::Scroll => ItemKind::Scroll(ScrollKind::ALL[index]),
            Self::Stone => ItemKind::Stone(StoneKind::ALL[index]),
            Self::Wand => ItemKind::Wand(WandKind::ALL[index]),
            Self::WepT1 => ItemKind::Weapon(MeleeWeaponKind::TIER1[index]),
            Self::WepT2 => ItemKind::Weapon(MeleeWeaponKind::TIER2[index]),
            Self::WepT3 => ItemKind::Weapon(MeleeWeaponKind::TIER3[index]),
            Self::WepT4 => ItemKind::Weapon(MeleeWeaponKind::TIER4[index]),
            Self::WepT5 => ItemKind::Weapon(MeleeWeaponKind::TIER5[index]),
            Self::Armor => ItemKind::Armor(ArmorKind::ALL[index]),
            Self::MisT1 => ItemKind::Missile(MissileWeaponKind::TIER1[index]),
            Self::MisT2 => ItemKind::Missile(MissileWeaponKind::TIER2[index]),
            Self::MisT3 => ItemKind::Missile(MissileWeaponKind::TIER3[index]),
            Self::MisT4 => ItemKind::Missile(MissileWeaponKind::TIER4[index]),
            Self::MisT5 => ItemKind::Missile(MissileWeaponKind::TIER5[index]),
            Self::Ring => ItemKind::Ring(RingKind::ALL[index]),
            Self::Artifact => ItemKind::Artifact(ArtifactKind::ALL[index]),
            Self::Trinket => ItemKind::Trinket(TrinketKind::ALL[index]),
            Self::Weapon | Self::Missile => {
                unreachable!("WEAPON/MISSILE 类目无 classes，经 tier 类目路由")
            }
        }
    }
}

/// 单类目的可变状态（`Category` 枚举里的可变字段，L260-L273）。
#[derive(Debug, Clone)]
struct CategoryState {
    /// 当前牌堆余量（`probs`，L260）。
    probs: Vec<f32>,
    /// 当前用第二副类目内牌（`using2ndProbs`，L266；仅药水/卷轴有意义）。
    using_2nd_probs: bool,
    /// 类目私有随机流种子（`seed`，L272；`fullReset` 时从外部流取）。
    seed: Option<u64>,
    /// 已完成的私有流抽取数（`dropped`，L273；重放时跳过的 `Long` 数）。
    dropped: u32,
}

/// 掉落表状态机（`Generator.java` 静态字段集合）。
#[derive(Debug, Clone)]
pub struct Generator {
    /// 当前用第一副类目牌（`usingFirstDeck`，L621）。
    using_first_deck: bool,
    /// 类目牌堆余量（`categoryProbs`，L623；`LinkedHashMap` 按枚举序 → 数组）。
    category_probs: [f32; CATEGORY_COUNT],
    /// 各类目内牌堆状态，按 [`Category::ALL`] 序。
    states: [CategoryState; CATEGORY_COUNT],
}

impl Generator {
    /// 新建并执行一次 `fullReset`（L625-L636）——SPD 在开新局时调用。
    #[must_use]
    pub fn new(rng: &mut impl Rng) -> Self {
        let mut generator = Self {
            using_first_deck: false,
            category_probs: [0.0; CATEGORY_COUNT],
            states: Category::ALL.map(|cat| CategoryState {
                probs: cat.initial_probs().to_vec(),
                using_2nd_probs: false,
                seed: None,
                dropped: 0,
            }),
        };
        generator.full_reset(rng);
        generator
    }

    /// `fullReset`（L625-L636）：随机选类目牌副、重置全部类目内牌堆、
    /// 为 deck 类目取私有流种子。
    ///
    /// 随机消耗序（照抄）：`Int(2)`（L626）→ 按枚举序对每个类目：
    /// 有第二副内牌的先 `Int(2)`（L629，`&&` 短路——仅药水/卷轴消耗），
    /// deck 类目再取一个种子（L632 `Random.Long()`）。
    pub fn full_reset(&mut self, rng: &mut impl Rng) {
        self.using_first_deck = int(rng, 2) == 0;
        self.general_reset();
        for cat in Category::ALL {
            let index = cat as usize;
            self.states[index].using_2nd_probs = cat.default_probs2().is_some() && int(rng, 2) == 0;
            // L630 reset 会再翻转一次 using2ndProbs，净效果 = 随机初始牌副
            self.reset_category(cat);
            if cat.default_probs().is_some() {
                self.states[index].seed = Some(next_seed(rng));
                self.states[index].dropped = 0;
            }
        }
    }

    /// `generalReset`（L638-L643）：类目牌堆按当前牌副重置为满额。
    pub fn general_reset(&mut self) {
        for cat in Category::ALL {
            self.category_probs[cat as usize] = if self.using_first_deck {
                cat.first_prob()
            } else {
                cat.second_prob()
            };
        }
    }

    /// `reset(Category)`（L645-L654）：类目内牌堆补满；药水/卷轴在两副
    /// 内牌间交替（L647-L649），其余 deck 类目回默认表（L651）。
    /// 非 deck 类目（GOLD/ARMOR）无操作。
    pub fn reset_category(&mut self, cat: Category) {
        let index = cat as usize;
        if let Some(defaults) = cat.default_probs() {
            if let Some(defaults2) = cat.default_probs2() {
                self.states[index].using_2nd_probs = !self.states[index].using_2nd_probs;
                self.states[index].probs = if self.states[index].using_2nd_probs {
                    defaults2.to_vec()
                } else {
                    defaults.to_vec()
                };
            } else {
                self.states[index].probs = defaults.to_vec();
            }
        }
    }

    /// `undoDrop`（L662-L673）：把一件物品"洗回牌堆"——deck 类目中对应
    /// 下标权重 +1（不保序）。
    ///
    /// 注：Java L664 的 `cls.isAssignableFrom(cat.superClass)` 前置过滤疑似
    /// 写反（对具体物品类恒 false，仅 `Food.class` 这类同时充当 superClass
    /// 与 classes 成员的能通过）；此处按内层 `cls == cat.classes[i]`（L667）
    /// 的明显意图实现，差异记录于计划文档实现笔记。
    pub fn undo_drop(&mut self, kind: ItemKind) {
        for cat in Category::ALL {
            if cat.default_probs().is_none() {
                continue; // L665
            }
            for i in 0..cat.class_count() {
                if cat.kind_at(i) == kind {
                    self.states[cat as usize].probs[i] += 1.0; // L668
                }
            }
        }
    }

    /// `random()`（L675-L692）：从类目牌堆抽一类（抽空则换副补满再抽），
    /// 扣 1 权重后按类目生成物品。种子类目固定走默认表
    /// （L684-L688：种子的主要来源是草丛而非关卡生成，保持两边一致性）。
    pub fn random(&mut self, rng: &mut impl Rng, depth: i32) -> Item {
        let cat = match chances(rng, &self.category_probs) {
            Some(i) => Category::ALL[i],
            None => {
                // L677-L681：35 张类目牌抽空 → 换另一副
                self.using_first_deck = !self.using_first_deck;
                self.general_reset();
                let i = chances(rng, &self.category_probs).expect("重置后类目权重和恒为 35");
                Category::ALL[i]
            }
        };
        self.category_probs[cat as usize] -= 1.0; // L682

        if cat == Category::Seed {
            self.random_category_using_defaults(cat, rng, depth) // L688
        } else {
            self.random_category(cat, rng, depth) // L690
        }
    }

    /// `randomUsingDefaults()`（L694-L696）：按两副类目牌之和
    /// （`defaultCatProbs`，L641）抽类目，再走默认表生成。
    pub fn random_using_defaults(&mut self, rng: &mut impl Rng, depth: i32) -> Item {
        let default_cat_probs: [f32; CATEGORY_COUNT] =
            Category::ALL.map(|cat| cat.first_prob() + cat.second_prob());
        let i = chances(rng, &default_cat_probs).expect("defaultCatProbs 和恒为 70");
        self.random_category_using_defaults(Category::ALL[i], rng, depth)
    }

    /// `random(Category)`（L698-L741）：按类目生成一件物品。
    ///
    /// - 护甲/武器/投掷经 floor set 路由（L700-L705；`floor_set = depth / 5`，
    ///   见 L776/L797/L830）；
    /// - 神器走唯一性牌堆，耗尽则回退为戒指（L706-L709）；
    /// - 其余走类目内 deck 抽取（L710-L739，见 [`Self::draw_deck_index`]）。
    pub fn random_category(&mut self, cat: Category, rng: &mut impl Rng, depth: i32) -> Item {
        match cat {
            Category::Armor => self.random_armor(depth / 5, rng),
            Category::Weapon => self.random_weapon(depth / 5, false, rng),
            Category::Missile => self.random_missile(depth / 5, false, rng),
            Category::Artifact => match self.random_artifact(rng) {
                Some(item) => item,
                // L708-L709：神器抽空 → 改发戒指
                None => self.random_category(Category::Ring, rng, depth),
            },
            _ => {
                let i = self.draw_deck_index(cat, rng);
                let kind = cat.kind_at(i);
                // L729-L737：普通药水/卷轴有一次 exotic 升格判定。一期无
                // 异域水晶饰品，概率恒 0（ExoticCrystals.java L48-L57），但
                // 判定消耗一个 Float 的时序照抄，保证随机流布局与 Java 一致。
                // TODO(效果域)：exotic 变体替换表。
                if matches!(kind, ItemKind::Potion(_) | ItemKind::Scroll(_)) {
                    let _ = float(rng) < consumable_exotic_chance();
                }
                roll_item(kind, rng, depth)
            }
        }
    }

    /// 类目内 deck 抽取（L710-L727）：deck 类目且已播种时，从
    /// `seed` 重建私有流并跳过 `dropped` 个 `Long`（L712-L713），再按
    /// `probs` 权重抽下标；抽空则重置补牌后**用同一条流**再抽（L717-L720）；
    /// 抽中下标扣 1（L721），完成后 `dropped += 1`（L726）。
    ///
    /// 非 deck 类目（GOLD/ARMOR）直接用外部流抽，不扣减。
    fn draw_deck_index(&mut self, cat: Category, rng: &mut impl Rng) -> usize {
        let index = cat as usize;
        let is_deck = cat.default_probs().is_some();

        if is_deck && self.states[index].seed.is_some() {
            let seed = self.states[index].seed.expect("上一行已判 Some");
            let mut deck_rng = ItemRng::seed_from_u64(seed);
            for _ in 0..self.states[index].dropped {
                let _ = next_seed(&mut deck_rng); // L713：跳过一个 Random.Long()
            }
            let i = match chances(&mut deck_rng, &self.states[index].probs) {
                Some(i) => i,
                None => {
                    self.reset_category(cat);
                    chances(&mut deck_rng, &self.states[index].probs)
                        .expect("重置后类目内权重和恒为正")
                }
            };
            self.states[index].probs[i] -= 1.0;
            self.states[index].dropped += 1;
            i
        } else {
            let i = match chances(rng, &self.states[index].probs) {
                Some(i) => i,
                None => {
                    self.reset_category(cat);
                    chances(rng, &self.states[index].probs).expect("重置后权重和恒为正")
                }
            };
            if is_deck {
                self.states[index].probs[i] -= 1.0;
            }
            i
        }
    }

    /// `randomUsingDefaults(Category)`（L745-L769）：绕过 deck 状态、
    /// 始终按默认表抽（神器例外，必须走唯一性牌堆，L750）。
    pub fn random_category_using_defaults(
        &mut self,
        cat: Category,
        rng: &mut impl Rng,
        depth: i32,
    ) -> Item {
        if cat == Category::Weapon {
            return self.random_weapon(depth / 5, true, rng); // L746-L747
        }
        if cat == Category::Missile {
            return self.random_missile(depth / 5, true, rng); // L748-L749
        }
        if cat.default_probs().is_none() || cat == Category::Artifact {
            return self.random_category(cat, rng, depth); // L750-L751
        }
        if let Some(total) = cat.default_probs_total() {
            // L752-L753：药水/卷轴按两副之和抽；此分支 Java 无 exotic 判定
            let i = chances(rng, total).expect("defaultProbsTotal 和恒为正");
            return roll_item(cat.kind_at(i), rng, depth);
        }
        // L754-L767：其余 deck 类目直接按 defaultProbs 抽（不动牌堆状态）。
        // L757-L765 的 exotic 判定只匹配普通药水/卷轴，本分支类目不会命中，
        // 无随机消耗。
        let i = chances(rng, cat.default_probs().expect("上方已排除非 deck 类目"))
            .expect("defaultProbs 和恒为正");
        roll_item(cat.kind_at(i), rng, depth)
    }

    /// `randomArmor(floorSet)`（L779-L786）：按 floor set 权重选阶
    /// （只会命中前 5 个普通护甲，职业甲权重 0），再掷实例属性。
    pub fn random_armor(&mut self, floor_set: i32, rng: &mut impl Rng) -> Item {
        let floor_set = clamp_floor_set(floor_set); // L781 GameMath.gate
        let i = chances(rng, &FLOOR_SET_TIER_PROBS[floor_set]).expect("tier 权重行和恒为 100");
        // L783：ARMOR.classes[i]；Armor.random() 不读 depth
        roll_item(Category::Armor.kind_at(i), rng, 0)
    }

    /// `randomWeapon(floorSet, useDefaults)`（L808-L819）：按 floor set
    /// 权重选 tier 类目，再走 deck 或默认表抽取。
    pub fn random_weapon(
        &mut self,
        floor_set: i32,
        use_defaults: bool,
        rng: &mut impl Rng,
    ) -> Item {
        let floor_set = clamp_floor_set(floor_set); // L810
        let tier = WEP_TIERS
            [chances(rng, &FLOOR_SET_TIER_PROBS[floor_set]).expect("tier 权重行和恒为 100")];
        // tier 类目内不含金币，depth 无用；传 0
        if use_defaults {
            self.random_category_using_defaults(tier, rng, 0) // L814
        } else {
            self.random_category(tier, rng, 0) // L816
        }
    }

    /// `randomMissile(floorSet, useDefaults)`（L841-L852）。
    pub fn random_missile(
        &mut self,
        floor_set: i32,
        use_defaults: bool,
        rng: &mut impl Rng,
    ) -> Item {
        let floor_set = clamp_floor_set(floor_set); // L843
        let tier = MIS_TIERS
            [chances(rng, &FLOOR_SET_TIER_PROBS[floor_set]).expect("tier 权重行和恒为 100")];
        if use_defaults {
            self.random_category_using_defaults(tier, rng, 0) // L847
        } else {
            self.random_category(tier, rng, 0) // L849
        }
    }

    /// `randomArtifact`（L855-L879）：神器全局唯一——牌堆抽空**不重置**，
    /// 返回 `None`（调用方回退为戒指）。注意 `dropped` 无论是否抽中都自增
    /// （L866-L869 在 -1 检查之前执行）。
    pub fn random_artifact(&mut self, rng: &mut impl Rng) -> Option<Item> {
        let index = Category::Artifact as usize;

        let drawn = if let Some(seed) = self.states[index].seed {
            let mut deck_rng = ItemRng::seed_from_u64(seed);
            for _ in 0..self.states[index].dropped {
                let _ = next_seed(&mut deck_rng); // L861
            }
            let i = chances(&mut deck_rng, &self.states[index].probs); // L864
            self.states[index].dropped += 1; // L868
            i
        } else {
            chances(rng, &self.states[index].probs)
        };

        let i = drawn?; // L872-L874：耗尽 → None
        self.states[index].probs[i] -= 1.0; // L876
        // L877：Artifact.random() 从外部流掷诅咒
        Some(roll_item(Category::Artifact.kind_at(i), rng, 0))
    }

    /// `removeArtifact`（L881-L890）：把指定神器从牌堆中划掉
    /// （英雄初始神器等场景）；本就无余量时返回 false。
    pub fn remove_artifact(&mut self, artifact: ArtifactKind) -> bool {
        let index = Category::Artifact as usize;
        for (i, kind) in ArtifactKind::ALL.iter().enumerate() {
            if *kind == artifact && self.states[index].probs[i] > 0.0 {
                self.states[index].probs[i] = 0.0;
                return true;
            }
        }
        false
    }
}

/// `GameMath.gate(0, floorSet, 4)`（L781/L810/L843）。
fn clamp_floor_set(floor_set: i32) -> usize {
    floor_set.clamp(0, FLOOR_SET_TIER_PROBS.len() as i32 - 1) as usize
}

/// 一期的 `ExoticCrystals.consumableExoticChance()`（`ExoticCrystals.java`
/// L48-L57）：无饰品时 `trinketLevel == -1` → 概率 0。TODO(效果域)：饰品等级。
fn consumable_exotic_chance() -> f32 {
    0.0
}

/// `Reflection.newInstance(itemCls).random()` 的等价物：按种类掷实例属性
/// （数量/等级/诅咒）。`depth` 仅金币数额使用（`Gold.java` L91）。
///
/// 各类 `random()` 覆写：
/// - 金币：`quantity = IntRange(30 + depth*10, 60 + depth*20)`（`Gold.java` L89-L93）；
/// - 近战/投掷（`Weapon.java` L419-L449、`MissileWeapon.java` L358-L388）：
///   +0/+1/+2 = 75%/20%/5%（`Int(4)`、`Int(5)`），诅咒/附魔 roll 走
///   `Random.Long()` 播种的独立流（L434，隔离羊皮纸残片等浮动对关卡生成流的
///   影响）——30% 诅咒；≥90% 附魔属效果域 TODO；
/// - 护甲（`Armor.java` L654-L684）：同上，30% 诅咒、≥85% 铭刻 TODO；
/// - 法杖/戒指（`Wand.java` L546-L566、`Ring.java` L259-L278）：
///   +0/+1/+2 = 66.67%/26.67%/6.67%（`Int(3)`、`Int(5)`），30% 诅咒直接
///   从当前流掷（无独立流）；法杖充能 +n 属效果域 TODO；
/// - 神器（`Artifact.java` L218-L226）：恒 +0，30% 诅咒；
/// - 其余（药水/卷轴/种子/符石/食物/饰品）：`Item.random()` 默认无掷
///   （`Item.java` L566-L568）。
///
/// 一期无饰品修正：诅咒/附魔乘数恒 1（`ParchmentScrap.java` L67-L79）。
pub fn roll_item(kind: ItemKind, rng: &mut impl Rng, depth: i32) -> Item {
    let mut item = Item::new(kind);
    match kind {
        ItemKind::Gold => {
            item.quantity = int_range(rng, 30 + depth * 10, 60 + depth * 20);
        }
        ItemKind::Weapon(_) | ItemKind::Missile(_) | ItemKind::Armor(_) => {
            item.level = roll_upgrade_level(rng, 4); // Weapon.java L423-L430
            // Weapon.java L432-L446：独立流掷诅咒/附魔
            let mut effect_rng = ItemRng::seed_from_u64(next_seed(rng));
            let effect_roll = float(&mut effect_rng);
            if effect_roll < 0.3 {
                item.cursed = true;
            }
            // TODO(效果域)：effect_roll ≥ 0.9（武器附魔）/ ≥ 0.85（护甲铭刻）
        }
        ItemKind::Wand(_) | ItemKind::Ring(_) => {
            item.level = roll_upgrade_level(rng, 3); // Wand.java L550-L557
            // TODO(效果域)：法杖 curCharges += n（Wand.java L558）
            if float(rng) < 0.3 {
                item.cursed = true; // Wand.java L560-L563 / Ring.java L272-L275
            }
        }
        ItemKind::Artifact(_) => {
            item.cursed = float(rng) < 0.3; // Artifact.java L221-L224
        }
        _ => {}
    }
    item
}

/// 升级等级掷取（`Weapon.java` L423-L429 / `Wand.java` L550-L556 的共形）：
/// `Int(first) == 0` 则 +1，继而 `Int(5) == 0` 再 +1。
fn roll_upgrade_level(rng: &mut impl Rng, first: i32) -> i32 {
    let mut n = 0;
    if int(rng, first) == 0 {
        n += 1;
        if int(rng, 5) == 0 {
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use std::collections::HashMap;

    fn rng(seed: u64) -> ItemRng {
        ItemRng::seed_from_u64(seed)
    }

    /// 类目牌权重逐值对拍（`Generator.java` L222-L252），
    /// 两副牌总量各 35（L275-L277 注释）。
    #[test]
    fn category_deck_probs_match_java() {
        use Category as C;
        let expected: [(C, f32, f32); CATEGORY_COUNT] = [
            (C::Trinket, 0.0, 0.0),  // L222
            (C::Weapon, 2.0, 2.0),   // L224
            (C::WepT1, 0.0, 0.0),    // L225
            (C::WepT2, 0.0, 0.0),    // L226
            (C::WepT3, 0.0, 0.0),    // L227
            (C::WepT4, 0.0, 0.0),    // L228
            (C::WepT5, 0.0, 0.0),    // L229
            (C::Armor, 2.0, 1.0),    // L231
            (C::Missile, 1.0, 2.0),  // L233
            (C::MisT1, 0.0, 0.0),    // L234
            (C::MisT2, 0.0, 0.0),    // L235
            (C::MisT3, 0.0, 0.0),    // L236
            (C::MisT4, 0.0, 0.0),    // L237
            (C::MisT5, 0.0, 0.0),    // L238
            (C::Wand, 1.0, 1.0),     // L240
            (C::Ring, 1.0, 0.0),     // L241
            (C::Artifact, 0.0, 1.0), // L242
            (C::Food, 0.0, 0.0),     // L244
            (C::Potion, 8.0, 8.0),   // L246
            (C::Seed, 1.0, 1.0),     // L247
            (C::Scroll, 8.0, 8.0),   // L249
            (C::Stone, 1.0, 1.0),    // L250
            (C::Gold, 10.0, 10.0),   // L252
        ];
        for (i, (cat, first, second)) in expected.into_iter().enumerate() {
            assert_eq!(Category::ALL[i], cat, "枚举序必须与 Java 声明序一致");
            assert_eq!(cat.first_prob(), first, "{cat:?} firstProb");
            assert_eq!(cat.second_prob(), second, "{cat:?} secondProb");
        }
        let first_sum: f32 = Category::ALL.iter().map(|c| c.first_prob()).sum();
        let second_sum: f32 = Category::ALL.iter().map(|c| c.second_prob()).sum();
        assert_eq!(first_sum, 35.0, "第一副类目牌共 35 张（L275）");
        assert_eq!(second_sum, 35.0, "第二副类目牌共 35 张（L275）");
    }

    /// 类目内权重表逐值对拍（静态初始化块 L324-L610）。
    #[test]
    fn per_category_prob_tables_match_java() {
        // 药水（L342-L343）
        assert_eq!(
            Category::Potion.default_probs().unwrap(),
            &[0.0, 3.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(
            Category::Potion.default_probs2().unwrap(),
            &[0.0, 3.0, 2.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0]
        );
        // 卷轴（L376-L377）
        assert_eq!(
            Category::Scroll.default_probs().unwrap(),
            &[0.0, 3.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(
            Category::Scroll.default_probs2().unwrap(),
            &[0.0, 3.0, 2.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0]
        );
        // 种子（L359）/ 符石（L394）
        assert_eq!(
            Category::Seed.default_probs().unwrap(),
            &[0.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 1.0]
        );
        assert_eq!(
            Category::Stone.default_probs().unwrap(),
            &[0.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0]
        );
        // 法杖（L411）/ 戒指（L557）/ 饰品（L599）
        assert_eq!(Category::Wand.default_probs().unwrap(), &[3.0; 13]);
        assert_eq!(Category::Ring.default_probs().unwrap(), &[3.0; 12]);
        assert_eq!(Category::Trinket.default_probs().unwrap(), &[1.0; 17]);
        // 武器 tier（L426/L438/L449/L461/L473）
        assert_eq!(
            Category::WepT1.default_probs().unwrap(),
            &[2.0, 0.0, 2.0, 2.0, 2.0, 2.0],
            "法师之杖权重 0"
        );
        assert_eq!(
            Category::WepT2.default_probs().unwrap(),
            &[2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0],
            "鹤嘴锄权重 0"
        );
        assert_eq!(Category::WepT3.default_probs().unwrap(), &[2.0; 6]);
        assert_eq!(Category::WepT4.default_probs().unwrap(), &[2.0; 7]);
        assert_eq!(Category::WepT5.default_probs().unwrap(), &[2.0; 7]);
        // 投掷 tier（L502/L510/L518/L526/L534）
        assert_eq!(
            Category::MisT1.default_probs().unwrap(),
            &[3.0, 3.0, 3.0, 0.0],
            "飞镖权重 0"
        );
        for cat in [
            Category::MisT2,
            Category::MisT3,
            Category::MisT4,
            Category::MisT5,
        ] {
            assert_eq!(cat.default_probs().unwrap(), &[3.0; 3]);
        }
        // 食物（L541）/ 神器（L575）
        assert_eq!(Category::Food.default_probs().unwrap(), &[4.0, 1.0, 0.0]);
        assert_eq!(
            Category::Artifact.default_probs().unwrap(),
            &[
                1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0
            ],
            "暗影披风/圣典权重 0"
        );
        // 非 deck：金币（L327）/ 护甲（L490）
        assert!(Category::Gold.default_probs().is_none());
        assert!(Category::Armor.default_probs().is_none());
        assert_eq!(Category::Gold.initial_probs(), &[1.0]);
        assert_eq!(
            Category::Armor.initial_probs(),
            &[1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            "职业甲权重 0"
        );
        // 空 classes（L415-L416/L493-L494）
        assert_eq!(Category::Weapon.class_count(), 0);
        assert_eq!(Category::Missile.class_count(), 0);
        // 各表长度与 classes 数组一致
        for cat in Category::ALL {
            if let Some(probs) = cat.default_probs() {
                assert_eq!(probs.len(), cat.class_count(), "{cat:?}");
            }
        }
    }

    /// `defaultProbsTotal` = 两副内牌逐项和（L602-L609 的静态计算）。
    #[test]
    fn default_probs_total_is_elementwise_sum() {
        for cat in [Category::Potion, Category::Scroll] {
            let probs = cat.default_probs().unwrap();
            let probs2 = cat.default_probs2().unwrap();
            let total = cat.default_probs_total().unwrap();
            for i in 0..probs.len() {
                assert_eq!(total[i], probs[i] + probs2[i], "{cat:?}[{i}]");
            }
        }
        // 其余类目无第二副内牌，total 也应为 None
        for cat in Category::ALL {
            assert_eq!(
                cat.default_probs2().is_some(),
                cat.default_probs_total().is_some()
            );
        }
    }

    /// `floorSetTierProbs` 逐值对拍（L613-L619）。
    #[test]
    fn floor_set_tier_probs_match_java() {
        assert_eq!(FLOOR_SET_TIER_PROBS[0], [0.0, 75.0, 20.0, 4.0, 1.0]);
        assert_eq!(FLOOR_SET_TIER_PROBS[1], [0.0, 25.0, 50.0, 20.0, 5.0]);
        assert_eq!(FLOOR_SET_TIER_PROBS[2], [0.0, 0.0, 40.0, 50.0, 10.0]);
        assert_eq!(FLOOR_SET_TIER_PROBS[3], [0.0, 0.0, 20.0, 40.0, 40.0]);
        assert_eq!(FLOOR_SET_TIER_PROBS[4], [0.0, 0.0, 0.0, 20.0, 80.0]);
        for row in FLOOR_SET_TIER_PROBS {
            assert_eq!(row.iter().sum::<f32>(), 100.0);
        }
    }

    /// 固定种子的抽取序列钉死：同种子完全一致，且与首次实现时记录的
    /// 序列逐项相同（防回归；序列本身即本工程的确定性契约）。
    #[test]
    fn fixed_seed_draw_sequence_pinned() {
        let mut r = rng(20260813);
        let mut generator = Generator::new(&mut r);
        let kinds: Vec<ItemKind> = (0..12).map(|_| generator.random(&mut r, 1).kind).collect();

        // 同种子重放必须逐项一致
        let mut r2 = rng(20260813);
        let mut generator2 = Generator::new(&mut r2);
        let kinds2: Vec<ItemKind> = (0..12)
            .map(|_| generator2.random(&mut r2, 1).kind)
            .collect();
        assert_eq!(kinds, kinds2, "同种子抽取序列必须可复现");

        let expected: [ItemKind; 12] = [
            ItemKind::Artifact(ArtifactKind::AlchemistsToolkit),
            ItemKind::Stone(StoneKind::Shock),
            ItemKind::Scroll(ScrollKind::Teleportation),
            ItemKind::Gold,
            ItemKind::Seed(SeedKind::Sorrowmoss),
            ItemKind::Potion(PotionKind::Haste),
            ItemKind::Scroll(ScrollKind::MagicMapping),
            ItemKind::Potion(PotionKind::MindVision),
            ItemKind::Gold,
            ItemKind::Wand(WandKind::PrismaticLight),
            ItemKind::Gold,
            ItemKind::Gold,
        ];
        assert_eq!(kinds, expected, "钉死序列变动说明抽取语义被改动");
    }

    /// deck 递减：食物牌堆 {口粮 4, 馅饼 1}——每 5 抽恰好 4 口粮 + 1 馅饼，
    /// 抽空自动补牌进入下一轮，神秘肉（权重 0）永不出现（L541）。
    #[test]
    fn food_deck_decrements_and_replenishes() {
        let mut r = rng(7);
        let mut generator = Generator::new(&mut r);
        for round in 0..4 {
            let mut counts: HashMap<ItemKind, i32> = HashMap::new();
            for _ in 0..5 {
                let item = generator.random_category(Category::Food, &mut r, 1);
                *counts.entry(item.kind).or_insert(0) += 1;
            }
            assert_eq!(
                counts.get(&ItemKind::Food(FoodKind::Ration)),
                Some(&4),
                "第 {round} 轮：每轮恰 4 份口粮"
            );
            assert_eq!(
                counts.get(&ItemKind::Food(FoodKind::Pasty)),
                Some(&1),
                "第 {round} 轮：每轮恰 1 个馅饼"
            );
            assert_eq!(counts.get(&ItemKind::Food(FoodKind::MysteryMeat)), None);
        }
    }

    /// deck 递减：符石牌堆各 2 张——20 抽内每种中段符石恰好 2 次，
    /// 附魔石/重铸石（权重 0）永不出现（L394）。
    #[test]
    fn stone_deck_draws_each_exactly_twice_per_cycle() {
        let mut r = rng(11);
        let mut generator = Generator::new(&mut r);
        let mut counts: HashMap<ItemKind, i32> = HashMap::new();
        for _ in 0..20 {
            let item = generator.random_category(Category::Stone, &mut r, 1);
            *counts.entry(item.kind).or_insert(0) += 1;
        }
        for stone in &StoneKind::ALL[1..11] {
            assert_eq!(
                counts.get(&ItemKind::Stone(*stone)),
                Some(&2),
                "{stone:?} 每轮恰 2 次"
            );
        }
        assert_eq!(counts.get(&ItemKind::Stone(StoneKind::Enchantment)), None);
        assert_eq!(counts.get(&ItemKind::Stone(StoneKind::Augmentation)), None);
        // 抽空后第 21 抽触发重置补牌，不 panic
        let _ = generator.random_category(Category::Stone, &mut r, 1);
    }

    /// 药水双牌副：一副抽空（和 15）后交替到另一副（L647-L649）；
    /// 力量药水权重恒 0 永不从掉落表出现（L342-L343）。
    #[test]
    fn potion_deck_alternates_between_two_probs() {
        let mut r = rng(13);
        let mut generator = Generator::new(&mut r);
        let potion_index = Category::Potion as usize;
        let initial_deck2 = generator.states[potion_index].using_2nd_probs;
        for _ in 0..15 {
            let item = generator.random_category(Category::Potion, &mut r, 1);
            assert_ne!(
                item.kind,
                ItemKind::Potion(PotionKind::Strength),
                "力量药水不从掉落表出（L330 注释：由 posNeeded 保底投放）"
            );
        }
        // 第 15 抽抽走最后一张，第 16 抽触发 reset → 牌副翻转
        let _ = generator.random_category(Category::Potion, &mut r, 1);
        assert_ne!(
            generator.states[potion_index].using_2nd_probs, initial_deck2,
            "抽空后必须交替到另一副内牌"
        );
    }

    /// 类目私有流：类目内抽取序列只取决于 `seed + dropped + probs`，
    /// 与外部随机流的中途消耗无关（L711-L727 的重放语义）。
    #[test]
    fn deck_draws_are_independent_of_outer_rng() {
        let mut setup_rng = rng(17);
        let generator = Generator::new(&mut setup_rng);

        let mut g1 = generator.clone();
        let mut r1 = rng(100);
        let seq1: Vec<ItemKind> = (0..10)
            .map(|_| g1.random_category(Category::Scroll, &mut r1, 1).kind)
            .collect();

        let mut g2 = generator;
        let mut r2 = rng(9999);
        let seq2: Vec<ItemKind> = (0..10)
            .map(|i| {
                // 中途大量消耗外部流，卷轴序列不得受扰
                for _ in 0..(i * 7) {
                    let _ = float(&mut r2);
                }
                g2.random_category(Category::Scroll, &mut r2, 1).kind
            })
            .collect();

        assert_eq!(seq1, seq2, "类目私有流必须屏蔽外部随机时序差异");
    }

    /// 种子类目经 `random()` 走默认表（L684-L688）：不动 SEED 的 deck 状态。
    #[test]
    fn seed_category_via_random_uses_defaults() {
        let mut r = rng(23);
        let mut generator = Generator::new(&mut r);
        let seed_index = Category::Seed as usize;
        let probs_before = generator.states[seed_index].probs.clone();

        // 只留 SEED 类目权重，强制 random() 选中它
        generator.category_probs = [0.0; CATEGORY_COUNT];
        generator.category_probs[seed_index] = 1.0;
        let item = generator.random(&mut r, 1);

        assert!(matches!(item.kind, ItemKind::Seed(_)));
        assert_eq!(
            generator.states[seed_index].probs, probs_before,
            "randomUsingDefaults 不得消耗 SEED 牌堆"
        );
        assert_eq!(generator.states[seed_index].dropped, 0, "私有流不得前进");
    }

    /// 类目牌堆抽空换副（L677-L681）：35 抽后和为 0，第 36 抽翻转牌副。
    #[test]
    fn category_deck_flips_after_35_draws() {
        let mut r = rng(29);
        let mut generator = Generator::new(&mut r);
        let first_deck = generator.using_first_deck;
        for _ in 0..35 {
            let _ = generator.random(&mut r, 1);
        }
        let remaining: f32 = generator.category_probs.iter().sum();
        assert_eq!(remaining, 0.0, "35 张类目牌应恰好抽空");
        let _ = generator.random(&mut r, 1);
        assert_ne!(
            generator.using_first_deck, first_deck,
            "抽空后必须换另一副类目牌"
        );
    }

    /// 神器唯一性（L855-L879）：权重和 11，抽 11 次各不相同且不含 0 权重的
    /// 暗影披风/圣典；第 12 次起耗尽返回 `None`，永不补牌。
    #[test]
    fn artifacts_are_unique_and_never_replenish() {
        let mut r = rng(31);
        let mut generator = Generator::new(&mut r);
        let mut seen = Vec::new();
        for _ in 0..11 {
            let item = generator.random_artifact(&mut r).expect("前 11 次必有神器");
            let ItemKind::Artifact(kind) = item.kind else {
                panic!("神器类目抽出了 {:?}", item.kind);
            };
            assert!(!seen.contains(&kind), "{kind:?} 重复掉落");
            assert_ne!(kind, ArtifactKind::CloakOfShadows, "权重 0（L575）");
            assert_ne!(kind, ArtifactKind::HolyTome, "权重 0（L575）");
            seen.push(kind);
        }
        assert_eq!(generator.random_artifact(&mut r), None, "第 12 次应耗尽");
        assert_eq!(generator.random_artifact(&mut r), None, "神器牌堆永不重置");
        // 经 random_category 抽神器 → 耗尽回退为戒指（L708-L709）
        let fallback = generator.random_category(Category::Artifact, &mut r, 1);
        assert!(matches!(fallback.kind, ItemKind::Ring(_)));
    }

    /// `removeArtifact`（L881-L890）：划掉后不再掉落；重复划掉返回 false。
    #[test]
    fn remove_artifact_blocks_future_drops() {
        let mut r = rng(37);
        let mut generator = Generator::new(&mut r);
        assert!(generator.remove_artifact(ArtifactKind::HornOfPlenty));
        assert!(
            !generator.remove_artifact(ArtifactKind::HornOfPlenty),
            "已无余量"
        );
        assert!(
            !generator.remove_artifact(ArtifactKind::CloakOfShadows),
            "权重本就是 0"
        );
        for _ in 0..10 {
            if let Some(item) = generator.random_artifact(&mut r) {
                assert_ne!(item.kind, ItemKind::Artifact(ArtifactKind::HornOfPlenty));
            }
        }
    }

    /// `undoDrop`（L662-L673）：抽走一件后洗回，牌堆余量恢复。
    #[test]
    fn undo_drop_restores_deck_weight() {
        let mut r = rng(41);
        let mut generator = Generator::new(&mut r);
        let wand_index = Category::Wand as usize;
        let before = generator.states[wand_index].probs.clone();
        let item = generator.random_category(Category::Wand, &mut r, 1);
        let sum_after: f32 = generator.states[wand_index].probs.iter().sum();
        assert_eq!(sum_after, before.iter().sum::<f32>() - 1.0, "抽走应扣 1");
        generator.undo_drop(item.kind);
        assert_eq!(
            generator.states[wand_index].probs, before,
            "洗回后余量应复原"
        );
    }

    /// floor set 路由（L613-L619/L808-L819）：floor set 0 无 tier-1 权重，
    /// 武器只出 tier ≥ 2；floor set 4 只出 tier 4/5。护甲只出普通 5 阶
    /// （职业甲权重 0，L490）。越界 floor set 取边界（`GameMath.gate`）。
    #[test]
    fn floor_set_gates_weapon_and_armor_tiers() {
        let mut r = rng(43);
        let mut generator = Generator::new(&mut r);
        for _ in 0..60 {
            let item = generator.random_weapon(0, false, &mut r);
            let ItemKind::Weapon(w) = item.kind else {
                panic!("武器类目抽出了 {:?}", item.kind);
            };
            assert!(w.tier() >= 2, "floor set 0 的 tier-1 权重为 0（L614）");
        }
        for _ in 0..60 {
            let item = generator.random_weapon(9, false, &mut r); // gate 到 4
            let ItemKind::Weapon(w) = item.kind else {
                panic!("武器类目抽出了 {:?}", item.kind);
            };
            assert!(w.tier() >= 4, "floor set 4 只有 tier 4/5 权重（L618）");
        }
        for _ in 0..60 {
            let item = generator.random_armor(0, &mut r);
            let ItemKind::Armor(a) = item.kind else {
                panic!("护甲类目抽出了 {:?}", item.kind);
            };
            assert!(
                matches!(
                    a,
                    ArmorKind::Cloth
                        | ArmorKind::Leather
                        | ArmorKind::Mail
                        | ArmorKind::Scale
                        | ArmorKind::Plate
                ),
                "职业甲不参与随机掉落"
            );
        }
        for _ in 0..60 {
            let item = generator.random_missile(0, false, &mut r);
            let ItemKind::Missile(m) = item.kind else {
                panic!("投掷类目抽出了 {:?}", item.kind);
            };
            assert!(m.tier() >= 2, "floor set 0 的 tier-1 权重为 0");
        }
    }

    /// 金币数额（`Gold.java` L91）：`IntRange(30 + depth*10, 60 + depth*20)`。
    #[test]
    fn gold_quantity_scales_with_depth() {
        let mut r = rng(47);
        let mut generator = Generator::new(&mut r);
        for depth in [1, 5, 20] {
            let (min, max) = (30 + depth * 10, 60 + depth * 20);
            for _ in 0..50 {
                let item = generator.random_category(Category::Gold, &mut r, depth);
                assert_eq!(item.kind, ItemKind::Gold);
                assert!(
                    (min..=max).contains(&item.quantity),
                    "depth {depth}：{} ∉ [{min}, {max}]",
                    item.quantity
                );
            }
        }
    }

    /// 实例掷取：装备等级域 0..=2（Weapon.java L419-L430），投掷默认数量，
    /// 法杖/戒指等级域 0..=2（Wand.java L546-L557）。
    #[test]
    fn rolled_instances_stay_in_java_ranges() {
        let mut r = rng(53);
        for _ in 0..200 {
            let weapon = roll_item(ItemKind::Weapon(MeleeWeaponKind::Sword), &mut r, 1);
            assert!((0..=2).contains(&weapon.level));
            assert_eq!(weapon.quantity, 1);

            let missile = roll_item(ItemKind::Missile(MissileWeaponKind::Shuriken), &mut r, 1);
            assert_eq!(missile.quantity, 3, "MissileWeapon.java L350-L352");

            let wand = roll_item(ItemKind::Wand(WandKind::Frost), &mut r, 1);
            assert!((0..=2).contains(&wand.level));

            let potion = roll_item(ItemKind::Potion(PotionKind::Healing), &mut r, 1);
            assert_eq!(
                (potion.level, potion.quantity, potion.cursed),
                (0, 1, false)
            );
        }
        // 30% 诅咒率的量级 sanity（Weapon.java L436-L441）
        let mut cursed = 0;
        for _ in 0..1000 {
            if roll_item(ItemKind::Ring(RingKind::Haste), &mut r, 1).cursed {
                cursed += 1;
            }
        }
        assert!(
            (200..=400).contains(&cursed),
            "诅咒率应在 30% 附近，实测 {cursed}/1000"
        );
    }
}
