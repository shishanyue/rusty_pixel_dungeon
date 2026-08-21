//! 时间轮的 Bevy 适配层：组件/资源定义 + `process_turns` 独占系统。
//!
//! ## 行动分发方案取舍（计划文档要求二选一）
//!
//! 采用 **`EntityEvent` + 观察者**：`process_turns` 以 `world.trigger` 同步分发
//! [`ActTurn`]，各行为域（M4 的英雄/怪物/Buff）自行 `add_observer` 并按标记组件
//! 过滤属于自己的实体，调度器完全不感知具体行为类型，新增行为零侵入；代价是本
//! 系统必须独占 `&mut World`——观察者要同步跑完，循环才能读到"是否挂起/是否已
//! 花时间"。回合推进本就是全局串行逻辑，独占无并行损失。
//! 备选方案 trait 对象组件（`Box<dyn Act>`）虽可在普通系统里分发，但 `act()`
//! 需要任意世界副作用（攻击/生成/掉落），trait 方法在迭代查询时拿不到
//! `&mut World`，只能回传命令枚举，接口会随行为种类膨胀，且重回 OOP 分发风格，
//! 违背 01 号架构文档的组件组合原则，故弃用。

use bevy::prelude::*;

use super::scheduler::{self, Selected};

/// 时间轮成员（Actor.java 的 `time`/`actPriority` 实例字段，L41/L56）。
///
/// 加入/移除时间轮 = 挂上/移除本组件（对应 `Actor.add`/`Actor.remove`，
/// L328-368）。SPD 的 `add` 会把 `time += now`（L345，新建 Actor 的 `time` 为 0），
/// 故 spawn 方须以 [`TurnClock`] 的 `now` 为基准设置初始 `time`；延迟入场
/// （`addDelayed`，L332-334）即 `now + delay.max(0.0)`。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Actor {
    /// 下次行动的绝对时刻。失活 = `f32::MAX`（Java `diactivate()`，L107-109）。
    pub time: f32,
    /// 同 `time` 时的 tie-break，数值大者先行（Actor.java L55-56）；
    /// 常量表见 [`scheduler`](super::scheduler)。
    pub priority: i32,
}

impl Actor {
    /// 花费时间并防漂移取整（Actor.java `spend`/`spendConstant` L61-73，
    /// M1 无时间修饰因子，两者等价）。
    pub fn spend(&mut self, amount: f32) {
        self.time = scheduler::spend(self.time, amount);
    }
}

/// 全局回合时钟（Actor.java 静态字段 `now`，L154）。初值 0（`clear()` L162）。
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct TurnClock {
    /// 当前时刻，只由 [`process_turns`] 推进为选中者的 `time`（L271）。
    pub now: f32,
}

/// 回合推进状态。SPD 用 actor 线程的 wait/notify 表达"`act()` 返回 false，等待
/// 英雄输入"（Actor.java L294、L304-323）；这里改为显式资源：行为观察者置
/// `WaitingForInput` 即等价于 act 返回 false，输入侧（M4）恢复 `Processing`
/// 即等价于 `next()` + 唤醒线程。
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    /// 时间轮推进中。
    #[default]
    Processing,
    /// 挂起等待外部输入（英雄），[`process_turns`] 不推进。
    WaitingForInput,
}

/// "轮到该实体行动"事件，≈ 调用 `Actor.act()`（Actor.java L294）。
///
/// 行为观察者对自己名下的实体必须三选一，否则触发 [`process_turns`] 的活锁
/// 保护：花时间（[`Actor::spend`] 等）、把自己移出时间轮（despawn 或移除
/// [`Actor`] 组件）、或置 [`TurnState::WaitingForInput`]。
#[derive(EntityEvent, Debug)]
pub struct ActTurn {
    /// 轮到谁行动。
    pub entity: Entity,
}

/// 单帧行动预算。SPD 的 actor 线程可跨帧自旋（等待精灵动画节流，Actor.java
/// L274-286），Bevy 版在 `Update` 内联循环，需固定上限保住帧时长；数值只封顶
/// 每帧推进量，不影响调度语义。
pub const MAX_ACTS_PER_UPDATE: u32 = 100;

