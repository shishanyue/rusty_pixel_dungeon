//! `ItemKind` 体系：一期覆盖 `Generator.Category` 引用到的全部类目
//! （`Generator.java` L325-L600 各 `classes` 数组，子枚举 `ALL`/`TIERn`
//! 常量表**与 Java 数组同序**——deck 抽取下标即数组下标）。
//!
//! 一期只含种类与基础字段（stackable/默认数量/tier/基准价）；
//! 使用效果、食物饱食度、法杖充能、附魔/铭刻等属效果域（M4+），字段留 TODO。

/// 物品种类（顶层按 `Generator.Category` 的 superClass 分组）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// 金币（`Gold.java`；quantity 即金额）。
    Gold,
    /// 食物（`food/`）。
    Food(FoodKind),
    /// 药水（`potions/`，未鉴定时以颜色示人）。
    Potion(PotionKind),
    /// 种子（`plants/` 各 `Plant.Seed`）。
    Seed(SeedKind),
    /// 卷轴（`scrolls/`，未鉴定时以符文示人）。
    Scroll(ScrollKind),
    /// 符石（`stones/`）。
    Stone(StoneKind),
    /// 法杖（`wands/`）。
    Wand(WandKind),
    /// 近战武器（`weapon/melee/`，tier 1-5）。
    Weapon(MeleeWeaponKind),
    /// 护甲（`armor/`，普通 5 阶 + 职业甲）。
    Armor(ArmorKind),
    /// 投掷武器（`weapon/missiles/`，tier 1-5）。
    Missile(MissileWeaponKind),
    /// 戒指（`rings/`）。
    Ring(RingKind),
    /// 神器（`artifacts/`，一局唯一）。
    Artifact(ArtifactKind),
    /// 饰品（`trinkets/`，同时只能持有一个）。
    Trinket(TrinketKind),
}

impl ItemKind {
    /// 可堆叠位（`Item.java` L80 默认 false；覆写：`Gold.java` L43、
    /// `Food.java` L54、`Potion.java` L145、`Scroll.java` L100、
    /// `Plant.java` L137、`Runestone.java` L36、`MissileWeapon.java` L66）。
    #[must_use]
    pub const fn stackable(self) -> bool {
        matches!(
            self,
            Self::Gold
                | Self::Food(_)
                | Self::Potion(_)
                | Self::Seed(_)
                | Self::Scroll(_)
                | Self::Stone(_)
                | Self::Missile(_)
        )
    }

    /// 新实例默认数量（`Item.java` L81 `quantity = 1`；投掷武器构造时
    /// `quantity = defaultQuantity()`：`MissileWeapon.java` L67/L350-L352 为 3，
    /// `Dart.java` L257-L259 为 2）。
    #[must_use]
    pub const fn default_quantity(self) -> i32 {
        match self {
            Self::Missile(m) => m.default_quantity(),
            _ => 1,
        }
    }

    /// 单件基准价（金币）：各类 `value()` 在 quantity=1、无附魔/诅咒/等级修正
    /// 下的基线。修正公式（诅咒减半、等级翻倍等，如 `MeleeWeapon.java`
    /// L395-L407）属效果域 TODO。
    ///
    /// 来源：`Food.java` L140-L142、`Pasty.java` L227-L229、
    /// `MysteryMeat.java` L50-L52、`Potion.java` L440-L442、
    /// `Scroll.java` L278-L280、`Plant.java` L210-L212、
    /// `Runestone.java` L79-L81、`Wand.java` L576-L577、`Ring.java` L293-L294、
    /// `MeleeWeapon.java` L395-L396、`MissileWeapon.java` L696-L697、
    /// `Armor.java` L706-L709、`Artifact.java` L229-L230；
    /// 金币本身即价值（`Gold.java` L60-L77 拾取直接入 `Dungeon.gold`）；
    /// 饰品无金币价（`Item.java` L548-L550 默认 0，以炼金能量计价，TODO 效果域）。
    #[must_use]
    pub const fn base_value(self) -> i32 {
        match self {
            Self::Gold => 1,
            Self::Food(f) => f.value(),
            Self::Potion(_) | Self::Scroll(_) => 30,
            Self::Seed(_) => 10,
            Self::Stone(_) => 15,
            Self::Wand(_) | Self::Ring(_) => 75,
            Self::Weapon(w) => 20 * w.tier(),
            Self::Armor(a) => 20 * a.tier(),
            Self::Missile(m) => 5 * m.tier(),
            Self::Artifact(_) => 100,
            Self::Trinket(_) => 0,
        }
    }
}

