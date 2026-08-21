//! 英雄可玩竖切（`docs/plans/17-hero-slice.md`）：英雄生成、键盘输入 → 待执行
//! 动作、act 观察者（移动/花费时间/踩出口请求下楼）与时间轮的进出场集成。
//! M4（`docs/plans/22-mobs-combat.md`）增量：撞击攻击入口与经验/升级，
//! 改动清单见该计划实现笔记。
//!
//! 语义对照（行号未注明文件者指 `hero/Hero.java`）：
//! - `act()`（L831-929）：无 `curAction` → `ready()` 返回 false 等输入
//!   （L863-881，≈ 置 [`TurnState::WaitingForInput`]）；有则执行该动作。
//! - `actMove` → `getCloser`（L977-993 / L1830-1875）：走一步花费
//!   `delay / speed()`（L1863，基础 delay = 1）；走不了 → `ready()`（L989-992），
//!   不动不耗时。
//! - 撞击攻击：移动目标格有敌对怪 → 攻击替代移动（`handle` L1904-1910 把
//!   点击怪物格转成 `HeroAction.Attack`；`actAttack` L1409-1450 邻格直击 +
//!   `onAttackComplete` L2326 `spend(attackDelay())`）。M2 键盘单步下目标格
//!   恒为邻格，无需原文的先寻路后攻击。
//! - `earnExp`/`lvlUp`（L1967-2073）：升级曲线经 `bestiary`；任务书约定
//!   "升级回满血"（SPD 原文仅把 HT 增量补进 HP，`updateHT` L265-267）。
//! - `speed()`：`Char.java` L775-788 的基础值 `baseSpeed`，Buff/护甲乘子属 M4，
//!   M2 即 [`CharStats::base_speed`]。
//! - 对角步只要求**目标格** passable——SPD 的 `PathFinder` BFS 对角扩展语义
//!   （14 号域笔记第 1 条，贴墙斜穿在 Java 同样允许）。
//! - 输入桥：SPD 用 `GameScene`/`CellSelector` 点击选格 + 寻路，M2 以键盘
//!   8 向单步替代（任务书约定，点击寻路是 M3+）。
//!
//! 时间轮空转说明（消费方集成，`turn.rs` 未改）：[`TurnState`] 默认
//! `Processing`，但空轮时 `process_turns` 选不出行动者会立即返回，故
//! Loading/Title 阶段无需额外处理；只需退出 `InGame` 时把挂起状态复位
//! （见 [`reset_turn_wheel`]），防止下次进入带着 `WaitingForInput` 卡死。

use bevy::prelude::*;

use crate::levels::{Level, terrain::Terrain};
use crate::states::AppState;

use super::bestiary::{
    HERO_MAX_LEVEL, HeroClass, hero_attack_skill, hero_defense_skill, hero_max_exp, hero_max_ht,
};
use super::char_stats::CharStats;
use super::melee::{self, MeleeOutcome};
use super::mob::{ActorRng, Mob, exp_for_kill};
use super::scheduler::{HERO_PRIO, TICK};
use super::turn::{ActTurn, Actor, TurnClock, TurnState};

/// 格子坐标组件（SPD `Char.pos` 的线性索引 → `IVec2`，坐标约定见 `levels.rs`
/// 头注释：原点左上、y 向下）。M4 的怪物同样挂本组件。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPos(pub IVec2);

/// 英雄标记 + 待执行动作（Hero.java 的 `curAction` 字段）+ 等级/经验
/// （L217-L218 `lvl = 1`/`exp = 0`）。
///
/// `next_action` 由输入侧写入（≈ `CellSelector` → `hero.handle()`），
/// [`hero_act`] 在英雄回合取走执行；`None` 即"等待玩家输入"。
#[derive(Component, Debug)]
pub struct Hero {
    /// 下一回合要执行的动作；`None` 时英雄回合挂起时间轮。
    pub next_action: Option<HeroAction>,
    /// 当前等级（Hero.java L217，出生 1 级，上限 [`HERO_MAX_LEVEL`]）。
    pub lvl: i32,
    /// 升向下一级的已积累经验（L218；每级需求 [`hero_max_exp`]）。
    pub exp: i32,
}