/// 时间轮主循环（对照 `Actor.process()`，Actor.java L244-326）：
/// 选下一行动者（L253-266）→ `now = 其 time`（L271）→ 同步触发 [`ActTurn`]
/// （≈ `act()`，L294）→ 若被置为 [`TurnState::WaitingForInput`]（≈ act 返回
/// false）则挂起；直到时间轮为空、挂起或本帧预算耗尽。
pub(super) fn process_turns(world: &mut World) {
    let mut actors = world.query::<(Entity, &Actor)>();
    for _ in 0..MAX_ACTS_PER_UPDATE {
        if *world.resource::<TurnState>() != TurnState::Processing {
            return;
        }
        let Some(Selected { id, time }) =
            scheduler::select_next(actors.iter(world).map(|(e, a)| (e, a.time, a.priority)))
        else {
            return; // 时间轮为空（≈ L300-302 current == null → doNext = false）
        };
        world.resource_mut::<TurnClock>().now = time;
        world.trigger(ActTurn { entity: id });

        // 活锁保护（SPD 没有：act() 不花时间且返回 true 在 Java 一样是死循环，
        // 但卡的是独立 actor 线程；这里会卡死主帧循环，必须断开并告警）：
        // 行动后既没挂起、没花时间、也没把自己移出时间轮 → 无观察者认领或行为缺陷。
        if *world.resource::<TurnState>() == TurnState::Processing
            && world.get::<Actor>(id).is_some_and(|a| a.time == time)
        {
            warn_once!("实体 {id} 行动后未消耗时间、未挂起也未离场，中断本帧时间轮以防活锁");
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::{ActTurn, Actor, MAX_ACTS_PER_UPDATE, TurnClock, TurnState};
    use crate::actors::scheduler::{DEFAULT_PRIO, HERO_PRIO, VFX_PRIO};
    use crate::actors::{ActorsPlugin, DummyActor};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, ActorsPlugin));
        app
    }

    /// 按 `Actor.add` 语义入场（L336-355）：初始 `time` = 当前 `now`。
    fn spawn_dummy(app: &mut App) -> Entity {
        let now = app.world().resource::<TurnClock>().now;
        app.world_mut()
            .spawn((
                Actor {
                    time: now,
                    priority: DEFAULT_PRIO,
                },
                DummyActor::default(),
            ))
            .id()
    }

    fn acts(app: &App, entity: Entity) -> u32 {
        app.world()
            .get::<DummyActor>(entity)
            .expect("DummyActor 应存在")
            .acts
    }

    fn now(app: &App) -> f32 {
        app.world().resource::<TurnClock>().now
    }

    /// 同 time 同 priority 的 3 个 `DummyActor` 按生成序轮转，每帧恰好消耗
    /// `MAX_ACTS_PER_UPDATE` 次行动，计数均摊、时钟随之推进。
    #[test]
    fn dummy_rotation_counts_are_exact() {
        let mut app = test_app();
        let (a, b, c) = (
            spawn_dummy(&mut app),
            spawn_dummy(&mut app),
            spawn_dummy(&mut app),
        );

        for frame in 1..=2_u32 {
            app.update();
            let total = frame * MAX_ACTS_PER_UPDATE;
            // 轮转序 a,b,c,a,b,c…：第 n 次行动落在第 (n-1)%3 个实体上
            assert_eq!(acts(&app, a), total.div_ceil(3), "第 {frame} 帧后 a 计数");
            assert_eq!(acts(&app, b), (total + 1) / 3, "第 {frame} 帧后 b 计数");
            assert_eq!(acts(&app, c), total / 3, "第 {frame} 帧后 c 计数");
            // 第 total 次行动选中者的 time = (total-1)/3 个整 TICK
            #[expect(clippy::cast_precision_loss, reason = "测试值远小于 2^24，转换精确")]
            let expected_now = ((total - 1) / 3) as f32;
            assert_eq!(now(&app), expected_now, "第 {frame} 帧后时钟");
        }
    }

    /// 验收 d 后半：`TurnState::WaitingForInput` 时完全不推进，恢复后继续。
    #[test]
    fn waiting_for_input_freezes_wheel() {
        let mut app = test_app();
        let (a, b, c) = (
            spawn_dummy(&mut app),
            spawn_dummy(&mut app),
            spawn_dummy(&mut app),
        );
        app.update();
        let (snap_a, snap_b, snap_c, snap_now) =
            (acts(&app, a), acts(&app, b), acts(&app, c), now(&app));

        *app.world_mut().resource_mut::<TurnState>() = TurnState::WaitingForInput;
        app.update();
        app.update();
        assert_eq!(
            (acts(&app, a), acts(&app, b), acts(&app, c), now(&app)),
            (snap_a, snap_b, snap_c, snap_now),
            "挂起期间计数与时钟都不得变化"
        );

        *app.world_mut().resource_mut::<TurnState>() = TurnState::Processing;
        app.update();
        assert_eq!(
            acts(&app, a) + acts(&app, b) + acts(&app, c),
            2 * MAX_ACTS_PER_UPDATE,
            "恢复 Processing 后按预算继续推进"
        );
    }

    /// 英雄式挂起模式：行动观察者置 `WaitingForInput`（≈ `act()` 返回 false，
    /// Actor.java L294/L304），时间轮当帧立即停在它身上，其余人一步不走；
    /// 移出时间轮（`Actor.remove` L357-368 ≈ despawn）并恢复后照常轮转。
    #[test]
    fn actor_can_suspend_wheel_mid_frame() {
        #[derive(Component)]
        struct WaitsForInput;

        fn wait_act(on: On<ActTurn>, waiters: Query<&WaitsForInput>, mut state: ResMut<TurnState>) {
            if waiters.contains(on.entity) {
                *state = TurnState::WaitingForInput;
            }
        }

        let mut app = test_app();
        app.add_observer(wait_act);
        let (a, b, c) = (
            spawn_dummy(&mut app),
            spawn_dummy(&mut app),
            spawn_dummy(&mut app),
        );
        // 同 time 0 下 HERO_PRIO(0) > DEFAULT_PRIO(-100) → waiter 最先被选中
        let waiter = app
            .world_mut()
            .spawn((
                Actor {
                    time: 0.0,
                    priority: HERO_PRIO,
                },
                WaitsForInput,
            ))
            .id();

        app.update();
        assert_eq!(
            *app.world().resource::<TurnState>(),
            TurnState::WaitingForInput
        );
        assert_eq!((acts(&app, a), acts(&app, b), acts(&app, c)), (0, 0, 0));
        assert_eq!(now(&app), 0.0);

        app.world_mut().despawn(waiter);
        *app.world_mut().resource_mut::<TurnState>() = TurnState::Processing;
        app.update();
        assert_eq!(
            acts(&app, a) + acts(&app, b) + acts(&app, c),
            MAX_ACTS_PER_UPDATE,
            "waiter 离场后 Dummy 恢复满预算轮转"
        );
    }

    /// 无任何观察者认领的 Actor：活锁保护当帧断开时间轮（不会卡死测试进程），
    /// 后续帧同样立即断开，谁也不推进。
    #[test]
    fn unclaimed_actor_trips_livelock_guard() {
        let mut app = test_app();
        let dummy = spawn_dummy(&mut app);
        // VFX_PRIO 保证孤儿永远先于 Dummy 被选中
        app.world_mut().spawn(Actor {
            time: 0.0,
            priority: VFX_PRIO,
        });

        app.update();
        app.update();
        assert_eq!(acts(&app, dummy), 0, "孤儿挡在轮首，Dummy 不应行动");
        assert_eq!(now(&app), 0.0);
    }

    /// 空时间轮：循环立即退出，时钟保持原位。
    #[test]
    fn empty_wheel_is_noop() {
        let mut app = test_app();
        app.update();
        app.update();
        assert_eq!(now(&app), 0.0);
        assert_eq!(*app.world().resource::<TurnState>(), TurnState::Processing);
    }
}
