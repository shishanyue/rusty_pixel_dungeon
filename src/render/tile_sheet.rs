//! 地形 → 图集索引常量表与查表算法，照抄 SPD `tiles/DungeonTileSheet.java`
//! 与 `tiles/DungeonTerrainTilemap.java`（v3.3.8）。
//!
//! 图集（`tiles_sewers.png` 等）为 256×256 像素、16×16 格网（16px/格），
//! 索引按行主序编号：`xy(x, y) = (x-1) + 16*(y-1)`（Java L34-L39，x/y 从 1 起）。
//!
//! 渲染二期（26 号计划）主入口是 [`tile_visual_flat`]（平铺模式的
//! `getTileVisual`）：directVisuals 直查 + 水岸/深渊拼接 + tileVariance
//! 随机变体。raised 透视墙双层（非 flat 模式）常量已就位，算法留三期。
//! 行号未注明文件者均指 `DungeonTileSheet.java`。

use bevy::math::IVec2;

use crate::levels::{Level, terrain::Terrain};

/// 图集每行格数（Java L34 `WIDTH = 16`）
pub const ATLAS_COLUMNS: u32 = 16;

/// Java L36-L39 `xy(x, y)`：图集 1 起格坐标 → 行主序索引
const fn xy(x: u32, y: u32) -> u32 {
    (x - 1) + ATLAS_COLUMNS * (y - 1)
}

/// 地图边缘判定哨兵（Java L42）。Rust 侧接拼接算法时应改用
/// `Option<Terrain>`（越界 None）表达，此常量仅为对照保留。
pub const NULL_TILE: i32 = -1;

// ---------------------------------------------------------------------------
// Floor Tiles（Java L46-L71）
// ---------------------------------------------------------------------------

/// Java L50：`GROUND = xy(1, 1)`，24 slots
pub const GROUND: u32 = xy(1, 1);
pub const FLOOR: u32 = GROUND; // L51 (GROUND+0)
pub const FLOOR_DECO: u32 = GROUND + 1; // L52
pub const GRASS: u32 = GROUND + 2; // L53
pub const EMBERS: u32 = GROUND + 3; // L54
pub const FLOOR_SP: u32 = GROUND + 4; // L55
pub const FLOOR_ALT_1: u32 = GROUND + 6; // L57
pub const FLOOR_DECO_ALT: u32 = GROUND + 7; // L58
pub const GRASS_ALT: u32 = GROUND + 8; // L59
pub const EMBERS_ALT: u32 = GROUND + 9; // L60
pub const FLOOR_SP_ALT: u32 = GROUND + 10; // L61
pub const FLOOR_ALT_2: u32 = GROUND + 12; // L63
pub const ENTRANCE: u32 = GROUND + 16; // L65
pub const EXIT: u32 = GROUND + 17; // L66
pub const WELL: u32 = GROUND + 18; // L67
pub const EMPTY_WELL: u32 = GROUND + 19; // L68
pub const PEDESTAL: u32 = GROUND + 20; // L69
pub const ENTRANCE_SP: u32 = GROUND + 22; // L71

/// Java L73：`CHASM = xy(9, 2)`，8 slots。后 4 格为深渊上边缘拼接
/// （上邻居地形决定，Java L81-L132 `chasmStitcheable`/`stitchChasmTile`，第二阶段）
pub const CHASM: u32 = xy(9, 2);
pub const CHASM_FLOOR: u32 = CHASM + 1; // L75
pub const CHASM_FLOOR_SP: u32 = CHASM + 2; // L76
pub const CHASM_WALL: u32 = CHASM + 3; // L77
pub const CHASM_WATER: u32 = CHASM + 4; // L78

// ---------------------------------------------------------------------------
// Water Tiles（Java L135-L170）
// ---------------------------------------------------------------------------

/// Java L139：`WATER = xy(1, 3)`，16 slots。基准格 = 四邻皆水；
/// 后 15 格为水岸拼接：上邻是岸 +1、右 +2、下 +4、左 +8
/// （Java L143-L170 `waterStitcheable`/`stitchWaterTile`，第二阶段）
pub const WATER: u32 = xy(1, 3);

// ---------------------------------------------------------------------------
// Flat Tiles（Java L177-L216）
// ---------------------------------------------------------------------------

/// Java L181：`FLAT_WALLS = xy(1, 4)`，16 slots
pub const FLAT_WALLS: u32 = xy(1, 4);
pub const FLAT_WALL: u32 = FLAT_WALLS; // L182 (FLAT_WALLS+0)
pub const FLAT_WALL_DECO: u32 = FLAT_WALLS + 1; // L183
pub const FLAT_BOOKSHELF: u32 = FLAT_WALLS + 2; // L184
pub const FLAT_WALL_ALT: u32 = FLAT_WALLS + 4; // L186
pub const FLAT_WALL_DECO_ALT: u32 = FLAT_WALLS + 5; // L187
pub const FLAT_BOOKSHELF_ALT: u32 = FLAT_WALLS + 6; // L188
pub const FLAT_DOOR: u32 = FLAT_WALLS + 8; // L190
pub const FLAT_DOOR_OPEN: u32 = FLAT_WALLS + 9; // L191
pub const FLAT_DOOR_LOCKED: u32 = FLAT_WALLS + 10; // L192
pub const FLAT_DOOR_CRYSTAL: u32 = FLAT_WALLS + 11; // L193
pub const UNLOCKED_EXIT: u32 = FLAT_WALLS + 12; // L194
pub const LOCKED_EXIT: u32 = FLAT_WALLS + 13; // L195

