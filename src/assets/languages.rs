//! 语言元数据（`assets/languages/languages.json`）。
//! 消息文本的加载与回退链见 `docs/plans/12-i18n-messages.md`（i18n 域）。

use std::sync::Arc;

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    platform::collections::HashMap,
    prelude::*,
};
use bevy_asset_loader::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::states::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub enum LanguageType {
    English,
    ChiSmpl,
    Korean,
    Russian,
    Spanish,
    Portuguese,
    French,
    German,
    ChiTrad,
    Japanese,
    Polish,
    Italian,
    Turkish,
    Vietnamese,
    Ukrainian,
    Indonesia,
    Czech,
    Dutch,
    Swedish,
    Hungarian,
    Finnish,
    Greek,
    Belarusian,
    Catalan,
    Galicia,
    Basque,
    Esperanto,
}

/// 翻译完成度（低于 80% 的语言不收录）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub enum LanguageStatus {
    /// 约 80-99% 翻译完成
    Unfinish,
    /// 100% 翻译但未审校
    Unreviewed,
    /// 100% 审校完成
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

#[derive(Debug, Asset, TypePath, Deserialize, Serialize)]
pub struct LanguagesAssets(pub Vec<Language>);

#[derive(Default, TypePath)]
pub struct LanguagesAssetLoader;

#[derive(Debug, Error)]
pub enum LanguagesLoaderError {
    #[error("无法读取文件: {0}")]
    Io(#[from] std::io::Error),
    #[error("无法解析 languages.json: {0}")]
    Parse(#[from] serde_json::Error),
}

impl AssetLoader for LanguagesAssetLoader {
    type Asset = LanguagesAssets;
    type Settings = ();
    type Error = LanguagesLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

#[derive(Debug, AssetCollection, Resource)]
pub struct LanguageCollection {
    #[asset(path = "languages/languages.json")]
    pub languages: Handle<LanguagesAssets>,
}

/// 语言类型 → 元数据。装载完成后由 `FromWorld` 构建（`init_resource` 收尾步骤）。
#[derive(Debug, Resource)]
pub struct LanguageServer {
    languages: HashMap<LanguageType, Arc<Language>>,
}

impl LanguageServer {
    /// 取语言元数据；未收录语言回退英语
    pub fn get(&self, lang: LanguageType) -> Arc<Language> {
        self.languages
            .get(&lang)
            .or_else(|| self.languages.get(&LanguageType::English))
            .expect("languages.json 必须包含 English")
            .clone()
    }

    /// 语言代码（`languages.json` 的 `code` 字段，如 `"zh"`）→ 语言元数据；
    /// 未知代码返回 `None`。`Settings.local_code` → [`LanguageType`] 的解析入口。
    pub fn get_by_code(&self, code: &str) -> Option<Arc<Language>> {
        self.languages
            .values()
            .find(|lang| lang.code == code)
            .cloned()
    }
}

impl FromWorld for LanguageServer {
    fn from_world(world: &mut World) -> Self {
        let collection = world.resource::<LanguageCollection>();
        let assets = world.resource::<Assets<LanguagesAssets>>();
        let languages = assets
            .get(&collection.languages)
            .expect("LanguageCollection 装载完成后资产必然存在");

        Self {
            languages: languages
                .0
                .iter()
                .map(|lang| (lang.language_type, Arc::new(lang.clone())))
                .collect(),
        }
    }
}

pub struct LanguagePlugin;

impl Plugin for LanguagePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LanguagesAssets>()
            .init_asset_loader::<LanguagesAssetLoader>()
            .configure_loading_state(
                LoadingStateConfig::new(AppState::Loading)
                    .load_collection::<LanguageCollection>()
                    .finally_init_resource::<LanguageServer>(),
            );
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// 用真实 languages.json 构建（不经 Bevy App，FromWorld 的纯数据部分）
    fn server_from_real_json() -> LanguageServer {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/languages/languages.json");
        let assets: LanguagesAssets =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        LanguageServer {
            languages: assets
                .0
                .iter()
                .map(|lang| (lang.language_type, Arc::new(lang.clone())))
                .collect(),
        }
    }

    /// code → 枚举映射（`Settings.local_code` 解析用）
    #[test]
    fn code_maps_to_language_type() {
        let server = server_from_real_json();
        assert_eq!(
            server.get_by_code("zh").unwrap().language_type,
            LanguageType::ChiSmpl
        );
        assert_eq!(
            server.get_by_code("zh-hant").unwrap().language_type,
            LanguageType::ChiTrad
        );
        assert_eq!(
            server.get_by_code("en").unwrap().language_type,
            LanguageType::English
        );
        assert!(server.get_by_code("xx").is_none());
    }

    /// 未收录语言（如 <80% 翻译的 Finnish）回退英语元数据
    #[test]
    fn unlisted_language_falls_back_to_english() {
        let server = server_from_real_json();
        assert_eq!(
            server.get(LanguageType::Finnish).language_type,
            LanguageType::English
        );
    }
}
