//! `Level` 资源 ↔ 地形/水面双层 tilemap 实体：坐标换算、生成与销毁。
//!
//! 坐标约定（与 `scenes/in_game.rs` 调试方块视图完全重合）：
//! 关卡格 (x, y)（y 向下、行 0 在上）→ 世界 `((x-(w-1)/2)*16, ((h-1)/2-y)*16)`，
//! 即地图中心在原点、y 翻转、格边长 16。
//!
//! 图层结构（SPD `GameScene.java` L244-L274 的 terrain Group 顺序）：
//! 水面层（[`WATER_LAYER_Z`]，`water0.png` 全图平铺）压在地形层
//! （[`TERRAIN_LAYER_Z`]）之下；地形层的水格中心透明（岸边拼接格）或整格
//! 不渲染（纯水格，`DungeonTerrainTilemap.java` L114-L117 `needsRender`），
//! 露出底下的水面。两层 tile 都带迷雾染色（`TileColor`）。

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::{
    levels::Level,
    render::{
        tile_sheet,
        visibility::{VisibilityMap, fog_color},
    },
    states::AppState,
};

/// 单格边长（世界单位/像素），与调试视图 `TILE_SIZE` 及图集格宽一致
pub const TILE_PIXELS: f32 = 16.0;

/// 地形层 z：压在角色/调试方块（z ≥ 0）之下
pub const TERRAIN_LAYER_Z: f32 = -10.0;

/// 水面层 z：压在地形层之下（SPD 的 water 先于 tiles 加入 terrain Group，
/// `GameScene.java` L247/L273-L274）
pub const WATER_LAYER_Z: f32 = -11.0;

/// 本域生成的关卡渲染层根实体统一标记：despawn/防御性清理按它查询
#[derive(Component, Debug)]
pub struct LevelRenderRoot;

/// 地形 tilemap 根实体标记。tile 实体挂其子级，despawn 根实体即级联清空
#[derive(Component, Debug)]
pub struct TerrainTilemap;

/// 水面 tilemap 根实体标记
#[derive(Component, Debug)]
pub struct WaterTilemap;

/// 当前使用的地形图集句柄（M3 固定下水道 `tiles_sewers.png`）。
/// 生产环境由 `render.rs` 从 `EnvironmentCollection` 缓存；
/// 无资产的集成测试可注入 `Handle::default()` 驱动完整插件路径。
#[derive(Resource, Debug, Clone)]
pub struct TerrainAtlas(pub Handle<Image>);

/// 当前使用的水面贴图句柄（M3 固定下水道 `water0.png`，
/// `SewerLevel.waterTex()`，`SewerLevel.java` L115-L117）。
/// 中转与测试注入模式同 [`TerrainAtlas`]。
#[derive(Resource, Debug, Clone)]
pub struct WaterTexture(pub Handle<Image>);

/// `water0.png` 为 32×32 像素 = 2×2 个 16px 子格
pub const WATER_TILE_COLUMNS: i32 = 2;

/// 关卡格 → [`TilePos`]：关卡 y 向下（行 0 在上），`bevy_ecs_tilemap`
/// 的 `TilePos` y 向上（行 0 在下），翻转 `y = h-1-cell.y`。
pub fn tile_pos_for_cell(cell: IVec2, level_height: usize) -> TilePos {
    TilePos {
        x: cell.x as u32,
        y: (level_height as i32 - 1 - cell.y) as u32,
    }
}

/// [`tile_pos_for_cell`] 的逆变换：`TilePos` → 关卡格（迷雾染色按格查表用）
pub fn cell_for_tile_pos(pos: TilePos, level_height: usize) -> IVec2 {
    IVec2::new(pos.x as i32, level_height as i32 - 1 - pos.y as i32)
}

