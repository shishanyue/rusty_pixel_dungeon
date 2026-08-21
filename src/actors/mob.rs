//! 怪物实体域：组件、按深度的生成数量与轮换表、进层生成系统
//! （`docs/plans/22-mobs-combat.md`）。AI 状态机见同域 [`ai`](super::ai)。
//!
//! 对照 SPD：
//! - 生成数量 `n_mobs` ← `RegularLevel.createMobs`/`mobLimit`
//!   （`levels/RegularLevel.java` L205-L316）；
//! - 轮换表 ← `MobSpawner.standardMobRotation`（`actors/mobs/MobSpawner.java`
//!   L71-L97，本文件行号注释均指该两文件）；
//! - 出生格约束 ← `createMobs` 的 do-while 过滤（L264-L275）与
//!   `randomRespawnCell`（L318-L346）的交集简化，见 [`spawn_mobs`]。

use bevy::prelude::*;
use rand::{Rng, RngExt, SeedableRng};

use crate::levels::{Feeling, Level};
use crate::states::AppState;

use super::bestiary::{MobKind, MobStats};
use super::hero::{GridPos, Hero};
use super::scheduler::MOB_PRIO;
use super::turn::{Actor, TurnClock};

/// actors 域回合逻辑统一随机源（战斗掷值、出生格采样、游荡目标）。
///
/// SPD 的对应物是 `Random` 静态生成器栈；本移植照确定性纪律
/// （`docs/plans/01`）收敛为显式资源：系统经 `ResMut` 取用，种子在应用边界
/// 取熵（与 `scenes::in_game` 的关卡种子同模式），测试覆写为固定种子对拍。
#[derive(Resource, Debug)]
pub struct ActorRng(pub rand::rngs::ChaCha12Rng);

impl FromWorld for ActorRng {
    fn from_world(_: &mut World) -> Self {
        Self(rand::rngs::ChaCha12Rng::seed_from_u64(rand::rng().random()))
    }
}

/// 怪物标记 + 种类 + 游荡目标（`Mob.java` 的 `target` 字段在 WANDERING 态的
/// 那一半语义；HUNTING 态的追击目标简化为"恒为英雄当前格"，见 `ai.rs`）。
#[derive(Component, Debug)]
pub struct Mob {
    /// 图鉴种类（数值表键）。
    pub kind: MobKind,
    /// 游荡目的地（`Mob.target`，`randomDestination` 产物）；`None` 待重选。
    pub wander_target: Option<IVec2>,
}

/// "本层怪物请生成"一次性请求：场景域在插入新 `Level` 时一并插入
/// （`scenes::in_game::insert_fresh_level`），[`spawn_mobs`] 消费后移除。
///
/// 不直接以 `Level` 资源变更为触发条件，是为了让既有的英雄/场景集成测试
/// （手工替换 `Level` 而不期待怪物）保持环境不变——生成怪物是"进入新层"
/// 流程的一部分，由场景域显式发起（对应 SPD `Level.create → createMobs`）。
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct MobSpawnRequest;

/// 切比雪夫格距（`Level.distance`，Level.java L1510-L1516）；
/// 邻格判定 `adjacent` 即距离恰为 1（L1518-L1520）。
#[must_use]
pub fn chebyshev(a: IVec2, b: IVec2) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// 进层应生成的怪物数（`RegularLevel.createMobs` L222 与 `mobLimit`
/// L206-L217）：首层固定 8 只教学怪（"so the player can get level 2"）；
/// 其余 `3 + depth % 5 + Int(3)`，Large 氛围 ×1.33 向上取整。
/// （`mobLimit` 首层无护符时为 0 属重生冷却路径，M4 暂不移植重生。）
pub fn n_mobs(depth: i32, feeling: Feeling, rng: &mut impl Rng) -> usize {
    if depth <= 1 {
        return 8; // L221-L222
    }
    let mut mobs = 3 + depth % 5 + rng.random_range(0..3); // L212
    if feeling == Feeling::Large {
        mobs = (mobs as f32 * 1.33).ceil() as i32; // L213-L215
    }
    mobs as usize
}

