//! 场景与 UI 域：场景骨架 + `TitleScene` + `InGame` 占位（见 `docs/plans/13-scenes-ui.md`）。
//!
//! 骨架约定：每个场景一个子插件，`OnEnter(状态)` 生成实体并挂 [`DespawnOnExit`]
//! （bevy 0.19 状态作用域实体，`init_state` 时自动启用清理系统），
//! 状态退出时由 `StateTransition` 调度自动销毁，无需手写 `OnExit` 清理。

use bevy::prelude::*;

pub mod in_game;
pub mod text;
pub mod title;

pub struct ScenesPlugin;

impl Plugin for ScenesPlugin {
    fn build(&self, app: &mut App) {
        app
            // SPD 标题星空为纯黑底（TitleScene.java 直接画在引擎默认黑色 clear 上）
            .insert_resource(ClearColor(Color::BLACK))
            .add_systems(Startup, spawn_camera)
            .add_plugins((title::TitleScenePlugin, in_game::InGameScenePlugin));
    }
}

/// 全局唯一 `Camera2d`：Startup 生成一次、跨场景复用，不挂状态作用域组件。
fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

#[cfg(test)]
mod tests {
    use bevy::{input::ButtonInput, prelude::*, state::app::StatesPlugin};

    use super::*;
    use crate::states::AppState;
    use in_game::InGameRoot;
    use title::TitleUiRoot;

    /// 无渲染 App：`MinimalPlugins` + bevy 状态调度 + 被测插件。
    /// 无资产集合 → 资产装饰系统被 `run_if` 跳过，只生成场景根实体
    /// （见各场景插件里的注释），足以覆盖状态闭环与实体清理。
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppState>()
            // MinimalPlugins 不含 InputPlugin，Esc 系统所需的键盘资源手动补齐
            .init_resource::<ButtonInput<KeyCode>>()
            // InGame 的 setup_level 消费 Dungeon（生产环境由 main 注册，此处对齐）
            .add_plugins((crate::setting::SettingPlugin, crate::dungeon::DungeonPlugin))
            .add_plugins(ScenesPlugin);
        // Startup + 初始进入 Loading
        app.update();
        app
    }

    /// 写入 `NextState` 并推进一帧，让 `StateTransition` 应用切换
    fn set_state(app: &mut App, state: AppState) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(state);
        app.update();
    }

    fn count<C: Component>(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<C>>()
            .iter(app.world())
            .count()
    }

    /// 状态闭环 Loading → Title → `InGame` → Title：
    /// `DespawnOnExit` 状态作用域实体在每次退出时被清理、重进时重新生成
    #[test]
    fn scene_entities_despawn_on_state_exit() {
        let mut app = test_app();
        assert_eq!(
            count::<TitleUiRoot>(&mut app),
            0,
            "Loading 阶段不应有 Title 实体"
        );

        set_state(&mut app, AppState::Title);
        assert_eq!(count::<TitleUiRoot>(&mut app), 1);
        assert_eq!(count::<InGameRoot>(&mut app), 0);

        set_state(&mut app, AppState::InGame);
        assert_eq!(
            count::<TitleUiRoot>(&mut app),
            0,
            "退出 Title 后场景实体应被清理"
        );
        assert_eq!(count::<InGameRoot>(&mut app), 1);

        set_state(&mut app, AppState::Title);
        assert_eq!(
            count::<InGameRoot>(&mut app),
            0,
            "退出 InGame 后场景实体应被清理"
        );
        assert_eq!(count::<TitleUiRoot>(&mut app), 1, "重进 Title 应重新生成");
    }

    /// 相机全局唯一：跨多次场景切换既不重复生成也不被状态清理
    #[test]
    fn camera_spawned_once_globally() {
        let mut app = test_app();
        set_state(&mut app, AppState::Title);
        set_state(&mut app, AppState::InGame);
        set_state(&mut app, AppState::Title);
        assert_eq!(count::<Camera2d>(&mut app), 1);
    }

    /// M2 竖切：进入 `InGame` 开新一局（depth 重置为 1）并生成关卡资源，
    /// 退出时移除（地形绘制归 16 号渲染域 tilemap，方块调试视图已下线）
    #[test]
    fn in_game_creates_and_tears_down_level() {
        use crate::{dungeon::Dungeon, levels::Level};

        let mut app = test_app();
        set_state(&mut app, AppState::Title);
        assert!(app.world().get_resource::<Level>().is_none());

        // 弄脏 depth，验证从标题进入即开新一局（Dungeon.init 语义）
        app.world_mut().resource_mut::<Dungeon>().depth = 7;
        set_state(&mut app, AppState::InGame);
        let level = app
            .world()
            .get_resource::<Level>()
            .expect("进入 InGame 应生成 Level");
        assert!(level.size() > 0);
        assert_eq!(
            app.world().resource::<Dungeon>().depth,
            1,
            "从标题进入应重置为新一局"
        );

        set_state(&mut app, AppState::Title);
        assert!(
            app.world().get_resource::<Level>().is_none(),
            "退出 InGame 应移除 Level 资源"
        );
    }

    /// `InGame` 按 Esc 返回 Title 并清理场景实体
    #[test]
    fn esc_returns_from_in_game_to_title() {
        let mut app = test_app();
        set_state(&mut app, AppState::Title);
        set_state(&mut app, AppState::InGame);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        // 第一帧 Esc 系统写入 NextState，第二帧 StateTransition 应用切换
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Title
        );
        assert_eq!(count::<InGameRoot>(&mut app), 0);
        assert_eq!(count::<TitleUiRoot>(&mut app), 1);
    }
}