/// 食物（`Generator.java` L537-L540 `FOOD.classes` 序）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoodKind {
    /// 口粮（`Food.class` 本体，L538）。
    Ration,
    /// 馅饼（L539；节日变体属表现层）。
    Pasty,
    /// 神秘肉（L540；prob 0，只从钓鱼/怪物掉落）。
    MysteryMeat,
}

impl FoodKind {
    /// 与 `FOOD.classes`（L537-L540）同序。
    pub const ALL: [Self; 3] = [Self::Ration, Self::Pasty, Self::MysteryMeat];

    /// 单件价格（`Food.java` L140-L142、`Pasty.java` L227-L229、
    /// `MysteryMeat.java` L50-L52）。饱食度（`energy`）属效果域 TODO。
    #[must_use]
    pub const fn value(self) -> i32 {
        match self {
            Self::Ration => 10,
            Self::Pasty => 20,
            Self::MysteryMeat => 5,
        }
    }
}

/// 药水（`Generator.java` L329-L341 `POTION.classes` 序）。效果 TODO（M4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PotionKind {
    /// 力量药水（L330；每章保底 2 瓶，见 `Dungeon.posNeeded()`）。
    Strength,
    /// 治疗药水（L331）。
    Healing,
    /// 心灵视界药水（L332）。
    MindVision,
    /// 冰霜药水（L333）。
    Frost,
    /// 液火药水（L334）。
    LiquidFlame,
    /// 毒气药水（L335）。
    ToxicGas,
    /// 急速药水（L336）。
    Haste,
    /// 隐身药水（L337）。
    Invisibility,
    /// 漂浮药水（L338）。
    Levitation,
    /// 麻痹瓦斯药水（L339）。
    ParalyticGas,
    /// 净化药水（L340）。
    Purity,
    /// 经验药水（L341）。
    Experience,
}

impl PotionKind {
    /// 与 `POTION.classes`（L329-L341）同序。
    pub const ALL: [Self; 12] = [
        Self::Strength,
        Self::Healing,
        Self::MindVision,
        Self::Frost,
        Self::LiquidFlame,
        Self::ToxicGas,
        Self::Haste,
        Self::Invisibility,
        Self::Levitation,
        Self::ParalyticGas,
        Self::Purity,
        Self::Experience,
    ];
}

/// 种子（`Generator.java` L346-L358 `SEED.classes` 序）。效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeedKind {
    /// 腐浆果（L347；任务物品，prob 0）。
    Rotberry,
    /// 太阳草（L348）。
    Sungrass,
    /// 隐没叶（L349）。
    Fadeleaf,
    /// 冰帽草（L350）。
    Icecap,
    /// 火焰花（L351）。
    Firebloom,
    /// 哀伤苔（L352）。
    Sorrowmoss,
    /// 疾蓟草（L353）。
    Swiftthistle,
    /// 目盲草（L354）。
    Blindweed,
    /// 风暴藤（L355）。
    Stormvine,
    /// 地缚根（L356）。
    Earthroot,
    /// 法皇草（L357）。
    Mageroyal,
    /// 星辰花（L358）。
    Starflower,
}

impl SeedKind {
    /// 与 `SEED.classes`（L346-L358）同序。
    pub const ALL: [Self; 12] = [
        Self::Rotberry,
        Self::Sungrass,
        Self::Fadeleaf,
        Self::Icecap,
        Self::Firebloom,
        Self::Sorrowmoss,
        Self::Swiftthistle,
        Self::Blindweed,
        Self::Stormvine,
        Self::Earthroot,
        Self::Mageroyal,
        Self::Starflower,
    ];
}

/// 卷轴（`Generator.java` L362-L375 `SCROLL.classes` 序）。效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollKind {
    /// 升级卷轴（L363；每章保底 3 张，见 `Dungeon.souNeeded()`）。
    Upgrade,
    /// 鉴定卷轴（L364）。
    Identify,
    /// 祛咒卷轴（L365）。
    RemoveCurse,
    /// 镜像卷轴（L366）。
    MirrorImage,
    /// 充能卷轴（L367）。
    Recharging,
    /// 传送卷轴（L368）。
    Teleportation,
    /// 摇篮曲卷轴（L369）。
    Lullaby,
    /// 魔法地图卷轴（L370）。
    MagicMapping,
    /// 激怒卷轴（L371）。
    Rage,
    /// 天罚卷轴（L372）。
    Retribution,
    /// 恐惧卷轴（L373）。
    Terror,
    /// 嬗变卷轴（L374）。
    Transmutation,
}

