//! 渲染域入口（M3 图集渲染 + 渲染二期）：[`RenderLevelPlugin`] 监听 `Level`
//! 资源的插入/替换/移除，用 `bevy_ecs_tilemap` 生成/销毁水面层 + 地形层，
//! 并维护迷雾（[`VisibilityMap`] 三态染色）。
//!
//! 坐标与 `scenes/in_game.rs` 调试方块视图完全重合、z 更低（[`TERRAIN_LAYER_Z`]
//! / [`WATER_LAYER_Z`]）。地形 → 图集索引的映射与拼接/变体算法见
//! [`tile_sheet`]（照抄 SPD `DungeonTileSheet.java`），迷雾见 [`visibility`]。

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::{
    assets::{EnvironmentCollection, EnvironmentType},
    levels::Level,
};

pub mod tile_sheet;
pub mod tilemap;
pub mod visibility;

pub use tilemap::{
    LevelRenderRoot, TERRAIN_LAYER_Z, TILE_PIXELS, TerrainAtlas, TerrainTilemap, WATER_LAYER_Z,
    WaterTexture, WaterTilemap, cell_center_world, cell_for_tile_pos, spawn_terrain_tilemap,
    spawn_water_tilemap, tile_pos_for_cell, water_tile_index,
};
pub use visibility::{
    CellVisibility, VIEW_DISTANCE, VIEW_DISTANCE_DARK, VISITED_BRIGHTNESS, VisibilityMap,
    fog_color,
};

pub struct RenderLevelPlugin;

impl Plugin for RenderLevelPlugin {
    fn build(&self, app: &mut App) {
        // TilemapPlugin 的渲染路径（render feature 且未开 atlas）在 build 时
        // 直接取 RenderApp 子应用，MinimalPlugins 集成测试环境没有它会 panic，
        // 因此仅在真实渲染环境挂载；纯组件数据的 spawn/despawn 不依赖该插件。
        if app.is_plugin_added::<bevy::render::RenderPlugin>()
            && !app.is_plugin_added::<TilemapPlugin>()
        {
            app.add_plugins(TilemapPlugin);
        }

        app.init_resource::<VisibilityMap>();

        app.add_systems(
            Update,
            (
                // 资产集合就绪后缓存两张贴图句柄（一次性）；EnvironmentCollection
                // 不存在（无资产的 MinimalPlugins 测试）则整条渲染链静默不生效，
                // 与项目既有 run_if(resource_exists) 测试模式一致
                cache_terrain_atlas.run_if(
                    resource_exists::<EnvironmentCollection>
                        .and_then(not(resource_exists::<TerrainAtlas>)),
                ),
                cache_water_texture.run_if(
                    resource_exists::<EnvironmentCollection>
                        .and_then(not(resource_exists::<WaterTexture>)),
                ),
                // 迷雾重算先于 spawn：新 tilemap 的 tile 出生即烘焙正确迷雾色
                visibility::recompute_visibility,
                // 变更边沿而非 added 边沿：覆盖式换层（下楼）也要重建，见系统注释
                tilemap::spawn_on_level_changed.run_if(
                    resource_exists_and_changed::<Level>
                        .and_then(resource_exists::<TerrainAtlas>)
                        .and_then(resource_exists::<WaterTexture>),
                ),
                tilemap::despawn_on_level_removed.run_if(resource_removed::<Level>),
                // 已存在 tile 的染色增量刷新（新 tile 由 spawn 烘焙初色）
                visibility::apply_fog.run_if(resource_exists_and_changed::<VisibilityMap>),
            )
                .chain(),
        );
    }
}

/// 把地形图集句柄从资产集合缓存为 [`TerrainAtlas`]（M3 固定下水道图集；
/// 分区域换图集是后续里程碑，届时改写本系统即可）
fn cache_terrain_atlas(mut commands: Commands, environment: Res<EnvironmentCollection>) {
    commands.insert_resource(TerrainAtlas(environment.get(EnvironmentType::TilesSewers)));
}