/// Java L197：`FLAT_OTHER = xy(1, 5)`，16 slots
pub const FLAT_OTHER: u32 = xy(1, 5);
pub const FLAT_ALCHEMY_POT: u32 = FLAT_OTHER; // L198 (FLAT_OTHER+0)
pub const FLAT_BARRICADE: u32 = FLAT_OTHER + 1; // L199
pub const FLAT_HIGH_GRASS: u32 = FLAT_OTHER + 2; // L200
pub const FLAT_FURROWED_GRASS: u32 = FLAT_OTHER + 3; // L201
pub const FLAT_HIGH_GRASS_ALT: u32 = FLAT_OTHER + 5; // L203
pub const FLAT_FURROWED_ALT: u32 = FLAT_OTHER + 6; // L204
pub const FLAT_STATUE: u32 = FLAT_OTHER + 8; // L206
pub const FLAT_STATUE_SP: u32 = FLAT_OTHER + 9; // L207
pub const FLAT_REGION_DECO: u32 = FLAT_OTHER + 10; // L208
pub const FLAT_REGION_DECO_ALT: u32 = FLAT_OTHER + 11; // L209
pub const FLAT_MINE_CRYSTAL: u32 = FLAT_OTHER + 12; // L211
pub const FLAT_MINE_CRYSTAL_ALT: u32 = FLAT_OTHER + 13; // L212
pub const FLAT_MINE_CRYSTAL_ALT_2: u32 = FLAT_OTHER + 14; // L213
// Java L214-L216：矿区滚石与水晶共用同批格（常量值相同，照抄）
pub const FLAT_MINE_BOULDER: u32 = FLAT_OTHER + 12; // L214
pub const FLAT_MINE_BOULDER_ALT: u32 = FLAT_OTHER + 13; // L215
pub const FLAT_MINE_BOULDER_ALT_2: u32 = FLAT_OTHER + 14; // L216

// ---------------------------------------------------------------------------
// Raised Tiles, Lower Layer（Java L218-L316）——透视墙下半层，第二阶段
// ---------------------------------------------------------------------------

/// Java L222：`RAISED_WALLS = xy(1, 6)`，32 slots。
/// 变体编码：右侧开放 +1、左侧开放 +2（Java L223 注释、L250-L265 `getRaisedWallTile`）
pub const RAISED_WALLS: u32 = xy(1, 6);
pub const RAISED_WALL: u32 = RAISED_WALLS; // L224 (RAISED_WALLS+0)
pub const RAISED_WALL_DECO: u32 = RAISED_WALLS + 4; // L225
/// 出现在上下向门洞背后的墙（Java L226 注释）
pub const RAISED_WALL_DOOR: u32 = RAISED_WALLS + 8; // L227
pub const RAISED_WALL_BOOKSHELF: u32 = RAISED_WALLS + 12; // L228
pub const RAISED_WALL_ALT: u32 = RAISED_WALLS + 16; // L230
pub const RAISED_WALL_DECO_ALT: u32 = RAISED_WALLS + 20; // L231
pub const RAISED_WALL_BOOKSHELF_ALT: u32 = RAISED_WALLS + 28; // L232

/// Java L267：`RAISED_DOORS = xy(1, 8)`，8 slots（L276-L284 `getRaisedDoorTile`）
pub const RAISED_DOORS: u32 = xy(1, 8);
pub const RAISED_DOOR: u32 = RAISED_DOORS; // L268 (RAISED_DOORS+0)
pub const RAISED_DOOR_OPEN: u32 = RAISED_DOORS + 1; // L269
pub const RAISED_DOOR_LOCKED: u32 = RAISED_DOORS + 2; // L270
pub const RAISED_DOOR_CRYSTAL: u32 = RAISED_DOORS + 3; // L271
/// 上下向门洞处铺的地板格（Java L272 注释）
pub const RAISED_DOOR_SIDEWAYS: u32 = RAISED_DOORS + 4; // L273

/// Java L297：`RAISED_OTHER = xy(9, 8)`，24 slots
pub const RAISED_OTHER: u32 = xy(9, 8);
pub const RAISED_ALCHEMY_POT: u32 = RAISED_OTHER; // L298 (RAISED_OTHER+0)
pub const RAISED_BARRICADE: u32 = RAISED_OTHER + 1; // L299
pub const RAISED_HIGH_GRASS: u32 = RAISED_OTHER + 2; // L300
pub const RAISED_FURROWED_GRASS: u32 = RAISED_OTHER + 3; // L301
pub const RAISED_HIGH_GRASS_ALT: u32 = RAISED_OTHER + 5; // L303
pub const RAISED_FURROWED_ALT: u32 = RAISED_OTHER + 6; // L304
pub const RAISED_STATUE: u32 = RAISED_OTHER + 8; // L306
pub const RAISED_STATUE_SP: u32 = RAISED_OTHER + 9; // L307
pub const RAISED_REGION_DECO: u32 = RAISED_OTHER + 10; // L308
pub const RAISED_REGION_DECO_ALT: u32 = RAISED_OTHER + 11; // L309
pub const RAISED_MINE_CRYSTAL: u32 = RAISED_OTHER + 12; // L311
pub const RAISED_MINE_CRYSTAL_ALT: u32 = RAISED_OTHER + 13; // L312
pub const RAISED_MINE_CRYSTAL_ALT_2: u32 = RAISED_OTHER + 14; // L313
pub const RAISED_MINE_BOULDER: u32 = RAISED_OTHER + 12; // L314
pub const RAISED_MINE_BOULDER_ALT: u32 = RAISED_OTHER + 13; // L315
pub const RAISED_MINE_BOULDER_ALT_2: u32 = RAISED_OTHER + 14; // L316

// ---------------------------------------------------------------------------
// Raised Tiles, Upper Layer（Java L319-L408）——透视墙上半层/悬垂层，第二阶段
// ---------------------------------------------------------------------------

/// Java L324：`WALLS_INTERNAL = xy(1, 10)`，48 slots。
/// 变体编码：右 +1、右下 +2、左下 +4、左 +8 开放
/// （Java L323 注释、L329-L342 `stitchInternalWallTile`）
pub const WALLS_INTERNAL: u32 = xy(1, 10);
pub const WALL_INTERNAL: u32 = WALLS_INTERNAL; // L325 (WALLS_INTERNAL+0)
pub const WALL_INTERNAL_DECO: u32 = WALLS_INTERNAL + 16; // L326
pub const WALL_INTERNAL_WOODEN: u32 = WALLS_INTERNAL + 32; // L327

