use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MessageType {
    Actors,
    Items,
    Journal,
    Levels,
    Misc,
    Plants,
    Scenes,
    Ui,
    Windows,
}

#[derive(Debug, Resource)]
pub struct AssetsPathServer {
    pub messages: Arc<HashMap<MessageType, &'static str>>,
}

impl Default for AssetsPathServer {
    fn default() -> Self {
        let mut messages = HashMap::new();

        messages.insert(MessageType::Actors, "messages/actors/actors");
        messages.insert(MessageType::Items, "messages/items/items");
        messages.insert(MessageType::Journal, "messages/journal/journal");
        messages.insert(MessageType::Levels, "messages/levels/levels");
        messages.insert(MessageType::Misc, "messages/misc/misc");
        messages.insert(MessageType::Plants, "messages/plants/plants");
        messages.insert(MessageType::Scenes, "messages/scenes/scenes");
        messages.insert(MessageType::Ui, "messages/ui/ui");
        messages.insert(MessageType::Windows, "messages/windows/windows");

        Self {
            messages: Arc::new(messages),
        }
    }
}

impl AssetsPathServer {
    pub fn get_message_path(&self, message_type: &MessageType) -> Option<&&'static str> {
        self.messages.get(message_type)
    }

    pub fn get_message(&self) -> Arc<HashMap<MessageType, &'static str>> {
        self.messages.clone()
    }
}

pub struct AssetsPathPlugin;

impl Plugin for AssetsPathPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetsPathServer>();
    }
}