/// 把水面贴图句柄缓存为 [`WaterTexture`]（M3 固定下水道 `water0.png`，
/// `Level.waterTex()` 的分区域版本随图集切换一并留后续里程碑）
fn cache_water_texture(mut commands: Commands, environment: Res<EnvironmentCollection>) {
    commands.insert_resource(WaterTexture(environment.get(EnvironmentType::WaterSewers)));
}

#[cfg(test)]
mod tests {
    use bevy::math::IRect;

    use super::*;
    use crate::{
        actors::{GridPos, Hero},
        levels::terrain::Terrain,
        states::AppState,
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(RenderLevelPlugin);
        app
    }

    fn inject_textures(app: &mut App) {
        app.insert_resource(TerrainAtlas(Handle::default()));
        app.insert_resource(WaterTexture(Handle::default()));
    }

    /// 内圈挖空的小关卡：(2,1) 单格水验证岸边拼接，(3..=5, 1..=3) 3×3 水塘的
    /// 中心 (4,2) 四邻皆水验证纯水格
    fn sample_level(width: usize, height: usize) -> Level {
        let mut level = Level::new(width, height, 1);
        level.fill(
            IRect::new(1, 1, width as i32 - 1, height as i32 - 1),
            Terrain::Empty,
        );
        level.set_terrain(IVec2::new(2, 1), Terrain::Water);
        if width >= 8 && height >= 6 {
            level.fill(IRect::new(3, 1, 6, 4), Terrain::Water);
        }
        level
    }

    fn roots<M: Component>(app: &mut App) -> Vec<Entity> {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<M>>();
        query.iter(world).collect()
    }

    fn single_root<M: Component>(app: &mut App) -> Entity {
        let found = roots::<M>(app);
        assert_eq!(found.len(), 1, "应有唯一 {} 根实体", core::any::type_name::<M>());
        found[0]
    }

    fn tile_count(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query::<&TilePos>();
        query.iter(world).len()
    }

    fn tile_at(app: &mut App, root: Entity, cell: IVec2, level_height: usize) -> Entity {
        let storage = app.world().get::<TileStorage>(root).unwrap();
        storage
            .get(&tile_pos_for_cell(cell, level_height))
            .expect("格内应有 tile")
    }