impl Default for Hero {
    fn default() -> Self {
        Self {
            next_action: None,
            lvl: 1,
            exp: 0,
        }
    }
}

impl Hero {
    /// `earnExp`（Hero.java L1967-L2065 剥除 Buff/物品/徽章后的骨架）：
    /// 入账经验并结算升级，返回升了几级（0 即未升）。
    ///
    /// 升级效果（L2023-L2033 + `updateHT` L257）：`lvl+1`、HT/命中/闪避按
    /// `bestiary` 曲线重算；**回满血**是任务书钉死的简化（SPD 原文只把 HT
    /// 增量补进当前 HP，L265-L267）。满级后经验清零（L2035-L2037，Bless
    /// Buff 属后续）。
    pub fn earn_exp(&mut self, stats: &mut CharStats, amount: i32) -> u32 {
        self.exp += amount; // L1971
        let mut level_ups = 0;
        while self.exp >= hero_max_exp(self.lvl) {
            // L2015
            self.exp -= hero_max_exp(self.lvl); // L2016
            if self.lvl < HERO_MAX_LEVEL {
                self.lvl += 1; // L2024
                level_ups += 1;
                stats.ht = hero_max_ht(self.lvl); // updateHT L257
                stats.attack_skill = hero_attack_skill(self.lvl); // L2032
                stats.defense_skill = hero_defense_skill(self.lvl); // L2033
                stats.hp = stats.ht; // 任务书简化：升级回满血
            } else {
                self.exp = 0; // L2035-L2037（满级溢出清零）
            }
        }
        level_ups
    }
}

/// `hero/HeroAction.java` 的 M2 子集：目前只有单步移动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroAction {
    /// 走向目标格（`HeroAction.Move.dst`；M2 恒为相邻格，8 向）。
    Move(IVec2),
}

/// 下楼请求：英雄踩上 [`Terrain::Exit`] 时由 [`hero_act`] 置位，场景域
/// （`scenes::in_game::descend`）消费并重建关卡。
///
/// 用直写资源而非事件：观察者在 `process_turns` 独占 World 期间运行，
/// `Commands` 的冲刷时机跨帧不定，直写 `ResMut` 立即可见（`turn.rs` 的
/// 挂起模式同理）。对应 SPD 的 `InterlevelScene.mode = DESCEND` 场景切换标志。
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DescendRequest(pub bool);

/// 键位 → 方向增量（格坐标 y 向下）。SPD 桌面端本就是 8 向键盘移动
/// （`SPDAction` 的 N/S/E/W 与四对角绑定），M2 映射为：
/// 方向键 / WASD / 小键盘 8462 = 四正向，QEZC / 小键盘 7913 = 四对角。
const KEY_DIRS: &[(KeyCode, IVec2)] = &[
    (KeyCode::ArrowUp, IVec2::new(0, -1)),
    (KeyCode::KeyW, IVec2::new(0, -1)),
    (KeyCode::Numpad8, IVec2::new(0, -1)),
    (KeyCode::ArrowDown, IVec2::new(0, 1)),
    (KeyCode::KeyS, IVec2::new(0, 1)),
    (KeyCode::Numpad2, IVec2::new(0, 1)),
    (KeyCode::ArrowLeft, IVec2::new(-1, 0)),
    (KeyCode::KeyA, IVec2::new(-1, 0)),
    (KeyCode::Numpad4, IVec2::new(-1, 0)),
    (KeyCode::ArrowRight, IVec2::new(1, 0)),
    (KeyCode::KeyD, IVec2::new(1, 0)),
    (KeyCode::Numpad6, IVec2::new(1, 0)),
    (KeyCode::KeyQ, IVec2::new(-1, -1)),
    (KeyCode::Numpad7, IVec2::new(-1, -1)),
    (KeyCode::KeyE, IVec2::new(1, -1)),
    (KeyCode::Numpad9, IVec2::new(1, -1)),
    (KeyCode::KeyZ, IVec2::new(-1, 1)),
    (KeyCode::Numpad1, IVec2::new(-1, 1)),
    (KeyCode::KeyC, IVec2::new(1, 1)),
    (KeyCode::Numpad3, IVec2::new(1, 1)),
];

