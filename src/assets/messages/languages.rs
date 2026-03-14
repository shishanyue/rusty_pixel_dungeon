use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use serde::{Deserialize, Serialize};
use solarborn::bevy_common_assets::json::JsonAssetPlugin;

use crate::states::SettingStates;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub enum LanguageType {
    English,
    ChiSmpl,
    Korean,
    Russian,
    Spanish,
    Portugues,
    French,
    German,
    ChiTrad,
    Japanese,
    Polish,
    Italian,
    Turkish,
    Vietnames,
    Ukranian,
    Indonesia,
    Czech,
    Dutch,
    Swedish,
    Hungarian,
    Finnish,
    Greek,
    Belarusia,
    Catalan,
    Galicia,
    Basque,
    Esperanto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
//below 80% translated languages are not added or removed
pub enum LanguageStatus {
    //unfinished, ~80-99% translated
    Unfinish,
    //unreviewed, but 100% translated
    Unreview,
    //complete, 100% reviewed
    Complete,
}

#[derive(Debug, Clone, Asset, TypePath, Deserialize, Serialize)]
pub struct Language {
    pub language_type: LanguageType,
    pub name: String,
    pub code: String,
    pub status: LanguageStatus,
    pub reviewers: Option<Vec<String>>,
    pub translators: Option<Vec<String>>,
}

impl Language {
    pub fn new(
        language_type: LanguageType,
        name: String,
        code: String,
        status: LanguageStatus,
        reviewers: Option<Vec<String>>,
        translators: Option<Vec<String>>,
    ) -> Self {
        Self {
            language_type,
            name,
            code,
            status,
            reviewers,
            translators,
        }
    }
}

#[derive(Debug, Asset, TypePath, Deserialize, Serialize)]
pub struct LanguagesAssets(pub Vec<Language>);

#[derive(Debug, AssetCollection, Resource)]
pub struct LanguageCollection {
    #[asset(path = "languages/languages.json")]
    pub languages: Handle<LanguagesAssets>,
}

#[derive(Debug, Resource)]
pub struct LanguageServer {
    pub languages: HashMap<LanguageType, Arc<Language>>,
}

impl LanguageServer {
    pub fn new(languages: HashMap<LanguageType, Arc<Language>>) -> Self {
        Self { languages }
    }

    pub fn match_code(&self, lang_type: LanguageType) -> Arc<Language> {
        return self
            .languages
            .get(&lang_type)
            .unwrap_or(self.languages.get(&LanguageType::English).unwrap())
            .clone();
    }
}

pub struct LanguagePlugin;

impl Plugin for LanguagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(JsonAssetPlugin::<LanguagesAssets>::new(&["json"]))
            .add_loading_state(
                LoadingState::new(SettingStates::LanguagesLoading)
                    .continue_to_state(SettingStates::PropertiesLoading)
                    .load_collection::<LanguageCollection>(),
            )
            .add_systems(OnEnter(SettingStates::PropertiesLoading), setup);
    }
}

fn setup(
    mut commands: Commands,
    language_collection: Res<LanguageCollection>,
    languages_assets: Res<Assets<LanguagesAssets>>,
) {
    let languages = languages_assets
        .get(&language_collection.languages)
        .unwrap();

    let language_map = languages
        .0
        .iter()
        .into_iter()
        .map(|lang| (lang.language_type, Arc::new(lang.clone())))
        .collect();

    println!("Loaded languages: {:?}", language_map);

    commands.insert_resource(LanguageServer::new(language_map));
}