impl ScrollKind {
    /// 与 `SCROLL.classes`（L362-L375）同序。
    pub const ALL: [Self; 12] = [
        Self::Upgrade,
        Self::Identify,
        Self::RemoveCurse,
        Self::MirrorImage,
        Self::Recharging,
        Self::Teleportation,
        Self::Lullaby,
        Self::MagicMapping,
        Self::Rage,
        Self::Retribution,
        Self::Terror,
        Self::Transmutation,
    ];
}

/// 符石（`Generator.java` L380-L393 `STONE.classes` 序）。效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoneKind {
    /// 附魔石（L381；6-19 层保底掉 1，prob 0）。
    Enchantment,
    /// 直觉石（L382；1-3 层额外掉 1）。
    Intuition,
    /// 侦测魔法石（L383）。
    DetectMagic,
    /// 羊群石（L384）。
    Flock,
    /// 电击石（L385）。
    Shock,
    /// 闪现石（L386）。
    Blink,
    /// 沉睡石（L387）。
    DeepSleep,
    /// 透视石（L388）。
    Clairvoyance,
    /// 挑衅石（L389）。
    Aggression,
    /// 爆破石（L390）。
    Blast,
    /// 恐惧石（L391）。
    Fear,
    /// 重铸石（L392；每层商店卖 1，prob 0）。
    Augmentation,
}

impl StoneKind {
    /// 与 `STONE.classes`（L380-L393）同序。
    pub const ALL: [Self; 12] = [
        Self::Enchantment,
        Self::Intuition,
        Self::DetectMagic,
        Self::Flock,
        Self::Shock,
        Self::Blink,
        Self::DeepSleep,
        Self::Clairvoyance,
        Self::Aggression,
        Self::Blast,
        Self::Fear,
        Self::Augmentation,
    ];
}

/// 法杖（`Generator.java` L397-L410 `WAND.classes` 序，共 13 支）。
/// 伤害/充能等效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WandKind {
    /// 魔法飞弹法杖（L398）。
    MagicMissile,
    /// 闪电法杖（L399）。
    Lightning,
    /// 解离法杖（L400）。
    Disintegration,
    /// 火焰冲击法杖（L401）。
    Fireblast,
    /// 腐蚀法杖（L402）。
    Corrosion,
    /// 冲击波法杖（L403）。
    BlastWave,
    /// 活化大地法杖（L404）。
    LivingEarth,
    /// 寒冰法杖（L405）。
    Frost,
    /// 棱彩光辉法杖（L406）。
    PrismaticLight,
    /// 守卫法杖（L407）。
    Warding,
    /// 输血法杖（L408）。
    Transfusion,
    /// 腐化法杖（L409）。
    Corruption,
    /// 再生法杖（L410）。
    Regrowth,
}

impl WandKind {
    /// 与 `WAND.classes`（L397-L410）同序。
    pub const ALL: [Self; 13] = [
        Self::MagicMissile,
        Self::Lightning,
        Self::Disintegration,
        Self::Fireblast,
        Self::Corrosion,
        Self::BlastWave,
        Self::LivingEarth,
        Self::Frost,
        Self::PrismaticLight,
        Self::Warding,
        Self::Transfusion,
        Self::Corruption,
        Self::Regrowth,
    ];
}

