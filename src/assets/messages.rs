use std::collections::HashMap;

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use solarborn::bevy_asset_loader::asset_collection::AssetCollection;

use crate::{
    assets::{languages::LanguagePlugin, paths::AssetsPathServer, properties::PropertiesAsset},
    states::LoadingStates,
};

#[derive(AssetCollection, Resource)]
pub struct MessagesCollection {
    #[asset(key = "properties", collection(typed))]
    handles: Vec<Handle<PropertiesAsset>>,
}

#[derive(Debug, Resource)]
pub struct MessageServer {
    pub bundles: HashMap<String, Handle<PropertiesAsset>>,
}

impl MessageServer {
    pub fn get(
        &self,
        path: &str,
        key: &str,
        properties_assets: &Res<Assets<PropertiesAsset>>,
    ) -> Option<String> {
        self.bundles
            .get(path)
            .and_then(|handle| properties_assets.get(handle))
            .and_then(|properties| properties.properties.get(key))
            .cloned()
    }
}

impl MessageServer {
    pub fn new(bundles: HashMap<String, Handle<PropertiesAsset>>) -> Self {
        Self { bundles }
    }
}

pub struct MessagePlugin;

impl Plugin for MessagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LanguagePlugin)
            .add_systems(Startup, setup)
            .add_loading_state(
                LoadingState::new(LoadingStates::PropertiesLoading)
                    .continue_to_state(LoadingStates::PropertiesProcessing)
                    .load_collection::<MessagesCollection>(),
            )
            .add_systems(OnEnter(LoadingStates::PropertiesProcessing), process);
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

fn process(
    mut commands: Commands,
    messages_collection: Res<MessagesCollection>,
    mut loading_states: ResMut<NextState<LoadingStates>>,
) {
    commands.insert_resource(MessageServer::new(
        messages_collection
            .handles
            .iter()
            .filter_map(|handle| handle.path().map(|path| (path.to_string(), handle.clone())))
            .collect(),
    ));

    loading_states.set(LoadingStates::MainMenu);
}
