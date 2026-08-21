//! 跨域小工具。

pub mod pathfinder;
pub mod shadow_caster;

pub use pathfinder::PathFinder;
pub use shadow_caster::cast_shadow;

/// 资产枚举 → 资产路径（由 `define_asset_type!` 宏实现）
pub trait PropertyPath {
    fn get_property_path(&self) -> &'static str;
}
