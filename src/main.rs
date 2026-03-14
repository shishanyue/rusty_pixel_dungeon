use bevy::app::App;
use solarborn::SolarBornPlugins;

use crate::{assets::AssetsPlugin, assets_path::AssetsPathPlugin, states::StatesPlugin};

pub mod assets;
pub mod assets_path;
pub mod setting;
pub mod states;

fn main() {
    App::new()
        .add_plugins(SolarBornPlugins)
        .add_plugins(AssetsPlugin)
        .add_plugins(AssetsPathPlugin)
        .add_plugins(StatesPlugin)
        .run();
}
