//! 迷雾（23 号计划并入 26 号渲染二期）：[`VisibilityMap`] 资源维护
//! 可见（heroFOV）/ 已探索（visited）两张表，tilemap 按三态染色。
//!
//! 语义对照：
//! - FOV 重算：`Level.updateFieldOfView`（`levels/Level.java` L1341 调
//!   `ShadowCaster.castShadow`）；触发时机 = 英雄 `GridPos` 变更或 `Level`
//!   资源替换（SPD 在每次英雄行动后 `Dungeon.observe`，效果一致）。
//! - visited 单调累积：`Dungeon.observe`（`Dungeon.java` L931
//!   `visited |= heroFOV`；L935-L938 紧邻 9 格恒记已探索）。
//! - 三态：`tiles/FogOfWar.java` L60-L63 的 VISIBLE/VISITED/INVISIBLE。
//!   MAPPED（魔法测绘卷轴）尚无来源，留 M4+。FogOfWar 的半格平滑迷雾网格
//!   不移植（任务书约定 per-tile 染色）。
//! - 对"场上无 Hero 实体"安全（22 号域并行改英雄）：无英雄 → 全图不可见
//!   （`Level.java` L1342-L1344 盲角色分支 `BArray.setFalse` 的语义），
//!   visited 保留。

use bevy::prelude::*;

use crate::{
    actors::{GridPos, Hero},
    levels::{Feeling, Level},
    utils::cast_shadow,
};

use super::tilemap::cell_for_tile_pos;

/// 默认英雄视距（`Level.java` L157 非黑暗挑战分支的 8）。
pub const VIEW_DISTANCE: i32 = 8;

/// [`Feeling::Dark`] 视距。任务书取 2（= `Level.java` L157 DARKNESS 挑战的
/// 数值）；Java 的 Dark feeling 实为 `round(5*8/8) = 5`（L269-L270），
/// 挑战系统落地后按 SPD 语义拆分校正（笔记记录）。
pub const VIEW_DISTANCE_DARK: i32 = 2;

/// 已探索格保留的亮度（任务书 ~0.45；`FogOfWar.java` L45 默认亮度的
/// visited 遮罩 0x99000000 ≈ 保留 40%，取 0.45 观感更接近截图基准）。
pub const VISITED_BRIGHTNESS: f32 = 0.45;

/// 单格可见性三态（`FogOfWar.java` L60-L63，MAPPED 暂缺来源不设）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellVisibility {
    /// 英雄当前视野内（`FOG_COLORS` visible 行：无遮罩）
    Visible,
    /// 到过/见过但当前不可见（`FOG_COLORS` visited 行：加深）
    Visited,
    /// 从未见过（`FOG_COLORS` invisible 行：全黑）
    Unknown,
}

/// 三态 → tile 染色（乘到贴图色上）。纯函数，供两层 tilemap 共用。
pub fn fog_color(state: CellVisibility) -> Color {
    match state {
        CellVisibility::Visible => Color::WHITE,
        CellVisibility::Visited => Color::srgb(
            VISITED_BRIGHTNESS,
            VISITED_BRIGHTNESS,
            VISITED_BRIGHTNESS,
        ),
        CellVisibility::Unknown => Color::BLACK,
    }
}

/// 英雄视野 + 已探索表。尺寸随 `Level` 重置（换层清空 visited）。
/// 无关卡时为空表（默认值），一切查询返回 [`CellVisibility::Unknown`]。
#[derive(Resource, Debug, Default)]
pub struct VisibilityMap {
    /// 当前视野（SPD `Level.heroFOV`），线性索引 = `y * width + x`
    visible: Vec<bool>,
    /// 已探索（SPD `Level.visited`），只增不减直到换层
    visited: Vec<bool>,
    width: usize,
    height: usize,
}

impl VisibilityMap {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn visible(&self) -> &[bool] {
        &self.visible
    }

    pub fn visited(&self) -> &[bool] {
        &self.visited
    }

    fn is_inside(&self, cell: IVec2) -> bool {
        cell.x >= 0
            && cell.y >= 0
            && (cell.x as usize) < self.width
            && (cell.y as usize) < self.height
    }

    /// 该格是否在英雄当前视野内（越界/空表 = 否）。
    /// 供跨域消费（怪物遮蔽标记等，23 号计划第 4 条）。
    pub fn is_visible(&self, cell: IVec2) -> bool {
        self.state_at(cell) == CellVisibility::Visible
    }

