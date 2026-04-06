use bevy::prelude::*;

use crate::{
    assets::{messages::MessagesCollection, properties::PropertiesAsset},
    levels::{FellingType, cave_level::CaveLevel},
    states::LoadingAssetStates,
    utils::PropertyPath,
};

pub struct TestPlugin;

impl Plugin for TestPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(LoadingAssetStates::Loaded), setup);
    }
}

fn setup(
    mut commands: Commands,
    messages_collection: Res<MessagesCollection>,
    properties_assets: Res<Assets<PropertiesAsset>>,
) {
    //messages_collection.get(path, key, &properties_assets)
    //commands.spawn(CaveLevel);
}