/// 近战武器（`Generator.java` L418-L474 五个 tier 类目的 `classes` 并集）。
/// 伤害域/力量需求/特性等效果 TODO；tier 划分见 [`Self::tier`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeleeWeaponKind {
    // ---- tier 1（WEP_T1.classes，L418-L425）----
    /// 破旧短剑（L419）。
    WornShortsword,
    /// 法师之杖（L420；prob 0，法师初始装备）。
    MagesStaff,
    /// 匕首（L421）。
    Dagger,
    /// 拳套（L422）。
    Gloves,
    /// 刺剑（L423）。
    Rapier,
    /// 短棍（L424）。
    Cudgel,
    // ---- tier 2（WEP_T2.classes，L429-L437）----
    /// 短剑（L430）。
    Shortsword,
    /// 手斧（L431）。
    HandAxe,
    /// 长矛（L432）。
    Spear,
    /// 木棍（L433）。
    Quarterstaff,
    /// 短刀（L434）。
    Dirk,
    /// 镰刀（L435）。
    Sickle,
    /// 鹤嘴锄（L436；prob 0，矿工任务物品）。
    Pickaxe,
    // ---- tier 3（WEP_T3.classes，L441-L448）----
    /// 长剑（L442）。
    Sword,
    /// 钉头锤（L443）。
    Mace,
    /// 弯刀（L444）。
    Scimitar,
    /// 圆盾（L445）。
    RoundShield,
    /// 钗（L446）。
    Sai,
    /// 长鞭（L447）。
    Whip,
    // ---- tier 4（WEP_T4.classes，L452-L460）----
    /// 巨剑（L453）。
    Longsword,
    /// 战斧（L454）。
    BattleAxe,
    /// 连枷（L455）。
    Flail,
    /// 符文之刃（L456）。
    RunicBlade,
    /// 刺客之刃（L457）。
    AssassinsBlade,
    /// 弩（L458）。
    Crossbow,
    /// 武士刀（L459）。
    Katana,
    // ---- tier 5（WEP_T5.classes，L464-L472）----
    /// 大剑（L465）。
    Greatsword,
    /// 战锤（L466）。
    WarHammer,
    /// 长柄刀（L467）。
    Glaive,
    /// 巨斧（L468）。
    Greataxe,
    /// 巨盾（L469）。
    Greatshield,
    /// 臂铠（L470）。
    Gauntlet,
    /// 战镰（L471）。
    WarScythe,
}

impl MeleeWeaponKind {
    /// 与 `WEP_T1.classes`（L418-L425）同序。
    pub const TIER1: [Self; 6] = [
        Self::WornShortsword,
        Self::MagesStaff,
        Self::Dagger,
        Self::Gloves,
        Self::Rapier,
        Self::Cudgel,
    ];
    /// 与 `WEP_T2.classes`（L429-L437）同序。
    pub const TIER2: [Self; 7] = [
        Self::Shortsword,
        Self::HandAxe,
        Self::Spear,
        Self::Quarterstaff,
        Self::Dirk,
        Self::Sickle,
        Self::Pickaxe,
    ];
    /// 与 `WEP_T3.classes`（L441-L448）同序。
    pub const TIER3: [Self; 6] = [
        Self::Sword,
        Self::Mace,
        Self::Scimitar,
        Self::RoundShield,
        Self::Sai,
        Self::Whip,
    ];
    /// 与 `WEP_T4.classes`（L452-L460）同序。
    pub const TIER4: [Self; 7] = [
        Self::Longsword,
        Self::BattleAxe,
        Self::Flail,
        Self::RunicBlade,
        Self::AssassinsBlade,
        Self::Crossbow,
        Self::Katana,
    ];
    /// 与 `WEP_T5.classes`（L464-L472）同序。
    pub const TIER5: [Self; 7] = [
        Self::Greatsword,
        Self::WarHammer,
        Self::Glaive,
        Self::Greataxe,
        Self::Greatshield,
        Self::Gauntlet,
        Self::WarScythe,
    ];

    /// 武器阶级（各类构造器 `tier` 字段，与 `Generator` 的 tier 类目一致：
    /// 如 `WornShortsword.java` L37、`MagesStaff.java` L76、`Pickaxe.java` L58）。
    #[must_use]
    pub const fn tier(self) -> i32 {
        match self {
            Self::WornShortsword
            | Self::MagesStaff
            | Self::Dagger
            | Self::Gloves
            | Self::Rapier
            | Self::Cudgel => 1,
            Self::Shortsword
            | Self::HandAxe
            | Self::Spear
            | Self::Quarterstaff
            | Self::Dirk
            | Self::Sickle
            | Self::Pickaxe => 2,
            Self::Sword
            | Self::Mace
            | Self::Scimitar
            | Self::RoundShield
            | Self::Sai
            | Self::Whip => 3,
            Self::Longsword
            | Self::BattleAxe
            | Self::Flail
            | Self::RunicBlade
            | Self::AssassinsBlade
            | Self::Crossbow
            | Self::Katana => 4,
            Self::Greatsword
            | Self::WarHammer
            | Self::Glaive
            | Self::Greataxe
            | Self::Greatshield
            | Self::Gauntlet
            | Self::WarScythe => 5,
        }
    }
}

