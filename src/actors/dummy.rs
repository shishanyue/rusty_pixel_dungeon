//! M1 轮转验证用的哑元行动者：act = spend(TICK)，不含任何英雄/怪物逻辑
//! （那些是 M4 的事，见 `docs/plans/02-roadmap.md`）。

use bevy::prelude::*;

use super::scheduler::TICK;
use super::turn::{ActTurn, Actor};

/// 哑元行动者标记 + 行动计数（集成测试断言用）。
#[derive(Component, Debug, Default)]
pub struct DummyActor {
    /// 已行动次数。
    pub acts: u32,
}

/// `DummyActor` 的 act()：固定 `spend(TICK)` 后继续（≈ Java act 返回 true）。
///
/// 顶层观察者会收到**所有** [`ActTurn`]，按自己的标记组件过滤出名下实体、
/// 其余直接放行——这是 M4 各行为域（英雄/怪物/Buff 观察者）复用的分发模式。
pub(super) fn dummy_act(on: On<ActTurn>, mut dummies: Query<(&mut Actor, &mut DummyActor)>) {
    let Ok((mut actor, mut dummy)) = dummies.get_mut(on.entity) else {
        return; // 非 DummyActor，交给其他行为观察者
    };
    dummy.acts += 1;
    actor.spend(TICK);
}
