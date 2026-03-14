use std::{collections::HashMap, io::{BufReader, Cursor}, sync::Arc};

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use java_properties::PropertiesIter;
use solarborn::thiserror::Error;

#[derive(Debug, Asset, TypePath)]
pub struct PropertiesAsset {
    pub properties: Arc<HashMap<String, String>>,
}
#[derive(Default, TypePath)]
pub struct PropertiesAssetLoader;

#[derive(Debug, Error)]
pub enum PropertiesLoaderError {
    #[error("Could not load file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse properties file: {0}")]
    Parse(#[from] java_properties::PropertiesError),
}

impl AssetLoader for PropertiesAssetLoader {
    type Asset = PropertiesAsset;

    type Settings = ();

    type Error = PropertiesLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        let mut properties = HashMap::new();

        reader.read_to_end(&mut bytes).await?;

        PropertiesIter::new(BufReader::new(Cursor::new(bytes))).read_into(|k, v| {
            properties.insert(k, v);
        })?;

        Ok(PropertiesAsset {
            properties: Arc::new(properties),
        })
    }
    
    fn extensions(&self) -> &[&str] {
        &["properties"]
    }
}