/// 按深度的标准轮换表（`MobSpawner.standardMobRotation` L71-L97）。
///
/// 图鉴目前只有 Rat/Snake/Crab（15 号域交付），未入库的怪按最近角色替换，
/// TODO（M4+ 图鉴扩充后还原原表）：
/// Gnoll（基础近战）→ Rat；Swarm（骚扰）→ Snake；Slime（肉盾）→ Crab。
#[must_use]
pub fn mob_rotation(depth: i32) -> &'static [MobKind] {
    use MobKind::{Crab, Rat, Snake};
    match depth {
        // L80-L84：2 鼠 + 1 蛇 + 2 豺（→鼠）
        2 => &[Rat, Rat, Snake, Rat, Rat],
        // L85-L91：1 鼠 + 1 蛇 + 3 豺（→鼠）+ 1 蜂群（→蛇）+ 1 蟹
        3 => &[Rat, Snake, Rat, Rat, Rat, Snake, Crab],
        // L92-L97：1 豺（→鼠）+ 1 蜂群（→蛇）+ 2 蟹 + 2 泥怪（→蟹）
        4 => &[Rat, Snake, Crab, Crab, Crab, Crab],
        // L75-L79：深度 1 与兜底——3 鼠 + 1 蛇
        _ => &[Rat, Rat, Rat, Snake],
    }
}

/// 击杀经验（`Mob.destroy`，Mob.java L853：英雄等级超过 `maxLvl` 后不再给）。
#[must_use]
pub fn exp_for_kill(hero_lvl: i32, stats: &MobStats) -> i32 {
    if hero_lvl <= stats.max_lvl {
        stats.exp
    } else {
        0
    }
}

/// SPD 前向 Fisher-Yates（`Random.shuffle`，Random.java L271-L280）。
/// 与 `levels::random::shuffle` 有意重复：文件所有权硬边界（15 号域先例）。
fn shuffle<T>(rng: &mut impl Rng, items: &mut [T]) {
    if items.is_empty() {
        return;
    }
    for i in 0..items.len() - 1 {
        let j = i + rng.random_range(0..(items.len() - i) as i32) as usize;
        if j != i {
            items.swap(i, j);
        }
    }
}

/// 消费 [`MobSpawnRequest`]：清掉上一层残留的怪物实体，再按
/// `n_mobs(depth)` 与轮换表生成本层怪物（`RegularLevel.createMobs`）。
///
/// 出生格约束（任务书钉死的简化交集，对照 L264-L275 / L318-L346）：
/// passable、非入口、非出口、不与英雄相邻（切比雪夫 > 1，含英雄自身格）、
/// 单格单怪（采样不放回）。SPD 另有"入口 8 格 FOV 与 8 步步行禁区"
/// （L235-L252）与房间驱动落位——生成域的房间数据不出 `Level` API，
/// 简化为全图筛选后随机采样，记实现笔记。
///
/// 轮换抽取照 `Level.createMob` 的"耗尽重灌 + 洗牌"语义
/// （Level.java L508-L516）：首层 8 只恰为两整轮 → 6 鼠 2 蛇，比例可测。
pub(super) fn spawn_mobs(
    mut commands: Commands,
    level: Res<Level>,
    clock: Res<TurnClock>,
    mut rng: ResMut<ActorRng>,
    heroes: Query<&GridPos, With<Hero>>,
    old_mobs: Query<Entity, With<Mob>>,
) {
    commands.remove_resource::<MobSpawnRequest>();
    for entity in &old_mobs {
        commands.entity(entity).despawn();
    }

    // 英雄尚未生成时以入口为准——进层/下楼英雄总在入口（Dungeon.switchLevel）
    let hero_pos = heroes.single().map_or(level.entrance, |pos| pos.0);

    let mut cells: Vec<IVec2> = (0..level.size())
        .filter_map(|i| {
            let pos = level.pos_of(i);
            (level.passable[i]
                && pos != level.entrance
                && pos != level.exit
                && chebyshev(pos, hero_pos) > 1)
                .then_some(pos)
        })
        .collect();

    let count = n_mobs(level.depth, level.feeling, &mut rng.0);
    let rotation = mob_rotation(level.depth);
    let mut pool: Vec<MobKind> = Vec::new();
    let mut spawned = 0_usize;

    for _ in 0..count {
        if cells.is_empty() {
            break; // 可用格耗尽（小图测试防御；SPD 的 30 次重试同为尽力而为）
        }
        if pool.is_empty() {
            pool = rotation.to_vec();
            shuffle(&mut rng.0, &mut pool);
        }
        let kind = pool.pop().expect("轮换表非空");
        let cell = cells.swap_remove(rng.0.random_range(0..cells.len() as i32) as usize);
        commands.spawn((
            Mob {
                kind,
                wander_target: None,
            },
            super::ai::AiState::Sleeping, // Mob.java L118：出生即沉睡
            GridPos(cell),
            kind.stats().char_stats,
            // Actor.add 语义（Actor.java L336-L355）：入场 time = 当前 now
            Actor {
                time: clock.now,
                priority: MOB_PRIO,
            },
            DespawnOnExit(AppState::InGame),
        ));
        spawned += 1;
    }
    info!("生成 {spawned} 只怪物（depth {}）", level.depth);
}

