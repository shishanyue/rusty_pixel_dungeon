//! `.properties` 消息文件的资产加载器（SPD 消息格式为 java properties）。

use std::{collections::HashMap, fmt::Write as _, io::Cursor, sync::Arc};

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use java_properties::PropertiesIter;
use thiserror::Error;

#[derive(Debug, Asset, TypePath)]
pub struct PropertiesAsset {
    pub properties: Arc<HashMap<String, String>>,
}

#[derive(Default, TypePath)]
pub struct PropertiesAssetLoader;

#[derive(Debug, Error)]
pub enum PropertiesLoaderError {
    #[error("无法读取文件: {0}")]
    Io(#[from] std::io::Error),
    #[error("properties 文件不是合法 UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("无法解析 properties 文件: {0}")]
    Parse(#[from] java_properties::PropertiesError),
    #[error("properties 文件含增补平面字符 {0:?}（java-properties 的 \\uXXXX 解码不支持代理对）")]
    SupplementaryChar(char),
}

/// 解析 SPD 消息 `.properties` 字节流（UTF-8 编码，与 libgdx `I18NBundle` 默认一致）。
///
/// `java-properties` 固定按 windows-1252 解码，直接喂原始字节会把中文等
/// 多字节文本读成乱码；这里先把非 ASCII 字符预转义成 `\uXXXX` 再交给解析器还原。
/// 转义序列只含 ASCII，不会引入新的行分隔/键值分隔/注释语义。
pub fn parse_properties(bytes: &[u8]) -> Result<HashMap<String, String>, PropertiesLoaderError> {
    let text = std::str::from_utf8(bytes)?;
    // 纯 ASCII（英文基线的常态）直接零拷贝走原文
    let escaped = if text.is_ascii() {
        None
    } else {
        Some(escape_non_ascii(text)?)
    };
    let source = escaped.as_deref().unwrap_or(text);

    let mut properties = HashMap::new();
    PropertiesIter::new(Cursor::new(source.as_bytes())).read_into(|k, v| {
        properties.insert(k, v);
    })?;
    Ok(properties)
}

/// 非 ASCII 字符 → `\uXXXX` 转义。增补平面字符无法用单个 `\uXXXX` 表达
/// （java-properties 拒绝代理对），SPD 消息文件已核实不含，遇到即报错而非静默损坏。
fn escape_non_ascii(text: &str) -> Result<String, PropertiesLoaderError> {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let code = u32::from(c);
            if code > 0xFFFF {
                return Err(PropertiesLoaderError::SupplementaryChar(c));
            }
            write!(out, "\\u{code:04x}").expect("向 String 写入不会失败");
        }
    }
    Ok(out)
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
        reader.read_to_end(&mut bytes).await?;

        Ok(PropertiesAsset {
            properties: Arc::new(parse_properties(&bytes)?),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["properties"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UTF-8 中文不经窗口 1252 乱码（曾用默认编码直读字节，中文必坏）
    #[test]
    fn parse_keeps_utf8_text() {
        let props = parse_properties("k.name=啮齿小鼠\nk.plain=rat\n".as_bytes()).unwrap();
        assert_eq!(props["k.name"], "啮齿小鼠");
        assert_eq!(props["k.plain"], "rat");
    }

    /// properties 自身的 `\n`、`\uXXXX` 转义仍照常还原
    #[test]
    fn parse_keeps_builtin_escapes() {
        let props = parse_properties(br"k=a\nb\u4e2d".as_slice()).unwrap();
        assert_eq!(props["k"], "a\nb中");
    }

    /// 增补平面字符明确报错（而非静默损坏）
    #[test]
    fn parse_rejects_supplementary_chars() {
        let err = parse_properties("k=😀\n".as_bytes()).unwrap_err();
        assert!(matches!(
            err,
            PropertiesLoaderError::SupplementaryChar('😀')
        ));
    }
}