    /// 双层 tilemap 随 Level 插入而生成、随 Level 移除而销毁（含 tile 级联）；
    /// 地形层水格出拼接索引，水面层全图平铺
    #[test]
    fn tilemap_follows_level_resource() {
        let mut app = test_app();
        inject_textures(&mut app);
        app.insert_resource(sample_level(8, 6));
        app.update();

        let terrain_root = single_root::<TerrainTilemap>(&mut app);
        let water_root = single_root::<WaterTilemap>(&mut app);
        assert_eq!(roots::<LevelRenderRoot>(&mut app).len(), 2, "水面 + 地形两层");
        assert_eq!(tile_count(&mut app), 2 * 8 * 6, "每层每格一个 tile 实体");

        let world = app.world();
        let size = world.get::<TilemapSize>(terrain_root).unwrap();
        assert_eq!((size.x, size.y), (8, 6));
        assert_eq!(
            world.get::<Transform>(terrain_root).unwrap().translation,
            Vec3::new(0.0, 0.0, TERRAIN_LAYER_Z),
            "地形层原点 + 沉到角色之下"
        );
        assert_eq!(
            world.get::<Transform>(water_root).unwrap().translation,
            Vec3::new(0.0, 0.0, WATER_LAYER_Z),
            "水面层压在地形层之下"
        );
        assert_eq!(
            world.get::<TilemapAnchor>(terrain_root),
            Some(&TilemapAnchor::Center)
        );
        for root in [terrain_root, water_root] {
            assert_eq!(
                world.get::<DespawnOnExit<AppState>>(root).map(|d| d.0),
                Some(AppState::InGame),
                "退出 InGame 的双保险标记"
            );
        }

        // 关卡格 (2,1) 单格水：上 Wall／右 Water 不算岸，下 (2,2) Empty +4、
        // 左 (1,1) Empty +8 → WATER+12（stitchWaterTile，Java L162-L170）
        let shore = tile_at(&mut app, terrain_root, IVec2::new(2, 1), 6);
        let world = app.world();
        assert_eq!(
            world.get::<TileTextureIndex>(shore),
            Some(&TileTextureIndex(tile_sheet::WATER + 12))
        );
        assert_eq!(
            world.get::<TileVisible>(shore),
            Some(&TileVisible(true)),
            "岸边拼接格正常渲染"
        );
        // 水塘中心 (4,2) 四邻皆水 → 基准格且不渲染（needsRender，
        // DungeonTerrainTilemap.java L114-L117），露出水面层
        let pure = tile_at(&mut app, terrain_root, IVec2::new(4, 2), 6);
        let world = app.world();
        assert_eq!(
            world.get::<TileTextureIndex>(pure),
            Some(&TileTextureIndex(tile_sheet::WATER))
        );
        assert_eq!(world.get::<TileVisible>(pure), Some(&TileVisible(false)));

        // 左上角 (0,0) = Wall → 平铺墙（含确定性变体）
        let corner = tile_at(&mut app, terrain_root, IVec2::new(0, 0), 6);
        let level = app.world().resource::<Level>();
        let variance =
            tile_sheet::tile_variance(tile_sheet::variance_seed(level), level.size());
        let expected = tile_sheet::tile_visual_flat(level, &variance, IVec2::ZERO);
        let world = app.world();
        assert_eq!(
            world.get::<TileTextureIndex>(corner),
            Some(&TileTextureIndex(expected))
        );

        // 水面层：以左上角为原点 2×2 平铺，全图有 tile（非水格被地形盖住）
        let water_tile = tile_at(&mut app, water_root, IVec2::new(4, 2), 6);
        let world = app.world();
        assert_eq!(
            world.get::<TileTextureIndex>(water_tile),
            Some(&TileTextureIndex(water_tile_index(IVec2::new(4, 2))))
        );
        assert_eq!(world.get::<TileVisible>(water_tile), Some(&TileVisible(true)));

        // tile 是根的子级，销毁可级联
        assert_eq!(
            world.get::<ChildOf>(shore).map(ChildOf::parent),
            Some(terrain_root)
        );

        // 移除 Level → 两层根与全部 tile 销毁
        app.world_mut().remove_resource::<Level>();
        app.update();
        assert!(
            roots::<LevelRenderRoot>(&mut app).is_empty(),
            "Level 移除后 tilemap 应销毁"
        );
        assert_eq!(tile_count(&mut app), 0, "tile 实体应随根级联销毁");
    }

    /// 无贴图资源（无资产测试环境）时静默跳过；补上贴图并重插 Level 后恢复生成
    #[test]
    fn tilemap_skipped_without_atlas_then_recovers() {
        let mut app = test_app();
        app.insert_resource(sample_level(4, 4));
        app.update();
        assert!(
            roots::<LevelRenderRoot>(&mut app).is_empty(),
            "无 TerrainAtlas/WaterTexture 不应生成"
        );

        // 变更边沿：补上贴图后覆盖式重插 Level 即可恢复（无需先 remove）
        inject_textures(&mut app);
        app.insert_resource(sample_level(5, 3));
        app.update();
        assert_eq!(roots::<TerrainTilemap>(&mut app).len(), 1);
        assert_eq!(roots::<WaterTilemap>(&mut app).len(), 1);
        assert_eq!(tile_count(&mut app), 2 * 5 * 3);
    }