/// Java L345：`WALLS_OVERHANG = xy(1, 13)`，32 slots。
/// 变体编码：右下开放 +1、左下开放 +2（Java L344 注释、L355-L371 `stitchWallOverhangTile`）
pub const WALLS_OVERHANG: u32 = xy(1, 13);
pub const WALL_OVERHANG: u32 = WALLS_OVERHANG; // L346 (WALLS_OVERHANG+0)
pub const WALL_OVERHANG_DECO: u32 = WALLS_OVERHANG + 4; // L347
pub const WALL_OVERHANG_WOODEN: u32 = WALLS_OVERHANG + 8; // L348
pub const DOOR_SIDEWAYS_OVERHANG: u32 = WALLS_OVERHANG + 16; // L349
pub const DOOR_SIDEWAYS_OVERHANG_CLOSED: u32 = WALLS_OVERHANG + 20; // L350
pub const DOOR_SIDEWAYS_OVERHANG_LOCKED: u32 = WALLS_OVERHANG + 24; // L351
pub const DOOR_SIDEWAYS_OVERHANG_CRYSTAL: u32 = WALLS_OVERHANG + 28; // L352

/// Java L373：`DOOR_OVERHANG = xy(1, 15)`，8 slots
pub const DOOR_OVERHANG: u32 = xy(1, 15);
pub const DOOR_OVERHANG_OPEN: u32 = DOOR_OVERHANG + 1; // L374
pub const DOOR_OVERHANG_CRYSTAL: u32 = DOOR_OVERHANG + 2; // L375
pub const DOOR_SIDEWAYS: u32 = DOOR_OVERHANG + 3; // L376
pub const DOOR_SIDEWAYS_LOCKED: u32 = DOOR_OVERHANG + 4; // L377
pub const DOOR_SIDEWAYS_CRYSTAL: u32 = DOOR_OVERHANG + 5; // L378
/// 出口目前平铺渲染，因此实际是"下垂"（Java L379 注释）
pub const EXIT_UNDERHANG: u32 = DOOR_OVERHANG + 6; // L380

/// Java L383：`OTHER_OVERHANG = xy(9, 15)`，24 slots
pub const OTHER_OVERHANG: u32 = xy(9, 15);
pub const ALCHEMY_POT_OVERHANG: u32 = OTHER_OVERHANG; // L384 (OTHER_OVERHANG+0)
pub const BARRICADE_OVERHANG: u32 = OTHER_OVERHANG + 1; // L385
pub const HIGH_GRASS_OVERHANG: u32 = OTHER_OVERHANG + 2; // L386
pub const FURROWED_OVERHANG: u32 = OTHER_OVERHANG + 3; // L387
pub const HIGH_GRASS_OVERHANG_ALT: u32 = OTHER_OVERHANG + 5; // L389
pub const FURROWED_OVERHANG_ALT: u32 = OTHER_OVERHANG + 6; // L390
pub const STATUE_OVERHANG: u32 = OTHER_OVERHANG + 8; // L392
pub const STATUE_SP_OVERHANG: u32 = OTHER_OVERHANG + 9; // L393
pub const REGION_DECO_OVERHANG: u32 = OTHER_OVERHANG + 10; // L394
pub const REGION_DECO_ALT_OVERHANG: u32 = OTHER_OVERHANG + 11; // L395
pub const MINE_CRYSTAL_OVERHANG: u32 = OTHER_OVERHANG + 12; // L397
pub const MINE_CRYSTAL_OVERHANG_ALT: u32 = OTHER_OVERHANG + 13; // L398
pub const MINE_CRYSTAL_OVERHANG_ALT_2: u32 = OTHER_OVERHANG + 14; // L399
pub const MINE_BOULDER_OVERHANG: u32 = OTHER_OVERHANG + 12; // L400
pub const MINE_BOULDER_OVERHANG_ALT: u32 = OTHER_OVERHANG + 13; // L401
pub const MINE_BOULDER_OVERHANG_ALT_2: u32 = OTHER_OVERHANG + 14; // L402
pub const HIGH_GRASS_UNDERHANG: u32 = OTHER_OVERHANG + 18; // L404
pub const FURROWED_UNDERHANG: u32 = OTHER_OVERHANG + 19; // L405
pub const HIGH_GRASS_UNDERHANG_ALT: u32 = OTHER_OVERHANG + 21; // L407
pub const FURROWED_UNDERHANG_ALT: u32 = OTHER_OVERHANG + 22; // L408

// ---------------------------------------------------------------------------
// 直查表（Java L410-L465）
// ---------------------------------------------------------------------------

/// `directVisuals`（L415-L438）：无需拼接、平铺/透视两种模式下都直接显示的地形。
/// 命中者经 [`visual_with_alts`] 取变体（`DungeonTerrainTilemap.java` L43-L44）。
pub const fn direct_visual(terrain: Terrain) -> Option<u32> {
    use Terrain as T;
    Some(match terrain {
        // 陷阱与自定义装饰直接显示为地板（L427-L431）
        T::Empty
        | T::SecretTrap
        | T::Trap
        | T::InactiveTrap
        | T::CustomDeco
        | T::CustomDecoEmpty => FLOOR, // L417, L427-L431
        T::Grass => GRASS,                // L418
        T::EmptyWell => EMPTY_WELL,       // L419
        T::Entrance => ENTRANCE,          // L420
        T::Exit => EXIT,                  // L421
        T::Embers => EMBERS,              // L422
        T::Pedestal => PEDESTAL,          // L423
        T::EmptySp => FLOOR_SP,           // L424
        T::EntranceSp => ENTRANCE_SP,     // L425
        T::EmptyDeco => FLOOR_DECO,       // L433
        T::LockedExit => LOCKED_EXIT,     // L434
        T::UnlockedExit => UNLOCKED_EXIT, // L435
        T::Well => WELL,                  // L436
        _ => return None,
    })
}