#[cfg(test)]
mod tests {
    use bevy::{input::ButtonInput, math::IRect, prelude::*, state::app::StatesPlugin};
    use rand::SeedableRng;
    use rand::rngs::ChaCha12Rng;

    use super::*;
    use crate::actors::{ActorsPlugin, AiState, CharStats};
    use crate::dungeon::DungeonPlugin;
    use crate::levels::terrain::Terrain;
    use crate::scenes::ScenesPlugin;
    use crate::setting::SettingPlugin;

    /// 轮换表对拍（MobSpawner.java L71-L97 + 替换映射）：
    /// 首层 3 鼠 1 蛇原样；2-4 层长度与替换后构成正确。
    #[test]
    fn rotation_tables_match_spd_with_substitutions() {
        use MobKind::{Crab, Rat, Snake};
        let count = |depth: i32, kind: MobKind| {
            mob_rotation(depth).iter().filter(|k| **k == kind).count()
        };

        assert_eq!(mob_rotation(1), &[Rat, Rat, Rat, Snake], "L75-L79 原样");
        // depth 2（L80-L84）：2 鼠 + 1 蛇 + 2 豺→鼠
        assert_eq!(mob_rotation(2).len(), 5);
        assert_eq!(count(2, Rat), 4);
        assert_eq!(count(2, Snake), 1);
        // depth 3（L85-L91）：1 鼠 + 1 蛇 + 3 豺→鼠 + 1 蜂群→蛇 + 1 蟹
        assert_eq!(mob_rotation(3).len(), 7);
        assert_eq!(count(3, Rat), 4);
        assert_eq!(count(3, Snake), 2);
        assert_eq!(count(3, Crab), 1);
        // depth 4（L92-L97）：1 豺→鼠 + 1 蜂群→蛇 + 2 蟹 + 2 泥→蟹
        assert_eq!(mob_rotation(4).len(), 6);
        assert_eq!(count(4, Rat), 1);
        assert_eq!(count(4, Snake), 1);
        assert_eq!(count(4, Crab), 4);
        // 兜底（深度越界回落首层表）
        assert_eq!(mob_rotation(99), mob_rotation(1));
    }

    /// 生成数量对拍：首层恒 8（L221-L222，不掷随机）；2-4 层落在
    /// `3 + depth % 5 + [0, 2]`（L212）；Large 氛围 ×1.33 向上取整（L213-L215）。
    #[test]
    fn n_mobs_matches_regular_level_formula() {
        let mut rng = ChaCha12Rng::seed_from_u64(0);
        for _ in 0..16 {
            assert_eq!(n_mobs(1, Feeling::None, &mut rng), 8);
        }
        for depth in 2..=4 {
            for seed in 0..32_u64 {
                let mut rng = ChaCha12Rng::seed_from_u64(seed);
                let base = (3 + depth % 5) as usize;
                let n = n_mobs(depth, Feeling::None, &mut rng);
                assert!((base..=base + 2).contains(&n), "depth {depth}: {n}");

                let mut rng = ChaCha12Rng::seed_from_u64(seed);
                let large = n_mobs(depth, Feeling::Large, &mut rng);
                assert_eq!(large, (n as f32 * 1.33).ceil() as usize, "Large ×1.33");
            }
        }
    }

