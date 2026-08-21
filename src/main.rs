use bevy::prelude::*;

use crate::{
    actors::ActorsPlugin, assets::AssetsPlugin, dungeon::DungeonPlugin, scenes::ScenesPlugin,
    setting::SettingPlugin, states::StatesPlugin,
};

pub mod actors;
pub mod assets;
pub mod audio;
pub mod dungeon;
pub mod items;
pub mod levels;
pub mod render;
pub mod scenes;
pub mod setting;
pub mod sprites;
pub mod states;
pub mod utils;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: String::from("Rusty Pixel Dungeon"),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins((
            StatesPlugin,
            SettingPlugin,
            AssetsPlugin,
            DungeonPlugin,
            ActorsPlugin,
            ScenesPlugin,
            render::RenderLevelPlugin,
            audio::GameMusicPlugin,
        ))
        .run();
}