/// 格中心的世界坐标（调试视图公式，坐标约定的唯一权威表达）。
/// tilemap 侧由 `TilemapAnchor::Center` 复现同一结果，
/// 等价性推导见 [`spawn_terrain_tilemap`] 内注释，并有测试逐点对拍。
pub fn cell_center_world(cell: IVec2, level_width: usize, level_height: usize) -> Vec2 {
    Vec2::new(
        (cell.x as f32 - (level_width as f32 - 1.0) / 2.0) * TILE_PIXELS,
        ((level_height as f32 - 1.0) / 2.0 - cell.y as f32) * TILE_PIXELS,
    )
}

/// 水面层每格的子格索引：SPD 的 `SkinnedBlock` 以关卡左上角为原点全图
/// 重复平铺水贴图（`GameScene.java` L247-L250），32px 贴图 = 2×2 个 16px
/// 子格 → 取 `(x mod 2, y mod 2)`，图集行主序（行 0 在贴图顶部）。
pub fn water_tile_index(cell: IVec2) -> u32 {
    (cell.y.rem_euclid(WATER_TILE_COLUMNS) * WATER_TILE_COLUMNS
        + cell.x.rem_euclid(WATER_TILE_COLUMNS)) as u32
}

/// 双层共用的 tilemap 构建：根实体（`TilemapBundle` + [`LevelRenderRoot`] +
/// `DespawnOnExit(InGame)`）+ 每格一个 tile 子实体（初始迷雾色已烘焙——
/// spawn 帧内 `Commands` 未冲刷，`apply_fog` 看不到新 tile，必须出生即正确）。
fn spawn_layer(
    commands: &mut Commands,
    level: &Level,
    texture: Handle<Image>,
    z: f32,
    name: &'static str,
    visibility: &VisibilityMap,
    mut tile_for_cell: impl FnMut(IVec2) -> (u32, bool),
) -> Entity {
    let map_size = TilemapSize {
        x: level.width() as u32,
        y: level.height() as u32,
    };
    let tilemap_entity = commands.spawn_empty().id();
    let mut storage = TileStorage::empty(map_size);

    for index in 0..level.size() {
        let cell = level.pos_of(index);
        let tile_pos = tile_pos_for_cell(cell, level.height());
        let (texture_index, visible) = tile_for_cell(cell);
        let tile_entity = commands
            .spawn((
                TileBundle {
                    position: tile_pos,
                    tilemap_id: TilemapId(tilemap_entity),
                    texture_index: TileTextureIndex(texture_index),
                    visible: TileVisible(visible),
                    color: TileColor(fog_color(visibility.state_at(cell))),
                    ..default()
                },
                // 挂为根实体子级：despawn 根即级联清 tile，DespawnOnExit 双保险同样生效
                ChildOf(tilemap_entity),
            ))
            .id();
        storage.set(&tile_pos, tile_entity);
    }

    let tile_size = TilemapTileSize {
        x: TILE_PIXELS,
        y: TILE_PIXELS,
    };
    // 锚点换算推导：方形地图的整图 AABB 为 min=(-8,-8)、max=((w-1)*16+8, (h-1)*16+8)
    // （tile 中心距 ±半格边界，bevy_ecs_tilemap src/helpers/transform.rs `chunk_aabb`）。
    // `TilemapAnchor::Center` 平移 -(max+min)/2 = (-(w-1)*8, -(h-1)*8)（src/anchor.rs L68），
    // 于是 TilePos(tx,ty) 中心 = ((tx-(w-1)/2)*16, (ty-(h-1)/2)*16)；
    // 代入 ty = h-1-y 得 ((x-(w-1)/2)*16, ((h-1)/2-y)*16)，与调试视图公式逐字相等，
    // 因此 Transform 平移只需抬 z，x/y 留在原点。
    commands.entity(tilemap_entity).insert((
        TilemapBundle {
            grid_size: tile_size.into(),
            map_type: TilemapType::Square,
            size: map_size,
            storage,
            texture: TilemapTexture::Single(texture),
            tile_size,
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, z),
            ..default()
        },
        LevelRenderRoot,
        DespawnOnExit(AppState::InGame),
        Name::new(name),
    ));
    tilemap_entity
}

