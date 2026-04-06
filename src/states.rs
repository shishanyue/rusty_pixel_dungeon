use bevy::prelude::*;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum LoadingAssetStates {
    #[default]
    MessagesLoading,
    LanguagesLoading,
    EffectsLoading,
    EnvironmentLoading,
    InterfacesLoading,
    MusicLoading,
    SoundsLoading,
    SplashesLoading,
    SpritesLoading,
    Loaded,
}

pub struct StatesPlugin;

impl Plugin for StatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LoadingAssetStates>();
    }
}