/// `Level` 就绪且场上无英雄 → 在 `level.entrance` 生成（`Dungeon.switchLevel`
/// L474-481：英雄放入口）。进 `InGame` 首帧与下楼重建共用这一条路径。
///
/// 入时间轮语义照 `Actor.add`（Actor.java L336-355：新 Actor `time = 0`，
/// add 时 `+= now`），即初始 `time` 取当前时钟。
pub(super) fn spawn_hero(
    mut commands: Commands,
    level: Res<Level>,
    clock: Res<TurnClock>,
    heroes: Query<(), With<Hero>>,
) {
    if !heroes.is_empty() {
        return;
    }
    commands.spawn((
        Hero::default(),
        GridPos(level.entrance),
        // M2 固定战士（职业选择场景是 M4+）；出生数值见 bestiary 对拍表
        HeroClass::Warrior.starting_stats(),
        Actor {
            time: clock.now,
            priority: HERO_PRIO,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

/// 键盘 → 待执行动作。只在时间轮挂起等英雄时受理（≈ `GameScene.ready` 后
/// `CellSelector` 才可选格）；写入动作并把 [`TurnState`] 置回 `Processing`
/// ≈ `hero.handle()` 后的 `next()` 唤醒。
///
/// 同帧多键按向量合成再钳到 [-1,1]（W+D = 右上对角，对冲键相互抵消），
/// 单键即 8 向之一。
pub(super) fn hero_keyboard_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<TurnState>,
    mut heroes: Query<(&mut Hero, &GridPos)>,
) {
    if *state != TurnState::WaitingForInput {
        return;
    }
    let Ok((mut hero, pos)) = heroes.single_mut() else {
        return;
    };
    let dir = KEY_DIRS
        .iter()
        .filter(|(key, _)| keyboard.just_pressed(*key))
        .fold(IVec2::ZERO, |acc, (_, delta)| acc + *delta)
        .clamp(IVec2::NEG_ONE, IVec2::ONE);
    if dir == IVec2::ZERO {
        return;
    }
    hero.next_action = Some(HeroAction::Move(pos.0 + dir));
    *state = TurnState::Processing;
}

/// 英雄的 act()（对照 Hero.java L831-929 中与移动相关的最小骨架）：
/// 无待执行动作 → 挂起等输入；目标格有存活怪 → 撞击攻击（`handle`
/// L1904-1910 + `actAttack` L1409-1450）；否则移动并花费时间。
///
/// 顶层观察者收到所有 [`ActTurn`]，按 [`Hero`] 组件过滤（`dummy.rs` 分发模式）。
pub(super) fn hero_act(
    on: On<ActTurn>,
    mut heroes: Query<(&mut Actor, &mut Hero, &mut GridPos, &mut CharStats), Without<Mob>>,
    mut mobs: Query<(Entity, &GridPos, &mut CharStats, &mut Actor, &Mob), Without<Hero>>,
    level: Option<Res<Level>>,
    mut state: ResMut<TurnState>,
    mut descend: ResMut<DescendRequest>,
    mut rng: ResMut<ActorRng>,
    mut commands: Commands,
) {
    let Ok((mut actor, mut hero, mut pos, mut stats)) = heroes.get_mut(on.entity) else {
        return; // 非英雄，交给其他行为观察者
    };
    let Some(HeroAction::Move(dst)) = hero.next_action.take() else {
        // curAction == null 分支（L863-881）：ready() 后 act 返回 false → 挂起
        *state = TurnState::WaitingForInput;
        return;
    };

    // 撞击攻击：移动目标格有存活怪 → 攻击替代移动（handle L1904-1910；
    // 键盘单步下目标恒为邻格，canAttack 恒真，直接进入 attack 结算）。
    // 单格占用的英雄侧即由此保证：Char 所占格永远走不进去。
    let bump = mobs
        .iter_mut()
        .find(|(_, mob_pos, mob_stats, ..)| mob_pos.0 == dst && mob_stats.is_alive());
    if let Some((mob_entity, _, mut mob_stats, mut mob_actor, mob)) = bump {
        match melee::resolve_melee(&stats, &mob_stats, &mut rng.0) {
            MeleeOutcome::Miss => {
                info!("英雄攻击 {:?}：被闪避", mob.kind);
            }
            MeleeOutcome::Hit {
                rolled,
                blocked,
                taken,
            } => {
                mob_stats.take_damage(taken);
                info!(
                    "英雄攻击 {:?}：命中 {taken} 伤害（掷 {rolled} - 甲 {blocked}），{:?} HP {}/{}",
                    mob.kind, mob.kind, mob_stats.hp, mob_stats.ht
                );
                if !mob_stats.is_alive() {
                    // 死亡结算（Mob.destroy L830-L873）：出时间轮 + 移出场景 +
                    // 授予经验。观察者 Commands 冲刷在 process_turns 独占期后，
                    // 先直写失活（time = MAX ≈ Actor.diactivate）保证本帧
                    // 时间轮不再选中尸体，despawn 走延迟命令。
                    mob_actor.time = f32::MAX;
                    commands.entity(mob_entity).despawn();
                    let exp = exp_for_kill(hero.lvl, &mob.kind.stats());
                    info!("{:?} 被击杀，EXP +{exp}", mob.kind);
                    let level_ups = hero.earn_exp(&mut stats, exp);
                    if level_ups > 0 {
                        info!("英雄升到 {} 级，生命回满（HP {}/{}）", hero.lvl, stats.hp, stats.ht);
                    }
                }
            }
        }
        // onAttackComplete L2326：spend(attackDelay())，徒手基础 1.0，
        // 不除以移动速度（攻击耗时与 speed 无关）
        actor.spend(stats.attack_delay);
        return;
    }

    // 可走判定 = 目标格在图内且 passable（getCloser 的寻路以 passable 为底图，
    // L1801/L1809；对角步同样只看目标格，见模块注释）。越界防御先于 index。
    let Some(level) = level.filter(|l| l.is_inside(dst) && l.passable[l.index(dst)]) else {
        // 走不了 ≈ actMove 的 getCloser 失败分支（L989-992）：ready()，不动不耗时
        *state = TurnState::WaitingForInput;
        return;
    };
    pos.0 = dst;
    // getCloser L1832/L1863：基础 delay = 1，spend(delay / speed())；
    // speed 的 Buff/护甲乘子属 M4（Char.java L775-788），M2 即 base_speed
    actor.spend(TICK / stats.base_speed);

    if level.terrain(dst) == Terrain::Exit {
        // M2 竖切"走上即下楼"（SPD 是站上后另发 LvlTransition 动作走过场，
        // 任务书约定简化）。挂起时间轮，本帧内由场景域重建关卡。
        descend.0 = true;
        *state = TurnState::WaitingForInput;
    }
}

/// 离开 InGame：时间轮回到初始状态（`Actor.clear` 语义，Actor.java L160-168），
/// 清掉可能残留的挂起/下楼标志，防止下次进入卡死；英雄实体由
/// [`DespawnOnExit`] 随状态退出清理。
pub(super) fn reset_turn_wheel(
    mut clock: ResMut<TurnClock>,
    mut state: ResMut<TurnState>,
    mut descend: ResMut<DescendRequest>,
) {
    *clock = TurnClock::default();
    *state = TurnState::default();
    descend.0 = false;
}

#[cfg(test)]
mod tests {
    use bevy::{input::ButtonInput, math::IRect, prelude::*, state::app::StatesPlugin};

    use super::*;
    use crate::actors::ActorsPlugin;
    use crate::dungeon::{Dungeon, DungeonPlugin};
    use crate::scenes::{ScenesPlugin, in_game::grid_to_world};
    use crate::setting::SettingPlugin;

    /// 手工关卡的固定入口/出口（见 [`boxed_level`]）。
    const ENTRANCE: IVec2 = IVec2::new(2, 4);
    const EXIT: IVec2 = IVec2::new(6, 4);

    /// 无渲染 App：MinimalPlugins + 状态调度 + 本域与场景域插件的生产组合
    /// （`scenes::tests` 同构，另加 `ActorsPlugin`）。
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppState>()
            // MinimalPlugins 不含 InputPlugin，键盘资源手动补齐（手动管理 just_pressed）
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins((SettingPlugin, DungeonPlugin, ActorsPlugin, ScenesPlugin));
        // Startup + 初始进入 Loading
        app.update();
        app
    }

    fn set_state(app: &mut App, state: AppState) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(state);
        app.update();
    }

    /// 按一次键并推进一帧；随后手动清掉 `just_pressed`（无 `InputPlugin` 代劳）。
    fn press_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.clear();
    }

    /// 9×9 手工关卡：外圈全墙、内部全 Empty，入口 (2,4)、出口 (6,4) 同排。
    /// 替换掉熵种子生成的真实关卡，让移动断言与随机布局解耦。
    fn boxed_level(depth: i32) -> Level {
        let mut level = Level::new(9, 9, depth);
        level.fill(IRect::new(1, 1, 8, 8), Terrain::Empty);
        level.set_terrain(ENTRANCE, Terrain::Entrance);
        level.set_terrain(EXIT, Terrain::Exit);
        level.entrance = ENTRANCE;
        level.exit = EXIT;
        level
    }

    /// 换装手工关卡并把英雄拉到其入口，推进一帧让视图/相机消化。
    /// 顺带清掉进场时随真实关卡生成的怪物（M4 起 `insert_fresh_level` 附带
    /// `MobSpawnRequest`）：本模块只验证英雄行为，保持无怪确定性环境，
    /// 怪物行为断言归 `ai.rs`/`mob.rs` 测试。
    fn install_boxed_level(app: &mut App) {
        app.world_mut().insert_resource(boxed_level(1));
        let mobs: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<Mob>>()
            .iter(app.world())
            .collect();
        for mob in mobs {
            app.world_mut().despawn(mob);
        }
        place_hero(app, ENTRANCE);
        app.update();
    }

    fn place_hero(app: &mut App, pos: IVec2) {
        let mut heroes = app.world_mut().query_filtered::<&mut GridPos, With<Hero>>();
        heroes.single_mut(app.world_mut()).expect("应有唯一英雄").0 = pos;
    }

    fn hero_pos(app: &mut App) -> IVec2 {
        let mut heroes = app.world_mut().query_filtered::<&GridPos, With<Hero>>();
        heroes.single(app.world()).expect("应有唯一英雄").0
    }

    fn hero_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<Hero>>()
            .iter(app.world())
            .count()
    }

    fn clock_now(app: &App) -> f32 {
        app.world().resource::<TurnClock>().now
    }

    fn turn_state(app: &App) -> TurnState {
        *app.world().resource::<TurnState>()
    }

    /// 验收 a：进 `InGame` 一帧内，英雄以战士出生数值生成在关卡入口、
    /// 带 `HERO_PRIO` 入时间轮，且时间轮已挂起等输入。
    #[test]
    fn hero_spawns_at_entrance_and_enters_wheel() {
        let mut app = test_app();
        set_state(&mut app, AppState::InGame);

        let entrance = app.world().resource::<Level>().entrance;
        let mut heroes = app
            .world_mut()
            .query_filtered::<(&GridPos, &Actor, &CharStats), With<Hero>>();
        let (pos, actor, stats) = heroes.single(app.world()).expect("应有唯一英雄");
        assert_eq!(pos.0, entrance, "英雄应生成在入口");
        assert_eq!(actor.priority, HERO_PRIO);
        assert_eq!(actor.time, 0.0, "Actor.add 语义：入场 time = now(0)");
        assert_eq!(*stats, HeroClass::Warrior.starting_stats());
        assert_eq!(
            turn_state(&app),
            TurnState::WaitingForInput,
            "首帧后应挂起等输入"
        );
        assert_eq!(clock_now(&app), 0.0);
    }

    /// 验收 b：按右（D）→ 格 +X、时钟 +1/speed；对角键（E）同为一步一回合。
    #[test]
    fn move_key_steps_hero_and_spends_time() {
        let mut app = test_app();
        set_state(&mut app, AppState::InGame);
        install_boxed_level(&mut app);

        press_key(&mut app, KeyCode::KeyD);
        assert_eq!(hero_pos(&mut app), ENTRANCE + IVec2::X, "D → +X 一格");
        assert_eq!(clock_now(&app), 1.0, "spend(1/speed)，战士 speed=1");
        assert_eq!(
            turn_state(&app),
            TurnState::WaitingForInput,
            "行动完回到待输入"
        );

        // 对角步只要求目标格 passable（SPD PathFinder 对角语义）
        press_key(&mut app, KeyCode::KeyE);
        assert_eq!(
            hero_pos(&mut app),
            ENTRANCE + IVec2::new(2, -1),
            "E → 右上一格"
        );
        assert_eq!(clock_now(&app), 2.0);
    }

    /// 移动耗时随速度缩放（getCloser L1863 spend(1/speed)；速度基础值语义
    /// 见 Char.java L775-788）：base_speed=2 时每步 0.5。
    #[test]
    fn move_cost_scales_with_speed() {
        let mut app = test_app();
        set_state(&mut app, AppState::InGame);
        install_boxed_level(&mut app);

        let mut heroes = app
            .world_mut()
            .query_filtered::<&mut CharStats, With<Hero>>();
        heroes
            .single_mut(app.world_mut())
            .expect("应有唯一英雄")
            .base_speed = 2.0;

        press_key(&mut app, KeyCode::KeyD);
        assert_eq!(clock_now(&app), 0.5);
        press_key(&mut app, KeyCode::KeyD);
        assert_eq!(clock_now(&app), 1.0);
    }

    /// 验收 c：面向墙按键 → 位置与时钟均不变、仍在待输入（actMove 失败分支
    /// L989-992：ready()，不耗时）；随后正常方向仍可行动（未卡死）。
    #[test]
    fn bumping_wall_costs_nothing() {
        let mut app = test_app();
        set_state(&mut app, AppState::InGame);
        install_boxed_level(&mut app);
        // (1,4) 紧贴左侧外墙 (0,4)
        place_hero(&mut app, IVec2::new(1, 4));

        press_key(&mut app, KeyCode::KeyA);
        assert_eq!(hero_pos(&mut app), IVec2::new(1, 4), "撞墙不动");
        assert_eq!(clock_now(&app), 0.0, "撞墙不耗时");
        assert_eq!(turn_state(&app), TurnState::WaitingForInput, "保持待输入");

        press_key(&mut app, KeyCode::KeyD);
        assert_eq!(hero_pos(&mut app), IVec2::new(2, 4), "撞墙后仍可正常行动");
        assert_eq!(clock_now(&app), 1.0);
    }

    /// 验收 d：走上出口 → depth+1、新 Level 生成、英雄在新层入口、
    /// 时间轮清零（SPD Dungeon.newLevel L297-300 的 Actor.clear 语义）。
    #[test]
    fn stepping_on_exit_descends_to_next_depth() {
        let mut app = test_app();
        set_state(&mut app, AppState::InGame);
        install_boxed_level(&mut app);
        place_hero(&mut app, EXIT - IVec2::X);

        press_key(&mut app, KeyCode::KeyD); // 踩上出口，当帧受理下楼
        app.update(); // 新英雄在下一帧生成

        assert_eq!(app.world().resource::<Dungeon>().depth, 2, "下楼后加深一层");
        let level = app.world().resource::<Level>();
        assert_eq!(level.depth, 2, "应生成新一层关卡");
        let entrance = level.entrance;
        assert_eq!(hero_pos(&mut app), entrance, "英雄应在新层入口");
        assert_eq!(clock_now(&app), 0.0, "时间轮随换层清零");
        assert_eq!(
            turn_state(&app),
            TurnState::WaitingForInput,
            "新层首帧后待输入"
        );
        assert!(
            !app.world().resource::<DescendRequest>().0,
            "下楼请求应被消费"
        );
    }

    /// 验收 e：Esc 返回 Title（英雄清理 + 时间轮复位）再进 `InGame`，
    /// 一切重新初始化且可继续行动，不 panic。
    #[test]
    fn esc_roundtrip_reinitializes_cleanly() {
        let mut app = test_app();
        set_state(&mut app, AppState::Title);
        set_state(&mut app, AppState::InGame);
        install_boxed_level(&mut app);
        press_key(&mut app, KeyCode::KeyD);
        assert_eq!(clock_now(&app), 1.0);

        press_key(&mut app, KeyCode::Escape); // Esc 系统写入 NextState
        app.update(); // StateTransition 应用切换
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Title
        );
        assert_eq!(hero_count(&mut app), 0, "英雄应随状态退出清理");
        assert_eq!(clock_now(&app), 0.0, "TurnClock 应复位");
        assert_eq!(turn_state(&app), TurnState::Processing, "TurnState 应复位");

        set_state(&mut app, AppState::InGame);
        let entrance = app.world().resource::<Level>().entrance;
        assert_eq!(hero_pos(&mut app), entrance, "重进后英雄在新关卡入口");
        assert_eq!(turn_state(&app), TurnState::WaitingForInput);

        install_boxed_level(&mut app);
        press_key(&mut app, KeyCode::KeyD);
        assert_eq!(
            hero_pos(&mut app),
            ENTRANCE + IVec2::X,
            "重进后仍可正常行动"
        );
        assert_eq!(clock_now(&app), 1.0);
    }

    /// `earn_exp` 纯逻辑对拍：曲线经 `bestiary`（`Hero.java` L2015-L2033 +
    /// `updateHT` L257），升级回满血是任务书钉死的简化；一次入账可连升多级；
    /// 不足一级只积累。
    #[test]
    fn earn_exp_levels_up_and_heals_to_full() {
        let mut hero = Hero::default();
        let mut stats = HeroClass::Warrior.starting_stats();
        stats.hp = 3; // 残血升级应回满

        assert_eq!(hero.earn_exp(&mut stats, 4), 0, "不足一级不升");
        assert_eq!((hero.lvl, hero.exp), (1, 4));
        assert_eq!(stats.hp, 3, "未升级不回血");

        // 1→2 级需 10（hero_max_exp(1)），现有 4 + 6 = 10
        assert_eq!(hero.earn_exp(&mut stats, 6), 1);
        assert_eq!((hero.lvl, hero.exp), (2, 0));
        assert_eq!(stats.ht, hero_max_ht(2), "HT 随级重算（25）");
        assert_eq!(stats.hp, stats.ht, "任务书简化：升级回满血");
        assert_eq!(stats.attack_skill, hero_attack_skill(2));
        assert_eq!(stats.defense_skill, hero_defense_skill(2));

        // 一次入账连升两级：2→3 需 15、3→4 需 20
        assert_eq!(hero.earn_exp(&mut stats, 35), 2);
        assert_eq!((hero.lvl, hero.exp), (4, 0));
        assert_eq!(stats.ht, hero_max_ht(4));
    }

    /// 渲染集成：英雄调试方块与相机同帧锁定英雄的世界坐标
    /// （坐标约定：格 (x,y) → ((x-(w-1)/2)*16, ((h-1)/2-y)*16)）。
    #[test]
    fn marker_and_camera_lock_to_hero() {
        let mut app = test_app();
        set_state(&mut app, AppState::InGame);
        install_boxed_level(&mut app);

        press_key(&mut app, KeyCode::KeyD);
        let expected = grid_to_world(ENTRANCE + IVec2::X, 9, 9);

        let mut hero_transforms = app.world_mut().query_filtered::<&Transform, With<Hero>>();
        let marker = hero_transforms
            .single(app.world())
            .expect("英雄应挂调试方块");
        assert_eq!(marker.translation.truncate(), expected, "方块随格移动");
        assert!(marker.translation.z > 0.0, "英雄 z 应高于地块（z=0）");

        let mut cameras = app
            .world_mut()
            .query_filtered::<&Transform, With<Camera2d>>();
        let camera = cameras.single(app.world()).expect("全局相机应唯一");
        assert_eq!(camera.translation.truncate(), expected, "相机锁定英雄");
    }
}
