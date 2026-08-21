//! 怪物 AI 状态机与行动观察者（`docs/plans/22-mobs-combat.md`）。
//!
//! 逐行对照 `actors/mobs/Mob.java`（下文行号未注明文件者均指该文件）：
//! `act()` 主流程（L225-L270）→ 内部类状态机 `Sleeping/Wandering/Hunting`
//! （L1073-L1318）。手写 enum，不引状态机库。
//!
//! ## 任务书钉死的简化（对照原文的差异清单见计划文档实现笔记）
//!
//! - 状态只保留三态：无 `FLEEING/INVESTIGATING/PASSIVE`，无 `alerted`
//!   跨怪传播（Swarm Intelligence 挑战）与 `recentlyAttackedBy` 换目标；
//! - 睡眠/游荡的警觉掷值（L1118 `1/(dist+stealth)`、L1170
//!   `1/(dist/2+stealth)`）简化为"英雄进 FOV 即察觉"；
//! - Hunting 丢失视野持续追击英雄当前格（原文回落 WANDERING 并记忆最后
//!   目击点，L1252-L1260/L1311-L1315）；
//! - 视距固定 8（`Char.java` L190 `viewDistance` 默认值；SPD 的黑暗氛围
//!   减视距属 26 号渲染/迷雾域）。
//!
//! ## 与时间轮的契约
//!
//! 每次 act 必花时间（任一分支都 `spend`），满足 `process_turns` 的活锁
//! 保护；唯一的"零耗时转移"是游荡察觉敌人（L1186-L1204 `noticeEnemy`
//! 返回 true 且不 spend，Java 靠时间轮同刻重选实现"当即再动"），此处
//! 等价地在同一次 act 内直落 Hunting 分支执行。

use bevy::prelude::*;
use rand::{Rng, RngExt};

use crate::levels::Level;
use crate::states::AppState;
use crate::utils::{PathFinder, cast_shadow};

use super::char_stats::CharStats;
use super::hero::{GridPos, Hero};
use super::melee::{self, MeleeOutcome};
use super::mob::{ActorRng, Mob, chebyshev};
use super::scheduler::TICK;
use super::turn::{ActTurn, Actor, TurnState};

/// 怪物视距（`Char.java` L190 `viewDistance = 8` 默认值）。氛围修正属后续。
pub const MOB_VIEW_DISTANCE: i32 = 8;

/// 惊醒耗时（Mob.java L134 `TIME_TO_WAKE_UP = 1f`，`awaken` L1160 消费）。
const TIME_TO_WAKE_UP: f32 = 1.0;

/// AI 状态（Mob.java L112-L118 的实例字段状态机收敛为三态枚举，
/// 简化清单见模块注释）。出生即 [`AiState::Sleeping`]（L118）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiState {
    /// 沉睡（`Sleeping` L1073-L1162）：静止，英雄进 FOV 惊醒转 Hunting。
    Sleeping,
    /// 游荡（`Wandering` L1164-L1226）：朝随机目的地踱步，见敌当即转 Hunting。
    Wandering,
    /// 追猎（`Hunting` L1231-L1318）：邻格攻击，否则寻路逼近。
    Hunting,
}