/// 按 `Level` 生成地形层：拼接 + 变体 + needsRender + 迷雾初色。返回根实体。
///
/// 独立成公共函数：插件系统与集成测试共用同一构建路径。
pub fn spawn_terrain_tilemap(
    commands: &mut Commands,
    level: &Level,
    atlas: Handle<Image>,
    visibility: &VisibilityMap,
) -> Entity {
    // 方差数组随 tilemap 重建整表重算（GameScene.java L271 每次进场
    // setupVariance 的时机语义；种子来源差异见 tile_sheet::variance_seed）
    let variance = tile_sheet::tile_variance(tile_sheet::variance_seed(level), level.size());
    let entity = spawn_layer(
        commands,
        level,
        atlas,
        TERRAIN_LAYER_Z,
        "terrain_tilemap",
        visibility,
        |cell| {
            let visual = tile_sheet::tile_visual_flat(level, &variance, cell);
            // needsRender（DungeonTerrainTilemap.java L114-L117）：四邻皆水的
            // 纯水格不渲染地形层，完全露出水面层；岸边拼接格（WATER+1..15）
            // 中心透明、边缘不透明，正常渲染
            (visual, visual != tile_sheet::WATER)
        },
    );
    commands.entity(entity).insert(TerrainTilemap);
    entity
}

/// 按 `Level` 生成水面层：全图平铺 `water0.png` 子格（SPD 即全图铺，
/// 非水格被上层不透明地形盖住，`GameScene.java` L259 注释"水面无 alpha"）。
/// 滚动动画（L860 `waterOfs -= 5*elapsed`，5px/s 向下）留三期：
/// `bevy_ecs_tilemap` 无逐层 UV 偏移，需自定义材质或滚动贴图本身。
pub fn spawn_water_tilemap(
    commands: &mut Commands,
    level: &Level,
    texture: Handle<Image>,
    visibility: &VisibilityMap,
) -> Entity {
    let entity = spawn_layer(
        commands,
        level,
        texture,
        WATER_LAYER_Z,
        "water_tilemap",
        visibility,
        |cell| (water_tile_index(cell), true),
    );
    commands.entity(entity).insert(WaterTilemap);
    entity
}

/// `Level` 插入或整体替换（`resource_exists_and_changed`）时重建双层 tilemap。
///
/// 用变更边沿而非 `resource_added`：bevy 0.19 覆盖式 `insert_resource`
/// 只更新 changed tick、不重置 added tick，而下楼（`scenes::in_game::descend`）
/// 正是不 remove 直接覆盖插入新 `Level`——按 added 边沿监听会漏掉换层。
/// 先防御性清掉旧实体，同帧 remove+insert 丢失 `resource_removed` 边沿
/// 也不留双份。
pub(crate) fn spawn_on_level_changed(
    mut commands: Commands,
    level: Res<Level>,
    atlas: Res<TerrainAtlas>,
    water: Res<WaterTexture>,
    visibility: Res<VisibilityMap>,
    existing: Query<Entity, With<LevelRenderRoot>>,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    spawn_terrain_tilemap(&mut commands, &level, atlas.0.clone(), &visibility);
    spawn_water_tilemap(&mut commands, &level, water.0.clone(), &visibility);
}