/// `directFlatVisuals`（L441-L465）：平铺显示时无需拼接的墙/门等。
/// 命中者经 [`visual_with_alts`] 取变体（`DungeonTerrainTilemap.java` L100-L104）。
pub const fn direct_flat_visual(terrain: Terrain) -> Option<u32> {
    use Terrain as T;
    Some(match terrain {
        // 未发现的密门视觉上就是墙（L464）
        T::Wall | T::SecretDoor => FLAT_WALL, // L443, L464
        T::Door => FLAT_DOOR,                 // L444
        T::OpenDoor => FLAT_DOOR_OPEN,        // L445
        // 骷髅钥匙锁门与普通锁门同图（L446-L447）
        T::LockedDoor | T::HeroLockedDoor => FLAT_DOOR_LOCKED, // L446-L447
        T::CrystalDoor => FLAT_DOOR_CRYSTAL,                   // L448
        T::WallDeco => FLAT_WALL_DECO,                         // L449
        T::Bookshelf => FLAT_BOOKSHELF,                        // L450
        T::Alchemy => FLAT_ALCHEMY_POT,                        // L451
        T::Barricade => FLAT_BARRICADE,                        // L452
        T::HighGrass => FLAT_HIGH_GRASS,                       // L453
        T::FurrowedGrass => FLAT_FURROWED_GRASS,               // L454
        T::Statue => FLAT_STATUE,                              // L456
        T::StatueSp => FLAT_STATUE_SP,                         // L457
        T::RegionDeco => FLAT_REGION_DECO,                     // L458
        T::RegionDecoAlt => FLAT_REGION_DECO_ALT,              // L459
        T::MineCrystal => FLAT_MINE_CRYSTAL,                   // L461
        T::MineBoulder => FLAT_MINE_BOULDER,                   // L462
        _ => return None,
    })
}

/// 地形 → 图集索引的平面映射（每地形固定一格，无变体无拼接）。
///
/// = `directVisuals` ∪ {Water/Chasm 基准格} ∪ `directFlatVisuals`，
/// 恰好覆盖全部 39 种地形（有测试）。运行时渲染走 [`tile_visual_flat`]；
/// 本函数保留为无邻接上下文时的兜底查表（调试/单格图标类用途）。
pub const fn flat_tile_index(terrain: Terrain) -> u32 {
    if let Some(visual) = direct_visual(terrain) {
        return visual;
    }
    match terrain {
        Terrain::Water => WATER,
        Terrain::Chasm => CHASM,
        _ => match direct_flat_visual(terrain) {
            Some(visual) => visual,
            // 三表覆盖全部 39 种地形（flat_mapping_stays_inside_atlas 测试遍历验证）
            None => unreachable!(),
        },
    }
}

// ---------------------------------------------------------------------------
// 深渊拼接（Java L80-L132）
// ---------------------------------------------------------------------------

/// `stitchChasmTile`（L123-L132）+ `chasmStitcheable` 表（L81-L121）：
/// Chasm 格按**上邻**地形选上边缘拼接格。`above == None` 对应 Java 的
/// `NULL_TILE`（地图上边缘，`DungeonTerrainTilemap.java` L55），
/// 落到 `SparseArray.get` 的默认值 `CHASM`（L131）。
pub fn stitch_chasm_tile(above: Option<Terrain>, depth: i32) -> u32 {
    use Terrain as T;
    let Some(above) = above else {
        return CHASM;
    };
    // REGION_DECO_ALT 各区域视觉不同，按深度特判（L124-L130）
    if above == T::RegionDecoAlt {
        return if depth <= 5 {
            CHASM_FLOOR_SP
        } else if depth <= 10 {
            CHASM
        } else if depth <= 20 {
            CHASM_FLOOR_SP
        } else {
            CHASM_FLOOR
        };
    }
    match above {
        // floor 组（L84-L104）
        T::Empty
        | T::Grass
        | T::Embers
        | T::EmptyWell
        | T::HighGrass
        | T::FurrowedGrass
        | T::EmptyDeco
        | T::CustomDeco
        | T::Well
        | T::Statue
        | T::RegionDeco
        | T::SecretTrap
        | T::InactiveTrap
        | T::Trap
        | T::Bookshelf
        | T::Barricade
        | T::Pedestal
        | T::CustomDecoEmpty
        | T::MineBoulder
        | T::MineCrystal => CHASM_FLOOR,
        // special floor 组（L107-L108）
        T::EmptySp | T::StatueSp => CHASM_FLOOR_SP,
        // wall 组（L111-L117）——注意 CrystalDoor 不在表内，落默认值
        T::Wall
        | T::Door
        | T::OpenDoor
        | T::LockedDoor
        | T::HeroLockedDoor
        | T::SecretDoor
        | T::WallDeco => CHASM_WALL,
        // water（L120）
        T::Water => CHASM_WATER,
        // 表外地形（Entrance/Exit/Alchemy/Chasm/CrystalDoor…）落默认值（L131）
        _ => CHASM,
    }
}

// ---------------------------------------------------------------------------
// 水岸拼接（Java L142-L170）
// ---------------------------------------------------------------------------

/// `waterStitcheable`（表 L143-L151 + 深度特判 L153-L160）：该地形是否算"岸"。
/// 注意门（含锁门/水晶门）都算岸（L150），墙/深渊/水本身不算。
pub fn water_stitcheable(tile: Terrain, depth: i32) -> bool {
    use Terrain as T;
    // REGION_DECO_ALT 仅在恶魔厅（depth > 20）可拼接（L154-L158）
    if tile == T::RegionDecoAlt {
        return depth > 20;
    }
    matches!(
        tile,
        T::Empty
            | T::Grass
            | T::EmptyWell
            | T::Entrance
            | T::Exit
            | T::Embers
            | T::Barricade
            | T::HighGrass
            | T::FurrowedGrass
            | T::SecretTrap
            | T::Trap
            | T::InactiveTrap
            | T::EmptyDeco
            | T::CustomDeco
            | T::Well
            | T::Statue
            | T::RegionDeco
            | T::Alchemy
            | T::CustomDecoEmpty
            | T::MineCrystal
            | T::MineBoulder
            | T::Door
            | T::OpenDoor
            | T::LockedDoor
            | T::HeroLockedDoor
            | T::CrystalDoor
    )
}

/// `stitchWaterTile`（L162-L170）：水格按四邻是否为岸取拼接格，
/// 编码：上岸 +1、右 +2、下 +4、左 +8；四邻皆水（或墙等非岸）= 基准格。
pub fn stitch_water_tile(
    top: Terrain,
    right: Terrain,
    bottom: Terrain,
    left: Terrain,
    depth: i32,
) -> u32 {
    let mut result = WATER;
    if water_stitcheable(top, depth) {
        result += 1;
    }
    if water_stitcheable(right, depth) {
        result += 2;
    }
    if water_stitcheable(bottom, depth) {
        result += 4;
    }
    if water_stitcheable(left, depth) {
        result += 8;
    }
    result
}

