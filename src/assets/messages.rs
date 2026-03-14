use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use solarborn::bevy_asset_loader::asset_collection::AssetCollection;

use crate::{
    assets::{messages::languages::LanguagePlugin, properties::PropertiesAsset},
    assets_path::AssetsPathServer,
    states::SettingStates,
};

pub mod languages;

#[derive(AssetCollection, Resource)]
pub struct MessagesCollection {
    #[asset(key = "properties", collection(typed))]
    pub bundles: Vec<Handle<PropertiesAsset>>,
}

pub struct MessagePlugin;

impl Plugin for MessagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LanguagePlugin)
            .add_systems(Startup, setup)
            .add_loading_state(
                LoadingState::new(SettingStates::PropertiesLoading)
                    .continue_to_state(SettingStates::MainMenu)
                    .load_collection::<MessagesCollection>(),
            );
    }
}

fn setup(assets_path_server: Res<AssetsPathServer>, mut dynamic_assets: ResMut<DynamicAssets>) {
    dynamic_assets.register_asset(
        "properties",
        Box::new(StandardDynamicAsset::Files {
            paths: assets_path_server
                .get_message()
                .iter()
                .map(|(_, path)| path.to_string() + ".properties")
                .collect(),
        }),
    );
}
