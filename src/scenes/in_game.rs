//! `InGame` 场景：进入时生成真实关卡（`generate_level`），地形由 16 号渲染域的
//! tilemap 绘制；英雄方块 + 相机跟随归本文件（英雄逻辑在 `actors::hero`），
//! 踩出口下楼重建关卡，Esc 返回 Title。

use bevy::prelude::*;
use rand::RngExt;

use super::text;
use crate::{
    actors::{
        CharStats, DescendRequest, GridPos, Hero, Mob, MobKind, MobSpawnRequest, TurnClock,
        TurnState, TurnWheelSet, bestiary::hero_max_exp,
    },
    assets::FontAssets,
    dungeon::Dungeon,
    levels::{Level, generate_level},
    setting::Settings,
    states::AppState,
};

/// 调试渲染的单格边长（世界单位）
const TILE_SIZE: f32 = 16.0;

/// 英雄调试方块的 z：压在地块（z=0）之上
const HERO_Z: f32 = 1.0;

/// 怪物调试方块的 z：与英雄同层（单格占用保证互不重叠）
const MOB_Z: f32 = 1.0;

/// 英雄调试方块配色：亮黄，与地形调试色板（墙灰/入口绿/出口红）拉开
const HERO_COLOR: Color = Color::srgb(1.0, 0.9, 0.2);

/// 怪物调试配色（任务书约定：Rat 棕 / Snake 青 / Crab 红橙），
/// 真精灵是 25 号域数据 + 下波接线。
fn mob_color(kind: MobKind) -> Color {
    match kind {
        MobKind::Rat => Color::srgb(0.62, 0.4, 0.22),
        MobKind::Snake => Color::srgb(0.2, 0.85, 0.75),
        MobKind::Crab => Color::srgb(0.95, 0.4, 0.15),
    }
}

pub struct InGameScenePlugin;