    /// 覆盖式换层（下楼路径：`insert_resource` 直接覆盖、不先 remove——
    /// bevy 0.19 不重置 added tick）也必须重建 tilemap
    #[test]
    fn level_overwrite_without_remove_rebuilds_tilemap() {
        let mut app = test_app();
        inject_textures(&mut app);
        app.insert_resource(sample_level(8, 6));
        app.update();
        assert_eq!(tile_count(&mut app), 2 * 8 * 6);

        app.insert_resource(sample_level(5, 3));
        app.update();

        let terrain_root = single_root::<TerrainTilemap>(&mut app);
        let size = app.world().get::<TilemapSize>(terrain_root).unwrap();
        assert_eq!((size.x, size.y), (5, 3), "覆盖插入后应按新关卡重建");
        assert_eq!(tile_count(&mut app), 2 * 5 * 3, "不残留旧层 tile");
    }

    /// 同帧 remove+insert 会丢失 `resource_removed` 边沿，
    /// spawn 侧的防御性清理保证不残留双份 tilemap
    #[test]
    fn same_frame_level_swap_leaves_single_tilemap() {
        let mut app = test_app();
        inject_textures(&mut app);
        app.insert_resource(sample_level(8, 6));
        app.update();
        assert_eq!(roots::<LevelRenderRoot>(&mut app).len(), 2);

        app.world_mut().remove_resource::<Level>();
        app.insert_resource(sample_level(5, 3));
        app.update();

        let terrain_root = single_root::<TerrainTilemap>(&mut app);
        assert_eq!(roots::<LevelRenderRoot>(&mut app).len(), 2, "换层后每层各一");
        let size = app.world().get::<TilemapSize>(terrain_root).unwrap();
        assert_eq!((size.x, size.y), (5, 3));
        assert_eq!(tile_count(&mut app), 2 * 5 * 3);
    }

    /// 迷雾端到端：tile 出生即带三态初色，英雄移动后增量刷新两层染色，
    /// 英雄消失可见区退化为已探索
    #[test]
    fn fog_tracks_hero_movement() {
        let mut app = test_app();
        inject_textures(&mut app);
        let mut level = Level::new(25, 25, 1);
        level.fill(IRect::new(1, 1, 24, 24), Terrain::Empty);
        app.insert_resource(level);
        let hero = app
            .world_mut()
            .spawn((Hero::default(), GridPos(IVec2::new(4, 4))))
            .id();
        app.update();

        let terrain_root = single_root::<TerrainTilemap>(&mut app);
        let water_root = single_root::<WaterTilemap>(&mut app);
        let near = tile_at(&mut app, terrain_root, IVec2::new(4, 4), 25);
        let far = tile_at(&mut app, terrain_root, IVec2::new(20, 20), 25);
        let water_near = tile_at(&mut app, water_root, IVec2::new(4, 4), 25);
        let world = app.world();
        let visible = fog_color(CellVisibility::Visible);
        let visited = fog_color(CellVisibility::Visited);
        let unknown = fog_color(CellVisibility::Unknown);
        assert_eq!(world.get::<TileColor>(near).unwrap().0, visible, "出生烘焙");
        assert_eq!(world.get::<TileColor>(water_near).unwrap().0, visible, "水面层同染");
        assert_eq!(world.get::<TileColor>(far).unwrap().0, unknown, "远处未知全黑");

        // 英雄瞬移到远角：旧视野退化为已探索、新视野点亮（含水面层）
        app.world_mut().get_mut::<GridPos>(hero).unwrap().0 = IVec2::new(20, 20);
        app.update();
        let world = app.world();
        assert_eq!(world.get::<TileColor>(far).unwrap().0, visible);
        assert_eq!(world.get::<TileColor>(near).unwrap().0, visited);
        assert_eq!(world.get::<TileColor>(water_near).unwrap().0, visited);
        // 两段视野都不覆盖的格保持未知
        let never_seen = tile_at(&mut app, terrain_root, IVec2::new(20, 4), 25);
        assert_eq!(app.world().get::<TileColor>(never_seen).unwrap().0, unknown);

        // 英雄消失（22 号域并行改英雄）：不 panic，可见区退化为已探索
        app.world_mut().despawn(hero);
        app.update();
        let world = app.world();
        assert_eq!(world.get::<TileColor>(far).unwrap().0, visited);
        assert_eq!(world.get::<TileColor>(near).unwrap().0, visited);
    }
}