/// 怪物的 act()（对照 `Mob.act` L225-L270 + 状态机分发）。顶层观察者收到
/// 所有 [`ActTurn`]，按 [`Mob`] 组件过滤（`dummy.rs` 分发模式）。
///
/// 死亡即时性说明：观察者内 `Commands` 的冲刷在 `process_turns` 独占期之后
/// （17 号域笔记同款结论），故"击杀"由攻击方立即把受害者 HP 置 0 并失活/
/// 挂起（直写组件/资源），despawn 走延迟命令——本观察者对英雄之死即如此。
pub(super) fn mob_act(
    on: On<ActTurn>,
    mut mobs: Query<(&mut Actor, &mut Mob, &mut AiState, &mut GridPos, &CharStats), Without<Hero>>,
    mut heroes: Query<(&GridPos, &mut CharStats), (With<Hero>, Without<Mob>)>,
    level: Option<Res<Level>>,
    mut state: ResMut<TurnState>,
    mut rng: ResMut<ActorRng>,
    // Option：turn.rs 的无状态测试环境没有 `NextState`（不装 StatesPlugin）
    mut next_state: Option<ResMut<NextState<AppState>>>,
) {
    if !mobs.contains(on.entity) {
        return; // 非怪物，交给其他行为观察者
    }
    let Some(level) = level else {
        // 无关卡语境（怪物只随关卡生成，理论不可达）：跳过回合防活锁
        if let Ok((mut actor, ..)) = mobs.get_mut(on.entity) {
            actor.spend(TICK);
        }
        return;
    };

    // 英雄（存活者才是可选敌人，Mob.act L252 `enemy.isAlive()`）
    let mut hero = heroes
        .single_mut()
        .ok()
        .filter(|(_, stats)| stats.is_alive());

    // 占用底图：任何存活 Char 所站格不可走（Dungeon.findPassable 语义；SPD
    // 只排除自身 FOV 内可见者，简化为全排，记笔记）。自身格同样置 false，
    // PathFinder 的 `n == from` 旁路（对照 SPD）保证不影响以自己为起点的寻路。
    let mut passable = level.passable.clone();
    for (_, _, _, mob_pos, mob_stats) in mobs.iter() {
        // is_inside 防御新旧关卡交替帧（Level 已换、旧怪 despawn 命令未冲刷）
        if mob_stats.is_alive() && level.is_inside(mob_pos.0) {
            let index = level.index(mob_pos.0);
            passable[index] = false;
        }
    }
    if let Some((hero_pos, _)) = &hero {
        passable[level.index(hero_pos.0)] = false;
    }

    let Ok((mut actor, mut mob, mut ai, mut pos, stats)) = mobs.get_mut(on.entity) else {
        return;
    };
    if !stats.is_alive() {
        return; // 已死待清理（同帧被击杀后不应再被选中，防御分支）
    }
    if !level.is_inside(pos.0) {
        // 关卡已被整体替换而本怪尚未随之清理（测试手工换图等场景）：
        // 跳过回合防活锁，等 spawn_mobs / DespawnOnExit 收尸
        actor.spend(TICK);
        return;
    }

    // 各怪自算 FOV（Mob.act L252 `fieldOfView[enemy.pos]`；SPD 由
    // Level.updateFieldOfView 维护每怪缓存数组，此处每次行动即算即弃）
    let mut fov = vec![false; level.size()];
    cast_shadow(
        pos.0.x,
        pos.0.y,
        level.width() as i32,
        &mut fov,
        &level.los_blocking,
        MOB_VIEW_DISTANCE,
    );
    let enemy_in_fov = hero
        .as_ref()
        .is_some_and(|(hero_pos, _)| fov[level.index(hero_pos.0)]);

    // 状态机分发（Mob.act L261 `state.act(enemyInFOV, justAlerted)`）。
    // 循环至多两轮：仅"游荡察觉"经 continue 直落 Hunting（见模块注释）。
    loop {
        match *ai {
            AiState::Sleeping => {
                if enemy_in_fov {
                    // awaken 敌在视野分支（L1139-L1144）：转 HUNTING 并花
                    // 1 回合醒来（L1160）。警觉掷值（L1118）简化为见即醒。
                    *ai = AiState::Hunting;
                    info!("{:?} 被惊醒，开始追猎", mob.kind);
                    actor.spend(TIME_TO_WAKE_UP);
                } else {
                    // 继续沉睡（L1128-L1130）
                    actor.spend(TICK);
                }
                break;
            }
            AiState::Wandering => {
                if enemy_in_fov {
                    // noticeEnemy（L1186-L1204）：零耗时转 Hunting，当即行动
                    *ai = AiState::Hunting;
                    info!("{:?} 发现英雄，开始追猎", mob.kind);
                    continue;
                }
                // continueWandering（L1207-L1220）
                let step = mob
                    .wander_target
                    .filter(|target| *target != pos.0)
                    .and_then(|target| step_towards(&level, &passable, pos.0, target));
                if let Some(next) = step {
                    pos.0 = next;
                    actor.spend(TICK / stats.base_speed); // L1212 spend(1/speed)
                } else {
                    // 目的地失效/已达/堵死：重选并等待（L1214-L1216）
                    mob.wander_target = random_destination(&level, &mut rng.0);
                    actor.spend(TICK);
                }
                break;
            }
            AiState::Hunting => {
                // 丢失视野持续追击（任务书简化）：目标恒为英雄当前格
                let Some((hero_pos, hero_stats)) = &mut hero else {
                    // 无敌可寻（英雄已亡/离场）：原地等待
                    actor.spend(TICK);
                    break;
                };
                let hero_cell = hero_pos.0;
                if chebyshev(pos.0, hero_cell) == 1 {
                    // canAttack = 切比雪夫邻格（L477-L479，含对角）→ doAttack
                    // （L661-L673）：attack + spend(attackDelay)，基础 1.0
                    match melee::resolve_melee(stats, hero_stats, &mut rng.0) {
                        MeleeOutcome::Miss => {
                            info!("{:?} 攻击英雄：被闪避", mob.kind);
                        }
                        MeleeOutcome::Hit {
                            rolled,
                            blocked,
                            taken,
                        } => {
                            hero_stats.take_damage(taken);
                            info!(
                                "{:?} 攻击英雄：命中 {taken} 伤害（掷 {rolled} - 甲 {blocked}），英雄 HP {}/{}",
                                mob.kind, hero_stats.hp, hero_stats.ht
                            );
                            if !hero_stats.is_alive() {
                                // 英雄死亡（Dungeon.fail 的最小语义）：挂起时间
                                // 轮并回 Title，GameOver 场景属后续里程碑
                                warn!("英雄被 {:?} 击杀，返回标题", mob.kind);
                                *state = TurnState::WaitingForInput;
                                if let Some(next) = next_state.as_mut() {
                                    next.set(AppState::Title);
                                }
                            }
                        }
                    }
                    actor.spend(stats.attack_delay);
                } else if let Some(next) = step_towards(&level, &passable, pos.0, hero_cell) {
                    // getCloser（L509-L631 的重算路径单步化）
                    pos.0 = next;
                    actor.spend(TICK / stats.base_speed); // Hunting L1265 spend(1/speed)
                } else {
                    // 目标不可达（如被其他 Char 堵路）：等待一回合再试
                    // （handleUnreachableTarget L1297-L1317 的换目标逻辑简化）
                    actor.spend(TICK);
                }
                break;
            }
        }
    }
}

