use bevy::prelude::*;
use solarborn::SolarBornPlugins;

use crate::{
    assets::AssetsPlugin, dungeon::DungeonPlugin, setting::SettingPlugin, states::StatesPlugin, test::TestPlugin,
};

pub mod assets;
pub mod dungeon;
pub mod global;
pub mod levels;
pub mod setting;
pub mod states;
pub mod test;
pub mod utils;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: String::from(
                            "Rusty Pixel Dungeon - A Roguelike Game Made with Rust and Bevy",
                        ),
                        ..Default::default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(SolarBornPlugins)
        .add_plugins(DungeonPlugin)
        .add_plugins(AssetsPlugin)
        .add_plugins(StatesPlugin)
        .add_plugins(SettingPlugin)
        .add_plugins(TestPlugin)
        .run();
}
