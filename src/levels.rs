use num_enum::IntoPrimitive;
use strum::{AsRefStr, EnumIter};

use crate::utils::{LevelPropertyPath, PropertyPath};

pub mod builder;
pub mod cave_level;
pub mod room;
pub mod terrain;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, IntoPrimitive, EnumIter, AsRefStr)]
#[repr(u8)]
// enum of items which have limited spawns, records how many have spawned
// could all be their own separate numbers, but this allows iterating, much nicer for bundling/initializing.
pub enum FellingType {
    None,
    Chasm,
    Water,
    Grass,
    Dark,
    Large,
    Traps,
    Secrets,
}

pub struct Feeling {
    pub feeling: FellingType,
    pub title: String,
    pub description: String,
}

impl LevelPropertyPath for Feeling {
    fn get_property_key(&self) -> String {
        format!(
            "levels.level$feeling.{}",
            self.feeling.as_ref().to_lowercase()
        )
    }
}
pub struct LevelCore {}
