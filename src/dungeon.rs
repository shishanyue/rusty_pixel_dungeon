use bevy::{platform::collections::HashMap, prelude::*};
use num_enum::IntoPrimitive;
use strum::{AsRefStr, EnumIter, IntoEnumIterator};

use crate::setting::Settings;

#[derive(Debug, Default, Resource)]
pub struct Dungeon {
    pub challenges: i32,
    pub mobs_to_champion: f32,
    pub level: Option<Entity>,

    pub depth: i32,
    pub branch: i32,
}

impl Dungeon {
    pub fn new_level(&mut self) {
        self.level = None;

        match self.branch {
            0 => match self.depth {
                1..=4 => {}
                5 => {}
                6..=9 => {}
                10 => {}
                11..=14 => {}
                15 => {}
                16..=19 => {}
                20 => {}
                21..=24 => {}
                25 => {}
                26 => {}
                _ => {}
            },
            1 => match self.depth {
                11..=14 => {}
                16..=19 => {}
                _ => {}
            },

            _ => {}
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, IntoPrimitive, EnumIter, AsRefStr)]
#[repr(u8)]
// enum of items which have limited spawns, records how many have spawned
// could all be their own separate numbers, but this allows iterating, much nicer for bundling/initializing.
pub enum LimitedDropType {
    // limited world drops
    StrengthPotions,
    UpgradeScrolls,
    ArcaneStyli,
    EnchStone,
    TrinketCata,
    IntStone,
    LabRoom, // actually a room, but logic is the same

    // Health potion sources
    // enemies
    SwarmHp,
    NecroHp,
    BatHp,
    WarlockHp,
    // Demon spawners are already limited in their spawnrate, no need to limit their health drops
    // alchemy
    CookingHp,
    BlandfruitSeed,

    // Other limited enemy drops
    SlimeWep,
    SkeleWep,
    TheifMisc,
    GuardArm,
    ShamanWand,
    Dm200Equip,
    GolemEquip,

    // containers
    VelvetPouch,
    ScrollHolder,
    PotionBandolier,
    MagicalHolster,

    // lore documents
    LoreSewers,
    LorePrison,
    LoreCaves,
    LoreCity,
    LoreHalls,
}

#[derive(Debug, Clone)]
pub struct LimitedDrops {
    pub counts: HashMap<LimitedDropType, u32>,
}

impl Default for LimitedDrops {
    fn default() -> Self {
        Self {
            counts: LimitedDropType::iter().map(|t| (t, 0)).collect(),
        }
    }
}

impl Dungeon {
    pub fn init(&mut self, settings: &Res<Settings>) {
        self.challenges = settings.challenges;
        self.depth = 1;
        self.branch = 0;
    }
}

impl LimitedDrops {
    /// Creates a new instance of LimitedDrops with default values
    pub fn new() -> Self {
        Self::default()
    }

    // Get the count of a specific drop type
    pub fn count(&self, drop_type: LimitedDropType) -> u32 {
        *self.counts.get(&drop_type).unwrap()
    }

    // Check if a specific drop type has been dropped
    pub fn dropped(&self, drop_type: LimitedDropType) -> bool {
        self.count(drop_type) != 0
    }

    // Set the count for a specific drop type
    pub fn set_count(&mut self, drop_type: LimitedDropType, count: u32) {
        self.counts.insert(drop_type, count);
    }

    // Record a drop for a specific drop type by setting its count to 1
    pub fn record_drop(&mut self, drop_type: LimitedDropType) {
        self.set_count(drop_type, 1);
    }

    // Reset all drop counts to 0
    pub fn reset(&mut self) {
        self.counts.iter_mut().for_each(|(_, count)| *count = 0);
    }
    /*
    // Store the current drop counts in a HashMap bundle
    pub fn store(&self, bundle: &mut HashMap<String, u32>) {
        self.counts.iter().for_each(|(drop_type, count)| {
            bundle.insert(drop_type.as_ref().to_owned(), *count);
        });
    }

    // Restore drop counts from a HashMap bundle (implementation not shown)
    pub fn restore(&mut self, bundle: &HashMap<String, u32>) {}
    */
}

pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dungeon>().add_systems(Startup, setup);
    }
}

fn setup(mut dungeon: ResMut<Dungeon>, settings: Res<Settings>) {
    dungeon.init(&settings);
}
