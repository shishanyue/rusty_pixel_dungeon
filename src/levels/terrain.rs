use bitflags::bitflags;
use lazy_static::lazy_static;

pub const CHASM: u8 = 0;
pub const EMPTY: u8 = 1;
pub const GRASS: u8 = 2;
pub const EMPTY_WELL: u8 = 3;
pub const WALL: u8 = 4;
pub const DOOR: u8 = 5;
pub const OPEN_DOOR: u8 = 6;
pub const ENTRANCE: u8 = 7;
pub const ENTRANCE_SP: u8 = 37;
pub const EXIT: u8 = 8;
pub const EMBERS: u8 = 9;
pub const LOCKED_DOOR: u8 = 10;
//a door that was locked by the skeleton key
pub const HERO_LKD_DR: u8 = 38;
pub const CRYSTAL_DOOR: u8 = 31;
pub const PEDESTAL: u8 = 11;
pub const WALL_DECO: u8 = 12;
pub const BARRICADE: u8 = 13;
pub const EMPTY_SP: u8 = 14;
pub const HIGH_GRASS: u8 = 15;
pub const FURROWED_GRASS: u8 = 30;
pub const SECRET_DOOR: u8 = 16;
pub const SECRET_TRAP: u8 = 17;
pub const TRAP: u8 = 18;
pub const INACTIVE_TRAP: u8 = 19;
pub const EMPTY_DECO: u8 = 20;
pub const LOCKED_EXIT: u8 = 21;
pub const UNLOCKED_EXIT: u8 = 22;
pub const WELL: u8 = 24;
pub const BOOKSHELF: u8 = 27;
pub const ALCHEMY: u8 = 28;
pub const CUSTOM_DECO_EMPTY: u8 = 32;//regular empty tile that can't be overridden, used for custom visuals mainly
//solid environment decorations
pub const CUSTOM_DECO: u8 = 23;//invisible decoration that will also be a custom visual, re-uses the old terrain ID for signs
pub const STATUE: u8 = 25;
pub const STATUE_SP: u8 = 26;
//These decorations are environment-specific
pub const REGION_DECO: u8 = 33;
pub const REGION_DECO_ALT: u8 = 34;//alt visual for region deco, sometimes SP, sometimes other
pub const MINE_CRYSTAL: u8 = 35;
pub const MINE_BOULDER: u8 = 36;
pub const WATER: u8 = 29;

lazy_static! {
    pub static ref FLAGS: [TerrainFlags; 256] = {
        let mut flags = [TerrainFlags::empty(); 256];

        flags[CHASM as usize] = TerrainFlags::AVOID | TerrainFlags::PIT;
        flags[EMPTY as usize] = TerrainFlags::PASSABLE;
        flags[GRASS as usize] = TerrainFlags::PASSABLE | TerrainFlags::FLAMABLE;
        flags[EMPTY_WELL as usize] = TerrainFlags::PASSABLE;
        flags[WATER as usize] = TerrainFlags::PASSABLE | TerrainFlags::LIQUID;
        flags[WALL as usize] = TerrainFlags::LOS_BLOCKING | TerrainFlags::SOLID;
        flags[DOOR as usize] = TerrainFlags::PASSABLE
            | TerrainFlags::LOS_BLOCKING
            | TerrainFlags::FLAMABLE
            | TerrainFlags::SOLID;
        flags[OPEN_DOOR as usize] = TerrainFlags::PASSABLE | TerrainFlags::FLAMABLE;
        flags[ENTRANCE as usize] = TerrainFlags::PASSABLE;
        flags[EXIT as usize] = TerrainFlags::PASSABLE;
        flags[EMBERS as usize] = TerrainFlags::PASSABLE;
        flags[LOCKED_DOOR as usize] = TerrainFlags::LOS_BLOCKING | TerrainFlags::SOLID;
        flags[CRYSTAL_DOOR as usize] = TerrainFlags::SOLID;
        flags[PEDESTAL as usize] = TerrainFlags::PASSABLE;
        flags[BARRICADE as usize] = TerrainFlags::FLAMABLE | TerrainFlags::SOLID | TerrainFlags::LOS_BLOCKING;
        flags[HIGH_GRASS as usize] =
            TerrainFlags::PASSABLE | TerrainFlags::LOS_BLOCKING | TerrainFlags::FLAMABLE;
        flags[TRAP as usize] = TerrainFlags::AVOID;
        flags[LOCKED_EXIT as usize] = TerrainFlags::SOLID;
        flags[UNLOCKED_EXIT as usize] = TerrainFlags::PASSABLE;
        flags[WELL as usize] = TerrainFlags::AVOID;
        flags[ALCHEMY as usize] = TerrainFlags::SOLID;
        flags[CUSTOM_DECO as usize] = TerrainFlags::SOLID;
        flags[STATUE as usize] = TerrainFlags::SOLID;
        flags[MINE_CRYSTAL as usize] = TerrainFlags::SOLID;
        flags[MINE_BOULDER as usize] = TerrainFlags::SOLID;
        
        flags[ENTRANCE_SP as usize] = flags[ENTRANCE as usize];
        flags[HERO_LKD_DR as usize] = flags[LOCKED_DOOR as usize];
        flags[WALL_DECO as usize] = flags[WALL as usize];
        flags[EMPTY_SP as usize] = flags[EMPTY as usize];
        flags[FURROWED_GRASS as usize] = flags[HIGH_GRASS as usize];
        // 包含 SECRET 标志的特殊地形
        flags[SECRET_DOOR as usize] = flags[WALL as usize] | TerrainFlags::SECRET;
        flags[SECRET_TRAP as usize] = flags[EMPTY as usize] | TerrainFlags::SECRET;
        flags[INACTIVE_TRAP as usize] = flags[EMPTY as usize];
        flags[EMPTY_DECO as usize] = flags[EMPTY as usize];
        flags[BOOKSHELF as usize] = flags[BARRICADE as usize];
        flags[CUSTOM_DECO_EMPTY as usize] = flags[EMPTY as usize];
        flags[STATUE_SP as usize] = flags[STATUE as usize];
        flags[REGION_DECO as usize] = flags[STATUE as usize];
        flags[REGION_DECO_ALT as usize] = flags[STATUE_SP as usize];

        flags
    };
}
/// 发现隐藏地形（如密门、陷阱）
/// 如果是隐藏地形，返回其真实形态，否则返回原地形
pub fn discover(terr: u8) -> u8 {
    match terr {
        SECRET_DOOR => DOOR,
        SECRET_TRAP => TRAP,
        _ => terr,
    }
}

// --- 定义标志位 ---
bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TerrainFlags: u8 {
        const PASSABLE      = 0b0000_0001;
        const LOS_BLOCKING  = 0b0000_0010;
        const FLAMABLE      = 0b0000_0100;
        const SECRET        = 0b0000_1000;
        const SOLID         = 0b0001_0000;
        const AVOID         = 0b0010_0000;
        const LIQUID        = 0b0100_0000;
        const PIT           = 0b1000_0000;
    }
}