    /// 格三态；越界/空表一律 [`CellVisibility::Unknown`]。
    pub fn state_at(&self, cell: IVec2) -> CellVisibility {
        if !self.is_inside(cell) {
            return CellVisibility::Unknown;
        }
        let index = cell.y as usize * self.width + cell.x as usize;
        if self.visible[index] {
            CellVisibility::Visible
        } else if self.visited[index] {
            CellVisibility::Visited
        } else {
            CellVisibility::Unknown
        }
    }

    /// 换层重置：两张表清零并重定尺寸（SPD 换层重建 `Level.visited` 的语义）。
    fn reset(&mut self, width: usize, height: usize) {
        let size = width * height;
        self.width = width;
        self.height = height;
        self.visible.clear();
        self.visible.resize(size, false);
        self.visited.clear();
        self.visited.resize(size, false);
    }

    /// 以英雄为源重算 FOV 并累积 visited。调用方保证 `hero` 在图内。
    fn update_fov(&mut self, level: &Level, hero: IVec2) {
        // Level.java L157 / 任务书：默认 8，Feeling::Dark 2（见常量注释）
        let distance = if level.feeling == Feeling::Dark {
            VIEW_DISTANCE_DARK
        } else {
            VIEW_DISTANCE
        };
        // Level.updateFieldOfView L1341
        cast_shadow(
            hero.x,
            hero.y,
            level.width() as i32,
            &mut self.visible,
            &level.los_blocking,
            distance,
        );
        // Dungeon.observe L931：visited |= heroFOV
        for (visited, &visible) in self.visited.iter_mut().zip(&self.visible) {
            *visited |= visible;
        }
        // Dungeon.observe L935-L938：紧邻 9 格即使被遮也恒记已探索
        for dy in -1..=1 {
            for dx in -1..=1 {
                let cell = hero + IVec2::new(dx, dy);
                if level.is_inside(cell) {
                    self.visited[level.index(cell)] = true;
                }
            }
        }
    }
}

/// 重算触发器：`Level` 替换（含覆盖式插入）/ 尺寸不符 → 重置 + 重算；
/// 英雄 `GridPos` 变更（含新英雄入场，`Changed` 含 `Added`）→ 重算。
/// 只在真正需要时可变解引用，避免每帧误标 `VisibilityMap` 已变更。
pub(crate) fn recompute_visibility(
    level: Option<Res<Level>>,
    heroes: Query<&GridPos, With<Hero>>,
    moved: Query<(), (With<Hero>, Changed<GridPos>)>,
    mut visibility: ResMut<VisibilityMap>,
) {
    let Some(level) = level else {
        // 退出 InGame（Level 撤除）：清空，防止旧局状态泄漏到下一局
        if visibility.width != 0 || visibility.height != 0 {
            *visibility = VisibilityMap::default();
        }
        return;
    };

    let level_swapped = level.is_changed()
        || visibility.width != level.width()
        || visibility.height != level.height();
    // 多英雄是并行开发期的非法态，取第一个保证不 panic
    let hero = heroes.iter().next().map(|pos| pos.0);

    if level_swapped {
        visibility.reset(level.width(), level.height());
    } else if moved.is_empty() {
        // 无触发。英雄消失（死亡/换层间隙）时清掉残留视野，visited 保留
        if hero.is_none() && visibility.visible.iter().any(|&v| v) {
            visibility.visible.fill(false);
        }
        return;
    }

    match hero {
        Some(pos) if level.is_inside(pos) => visibility.update_fov(&level, pos),
        // 场上无英雄（或坐标越界的非法态）：全图不可见（Level.java L1342-L1344）
        _ => visibility.visible.fill(false),
    }
}

