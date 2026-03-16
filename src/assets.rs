use bevy::prelude::*;

use crate::assets::{
    messages::MessagePlugin,
    paths::AssetsPathPlugin,
    properties::{PropertiesAsset, PropertiesAssetLoader},
};

pub mod languages;
pub mod messages;
pub mod paths;
pub mod properties;

pub const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/pixel_font.ttf");

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<PropertiesAsset>()
            .init_asset_loader::<PropertiesAssetLoader>()
            .add_plugins(MessagePlugin)
            .add_plugins(AssetsPathPlugin);

        let mut assets = app.world_mut().resource_mut::<Assets<Font>>();
        let asset = Font::try_from_bytes(DEFAULT_FONT_DATA.to_vec()).unwrap();
        assets.insert(AssetId::default(), asset).unwrap();
    }
}