// ---------------------------------------------------------------------------
// 随机变体（Java L468-L537）
// ---------------------------------------------------------------------------

/// `SplitMix64` 步进（Sebastiano Vigna 公开域实现）。选它替代 Java 侧
/// `Random.pushGenerator(seed)`（watabou `Random`）：方差只影响视觉、
/// 无需与 Java 逐值对拍，`SplitMix64` 零依赖且跨平台/跨版本字节稳定。
const fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `setupVariance`（L474-L483）：每格一个 `[0, 100)` 方差字节，同种子恒同输出。
/// Java 逐格 `Random.Int(100)`；此处 `SplitMix64` 取模（偏差 ~1e-17，视觉无感）。
pub fn tile_variance(seed: u64, size: usize) -> Vec<u8> {
    let mut state = seed;
    (0..size).map(|_| (splitmix64(&mut state) % 100) as u8).collect()
}

/// 方差种子。Java 用 `Dungeon.seedCurDepth()`（`GameScene.java` L271）；
/// 本项目运行种子（`RunSeed`）归 scenes 域不可依赖，改取 `Level` 的确定性
/// 只读特征（深度/尺寸/入口/出口）混合——同一关卡重进画面稳定，换层即变。
/// 同深度同尺寸且出入口重合的两张图会共享方差序列，纯视觉可接受（笔记说明）。
pub fn variance_seed(level: &Level) -> u64 {
    let mut state = 0x5D8E_9A6B_7C3F_2E1D;
    for feature in [
        level.depth as i64 as u64,
        level.width() as u64,
        level.height() as u64,
        level.entrance.x as i64 as u64,
        level.entrance.y as i64 as u64,
        level.exit.x as i64 as u64,
        level.exit.y as i64 as u64,
    ] {
        state ^= feature;
        splitmix64(&mut state);
    }
    state
}

/// `commonAltVisuals`（L486-L516）：50% 概率的普通变体（有稀有变体时 45%）。
/// 矿区水晶/滚石常量共用格位（值相同），Java 里的重复 put 在此合并为单臂。
pub const fn common_alt_visual(visual: u32) -> Option<u32> {
    Some(match visual {
        FLOOR => FLOOR_ALT_1,
        GRASS => GRASS_ALT,
        FLAT_WALL => FLAT_WALL_ALT,
        EMBERS => EMBERS_ALT,
        FLAT_WALL_DECO => FLAT_WALL_DECO_ALT,
        FLOOR_SP => FLOOR_SP_ALT,
        FLOOR_DECO => FLOOR_DECO_ALT,
        FLAT_BOOKSHELF => FLAT_BOOKSHELF_ALT,
        FLAT_HIGH_GRASS => FLAT_HIGH_GRASS_ALT,
        FLAT_FURROWED_GRASS => FLAT_FURROWED_ALT,
        FLAT_MINE_CRYSTAL => FLAT_MINE_CRYSTAL_ALT, // = FLAT_MINE_BOULDER（L500）
        RAISED_WALL => RAISED_WALL_ALT,
        RAISED_WALL_DECO => RAISED_WALL_DECO_ALT,
        RAISED_WALL_BOOKSHELF => RAISED_WALL_BOOKSHELF_ALT,
        RAISED_HIGH_GRASS => RAISED_HIGH_GRASS_ALT,
        RAISED_FURROWED_GRASS => RAISED_FURROWED_ALT,
        HIGH_GRASS_OVERHANG => HIGH_GRASS_OVERHANG_ALT,
        FURROWED_OVERHANG => FURROWED_OVERHANG_ALT,
        RAISED_MINE_CRYSTAL => RAISED_MINE_CRYSTAL_ALT, // = RAISED_MINE_BOULDER（L511）
        HIGH_GRASS_UNDERHANG => HIGH_GRASS_UNDERHANG_ALT,
        FURROWED_UNDERHANG => FURROWED_UNDERHANG_ALT,
        MINE_CRYSTAL_OVERHANG => MINE_CRYSTAL_OVERHANG_ALT, // = MINE_BOULDER_OVERHANG（L515）
        _ => return None,
    })
}

/// `rareAltVisuals`（L519-L528）：5% 概率的稀有变体，出现时覆盖普通变体。
pub const fn rare_alt_visual(visual: u32) -> Option<u32> {
    Some(match visual {
        FLOOR => FLOOR_ALT_2,
        FLAT_MINE_CRYSTAL => FLAT_MINE_CRYSTAL_ALT_2, // = FLAT_MINE_BOULDER（L523）
        RAISED_MINE_CRYSTAL => RAISED_MINE_CRYSTAL_ALT_2, // = RAISED_MINE_BOULDER（L525）
        MINE_CRYSTAL_OVERHANG => MINE_CRYSTAL_OVERHANG_ALT_2, // = MINE_BOULDER_OVERHANG（L527）
        _ => return None,
    })
}

/// `getVisualWithAlts`（L530-L537）：方差 ≥95 且有稀有变体 → 稀有；
/// 否则 ≥50 且有普通变体 → 普通；否则基准格。
pub const fn visual_with_alts(visual: u32, variance: u8) -> u32 {
    if variance >= 95
        && let Some(rare) = rare_alt_visual(visual)
    {
        return rare;
    }
    if variance >= 50
        && let Some(common) = common_alt_visual(visual)
    {
        return common;
    }
    visual
}

// ---------------------------------------------------------------------------
// 平铺模式查表主分派（DungeonTerrainTilemap.java L42-L106，flat = true）
// ---------------------------------------------------------------------------

