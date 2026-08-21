//! 物品数据域一期（docs/plans/21）：掉落表（`Generator`）与鉴定系统底座
//! （外观洗牌）。纯逻辑 + 数据表，不依赖 Bevy `World`，不接背包/UI/掉落钩子
//! （M5 集成）；随机源一律显式 `&mut impl Rng`（docs/plans/01 · 确定性）。
//!
//! 对照 Java：`items/Generator.java`（类目权重 + deck 递减补充）、
//! `items/Item.java`（实例字段与堆叠）、`items/ItemStatusHandler.java`
//! （未鉴定外观 ↔ 种类的每局洗牌双射）。

pub mod generator;
pub mod identification;
pub mod item;
pub mod kinds;
pub mod random;

pub use generator::{Category, Generator, roll_item};
pub use identification::{
    ItemStatusHandler, PotionColor, PotionStatusHandler, ScrollRune, ScrollStatusHandler,
};
pub use item::Item;
pub use kinds::ItemKind;
pub use random::ItemRng;
