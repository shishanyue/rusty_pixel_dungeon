//! 地形定义，对照 SPD `levels/Terrain.java`。
//!
//! 判别值必须与 Java 原版逐一相同（存档兼容与对拍的基础），
//! flags 映射照抄 Terrain.java 静态初始化块（v3.3.8, L83-L125）。

use bitflags::bitflags;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use strum::EnumIter;

bitflags! {
    /// 地形行为标志（FLAMABLE 沿用 SPD 原拼写，便于对照检索）
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct TerrainFlags: u8 {
        const PASSABLE     = 1 << 0;
        const LOS_BLOCKING = 1 << 1;
        const FLAMABLE     = 1 << 2;
        const SECRET       = 1 << 3;
        const SOLID        = 1 << 4;
        const AVOID        = 1 << 5;
        const LIQUID       = 1 << 6;
        const PIT          = 1 << 7;
    }
}

/// 地形类型。判别值 = SPD 的 int 常量（0..=38，23 与 31 等空位是历史原因，照抄）。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, EnumIter, IntoPrimitive, TryFromPrimitive,
)]
#[repr(u8)]
pub enum Terrain {
    Chasm = 0,
    Empty = 1,
    Grass = 2,
    EmptyWell = 3,
    #[default]
    Wall = 4,
    Door = 5,
    OpenDoor = 6,
    Entrance = 7,
    Exit = 8,
    Embers = 9,
    LockedDoor = 10,
    Pedestal = 11,
    WallDeco = 12,
    Barricade = 13,
    EmptySp = 14,
    HighGrass = 15,
    SecretDoor = 16,
    SecretTrap = 17,
    Trap = 18,
    InactiveTrap = 19,
    EmptyDeco = 20,
    LockedExit = 21,
    UnlockedExit = 22,
    /// 不可见的实体装饰（复用旧版 SIGN 的 ID）
    CustomDeco = 23,
    Well = 24,
    Statue = 25,
    StatueSp = 26,
    Bookshelf = 27,
    Alchemy = 28,
    Water = 29,
    FurrowedGrass = 30,
    CrystalDoor = 31,
    /// 不可被覆盖的普通空地，主要用于自定义视觉
    CustomDecoEmpty = 32,
    RegionDeco = 33,
    RegionDecoAlt = 34,
    MineCrystal = 35,
    MineBoulder = 36,
    EntranceSp = 37,
    /// 被骷髅钥匙锁上的门
    HeroLockedDoor = 38,
}

impl Terrain {
    /// 行为标志查表，映射照抄 SPD `Terrain.java` 静态块。
    pub const fn flags(self) -> TerrainFlags {
        use Terrain as T;
        const P: TerrainFlags = TerrainFlags::PASSABLE;
        const L: TerrainFlags = TerrainFlags::LOS_BLOCKING;
        const F: TerrainFlags = TerrainFlags::FLAMABLE;
        const SC: TerrainFlags = TerrainFlags::SECRET;
        const S: TerrainFlags = TerrainFlags::SOLID;
        const A: TerrainFlags = TerrainFlags::AVOID;

        match self {
            T::Chasm => TerrainFlags::AVOID.union(TerrainFlags::PIT),
            T::Empty
            | T::EmptyWell
            | T::Entrance
            | T::EntranceSp
            | T::Exit
            | T::Embers
            | T::Pedestal
            | T::InactiveTrap
            | T::EmptyDeco
            | T::EmptySp
            | T::UnlockedExit
            | T::CustomDecoEmpty => P,
            T::Grass | T::OpenDoor => P.union(F),
            T::Water => P.union(TerrainFlags::LIQUID),
            T::Wall | T::WallDeco | T::LockedDoor | T::HeroLockedDoor => L.union(S),
            T::Door => P.union(L).union(F).union(S),
            T::CrystalDoor
            | T::LockedExit
            | T::Alchemy
            | T::CustomDeco
            | T::Statue
            | T::StatueSp
            | T::RegionDeco
            | T::RegionDecoAlt
            | T::MineCrystal
            | T::MineBoulder => S,
            T::Barricade | T::Bookshelf => F.union(S).union(L),
            T::HighGrass | T::FurrowedGrass => P.union(L).union(F),
            T::SecretDoor => L.union(S).union(SC),
            T::SecretTrap => P.union(SC),
            T::Trap | T::Well => A,
        }
    }

    /// 发现隐藏地形：密门 → 门，暗陷阱 → 陷阱，其余原样返回。
    pub const fn discover(self) -> Terrain {
        match self {
            Terrain::SecretDoor => Terrain::Door,
            Terrain::SecretTrap => Terrain::Trap,
            other => other,
        }
    }

    pub const fn is_passable(self) -> bool {
        self.flags().contains(TerrainFlags::PASSABLE)
    }

    pub const fn is_solid(self) -> bool {
        self.flags().contains(TerrainFlags::SOLID)
    }

    pub const fn blocks_sight(self) -> bool {
        self.flags().contains(TerrainFlags::LOS_BLOCKING)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    /// 判别值与 SPD int 常量往返一致
    #[test]
    fn primitive_roundtrip() {
        for terrain in Terrain::iter() {
            let raw: u8 = terrain.into();
            assert_eq!(Terrain::try_from(raw).unwrap(), terrain);
        }
        assert_eq!(u8::from(Terrain::HeroLockedDoor), 38);
        assert_eq!(u8::from(Terrain::Water), 29);
        assert_eq!(u8::from(Terrain::CrystalDoor), 31);
        // 空位判别值不可解析
        assert!(Terrain::try_from(39u8).is_err());
    }

    /// flags 抽查，对照 Terrain.java L83-L125
    #[test]
    fn flags_match_spd() {
        use TerrainFlags as F;
        assert_eq!(Terrain::Chasm.flags(), F::AVOID | F::PIT);
        assert_eq!(
            Terrain::Door.flags(),
            F::PASSABLE | F::LOS_BLOCKING | F::FLAMABLE | F::SOLID
        );
        assert_eq!(
            Terrain::SecretDoor.flags(),
            F::LOS_BLOCKING | F::SOLID | F::SECRET
        );
        assert_eq!(Terrain::SecretTrap.flags(), F::PASSABLE | F::SECRET);
        assert_eq!(Terrain::Bookshelf.flags(), Terrain::Barricade.flags());
        assert_eq!(Terrain::FurrowedGrass.flags(), Terrain::HighGrass.flags());
        assert_eq!(Terrain::WallDeco.flags(), Terrain::Wall.flags());
        assert_eq!(Terrain::Water.flags(), F::PASSABLE | F::LIQUID);
        assert_eq!(Terrain::Well.flags(), F::AVOID);
        assert_eq!(Terrain::CrystalDoor.flags(), F::SOLID);
    }

    #[test]
    fn discover_reveals_secrets() {
        assert_eq!(Terrain::SecretDoor.discover(), Terrain::Door);
        assert_eq!(Terrain::SecretTrap.discover(), Terrain::Trap);
        assert_eq!(Terrain::Grass.discover(), Terrain::Grass);
    }
}