/// 朝 `to` 走一步：邻格直踏（`getCloser` 邻格分支 L517-L523，目标格被占则
/// 失败），否则 `PathFinder.get_step`（L598-L623 重算路径的首步）。
/// 返回 `None` 即无路可走（原文 `getCloser` 返回 false）。
fn step_towards(level: &Level, passable: &[bool], from: IVec2, to: IVec2) -> Option<IVec2> {
    if from == to {
        return None; // L511：target == pos 直接失败
    }
    if chebyshev(from, to) == 1 {
        // cellIsPathable（L489-L507）：占用/不可走的邻格不能直踏
        return passable[level.index(to)].then_some(to);
    }
    let mut pathfinder = PathFinder::new(level.width(), level.height());
    let step = pathfinder.get_step(level.index(from), level.index(to), passable)?;
    // get_step 极端情形可原地返回（周围无更近格），视同无路
    (step != level.index(from)).then(|| level.pos_of(step))
}

/// 游荡目的地（`Level.randomDestination` L784-L791）：全图随机 passable 格。
/// Java 是无界 do-while（真实关卡必有 passable 格）；此处 30 次尝试封顶，
/// 采不到返回 `None`（下回合重试），全墙测试图不会死循环。
fn random_destination(level: &Level, rng: &mut impl Rng) -> Option<IVec2> {
    for _ in 0..30 {
        let index = rng.random_range(0..level.size() as i32) as usize;
        if level.passable[index] {
            return Some(level.pos_of(index));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use bevy::{input::ButtonInput, math::IRect, prelude::*, state::app::StatesPlugin};
    use rand::SeedableRng;
    use rand::rngs::ChaCha12Rng;

    use super::*;
    use crate::actors::bestiary::MobKind;
    use crate::actors::char_stats::StatRange;
    use crate::actors::scheduler::MOB_PRIO;
    use crate::actors::turn::TurnClock;
    use crate::actors::ActorsPlugin;
    use crate::dungeon::DungeonPlugin;
    use crate::levels::terrain::Terrain;
    use crate::scenes::ScenesPlugin;
    use crate::setting::SettingPlugin;

    /// 无渲染 App（`hero.rs` 测试同构）并直接进入 `InGame`。
    fn test_app() -> App {
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
        app
    }

    /// 换装手工关卡：清掉进场真实关卡的怪（保持确定性），英雄归位，
    /// RNG 换固定种子（此后战斗/寻路掷值全部可复现）。
    fn install_level(app: &mut App, level: Level, hero_at: IVec2, seed: u64) {
        app.world_mut().insert_resource(level);
        let stale: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<Mob>>()
            .iter(app.world())
            .collect();
        for mob in stale {
            app.world_mut().despawn(mob);
        }
        place_hero(app, hero_at);
        app.world_mut().resource_mut::<ActorRng>().0 = ChaCha12Rng::seed_from_u64(seed);
        app.update();
    }

    /// 单行直走廊（y=4，x ∈ [1, width-2] 为 Empty）；入口/出口留在 (0,0) 墙内，
    /// 测试不触发下楼路径。
    fn corridor_level(width: usize) -> Level {
        let mut level = Level::new(width, 9, 1);
        level.fill(IRect::new(1, 4, width as i32 - 1, 5), Terrain::Empty);
        level
    }

    /// 双行走廊（y ∈ {3,4}）：给"绕行被占格"留第二条车道。
    fn twin_corridor_level(width: usize) -> Level {
        let mut level = Level::new(width, 9, 1);
        level.fill(IRect::new(1, 3, width as i32 - 1, 5), Terrain::Empty);
        level
    }

    /// 9×9 空房。
    fn room_level() -> Level {
        let mut level = Level::new(9, 9, 1);
        level.fill(IRect::new(1, 1, 8, 8), Terrain::Empty);
        level
    }

    /// 手工入轮一只怪（`spawn_mobs` 的组件清单 + 指定 AI 状态）。
    fn spawn_mob(app: &mut App, kind: MobKind, pos: IVec2, ai: AiState) -> Entity {
        let now = app.world().resource::<TurnClock>().now;
        app.world_mut()
            .spawn((
                Mob {
                    kind,
                    wander_target: None,
                },
                ai,
                GridPos(pos),
                kind.stats().char_stats,
                Actor {
                    time: now,
                    priority: MOB_PRIO,
                },
                DespawnOnExit(AppState::InGame),
            ))
            .id()
    }

    /// 按一次键并推进一帧（英雄行动 + 怪物跟进），随后手动清 `just_pressed`。
    fn press_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.clear();
    }

    fn place_hero(app: &mut App, pos: IVec2) {
        let mut heroes = app.world_mut().query_filtered::<&mut GridPos, With<Hero>>();
        heroes.single_mut(app.world_mut()).expect("应有唯一英雄").0 = pos;
    }

    fn hero_pos(app: &mut App) -> IVec2 {
        let mut heroes = app.world_mut().query_filtered::<&GridPos, With<Hero>>();
        heroes.single(app.world()).expect("应有唯一英雄").0
    }

    fn hero_stats(app: &mut App) -> CharStats {
        let mut heroes = app.world_mut().query_filtered::<&CharStats, With<Hero>>();
        *heroes.single(app.world()).expect("应有唯一英雄")
    }

    /// 抬高血线：受击类测试不允许命中死亡分支。
    fn set_hero_hp(app: &mut App, hp: i32) {
        let mut heroes = app
            .world_mut()
            .query_filtered::<&mut CharStats, With<Hero>>();
        let mut stats = heroes.single_mut(app.world_mut()).expect("应有唯一英雄");
        stats.ht = hp;
        stats.hp = hp;
    }

    fn grid_pos(app: &App, entity: Entity) -> IVec2 {
        app.world().get::<GridPos>(entity).expect("应有 GridPos").0
    }

    fn ai_state(app: &App, entity: Entity) -> AiState {
        *app.world().get::<AiState>(entity).expect("应有 AiState")
    }

    fn mob_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<Mob>>()
            .iter(app.world())
            .count()
    }

    fn clock_now(app: &App) -> f32 {
        app.world().resource::<TurnClock>().now
    }

    /// 单格占用不变量：所有存活 Char（英雄 + 怪）位置两两不同。
    fn assert_single_occupancy(app: &mut App) {
        let mut chars = app.world_mut().query::<(&GridPos, &CharStats)>();
        let mut seen = Vec::new();
        for (pos, stats) in chars.iter(app.world()) {
            if stats.is_alive() {
                assert!(!seen.contains(&pos.0), "多名角色挤在同格 {:?}", pos.0);
                seen.push(pos.0);
            }
        }
    }

    /// 验收：视野外睡怪静止；英雄进 FOV（视距 8）当回合惊醒转 Hunting
    /// （惊醒花 1 回合不动，Mob.java L1160），此后逐回合寻路逼近至邻格；
    /// 全程单格占用。
    #[test]
    fn sleeping_mob_wakes_on_sight_and_closes_in() {
        let mut app = test_app();
        install_level(&mut app, corridor_level(20), IVec2::new(2, 4), 11);
        let rat = spawn_mob(&mut app, MobKind::Rat, IVec2::new(15, 4), AiState::Sleeping);

        // 距离 13/12 > 8：任凭英雄走动，睡怪原地不醒
        press_key(&mut app, KeyCode::KeyD);
        press_key(&mut app, KeyCode::KeyA);
        assert_eq!(grid_pos(&app, rat), IVec2::new(15, 4), "视野外睡怪静止");
        assert_eq!(ai_state(&app, rat), AiState::Sleeping);

        // 传送进视距后行动一次：鼠在自己回合惊醒（原地）
        place_hero(&mut app, IVec2::new(8, 4));
        press_key(&mut app, KeyCode::KeyD); // 英雄 → (9,4)，距离 6
        assert_eq!(ai_state(&app, rat), AiState::Hunting, "见英雄即醒");
        assert_eq!(grid_pos(&app, rat), IVec2::new(15, 4), "惊醒耗 1 回合，不移动");

        // 此后每回合逼近（英雄迎面走，间距单调缩小至切比雪夫 1）
        let mut last = chebyshev(grid_pos(&app, rat), hero_pos(&mut app));
        for _ in 0..6 {
            if last == 1 {
                break;
            }
            press_key(&mut app, KeyCode::KeyD);
            let dist = chebyshev(grid_pos(&app, rat), hero_pos(&mut app));
            assert!(dist < last, "追猎怪应逐回合逼近：{dist} 未小于 {last}");
            assert_single_occupancy(&mut app);
            last = dist;
        }
        assert_eq!(last, 1, "最终应到达英雄邻格");
    }

    /// 验收：邻格攻击伤害在期望域（固定种子）——Rat（伤 1-4）打无甲英雄，
    /// 每回合承伤 ∈ [0,4]（0 即闪避），12 回合内至少命中一次；攻击不位移。
    #[test]
    fn adjacent_mob_attack_damage_in_expected_domain() {
        let mut app = test_app();
        install_level(&mut app, room_level(), IVec2::new(2, 4), 42);
        set_hero_hp(&mut app, 200);
        let rat = spawn_mob(&mut app, MobKind::Rat, IVec2::new(3, 3), AiState::Hunting);

        let mut hp = hero_stats(&mut app).hp;
        let mut hits = 0;
        for round in 0..12 {
            // 英雄在 (2,4)/(3,4) 间横跳：两格均与 (3,3) 对角相邻，
            // 鼠每回合恒处攻击分支，不会走位
            press_key(
                &mut app,
                if round % 2 == 0 {
                    KeyCode::KeyD
                } else {
                    KeyCode::KeyA
                },
            );
            let now = hero_stats(&mut app).hp;
            let taken = hp - now;
            assert!(
                (0..=4).contains(&taken),
                "Rat 伤害域 1-4、英雄甲 0：第 {round} 回合承伤 {taken}"
            );
            assert_eq!(grid_pos(&app, rat), IVec2::new(3, 3), "攻击分支不位移");
            if taken > 0 {
                hits += 1;
            }
            hp = now;
        }
        assert!(hits > 0, "12 回合内应至少命中一次（种子 42 确定性）");
    }

    /// 验收（占用·英雄侧）：向被怪占据的格移动 = 撞击攻击——英雄不位移、
    /// 怪不被顶开、耗时 attackDelay=1；徒手伤害落在期望域 0-2（鼠甲 0-1）。
    #[test]
    fn hero_bump_attack_replaces_move() {
        let mut app = test_app();
        install_level(&mut app, room_level(), IVec2::new(2, 4), 5);
        let rat = spawn_mob(&mut app, MobKind::Rat, IVec2::new(3, 4), AiState::Sleeping);

        let before = app.world().get::<CharStats>(rat).expect("应有数值").hp;
        press_key(&mut app, KeyCode::KeyD); // (3,4) 被占 → 撞击攻击
        assert_eq!(hero_pos(&mut app), IVec2::new(2, 4), "撞击攻击不位移");
        assert_eq!(grid_pos(&app, rat), IVec2::new(3, 4), "怪不被顶开");
        let after = app.world().get::<CharStats>(rat).expect("应有数值").hp;
        assert!(
            (0..=2).contains(&(before - after)),
            "徒手 1-2 伤 − 鼠甲 0-1：实伤 {}",
            before - after
        );
        assert_eq!(clock_now(&app), 1.0, "攻击花费 attackDelay = 1");
        assert_single_occupancy(&mut app);
    }

    /// 验收：怪 HP≤0 → 实体消失（同帧出轮 + 帧末 despawn）+ EXP 入账
    /// （Rat EXP=1，英雄 1 级 ≤ maxLvl 5）。
    #[test]
    fn killing_mob_awards_exp_and_despawns() {
        let mut app = test_app();
        install_level(&mut app, room_level(), IVec2::new(2, 4), 3);
        set_hero_hp(&mut app, 100);
        let rat = spawn_mob(&mut app, MobKind::Rat, IVec2::new(3, 4), AiState::Sleeping);
        {
            let mut stats = app.world_mut().get_mut::<CharStats>(rat).expect("应有数值");
            stats.hp = 1;
            stats.armor_range = StatRange::new(0, 0); // 命中即至少 1 伤 → 必杀
        }

        // 命中掷值受种子支配（鼠闪避 2 命中率高）：封顶 10 次内必然击杀
        let mut killed = false;
        for _ in 0..10 {
            press_key(&mut app, KeyCode::KeyD);
            if mob_count(&mut app) == 0 {
                killed = true;
                break;
            }
        }
        assert!(killed, "10 次撞击内应击杀 1 HP 的鼠（种子 3 确定性）");

        let mut heroes = app.world_mut().query::<&Hero>();
        let hero = heroes.single(app.world()).expect("应有唯一英雄");
        assert_eq!(hero.exp, 1, "Rat EXP=1 入账");
        assert_eq!(hero.lvl, 1, "1 点经验不足以升级（需 10）");
    }

    /// 验收：英雄 HP≤0 → 回 Title（实体清理 + 时间轮复位）。
    #[test]
    fn hero_death_returns_to_title() {
        let mut app = test_app();
        install_level(&mut app, room_level(), IVec2::new(2, 4), 9);
        let killer = spawn_mob(&mut app, MobKind::Rat, IVec2::new(2, 3), AiState::Hunting);
        {
            let mut stats = app
                .world_mut()
                .get_mut::<CharStats>(killer)
                .expect("应有数值");
            stats.attack_skill = 1_000_000; // 必中
            stats.damage_range = StatRange::new(100, 100); // 一击必杀
        }

        press_key(&mut app, KeyCode::KeyA); // 英雄走 (1,4)，鼠仍对角邻格 → 开打
        app.update(); // StateTransition 应用 Title 切换

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Title,
            "英雄死亡应回标题"
        );
        let mut heroes = app.world_mut().query_filtered::<(), With<Hero>>();
        assert_eq!(
            heroes.iter(app.world()).count(),
            0,
            "英雄实体随退出 InGame 清理"
        );
        assert_eq!(mob_count(&mut app), 0, "怪物实体随退出 InGame 清理");
        assert_eq!(clock_now(&app), 0.0, "时间轮复位");
        assert_eq!(*app.world().resource::<TurnState>(), TurnState::Processing);
    }

    /// 验收（占用·怪物侧）：追猎怪被同类堵住走廊时绕第二车道通过，
    /// 全程无重格、被占格不可走入，最终到达英雄邻格；路障原地不动。
    #[test]
    fn hunting_mob_detours_around_occupied_cell() {
        let mut app = test_app();
        install_level(&mut app, twin_corridor_level(16), IVec2::new(2, 4), 17);
        set_hero_hp(&mut app, 100);
        // 惰性路障：存活但失活（time=MAX 永不被选中），恒占 (6,4)——
        // 模拟"格被其他 Char 占用"，排除其自身走位的干扰
        let blocker = spawn_mob(&mut app, MobKind::Rat, IVec2::new(6, 4), AiState::Sleeping);
        app.world_mut()
            .get_mut::<Actor>(blocker)
            .expect("应有 Actor")
            .time = f32::MAX;
        let hunter = spawn_mob(&mut app, MobKind::Rat, IVec2::new(8, 4), AiState::Hunting);

        for round in 0..10 {
            // 英雄在 (2,4)/(2,3) 间原地折返，把回合让给追猎者
            press_key(
                &mut app,
                if round % 2 == 0 {
                    KeyCode::KeyW
                } else {
                    KeyCode::KeyS
                },
            );
            assert_ne!(grid_pos(&app, hunter), IVec2::new(6, 4), "被占格不可走入");
            assert_single_occupancy(&mut app);
            if chebyshev(grid_pos(&app, hunter), hero_pos(&mut app)) == 1 {
                break;
            }
        }
        assert_eq!(grid_pos(&app, blocker), IVec2::new(6, 4), "路障原地不动");
        assert_eq!(
            chebyshev(grid_pos(&app, hunter), hero_pos(&mut app)),
            1,
            "追猎者应绕过路障到达英雄邻格"
        );
    }

    /// 验收：Crab speed=2 → 每英雄回合两步（各花 0.5）；到邻格后改为
    /// 每回合一击（attackDelay=1 与移速无关）。
    #[test]
    fn crab_acts_twice_per_hero_turn() {
        let mut app = test_app();
        install_level(&mut app, corridor_level(16), IVec2::new(2, 4), 23);
        set_hero_hp(&mut app, 100);
        let crab = spawn_mob(&mut app, MobKind::Crab, IVec2::new(12, 4), AiState::Hunting);

        press_key(&mut app, KeyCode::KeyD); // 英雄 (3,4)
        assert_eq!(grid_pos(&app, crab), IVec2::new(10, 4), "一英雄回合走两步");
        assert_eq!(clock_now(&app), 1.0);

        press_key(&mut app, KeyCode::KeyD); // 英雄 (4,4)
        assert_eq!(grid_pos(&app, crab), IVec2::new(8, 4), "再一回合又两步");
        assert_eq!(clock_now(&app), 2.0);

        press_key(&mut app, KeyCode::KeyD); // 英雄 (5,4)，蟹 (7,4)→(6,4) 贴脸
        assert_eq!(grid_pos(&app, crab), IVec2::new(6, 4));
        assert_eq!(clock_now(&app), 3.0);

        // 邻格后：英雄撞击 + 蟹回击各一次（攻击耗时 1.0，蟹不再双动）
        let hp_before = hero_stats(&mut app).hp;
        press_key(&mut app, KeyCode::KeyD); // dst (6,4) 被蟹占 → 撞击攻击
        assert_eq!(grid_pos(&app, crab), IVec2::new(6, 4), "攻击分支不位移");
        let taken = hp_before - hero_stats(&mut app).hp;
        assert!(
            (0..=7).contains(&taken),
            "蟹伤害域 1-7、攻击每回合一次：承伤 {taken}"
        );
        assert_eq!(clock_now(&app), 4.0, "攻击耗时 1 与移速无关");
        assert_single_occupancy(&mut app);
    }
}