/// 护甲（`Generator.java` L477-L489 `ARMOR.classes` 序）。
/// 防御域/力量需求等效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmorKind {
    /// 布甲（L478，1 阶）。
    Cloth,
    /// 皮甲（L479，2 阶）。
    Leather,
    /// 锁甲（L480，3 阶）。
    Mail,
    /// 鳞甲（L481，4 阶）。
    Scale,
    /// 板甲（L482，5 阶）。
    Plate,
    /// 战士职业甲（L483；prob 0，转职获得）。
    Warrior,
    /// 法师职业甲（L484；prob 0）。
    Mage,
    /// 盗贼职业甲（L485；prob 0）。
    Rogue,
    /// 女猎手职业甲（L486；prob 0）。
    Huntress,
    /// 决斗家职业甲（L487；prob 0）。
    Duelist,
    /// 牧师职业甲（L488；prob 0）。
    Cleric,
}

impl ArmorKind {
    /// 与 `ARMOR.classes`（L477-L489）同序。
    pub const ALL: [Self; 11] = [
        Self::Cloth,
        Self::Leather,
        Self::Mail,
        Self::Scale,
        Self::Plate,
        Self::Warrior,
        Self::Mage,
        Self::Rogue,
        Self::Huntress,
        Self::Duelist,
        Self::Cleric,
    ];

    /// 护甲阶级（`ClothArmor.java` L35 起各构造器 `super(n)`；
    /// 职业甲统一 5 阶：`ClassArmor.java` L68）。
    #[must_use]
    pub const fn tier(self) -> i32 {
        match self {
            Self::Cloth => 1,
            Self::Leather => 2,
            Self::Mail => 3,
            Self::Scale => 4,
            Self::Plate
            | Self::Warrior
            | Self::Mage
            | Self::Rogue
            | Self::Huntress
            | Self::Duelist
            | Self::Cleric => 5,
        }
    }
}

/// 投掷武器（`Generator.java` L496-L535 五个 tier 类目的 `classes` 并集）。
/// 伤害/耐久等效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MissileWeaponKind {
    // ---- tier 1（MIS_T1.classes，L496-L501）----
    /// 投石（L497）。
    ThrowingStone,
    /// 飞刀（L498）。
    ThrowingKnife,
    /// 飞钉（L499）。
    ThrowingSpike,
    /// 飞镖（L500；prob 0，与弩联动）。
    Dart,
    // ---- tier 2（MIS_T2.classes，L505-L509）----
    /// 鱼叉（L506）。
    FishingSpear,
    /// 投掷棒（L507）。
    ThrowingClub,
    /// 手里剑（L508）。
    Shuriken,
    // ---- tier 3（MIS_T3.classes，L513-L517）----
    /// 标枪（L514）。
    ThrowingSpear,
    /// 苦无（L515）。
    Kunai,
    /// 流星锤（L516）。
    Bolas,
    // ---- tier 4（MIS_T4.classes，L521-L525）----
    /// 重标枪（L522）。
    Javelin,
    /// 战斧镖（L523）。
    Tomahawk,
    /// 重回旋镖（L524）。
    HeavyBoomerang,
    // ---- tier 5（MIS_T5.classes，L529-L533）----
    /// 三叉戟（L530）。
    Trident,
    /// 投掷战锤（L531）。
    ThrowingHammer,
    /// 力场方块（L532）。
    ForceCube,
}

impl MissileWeaponKind {
    /// 与 `MIS_T1.classes`（L496-L501）同序。
    pub const TIER1: [Self; 4] = [
        Self::ThrowingStone,
        Self::ThrowingKnife,
        Self::ThrowingSpike,
        Self::Dart,
    ];
    /// 与 `MIS_T2.classes`（L505-L509）同序。
    pub const TIER2: [Self; 3] = [Self::FishingSpear, Self::ThrowingClub, Self::Shuriken];
    /// 与 `MIS_T3.classes`（L513-L517）同序。
    pub const TIER3: [Self; 3] = [Self::ThrowingSpear, Self::Kunai, Self::Bolas];
    /// 与 `MIS_T4.classes`（L521-L525）同序。
    pub const TIER4: [Self; 3] = [Self::Javelin, Self::Tomahawk, Self::HeavyBoomerang];
    /// 与 `MIS_T5.classes`（L529-L533）同序。
    pub const TIER5: [Self; 3] = [Self::Trident, Self::ThrowingHammer, Self::ForceCube];