/// `Level` 移除（`resource_removed`）时销毁。正常退出 `InGame` 时
/// `DespawnOnExit` 已在状态切换期清掉实体，本系统作为非状态路径的兜底。
pub(crate) fn despawn_on_level_removed(
    mut commands: Commands,
    tilemaps: Query<Entity, With<LevelRenderRoot>>,
) {
    for entity in &tilemaps {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// y 翻转：关卡行 0（上）→ `TilePos` 最大行（上）
    #[test]
    fn tile_pos_flips_y() {
        assert_eq!(
            tile_pos_for_cell(IVec2::new(0, 0), 6),
            TilePos { x: 0, y: 5 }
        );
        assert_eq!(
            tile_pos_for_cell(IVec2::new(3, 5), 6),
            TilePos { x: 3, y: 0 }
        );
        assert_eq!(
            tile_pos_for_cell(IVec2::new(2, 1), 4),
            TilePos { x: 2, y: 2 }
        );
    }

    /// 逆变换往返一致（迷雾染色按 `TilePos` 反查关卡格用）
    #[test]
    fn cell_for_tile_pos_roundtrips() {
        for height in [1usize, 4, 7] {
            for y in 0..height as i32 {
                for x in 0..3 {
                    let cell = IVec2::new(x, y);
                    assert_eq!(
                        cell_for_tile_pos(tile_pos_for_cell(cell, height), height),
                        cell
                    );
                }
            }
        }
    }

    /// 水面子格索引：以关卡左上角为原点的 2×2 平铺（行主序，行 0 在贴图顶部）
    #[test]
    fn water_tile_index_tiles_from_top_left() {
        assert_eq!(water_tile_index(IVec2::new(0, 0)), 0);
        assert_eq!(water_tile_index(IVec2::new(1, 0)), 1);
        assert_eq!(water_tile_index(IVec2::new(0, 1)), 2);
        assert_eq!(water_tile_index(IVec2::new(1, 1)), 3);
        // 周期 2：偏移两格回到同一子格
        assert_eq!(water_tile_index(IVec2::new(2, 2)), 0);
        assert_eq!(water_tile_index(IVec2::new(3, 5)), 3);
    }

    /// 调试视图公式抽查（与 `scenes/in_game.rs` 的 `spawn_level_debug_view` 同式）
    #[test]
    fn cell_center_world_matches_debug_view_formula() {
        // 5x5 地图中心格在原点
        assert_eq!(cell_center_world(IVec2::new(2, 2), 5, 5), Vec2::ZERO);
        // 行 0 在上 → 世界 y 为正
        assert_eq!(
            cell_center_world(IVec2::new(0, 0), 5, 5),
            Vec2::new(-32.0, 32.0)
        );
        // 偶数尺寸：中心落在格间，半格 = 8
        assert_eq!(
            cell_center_world(IVec2::new(0, 0), 4, 4),
            Vec2::new(-24.0, 24.0)
        );
    }

    /// 锚点换算对拍：`TilemapAnchor::Center` + y 翻转后的 `center_in_world`
    /// 必须与调试视图公式逐点一致（奇/偶尺寸都验；所有值都是 8 的整数倍，f32 精确）
    #[test]
    fn anchor_center_matches_debug_view_formula() {
        for (w, h) in [(7usize, 5usize), (8, 6), (32, 32)] {
            let map_size = TilemapSize {
                x: w as u32,
                y: h as u32,
            };
            let tile_size = TilemapTileSize {
                x: TILE_PIXELS,
                y: TILE_PIXELS,
            };
            let grid_size: TilemapGridSize = tile_size.into();
            let cells = [
                IVec2::new(0, 0),
                IVec2::new(w as i32 - 1, 0),
                IVec2::new(0, h as i32 - 1),
                IVec2::new(w as i32 - 1, h as i32 - 1),
                IVec2::new(w as i32 / 2, h as i32 / 2),
            ];
            for cell in cells {
                let via_tilemap = tile_pos_for_cell(cell, h).center_in_world(
                    &map_size,
                    &grid_size,
                    &tile_size,
                    &TilemapType::Square,
                    &TilemapAnchor::Center,
                );
                assert_eq!(
                    via_tilemap,
                    cell_center_world(cell, w, h),
                    "格 {cell} @ {w}x{h}"
                );
            }
        }
    }
}