    /// 击杀经验门槛（Mob.java L853）：等级不超过 `maxLvl` 给 EXP，超过归零。
    #[test]
    fn exp_gate_follows_max_lvl() {
        let rat = MobKind::Rat.stats();
        assert_eq!(exp_for_kill(1, &rat), 1);
        assert_eq!(exp_for_kill(5, &rat), 1, "恰在 maxLvl 仍给");
        assert_eq!(exp_for_kill(6, &rat), 0, "超过 maxLvl 不给");
        let crab = MobKind::Crab.stats();
        assert_eq!(exp_for_kill(9, &crab), 4);
        assert_eq!(exp_for_kill(10, &crab), 0);
    }

    /// 切比雪夫距离与邻格语义（Level.java L1510-L1520）。
    #[test]
    fn chebyshev_matches_level_distance() {
        use bevy::math::IVec2;
        let o = IVec2::new(3, 3);
        assert_eq!(chebyshev(o, o), 0);
        assert_eq!(chebyshev(o, IVec2::new(4, 4)), 1, "对角也是邻格");
        assert_eq!(chebyshev(o, IVec2::new(3, 5)), 2);
        assert_eq!(chebyshev(o, IVec2::new(0, 4)), 3);
    }

    /// 生成系统集成验收（无渲染，手工铺图）：深度 1 恰 8 只、构成 6 鼠 2 蛇
    /// （轮换表两整轮）；出生格全 passable、非入口/出口、不与英雄相邻、
    /// 单格单怪；组件按 `Actor.add` 语义入轮（time = now、`MOB_PRIO`）、
    /// 出生即沉睡。
    #[test]
    fn spawn_mobs_respects_count_and_placement() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins((SettingPlugin, DungeonPlugin, ActorsPlugin, ScenesPlugin));
        app.update();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        // 手工 20×20 层（内域 18×18 全 Empty），入口/出口同排远隔
        let entrance = IVec2::new(2, 10);
        let exit = IVec2::new(17, 10);
        let mut level = Level::new(20, 20, 1);
        level.fill(IRect::new(1, 1, 19, 19), Terrain::Empty);
        level.set_terrain(entrance, Terrain::Entrance);
        level.set_terrain(exit, Terrain::Exit);
        level.entrance = entrance;
        level.exit = exit;
        app.world_mut().insert_resource(level);

        let mut heroes = app.world_mut().query_filtered::<&mut GridPos, With<Hero>>();
        heroes.single_mut(app.world_mut()).expect("应有英雄").0 = entrance;
        app.world_mut().resource_mut::<ActorRng>().0 = ChaCha12Rng::seed_from_u64(7);
        // 重新请求生成：spawn_mobs 应先清掉进场时真实关卡生成的旧怪
        app.world_mut().insert_resource(MobSpawnRequest);
        app.update();

        assert!(
            app.world().get_resource::<MobSpawnRequest>().is_none(),
            "生成请求应被消费"
        );
        let mut mobs = app
            .world_mut()
            .query::<(&Mob, &GridPos, &Actor, &CharStats, &AiState)>();
        let spawned: Vec<_> = mobs.iter(app.world()).collect();
        assert_eq!(spawned.len(), 8, "首层固定 8 只（RegularLevel L221-L222）");

        let rats = spawned
            .iter()
            .filter(|(mob, ..)| mob.kind == MobKind::Rat)
            .count();
        let snakes = spawned
            .iter()
            .filter(|(mob, ..)| mob.kind == MobKind::Snake)
            .count();
        assert_eq!((rats, snakes), (6, 2), "轮换表 [鼠×3, 蛇×1] 两整轮");

        let level = app.world().resource::<Level>();
        let mut cells = Vec::new();
        for (mob, pos, actor, stats, ai) in &spawned {
            assert!(level.passable[level.index(pos.0)], "出生格必须 passable");
            assert_ne!(pos.0, entrance, "不得生在入口");
            assert_ne!(pos.0, exit, "不得生在出口");
            assert!(
                chebyshev(pos.0, entrance) > 1,
                "不得与英雄相邻：{:?}",
                pos.0
            );
            assert!(!cells.contains(&pos.0), "单格单怪：{:?}", pos.0);
            cells.push(pos.0);
            assert_eq!(actor.priority, MOB_PRIO);
            assert_eq!(actor.time, 0.0, "Actor.add 语义：入场 time = now(0)");
            assert_eq!(**ai, AiState::Sleeping, "出生即沉睡");
            assert_eq!(
                **stats,
                mob.kind.stats().char_stats,
                "战斗数值来自 bestiary"
            );
        }
    }
}