    /// 投掷武器阶级（各类构造器 `tier` 字段，如 `ThrowingStone.java` L36、
    /// `Dart.java` L57）。
    #[must_use]
    pub const fn tier(self) -> i32 {
        match self {
            Self::ThrowingStone | Self::ThrowingKnife | Self::ThrowingSpike | Self::Dart => 1,
            Self::FishingSpear | Self::ThrowingClub | Self::Shuriken => 2,
            Self::ThrowingSpear | Self::Kunai | Self::Bolas => 3,
            Self::Javelin | Self::Tomahawk | Self::HeavyBoomerang => 4,
            Self::Trident | Self::ThrowingHammer | Self::ForceCube => 5,
        }
    }

    /// 每摞默认数量（`MissileWeapon.java` L350-L352 为 3；
    /// `Dart.java` L257-L259 覆写为 2）。
    #[must_use]
    pub const fn default_quantity(self) -> i32 {
        match self {
            Self::Dart => 2,
            _ => 3,
        }
    }
}

/// 戒指（`Generator.java` L544-L556 `RING.classes` 序）。效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RingKind {
    /// 精准之戒（L545）。
    Accuracy,
    /// 奥术之戒（L546）。
    Arcana,
    /// 元素之戒（L547）。
    Elements,
    /// 能量之戒（L548）。
    Energy,
    /// 闪避之戒（L549）。
    Evasion,
    /// 蛮力之戒（L550）。
    Force,
    /// 狂怒之戒（L551）。
    Furor,
    /// 急速之戒（L552）。
    Haste,
    /// 巨力之戒（L553）。
    Might,
    /// 神射之戒（L554）。
    Sharpshooting,
    /// 坚韧之戒（L555）。
    Tenacity,
    /// 财富之戒（L556）。
    Wealth,
}

impl RingKind {
    /// 与 `RING.classes`（L544-L556）同序。
    pub const ALL: [Self; 12] = [
        Self::Accuracy,
        Self::Arcana,
        Self::Elements,
        Self::Energy,
        Self::Evasion,
        Self::Force,
        Self::Furor,
        Self::Haste,
        Self::Might,
        Self::Sharpshooting,
        Self::Tenacity,
        Self::Wealth,
    ];
}

/// 神器（`Generator.java` L560-L574 `ARTIFACT.classes` 序）。效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    /// 炼金工具箱（L561）。
    AlchemistsToolkit,
    /// 鲜血圣杯（L562）。
    ChaliceOfBlood,
    /// 暗影披风（L563；prob 0，盗贼初始神器）。
    CloakOfShadows,
    /// 干枯玫瑰（L564）。
    DriedRose,
    /// 空灵锁链（L565）。
    EtherealChains,
    /// 圣典（L566；prob 0，牧师初始神器）。
    HolyTome,
    /// 丰饶之角（L567）。
    HornOfPlenty,
    /// 盗贼大师护腕（L568）。
    MasterThievesArmband,
    /// 自然之靴（L569）。
    SandalsOfNature,
    /// 骷髅钥匙（L570）。
    SkeletonKey,
    /// 预见护符（L571）。
    TalismanOfForesight,
    /// 时守者沙漏（L572）。
    TimekeepersHourglass,
    /// 不稳定法术书（L573）。
    UnstableSpellbook,
}

impl ArtifactKind {
    /// 与 `ARTIFACT.classes`（L560-L574）同序。
    pub const ALL: [Self; 13] = [
        Self::AlchemistsToolkit,
        Self::ChaliceOfBlood,
        Self::CloakOfShadows,
        Self::DriedRose,
        Self::EtherealChains,
        Self::HolyTome,
        Self::HornOfPlenty,
        Self::MasterThievesArmband,
        Self::SandalsOfNature,
        Self::SkeletonKey,
        Self::TalismanOfForesight,
        Self::TimekeepersHourglass,
        Self::UnstableSpellbook,
    ];
}

