use bevy::prelude::*;

use crate::{
    assets::{messages::MessageServer, properties::PropertiesAsset},
    levels::{FellingType, cave_level::CaveLevel},
    states::LoadingStates,
    utils::PropertyPath,
};

pub struct TestPlugin;

impl Plugin for TestPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(LoadingStates::MainMenu), setup);
    }
}

fn setup(
    mut commands: Commands,
    message_server: Res<MessageServer>,
    properties_assets: Res<Assets<PropertiesAsset>>,
) {
    commands.spawn(CaveLevel);
}