impl Plugin for InGameScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            (
                setup_level,
                spawn_in_game_root,
                // FontAssets 在 Loading 完成后必然存在；run_if 仅为无资产的
                // MinimalPlugins 集成测试留出跳过路径，生产环境恒为真
                spawn_hud_text.run_if(resource_exists::<FontAssets>),
            )
                .chain(),
        )
        .add_systems(OnExit(AppState::InGame), (teardown_level, reset_camera))
        .add_systems(
            Update,
            (
                return_to_title_on_esc,
                // 下楼在时间轮推进后同帧受理（地形 tilemap 由渲染域随 Level 替换重建）
                descend.run_if(descend_requested).after(TurnWheelSet),
                // 英雄/怪物方块与相机在行动结算后锁定，保证按键当帧视觉就位
                (sync_hero_marker, sync_mob_markers, camera_follow_hero)
                    .run_if(resource_exists::<Level>)
                    .after(TurnWheelSet),
                // HUD 响应式刷新：RunSeed 进场与每次下楼都被整体替换，
                // 以其变更作为"深度/种子已换"的信号（重建 vs 响应式，取响应式）
                refresh_hud_text.run_if(resource_exists_and_changed::<RunSeed>),
                // HP/等级行以英雄组件变更为信号（受击/升级/重生当帧刷新）
                refresh_hp_hud.after(TurnWheelSet),
            )
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// `InGame` 场景根节点标记：HUD 挂其下，集成测试用它断言清理
#[derive(Component)]
pub(crate) struct InGameRoot;

/// HUD 文本标记：响应式刷新用
#[derive(Component)]
struct HudText;

/// 英雄 HP/等级行标记：随英雄组件变更刷新
#[derive(Component)]
struct HpHudText;

/// 本层关卡的世界种子（HUD 展示 + 复现问题用；每层一换）
#[derive(Debug, Resource)]
pub(crate) struct RunSeed(pub u64);

/// 进入地牢即开新一局：重置 `Dungeon`（depth 归 1、限量掉落清零，对照 SPD
/// `Dungeon.init`）再生成首层。下楼不重进状态，不会触发本系统。
fn setup_level(mut commands: Commands, mut dungeon: ResMut<Dungeon>, settings: Res<Settings>) {
    dungeon.init(&settings);
    insert_fresh_level(&mut commands, dungeon.depth);
}

/// 生成 `depth` 对应的新关卡并整体替换 `Level`/`RunSeed` 资源
/// （进场与下楼共用）。种子在应用边界取熵（一层一个），生成管线内部
/// 依旧全程显式 RNG（确定性纪律）。同时发出一次性怪物生成请求
/// （actors 域消费，对应 SPD `Level.create → createMobs`）。
fn insert_fresh_level(commands: &mut Commands, depth: i32) {
    let seed: u64 = rand::rng().random();
    let level = generate_level(seed, depth);
    info!(
        "生成关卡：seed={seed} depth={depth} 尺寸={}x{}",
        level.width(),
        level.height()
    );
    commands.insert_resource(level);
    commands.insert_resource(RunSeed(seed));
    commands.insert_resource(MobSpawnRequest);
}

fn teardown_level(mut commands: Commands) {
    commands.remove_resource::<Level>();
    commands.remove_resource::<RunSeed>();
    commands.remove_resource::<MobSpawnRequest>();
}

/// 退出地牢时把全局相机放回原点：Title 的星空精灵以窗口原点为中心摆放。
/// 全局相机实体归 `scenes.rs` 所有，此处只改其 Transform（17 号计划约定）。
fn reset_camera(mut camera: Single<&mut Transform, With<Camera2d>>) {
    camera.translation = Vec3::ZERO;
}

/// 透明的全屏 UI 根（黑底交给全局 `ClearColor`，不能遮挡世界层的关卡方块）
fn spawn_in_game_root(mut commands: Commands) {
    commands.spawn((
        InGameRoot,
        DespawnOnExit(AppState::InGame),
        Node {
            width: percent(100),
            height: percent(100),
            padding: UiRect::all(px(8)),
            // HUD 两行（提示行 + HP 行）纵向排布
            flex_direction: FlexDirection::Column,
            ..default()
        },
    ));
}

/// 格坐标 → 世界坐标（M2 调试渲染约定，与 16 号渲染域一致）：地图中心对齐
/// 世界原点；地图行 0 在上（关卡 y 向下）、世界 y 向上，故翻转 y——
/// 格 (x,y) → ((x-(w-1)/2)·16, ((h-1)/2-y)·16)。
pub(crate) fn grid_to_world(pos: IVec2, width: usize, height: usize) -> Vec2 {
    Vec2::new(
        (pos.x as f32 - (width as f32 - 1.0) / 2.0) * TILE_SIZE,
        ((height as f32 - 1.0) / 2.0 - pos.y as f32) * TILE_SIZE,
    )
}

/// 英雄调试方块：亮黄小方块（`Sprite::from_color`；不裁 warrior.png——16 号
/// 渲染域并行中，竖切不碰图集，见 17 号计划笔记）。首见英雄补挂
/// Sprite/Transform，之后每帧随格坐标瞬移（平滑插值留给 M3）。
fn sync_hero_marker(
    mut commands: Commands,
    level: Res<Level>,
    mut heroes: Query<(Entity, &GridPos, Option<&mut Transform>), With<Hero>>,
) {
    for (entity, pos, transform) in &mut heroes {
        let translation = grid_to_world(pos.0, level.width(), level.height()).extend(HERO_Z);
        if let Some(mut transform) = transform {
            transform.translation = translation;
        } else {
            commands.entity(entity).insert((
                Sprite::from_color(HERO_COLOR, Vec2::splat(TILE_SIZE * 0.8)),
                Transform::from_translation(translation),
            ));
        }
    }
}

/// 怪物调试方块：按 `bestiary` 种类着色（模式照抄 [`sync_hero_marker`]）。
/// 首见补挂 Sprite/Transform，之后随格坐标瞬移；实体死亡随 despawn 消失。
/// 迷雾遮蔽对照 SPD 语义（怪物精灵仅在 `heroFOV[mob.pos]` 时可见）；
/// 无 `VisibilityMap` 资源的最小测试环境不遮蔽。
fn sync_mob_markers(
    mut commands: Commands,
    level: Res<Level>,
    fog: Option<Res<crate::render::VisibilityMap>>,
    mut mobs: Query<
        (Entity, &GridPos, &Mob, Option<(&mut Transform, &mut Visibility)>),
        Without<Hero>,
    >,
) {
    for (entity, pos, mob, parts) in &mut mobs {
        let translation = grid_to_world(pos.0, level.width(), level.height()).extend(MOB_Z);
        let seen = fog.as_ref().is_none_or(|fog| fog.is_visible(pos.0));
        let target_visibility = if seen {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Some((mut transform, mut visibility)) = parts {
            transform.translation = translation;
            visibility.set_if_neq(target_visibility);
        } else {
            commands.entity(entity).insert((
                Sprite::from_color(mob_color(mob.kind), Vec2::splat(TILE_SIZE * 0.7)),
                Transform::from_translation(translation),
                target_visibility,
            ));
        }
    }
}

/// 相机每帧直接锁定英雄世界坐标（lerp 是 M3 润色）；只改全局相机的 Transform。
fn camera_follow_hero(
    level: Res<Level>,
    hero: Single<&GridPos, With<Hero>>,
    mut camera: Single<&mut Transform, (With<Camera2d>, Without<Hero>)>,
) {
    let target = grid_to_world(hero.0, level.width(), level.height());
    camera.translation.x = target.x;
    camera.translation.y = target.y;
}

/// 下楼请求已置位（英雄踩上 Exit，由 `actors::hero` 的 act 观察者写入）
fn descend_requested(request: Option<Res<DescendRequest>>) -> bool {
    request.is_some_and(|request| request.0)
}

/// 下楼闭环：加深一层 → 移除旧英雄 → 时间轮清零（SPD `Dungeon.newLevel`
/// L297-300 换层前 `Actor.clear()` 的语义）→ 生成新层。旧方块视图由
/// [`refresh_level_debug_view`] 随 `Level` 替换重建，新英雄由 `actors` 的
/// 生成系统在新入口重生（`Dungeon.switchLevel` L474-481 英雄放入口）。
fn descend(
    mut commands: Commands,
    mut dungeon: ResMut<Dungeon>,
    mut request: ResMut<DescendRequest>,
    mut clock: ResMut<TurnClock>,
    mut state: ResMut<TurnState>,
    heroes: Query<Entity, With<Hero>>,
) {
    request.0 = false;
    dungeon.depth += 1;
    for hero in &heroes {
        commands.entity(hero).despawn();
    }
    *clock = TurnClock::default();
    *state = TurnState::default();
    insert_fresh_level(&mut commands, dungeon.depth);
}

/// HUD 单行文案：操作提示 + 深度 + 种子
fn hud_line(depth: i32, seed: u64) -> String {
    format!("{}  depth {depth}  seed {seed}", text::IN_GAME_HUD_PREFIX)
}

/// HP/等级行文案：`HP 20/20  Lv 1  EXP 0/10`
fn hp_hud_line(stats: &CharStats, hero: &Hero) -> String {
    format!(
        "{} {}/{}  {} {}  {} {}/{}",
        text::HUD_HP_LABEL,
        stats.hp,
        stats.ht,
        text::HUD_LVL_LABEL,
        hero.lvl,
        text::HUD_EXP_LABEL,
        hero.exp,
        hero_max_exp(hero.lvl)
    )
}

fn spawn_hud_text(
    mut commands: Commands,
    fonts: Res<FontAssets>,
    seed: Res<RunSeed>,
    dungeon: Res<Dungeon>,
    root: Single<Entity, With<InGameRoot>>,
) {
    let font = TextFont {
        font: fonts.pixel.clone().into(),
        font_size: FontSize::Px(14.0),
        ..default()
    };
    commands.entity(*root).with_children(|parent| {
        parent.spawn((
            HudText,
            Text::new(hud_line(dungeon.depth, seed.0)),
            font.clone(),
            TextColor(Color::srgb(0.75, 0.75, 0.75)),
        ));
        parent.spawn((
            // 英雄在 OnEnter 之后的 Update 才生成，先占位，
            // refresh_hp_hud 以组件变更（含 Added）为信号当帧补写
            HpHudText,
            Text::new(String::new()),
            font,
            TextColor(Color::srgb(0.9, 0.55, 0.55)),
        ));
    });
}

/// HUD 跟随下楼刷新（`Single` 在无 HUD 的无资产测试环境下自动跳过本系统）
fn refresh_hud_text(
    seed: Res<RunSeed>,
    dungeon: Res<Dungeon>,
    mut hud: Single<&mut Text, With<HudText>>,
) {
    hud.0 = hud_line(dungeon.depth, seed.0);
}

/// HP/等级行响应式刷新：英雄 `CharStats`（受击/升级）或 `Hero`（经验入账）
/// 变更时重写；`Single` 在无 HUD 环境下自动跳过。
fn refresh_hp_hud(
    heroes: Query<(&CharStats, &Hero), Or<(Changed<CharStats>, Changed<Hero>)>>,
    mut hud: Single<&mut Text, With<HpHudText>>,
) {
    let Ok((stats, hero)) = heroes.single() else {
        return;
    };
    hud.0 = hp_hud_line(stats, hero);
}

/// Esc 返回标题：M1 用来验证状态切换闭环（正式的游戏内菜单是 M2+ 范围）
fn return_to_title_on_esc(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(AppState::Title);
    }
}