/// 饰品（`Generator.java` L580-L598 `TRINKET.classes` 序）。效果 TODO。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrinketKind {
    /// 鼠颅骨（L581）。
    RatSkull,
    /// 羊皮纸残片（L582）。
    ParchmentScrap,
    /// 石化种子（L583）。
    PetrifiedSeed,
    /// 异域水晶（L584）。
    ExoticCrystals,
    /// 苔藓团（L585）。
    MossyClump,
    /// 次元日晷（L586）。
    DimensionalSundial,
    /// 十三叶草（L587）。
    ThirteenLeafClover,
    /// 陷阱机簧（L588）。
    TrapMechanism,
    /// 拟身兽牙（L589）。
    MimicTooth,
    /// 奇妙树脂（L590）。
    WondrousResin,
    /// 蝾螈之眼（L591）。
    EyeOfNewt,
    /// 盐块（L592）。
    SaltCube,
    /// 血之小瓶（L593）。
    VialOfBlood,
    /// 遗忘之碎片（L594）。
    ShardOfOblivion,
    /// 混沌香炉（L595）。
    ChaoticCenser,
    /// 雪貂尾羽（L596）。
    FerretTuft,
    /// 有裂纹的窥镜（L597）。
    CrackedSpyglass,
}

impl TrinketKind {
    /// 与 `TRINKET.classes`（L580-L598）同序。
    pub const ALL: [Self; 17] = [
        Self::RatSkull,
        Self::ParchmentScrap,
        Self::PetrifiedSeed,
        Self::ExoticCrystals,
        Self::MossyClump,
        Self::DimensionalSundial,
        Self::ThirteenLeafClover,
        Self::TrapMechanism,
        Self::MimicTooth,
        Self::WondrousResin,
        Self::EyeOfNewt,
        Self::SaltCube,
        Self::VialOfBlood,
        Self::ShardOfOblivion,
        Self::ChaoticCenser,
        Self::FerretTuft,
        Self::CrackedSpyglass,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 各表长度与 Java `classes` 数组逐一对应（`Generator.java` 行号见各常量文档）。
    #[test]
    fn table_lengths_match_java() {
        assert_eq!(FoodKind::ALL.len(), 3, "FOOD.classes L537-L540");
        assert_eq!(PotionKind::ALL.len(), 12, "POTION.classes L329-L341");
        assert_eq!(SeedKind::ALL.len(), 12, "SEED.classes L346-L358");
        assert_eq!(ScrollKind::ALL.len(), 12, "SCROLL.classes L362-L375");
        assert_eq!(StoneKind::ALL.len(), 12, "STONE.classes L380-L393");
        assert_eq!(WandKind::ALL.len(), 13, "WAND.classes L397-L410");
        assert_eq!(ArmorKind::ALL.len(), 11, "ARMOR.classes L477-L489");
        assert_eq!(RingKind::ALL.len(), 12, "RING.classes L544-L556");
        assert_eq!(ArtifactKind::ALL.len(), 13, "ARTIFACT.classes L560-L574");
        assert_eq!(TrinketKind::ALL.len(), 17, "TRINKET.classes L580-L598");
        let melee = MeleeWeaponKind::TIER1.len()
            + MeleeWeaponKind::TIER2.len()
            + MeleeWeaponKind::TIER3.len()
            + MeleeWeaponKind::TIER4.len()
            + MeleeWeaponKind::TIER5.len();
        assert_eq!(melee, 33, "WEP_T1..T5 共 6+7+6+7+7");
        let missiles = MissileWeaponKind::TIER1.len()
            + MissileWeaponKind::TIER2.len()
            + MissileWeaponKind::TIER3.len()
            + MissileWeaponKind::TIER4.len()
            + MissileWeaponKind::TIER5.len();
        assert_eq!(missiles, 16, "MIS_T1..T5 共 4+3+3+3+3");
    }

    /// tier 表与各类目归属一致（tier 类目下标 + 1 = tier 值）。
    #[test]
    fn tiers_match_category_grouping() {
        for w in MeleeWeaponKind::TIER1 {
            assert_eq!(w.tier(), 1, "{w:?}");
        }
        for w in MeleeWeaponKind::TIER2 {
            assert_eq!(w.tier(), 2, "{w:?}");
        }
        for w in MeleeWeaponKind::TIER3 {
            assert_eq!(w.tier(), 3, "{w:?}");
        }
        for w in MeleeWeaponKind::TIER4 {
            assert_eq!(w.tier(), 4, "{w:?}");
        }
        for w in MeleeWeaponKind::TIER5 {
            assert_eq!(w.tier(), 5, "{w:?}");
        }
        for m in MissileWeaponKind::TIER1 {
            assert_eq!(m.tier(), 1, "{m:?}");
        }
        for m in MissileWeaponKind::TIER5 {
            assert_eq!(m.tier(), 5, "{m:?}");
        }
        // 护甲 5 阶（ClothArmor..PlateArmor 构造器）+ 职业甲 5 阶（ClassArmor.java L68）
        assert_eq!(ArmorKind::Cloth.tier(), 1);
        assert_eq!(ArmorKind::Leather.tier(), 2);
        assert_eq!(ArmorKind::Mail.tier(), 3);
        assert_eq!(ArmorKind::Scale.tier(), 4);
        assert_eq!(ArmorKind::Plate.tier(), 5);
        assert_eq!(ArmorKind::Warrior.tier(), 5, "ClassArmor.java L68 super(5)");
    }

    /// stackable 位对拍（各类覆写行号见 `ItemKind::stackable` 文档）。
    #[test]
    fn stackable_flags_match_java() {
        assert!(ItemKind::Gold.stackable(), "Gold.java L43");
        assert!(
            ItemKind::Food(FoodKind::Ration).stackable(),
            "Food.java L54"
        );
        assert!(
            ItemKind::Potion(PotionKind::Healing).stackable(),
            "Potion.java L145"
        );
        assert!(
            ItemKind::Scroll(ScrollKind::Upgrade).stackable(),
            "Scroll.java L100"
        );
        assert!(
            ItemKind::Seed(SeedKind::Sungrass).stackable(),
            "Plant.java L137"
        );
        assert!(
            ItemKind::Stone(StoneKind::Intuition).stackable(),
            "Runestone.java L36"
        );
        assert!(
            ItemKind::Missile(MissileWeaponKind::Shuriken).stackable(),
            "MissileWeapon.java L66"
        );
        assert!(
            !ItemKind::Weapon(MeleeWeaponKind::Sword).stackable(),
            "Item.java L80 默认"
        );
        assert!(!ItemKind::Armor(ArmorKind::Plate).stackable());
        assert!(!ItemKind::Wand(WandKind::Frost).stackable());
        assert!(!ItemKind::Ring(RingKind::Wealth).stackable());
        assert!(!ItemKind::Artifact(ArtifactKind::DriedRose).stackable());
        assert!(!ItemKind::Trinket(TrinketKind::RatSkull).stackable());
    }

    /// 默认数量：投掷 3、飞镖 2、其余 1（`MissileWeapon.java` L350-L352、
    /// `Dart.java` L257-L259、`Item.java` L81）。
    #[test]
    fn default_quantities_match_java() {
        assert_eq!(
            ItemKind::Missile(MissileWeaponKind::Shuriken).default_quantity(),
            3
        );
        assert_eq!(
            ItemKind::Missile(MissileWeaponKind::Dart).default_quantity(),
            2
        );
        assert_eq!(ItemKind::Potion(PotionKind::Healing).default_quantity(), 1);
        assert_eq!(ItemKind::Gold.default_quantity(), 1);
    }

    /// 基准价对拍（行号见 `ItemKind::base_value` 文档）。
    #[test]
    fn base_values_match_java() {
        assert_eq!(ItemKind::Food(FoodKind::Ration).base_value(), 10);
        assert_eq!(ItemKind::Food(FoodKind::Pasty).base_value(), 20);
        assert_eq!(ItemKind::Food(FoodKind::MysteryMeat).base_value(), 5);
        assert_eq!(ItemKind::Potion(PotionKind::Strength).base_value(), 30);
        assert_eq!(ItemKind::Scroll(ScrollKind::Upgrade).base_value(), 30);
        assert_eq!(ItemKind::Seed(SeedKind::Rotberry).base_value(), 10);
        assert_eq!(ItemKind::Stone(StoneKind::Blink).base_value(), 15);
        assert_eq!(ItemKind::Wand(WandKind::MagicMissile).base_value(), 75);
        assert_eq!(ItemKind::Ring(RingKind::Might).base_value(), 75);
        assert_eq!(
            ItemKind::Weapon(MeleeWeaponKind::Greatsword).base_value(),
            100,
            "20*tier5"
        );
        assert_eq!(
            ItemKind::Armor(ArmorKind::Mail).base_value(),
            60,
            "20*tier3"
        );
        assert_eq!(
            ItemKind::Missile(MissileWeaponKind::Trident).base_value(),
            25,
            "5*tier5"
        );
        assert_eq!(
            ItemKind::Artifact(ArtifactKind::HornOfPlenty).base_value(),
            100
        );
        assert_eq!(ItemKind::Trinket(TrinketKind::SaltCube).base_value(), 0);
    }
}
