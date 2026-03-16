use bevy::prelude::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum LoadingStates {
    #[default]
    LanguagesLoading,
    PropertiesLoading,
    PropertiesProcessing,
    MainMenu,
}

pub struct StatesPlugin;

impl Plugin for StatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LoadingStates>();
    }
}
