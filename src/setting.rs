//! 用户设置，对照 SPD `SPDSettings.java`（M1 仅语言与挑战位，后续按需扩展）。

use bevy::prelude::*;

#[derive(Debug, Resource)]
pub struct Settings {
    /// 语言代码（对应 languages.json 的 `code` 字段与消息文件后缀）
    pub local_code: String,
    pub challenges: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            local_code: String::from("en"),
            challenges: 0,
        }
    }
}

pub struct SettingPlugin;

impl Plugin for SettingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>();
    }
}
