//! 回合调度与角色域：SPD `Actor` 时间轮（`docs/plans/11-turn-scheduler.md`）、
//! 角色战斗纯核（`docs/plans/15-char-combat-core.md`）、英雄可玩竖切
//! （`docs/plans/17-hero-slice.md`）与怪物 AI/战斗接线
//! （`docs/plans/22-mobs-combat.md`）。
//!
//! - [`scheduler`]：时间轮纯逻辑核（零 Bevy 依赖），时钟/选择/取整语义逐行
//!   对照 `actors/Actor.java`；
//! - [`combat`]：命中/伤害/护甲公式纯核，逐行对照 `Char.java`；
//! - [`bestiary`]：Hero 四职业与下水道三怪的初始数值表；
//! - [`melee`]：近战攻击结算纯核（hit → dr → damage 串接，`Char.attack`）；
//! - `hero`：英雄生成/键盘移动/act 观察者/撞击攻击/经验升级/下楼请求；
//! - `mob` + `ai`：怪物生成（数量/轮换/出生格）与三态 AI 状态机；
//! - Bevy 适配（组件/资源/事件）经本入口 re-export，跨域勿深入子模块取用。

use bevy::prelude::*;

use crate::levels::Level;
use crate::states::AppState;

mod ai;
pub mod bestiary;
pub mod char_stats;
pub mod combat;
mod dummy;
mod hero;
pub mod melee;
mod mob;
pub mod scheduler;
mod turn;

pub use ai::AiState;
pub use bestiary::{HeroClass, MobKind, MobStats};
pub use char_stats::{CharStats, StatRange};
pub use dummy::DummyActor;
pub use hero::{DescendRequest, GridPos, Hero, HeroAction};
pub use melee::MeleeOutcome;
pub use mob::{ActorRng, Mob, MobSpawnRequest};
pub use turn::{ActTurn, Actor, TurnClock, TurnState};

/// 时间轮推进（`process_turns`）所在的系统集：行为域的输入/生成排在其前，
/// 场景域的下楼重建（`scenes::in_game::descend`）排在其后，保证
/// "输入 → 行动 → 换层"在同一帧内按序完成。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TurnWheelSet;

/// 注册时间轮资源、`process_turns` 推进系统与行为观察者（M1 哑元 + 英雄）。
/// 战斗纯核（[`combat`]/[`bestiary`]/[`CharStats`]）是纯数据与纯函数，
/// 无系统可注册；M4 怪物/Buff 行为域接线时经本插件挂观察者。
pub struct ActorsPlugin;

impl Plugin for ActorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurnClock>()
            .init_resource::<TurnState>()
            .init_resource::<DescendRequest>()
            .init_resource::<ActorRng>()
            .add_systems(Update, turn::process_turns.in_set(TurnWheelSet))
            .add_systems(
                Update,
                // 资源存在性做门卫而非 in_state：Level 只在 InGame 存在（场景域
                // 管理其生命周期），且 turn.rs 的既有集成测试不含状态/输入插件。
                // 怪物生成由场景域的 MobSpawnRequest 显式触发（进层/下楼一次），
                // 排在英雄生成后：出生格约束需要英雄位置（chain 自动插同步点）
                (
                    hero::spawn_hero.run_if(resource_exists::<Level>),
                    // 多个 run_if 相与：Level 与生成请求同时存在才触发
                    mob::spawn_mobs
                        .run_if(resource_exists::<Level>)
                        .run_if(resource_exists::<MobSpawnRequest>),
                    hero::hero_keyboard_input.run_if(resource_exists::<ButtonInput<KeyCode>>),
                )
                    .chain()
                    .before(TurnWheelSet),
            )
            .add_systems(OnExit(AppState::InGame), hero::reset_turn_wheel)
            // DummyActor 仅供 M1 轮转验证与集成测试；真实行为按标记组件分流，
            // 互不认领对方实体（dummy.rs 分发模式）
            .add_observer(dummy::dummy_act)
            .add_observer(hero::hero_act)
            .add_observer(ai::mob_act);
    }
}
