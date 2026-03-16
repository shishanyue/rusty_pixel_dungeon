use bevy::prelude::*;

#[derive(Debug, Resource)]
pub struct Settings {
    pub local_code: String,
    pub challenges: i32,
}

pub struct SettingPlugin;

impl Plugin for SettingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings {
            local_code: String::from(""),
            challenges: 0,
        });
    }
}
