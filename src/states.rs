//! 顶层应用状态机：Loading（`bevy_asset_loader` 单状态装载全部集合）→ Title → `InGame`。

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Loading,
    Title,
    InGame,
}

pub struct StatesPlugin;

impl Plugin for StatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_systems(OnEnter(AppState::Title), || {
                info!("已进入 Title 状态，资产装载完成");
            })
            .add_systems(OnEnter(AppState::InGame), || {
                info!("已进入 InGame 状态");
            });
    }
}