/// 把三态染色刷到两层 tilemap 的所有 tile 上（`run_if` 变更门卫在插件侧）。
/// 新生成的 tile 由 spawn 侧按同一函数烘焙初色，本系统负责后续增量刷新；
/// 换层瞬间查询到的旧 tile 会被随根实体销毁，多染一次无害。
pub(crate) fn apply_fog(
    visibility: Res<VisibilityMap>,
    mut tiles: Query<(&bevy_ecs_tilemap::tiles::TilePos, &mut bevy_ecs_tilemap::tiles::TileColor)>,
) {
    if visibility.height == 0 {
        return;
    }
    for (pos, mut color) in &mut tiles {
        let cell = cell_for_tile_pos(*pos, visibility.height);
        let target = fog_color(visibility.state_at(cell));
        // 仅在颜色实际变化时可变解引用，避免无谓标脏 tilemap chunk
        if color.0 != target {
            color.0 = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::IRect;

    use super::*;
    use crate::{
        levels::terrain::Terrain, render::RenderLevelPlugin, utils::shadow_caster::rounding_table,
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(RenderLevelPlugin);
        app
    }

    /// 外圈全墙、内部全 Empty 的手工关卡（不依赖 `generate_level`——
    /// 24 号域会改生成期 RNG 消耗，出图不可作测试基线）
    fn boxed_level(width: usize, height: usize, depth: i32) -> Level {
        let mut level = Level::new(width, height, depth);
        level.fill(
            IRect::new(1, 1, width as i32 - 1, height as i32 - 1),
            Terrain::Empty,
        );
        level
    }

    fn spawn_hero(app: &mut App, pos: IVec2) -> Entity {
        app.world_mut()
            .spawn((Hero::default(), GridPos(pos)))
            .id()
    }

    fn state_of(app: &App, cell: IVec2) -> CellVisibility {
        app.world().resource::<VisibilityMap>().state_at(cell)
    }

    /// 三态 → 颜色纯函数：可见原色、已探索乘 [`VISITED_BRIGHTNESS`]、未知全黑
    #[test]
    fn fog_color_maps_three_states() {
        assert_eq!(fog_color(CellVisibility::Visible), Color::WHITE);
        assert_eq!(
            fog_color(CellVisibility::Visited),
            Color::srgb(0.45, 0.45, 0.45)
        );
        assert_eq!(fog_color(CellVisibility::Unknown), Color::BLACK);
    }

    /// 空表/越界一律未知，不 panic
    #[test]
    fn empty_map_reports_unknown() {
        let map = VisibilityMap::default();
        assert_eq!(map.state_at(IVec2::ZERO), CellVisibility::Unknown);
        assert_eq!(map.state_at(IVec2::new(-1, 3)), CellVisibility::Unknown);
    }

    /// 空场 FOV 与 `rounding` 表导出的圆形谓词全图逐格对拍
    /// （`ShadowCaster.java` L32-L46 的圆形修正；视距 8 = Level.java L157）
    #[test]
    fn empty_room_fov_matches_rounding_circle() {
        let mut app = test_app();
        app.insert_resource(boxed_level(25, 25, 1));
        spawn_hero(&mut app, IVec2::new(12, 12));
        app.update();

        let caps = &rounding_table()[VIEW_DISTANCE as usize];
        let map = app.world().resource::<VisibilityMap>();
        for y in 0..25 {
            for x in 0..25 {
                let (dx, dy) = ((x - 12i32).abs(), (y - 12i32).abs());
                let (a, b) = (dx.max(dy), dx.min(dy));
                let expected = a == 0 || (a <= VIEW_DISTANCE && b <= caps[a as usize]);
                assert_eq!(
                    map.state_at(IVec2::new(x, y)) == CellVisibility::Visible,
                    expected,
                    "格 ({x},{y}) 可见性与 rounding 圆不符"
                );
            }
        }
    }

    /// 墙柱阴影：柱后直线格全部不可见，柱本身与斜侧格可见
    /// （递归阴影投射语义，`ShadowCaster.java` L136-L152 的遮挡收缩）
    #[test]
    fn wall_blocks_sight_behind_it() {
        let mut app = test_app();
        let mut level = boxed_level(21, 21, 1);
        level.set_terrain(IVec2::new(10, 8), Terrain::Wall);
        app.insert_resource(level);
        spawn_hero(&mut app, IVec2::new(10, 10));
        app.update();

        // 柱本身可见（遮挡格自身进入视野）
        assert_eq!(state_of(&app, IVec2::new(10, 8)), CellVisibility::Visible);
        // 柱后正上方直线（视距 8 内）全部不可见
        for y in 2..=7 {
            assert_eq!(
                state_of(&app, IVec2::new(10, y)),
                CellVisibility::Unknown,
                "(10,{y}) 应被柱挡住"
            );
        }
        // 阴影锥外的斜侧格可见
        assert_eq!(state_of(&app, IVec2::new(9, 7)), CellVisibility::Visible);
        assert_eq!(state_of(&app, IVec2::new(11, 7)), CellVisibility::Visible);
        // 反方向不受影响
        assert_eq!(state_of(&app, IVec2::new(10, 13)), CellVisibility::Visible);
    }

    /// visited 单调累积：移动后旧视野退化为已探索、不清零；
    /// 两段视野都不覆盖的格保持未知
    #[test]
    fn visited_accumulates_monotonically() {
        let mut app = test_app();
        app.insert_resource(boxed_level(25, 25, 1));
        let hero = spawn_hero(&mut app, IVec2::new(4, 4));
        app.update();
        assert_eq!(state_of(&app, IVec2::new(4, 4)), CellVisibility::Visible);
        assert_eq!(state_of(&app, IVec2::new(20, 20)), CellVisibility::Unknown);

        app.world_mut().get_mut::<GridPos>(hero).unwrap().0 = IVec2::new(20, 20);
        app.update();
        assert_eq!(state_of(&app, IVec2::new(20, 20)), CellVisibility::Visible);
        assert_eq!(
            state_of(&app, IVec2::new(4, 4)),
            CellVisibility::Visited,
            "旧视野应保留为已探索"
        );
        assert_eq!(
            state_of(&app, IVec2::new(20, 4)),
            CellVisibility::Unknown,
            "从未见过的格保持未知"
        );
    }

    /// 换层重置：覆盖式插入新 `Level`（下楼路径，不 remove）即清空 visited
    #[test]
    fn level_swap_resets_visited() {
        let mut app = test_app();
        app.insert_resource(boxed_level(25, 25, 1));
        let hero = spawn_hero(&mut app, IVec2::new(4, 4));
        app.update();
        app.world_mut().get_mut::<GridPos>(hero).unwrap().0 = IVec2::new(20, 20);
        app.update();
        assert_eq!(state_of(&app, IVec2::new(4, 4)), CellVisibility::Visited);

        // 同尺寸覆盖插入（is_changed 边沿）：旧 visited 必须清空
        app.insert_resource(boxed_level(25, 25, 2));
        app.update();
        assert_eq!(
            state_of(&app, IVec2::new(4, 4)),
            CellVisibility::Unknown,
            "换层后旧探索区应清空"
        );
        // 新层立即以英雄现位重算（英雄仍在 (20,20)）
        assert_eq!(state_of(&app, IVec2::new(20, 20)), CellVisibility::Visible);
    }

    /// `Feeling::Dark` 视距 2（任务书取 DARKNESS 挑战数值，Level.java L157；
    /// 补角语义 ShadowCaster.java L87-L92：距离 2 是完整 5×5 方块）
    #[test]
    fn dark_feeling_shrinks_view_to_two() {
        let mut app = test_app();
        let mut level = boxed_level(15, 15, 1);
        level.feeling = Feeling::Dark;
        app.insert_resource(level);
        spawn_hero(&mut app, IVec2::new(7, 7));
        app.update();

        let map = app.world().resource::<VisibilityMap>();
        let visible_count = map.visible().iter().filter(|&&v| v).count();
        assert_eq!(visible_count, 25, "距离 2 补角后可见区恰为 5×5");
        for cell in [IVec2::new(9, 9), IVec2::new(5, 5), IVec2::new(7, 9)] {
            assert_eq!(map.state_at(cell), CellVisibility::Visible);
        }
        for cell in [IVec2::new(10, 7), IVec2::new(7, 4), IVec2::new(10, 10)] {
            assert_eq!(map.state_at(cell), CellVisibility::Unknown);
        }
    }

    /// 场上无 Hero / 英雄坐标越界 / 英雄消失都安全：无英雄全图不可见
    /// （Level.java L1342-L1344），visited 保留
    #[test]
    fn survives_missing_or_invalid_hero() {
        let mut app = test_app();
        app.insert_resource(boxed_level(25, 25, 1));
        app.update();
        let map = app.world().resource::<VisibilityMap>();
        assert!(map.visible().iter().all(|&v| !v), "无英雄应全图不可见");

        let hero = spawn_hero(&mut app, IVec2::new(4, 4));
        app.update();
        assert_eq!(state_of(&app, IVec2::new(4, 4)), CellVisibility::Visible);

        // 越界坐标（并行开发期的非法态）：清空视野而非 panic
        app.world_mut().get_mut::<GridPos>(hero).unwrap().0 = IVec2::new(-3, -3);
        app.update();
        assert_eq!(state_of(&app, IVec2::new(4, 4)), CellVisibility::Visited);

        // 英雄回到图内再消失：可见区退化为已探索
        app.world_mut().get_mut::<GridPos>(hero).unwrap().0 = IVec2::new(4, 4);
        app.update();
        assert_eq!(state_of(&app, IVec2::new(4, 4)), CellVisibility::Visible);
        app.world_mut().despawn(hero);
        app.update();
        assert_eq!(state_of(&app, IVec2::new(4, 4)), CellVisibility::Visited);
        assert!(
            app.world()
                .resource::<VisibilityMap>()
                .visible()
                .iter()
                .all(|&v| !v)
        );
    }
}