/// 平铺模式的 `getTileVisual`：directVisuals 直查（带变体）→ 水岸拼接 →
/// 深渊拼接 → directFlatVisuals（带变体）。raised（非 flat）分支是三期。
///
/// `variance` 必须是 [`tile_variance`] 生成的 `level.size()` 长数组。
/// 邻接读取经 `Level::terrain`（越界视为墙）：SPD 关卡边界恒实心墙，
/// Java 直接越界索引的位置在本移植下语义一致且对手工小图安全。
pub fn tile_visual_flat(level: &Level, variance: &[u8], cell: IVec2) -> u32 {
    let tile = level.terrain(cell);
    let index = level.index(cell);
    // L43-L44：直查命中即返回（带变体）
    if let Some(visual) = direct_visual(tile) {
        return visual_with_alts(visual, variance[index]);
    }
    match tile {
        // L46-L52：四邻 = PathFinder.CIRCLE4 顺序（上/右/下/左）
        Terrain::Water => stitch_water_tile(
            level.terrain(cell - IVec2::Y),
            level.terrain(cell + IVec2::X),
            level.terrain(cell + IVec2::Y),
            level.terrain(cell - IVec2::X),
            level.depth,
        ),
        // L54-L55：`pos > mapWidth ? map[pos - mapWidth] : NULL_TILE`——
        // 照抄严格大于：行 0 与格 (0,1)（pos == width）都取 None。
        // (0,1) 是 Java 的差一 quirk，边界恒墙使其不可观察
        Terrain::Chasm => {
            let above = (index > level.width()).then(|| level.terrain(cell - IVec2::Y));
            stitch_chasm_tile(above, level.depth)
        }
        // L100-L104：平铺直查（带变体）
        _ => visual_with_alts(
            direct_flat_visual(tile)
                .expect("directVisuals ∪ {Water,Chasm} ∪ directFlatVisuals 覆盖全部 39 种地形"),
            variance[index],
        ),
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    /// 常量数值抽查，对照 DungeonTileSheet.java 行号（xy 换算 L36-L39）
    #[test]
    fn constants_match_java_values() {
        // Floor 段（L50-L71）
        assert_eq!(FLOOR, 0);
        assert_eq!(GRASS, 2);
        assert_eq!(FLOOR_ALT_2, 12);
        assert_eq!(ENTRANCE, 16);
        assert_eq!(EXIT, 17);
        assert_eq!(ENTRANCE_SP, 22);
        // Chasm 段（L73-L78）：xy(9,2) = 24
        assert_eq!(CHASM, 24);
        assert_eq!(CHASM_WATER, 28);
        // Water 段（L139）：xy(1,3) = 32
        assert_eq!(WATER, 32);
        // Flat 段（L181-L216）：xy(1,4) = 48、xy(1,5) = 64
        assert_eq!(FLAT_WALL, 48);
        assert_eq!(FLAT_DOOR, 56);
        assert_eq!(UNLOCKED_EXIT, 60);
        assert_eq!(LOCKED_EXIT, 61);
        assert_eq!(FLAT_HIGH_GRASS, 66);
        assert_eq!(FLAT_MINE_BOULDER, FLAT_MINE_CRYSTAL); // L211/L214 共用格
        // Raised 下半层（L222-L316）：xy(1,6) = 80、xy(1,8) = 112、xy(9,8) = 120
        assert_eq!(RAISED_WALL, 80);
        assert_eq!(RAISED_WALL_BOOKSHELF_ALT, 108);
        assert_eq!(RAISED_DOOR, 112);
        assert_eq!(RAISED_DOOR_SIDEWAYS, 116);
        assert_eq!(RAISED_STATUE, 128);
        // Raised 上半层（L324-L408）：xy(1,10) = 144、xy(1,13) = 192、
        // xy(1,15) = 224、xy(9,15) = 232
        assert_eq!(WALL_INTERNAL, 144);
        assert_eq!(WALL_OVERHANG, 192);
        assert_eq!(DOOR_SIDEWAYS_OVERHANG_CRYSTAL, 220);
        assert_eq!(EXIT_UNDERHANG, 230);
        assert_eq!(HIGH_GRASS_UNDERHANG, 250);
        assert_eq!(FURROWED_UNDERHANG_ALT, 254);
    }

    /// 平面映射抽查，对照 directVisuals（L415-L438）与 directFlatVisuals（L441-L465）
    #[test]
    fn flat_mapping_matches_java_tables() {
        assert_eq!(flat_tile_index(Terrain::Empty), FLOOR); // L417
        assert_eq!(flat_tile_index(Terrain::Trap), FLOOR); // L428：陷阱平铺为地板
        assert_eq!(flat_tile_index(Terrain::EmptyDeco), FLOOR_DECO); // L433
        assert_eq!(flat_tile_index(Terrain::Entrance), 16); // L420
        assert_eq!(flat_tile_index(Terrain::Exit), 17); // L421
        assert_eq!(flat_tile_index(Terrain::UnlockedExit), 60); // L435
        assert_eq!(flat_tile_index(Terrain::Water), 32); // 水基准格（L139）
        assert_eq!(flat_tile_index(Terrain::Chasm), 24); // 深渊基准格（L73）
        assert_eq!(flat_tile_index(Terrain::Wall), 48); // L443
        assert_eq!(flat_tile_index(Terrain::SecretDoor), FLAT_WALL); // L464：密门=墙
        assert_eq!(flat_tile_index(Terrain::Door), 56); // L444
        assert_eq!(flat_tile_index(Terrain::HeroLockedDoor), FLAT_DOOR_LOCKED); // L447
        assert_eq!(flat_tile_index(Terrain::CrystalDoor), 59); // L448
        assert_eq!(flat_tile_index(Terrain::HighGrass), 66); // L453
        assert_eq!(flat_tile_index(Terrain::RegionDecoAlt), 75); // L459
        assert_eq!(flat_tile_index(Terrain::MineBoulder), 76); // L462
    }

    /// 平面映射对全部地形封闭，且索引都落在 16×16 图集内
    #[test]
    fn flat_mapping_stays_inside_atlas() {
        for terrain in Terrain::iter() {
            let index = flat_tile_index(terrain);
            assert!(
                index < ATLAS_COLUMNS * ATLAS_COLUMNS,
                "{terrain:?} → {index} 超出图集"
            );
        }
    }

    /// `stitchWaterTile` 邻接编码手算对拍（L162-L170：上岸 +1/右 +2/下 +4/左 +8）
    #[test]
    fn stitch_water_tile_encodes_neighbours() {
        use Terrain as T;
        // 四邻皆水 → 基准格
        assert_eq!(
            stitch_water_tile(T::Water, T::Water, T::Water, T::Water, 1),
            WATER
        );
        // 单侧岸位
        assert_eq!(
            stitch_water_tile(T::Empty, T::Water, T::Water, T::Water, 1),
            WATER + 1
        );
        assert_eq!(
            stitch_water_tile(T::Water, T::Grass, T::Water, T::Water, 1),
            WATER + 2
        );
        assert_eq!(
            stitch_water_tile(T::Water, T::Water, T::Entrance, T::Water, 1),
            WATER + 4
        );
        assert_eq!(
            stitch_water_tile(T::Water, T::Water, T::Water, T::Door, 1),
            WATER + 8
        );
        // 组合位：上+下 = +5；四面全岸 = +15
        assert_eq!(
            stitch_water_tile(T::Empty, T::Water, T::Empty, T::Water, 1),
            WATER + 5
        );
        assert_eq!(
            stitch_water_tile(T::Empty, T::Grass, T::Trap, T::Statue, 1),
            WATER + 15
        );
        // 墙/深渊不算岸（不在 L143-L151 表内）
        assert_eq!(
            stitch_water_tile(T::Wall, T::Chasm, T::Water, T::Water, 1),
            WATER
        );
    }

    /// `waterStitcheable` 语义抽查（表 L143-L151 + `REGION_DECO_ALT` 特判 L153-L160）
    #[test]
    fn water_stitcheable_matches_java_table() {
        use Terrain as T;
        // 门族全算岸（L150）
        for door in [
            T::Door,
            T::OpenDoor,
            T::LockedDoor,
            T::HeroLockedDoor,
            T::CrystalDoor,
        ] {
            assert!(water_stitcheable(door, 1), "{door:?} 应算岸");
        }
        for shore in [
            T::Empty,
            T::Grass,
            T::Entrance,
            T::Exit,
            T::Alchemy,
            T::Well,
            T::MineBoulder,
        ] {
            assert!(water_stitcheable(shore, 1), "{shore:?} 应算岸");
        }
        // 表外地形：墙族/书架/特殊地板/水与深渊自身都不算
        for not_shore in [
            T::Wall,
            T::WallDeco,
            T::SecretDoor,
            T::Bookshelf,
            T::Water,
            T::Chasm,
            T::EmptySp,
            T::Pedestal,
            T::LockedExit,
        ] {
            assert!(!water_stitcheable(not_shore, 1), "{not_shore:?} 不应算岸");
        }
        // REGION_DECO_ALT 仅恶魔厅（depth > 20）算岸（L154-L158）
        assert!(!water_stitcheable(T::RegionDecoAlt, 20));
        assert!(water_stitcheable(T::RegionDecoAlt, 21));
    }

    /// `stitchChasmTile` 上邻选格（表 L81-L121 + 特判 L123-L132）
    #[test]
    fn stitch_chasm_tile_selects_by_above() {
        use Terrain as T;
        // 地图上边缘（NULL_TILE）→ SparseArray 默认值 CHASM（L131）
        assert_eq!(stitch_chasm_tile(None, 1), CHASM);
        // floor 组（L84-L104，注意 Bookshelf/Barricade 也在其中）
        for above in [
            T::Empty,
            T::Grass,
            T::HighGrass,
            T::Bookshelf,
            T::Barricade,
            T::MineCrystal,
            T::Trap,
        ] {
            assert_eq!(stitch_chasm_tile(Some(above), 1), CHASM_FLOOR, "{above:?}");
        }
        // special floor 组（L107-L108）
        assert_eq!(stitch_chasm_tile(Some(T::EmptySp), 1), CHASM_FLOOR_SP);
        assert_eq!(stitch_chasm_tile(Some(T::StatueSp), 1), CHASM_FLOOR_SP);
        // wall 组（L111-L117）
        for above in [
            T::Wall,
            T::Door,
            T::OpenDoor,
            T::SecretDoor,
            T::WallDeco,
            T::HeroLockedDoor,
        ] {
            assert_eq!(stitch_chasm_tile(Some(above), 1), CHASM_WALL, "{above:?}");
        }
        // water（L120）
        assert_eq!(stitch_chasm_tile(Some(T::Water), 1), CHASM_WATER);
        // 表外地形落默认值（CrystalDoor 不在 wall 组；Entrance/Exit/Alchemy 缺席）
        for above in [T::Chasm, T::Entrance, T::Exit, T::Alchemy, T::CrystalDoor] {
            assert_eq!(stitch_chasm_tile(Some(above), 1), CHASM, "{above:?}");
        }
        // REGION_DECO_ALT 按深度（L125-L130）
        assert_eq!(stitch_chasm_tile(Some(T::RegionDecoAlt), 5), CHASM_FLOOR_SP);
        assert_eq!(stitch_chasm_tile(Some(T::RegionDecoAlt), 10), CHASM);
        assert_eq!(stitch_chasm_tile(Some(T::RegionDecoAlt), 20), CHASM_FLOOR_SP);
        assert_eq!(stitch_chasm_tile(Some(T::RegionDecoAlt), 21), CHASM_FLOOR);
    }

    /// `getVisualWithAlts` 阈值语义（L530-L537）：≥95 稀有、≥50 普通、否则基准
    #[test]
    fn visual_with_alts_thresholds() {
        // FLOOR 兼有普通/稀有变体（L488/L521）
        assert_eq!(visual_with_alts(FLOOR, 0), FLOOR);
        assert_eq!(visual_with_alts(FLOOR, 49), FLOOR);
        assert_eq!(visual_with_alts(FLOOR, 50), FLOOR_ALT_1);
        assert_eq!(visual_with_alts(FLOOR, 94), FLOOR_ALT_1);
        assert_eq!(visual_with_alts(FLOOR, 95), FLOOR_ALT_2);
        assert_eq!(visual_with_alts(FLOOR, 99), FLOOR_ALT_2);
        // GRASS 只有普通变体：≥95 落回普通分支（Java else-if 链语义）
        assert_eq!(visual_with_alts(GRASS, 95), GRASS_ALT);
        // 平铺墙变体（L490）
        assert_eq!(visual_with_alts(FLAT_WALL, 50), FLAT_WALL_ALT);
        assert_eq!(visual_with_alts(FLAT_WALL, 95), FLAT_WALL_ALT);
        // 无变体的视觉恒原样（入口、水岸拼接格）
        assert_eq!(visual_with_alts(ENTRANCE, 99), ENTRANCE);
        assert_eq!(visual_with_alts(WATER + 7, 99), WATER + 7);
    }

    /// `tileVariance`：同种子恒同、不同种子有别、值域 [0, 100)
    #[test]
    fn tile_variance_is_deterministic() {
        let a = tile_variance(0xDEAD_BEEF, 400);
        let b = tile_variance(0xDEAD_BEEF, 400);
        assert_eq!(a, b, "同种子必须逐字节相同");
        assert!(a.iter().all(|&v| v < 100), "值域必须是 [0, 100)");
        let c = tile_variance(0xDEAD_BEF0, 400);
        assert_ne!(a, c, "不同种子输出应不同");
        // 常见/稀有阈值两侧都有样本（400 格下缺失的概率 < 1e-8，种子固定不抖动）
        assert!(a.iter().any(|&v| v >= 95));
        assert!(a.iter().any(|&v| v < 50));
    }

    /// `variance_seed`：同特征关卡稳定，深度/出口变化即换种子
    #[test]
    fn variance_seed_tracks_level_features() {
        fn make(depth: i32, exit: IVec2) -> Level {
            let mut level = Level::new(9, 7, depth);
            level.entrance = IVec2::new(2, 3);
            level.exit = exit;
            level
        }
        let base = make(3, IVec2::new(6, 4));
        assert_eq!(variance_seed(&base), variance_seed(&make(3, IVec2::new(6, 4))));
        assert_ne!(variance_seed(&base), variance_seed(&make(4, IVec2::new(6, 4))));
        assert_ne!(variance_seed(&base), variance_seed(&make(3, IVec2::new(6, 5))));
    }

    /// 平铺主分派手工铺图对拍（`DungeonTerrainTilemap.java` L42-L106）：
    /// 直查/水岸拼接/深渊拼接/平铺直查四条路径 + 上边缘 `NULL_TILE` 语义
    #[test]
    fn tile_visual_flat_dispatches_like_java() {
        use bevy::math::IRect;
        use Terrain as T;

        let mut level = Level::new(7, 7, 1);
        level.fill(IRect::new(1, 1, 6, 6), T::Empty);
        level.set_terrain(IVec2::new(2, 0), T::Chasm); // 行 0：上邻 NULL
        level.set_terrain(IVec2::new(0, 1), T::Chasm); // pos == width 的 Java quirk
        level.set_terrain(IVec2::new(1, 1), T::Water);
        level.set_terrain(IVec2::new(4, 1), T::Water);
        level.set_terrain(IVec2::new(5, 1), T::Chasm); // 上邻 (5,0) Wall
        level.set_terrain(IVec2::new(4, 2), T::Chasm); // 上邻 (4,1) Water
        level.set_terrain(IVec2::new(3, 3), T::Water); // 四邻全 Empty
        level.set_terrain(IVec2::new(4, 4), T::Trap); // directVisuals：陷阱平铺为地板
        level.set_terrain(IVec2::new(3, 5), T::Chasm); // 上邻 (3,4) Empty
        level.set_terrain(IVec2::new(5, 5), T::SecretDoor); // 平铺直查：密门=墙
        let zero = vec![0u8; level.size()];

        // 水岸拼接：(3,3) 四邻全岸 → +15；(1,1) 上墙左深渊 → 右+2 下+4 = +6；
        // (4,1) 上墙右深渊下深渊 → 左+8
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(3, 3)), WATER + 15);
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(1, 1)), WATER + 6);
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(4, 1)), WATER + 8);
        // 深渊拼接：行 0 与 pos == width 都取 NULL → 基准格（L55 严格大于）
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(2, 0)), CHASM);
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(0, 1)), CHASM);
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(5, 1)), CHASM_WALL);
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(4, 2)), CHASM_WATER);
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(3, 5)), CHASM_FLOOR);
        // 直查（方差 0 → 基准格）
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(2, 2)), FLOOR);
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(4, 4)), FLOOR); // L428
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(0, 0)), FLAT_WALL);
        assert_eq!(tile_visual_flat(&level, &zero, IVec2::new(5, 5)), FLAT_WALL); // L464

        // 方差接入两条直查路径（拼接路径不吃变体，与 Java 一致）
        let mut variance = zero;
        variance[level.index(IVec2::new(2, 2))] = 77;
        variance[level.index(IVec2::new(0, 0))] = 95; // 墙无稀有变体 → 普通变体
        variance[level.index(IVec2::new(3, 3))] = 99;
        assert_eq!(
            tile_visual_flat(&level, &variance, IVec2::new(2, 2)),
            FLOOR_ALT_1
        );
        assert_eq!(
            tile_visual_flat(&level, &variance, IVec2::new(0, 0)),
            FLAT_WALL_ALT
        );
        assert_eq!(
            tile_visual_flat(&level, &variance, IVec2::new(3, 3)),
            WATER + 15,
            "拼接格不受方差影响"
        );
    }

    /// 主分派对全部 39 种地形封闭（放到 3×3 中心逐一调用，不 panic 不越图集）
    #[test]
    fn tile_visual_flat_covers_all_terrains() {
        for terrain in Terrain::iter() {
            let mut level = Level::new(3, 3, 1);
            level.set_terrain(IVec2::ONE, terrain);
            let zero = vec![0u8; level.size()];
            for variance in [0u8, 60, 97] {
                let mut table = zero.clone();
                table[level.index(IVec2::ONE)] = variance;
                let visual = tile_visual_flat(&level, &table, IVec2::ONE);
                assert!(
                    visual < ATLAS_COLUMNS * ATLAS_COLUMNS,
                    "{terrain:?} 方差 {variance} → {visual} 超出图集"
                );
            }
        }
    }
}
