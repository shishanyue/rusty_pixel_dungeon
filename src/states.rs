use bevy::prelude::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum SettingStates {
    #[default]
    LanguagesLoading,
    PropertiesLoading,
    MainMenu,
}

pub struct StatesPlugin;

impl Plugin for StatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<SettingStates>();
    }
}
