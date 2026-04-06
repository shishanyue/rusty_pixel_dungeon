use bevy::{platform::collections::HashMap, prelude::*};
use bevy_asset_loader::prelude::*;
use solarborn::{
    bevy_asset_loader::asset_collection::AssetCollection, bevy_kira_audio::AudioSource,
};
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    assets::{
        definitions::{PropertiesAsset, PropertiesAssetLoader},
        languages::LanguagePlugin,
    },
    states::LoadingAssetStates,
    utils::PropertyPath,
};

// Macro to simplify asset type definitions
macro_rules! define_asset_type {
    ($name:ident, $base_path:expr, { $($variant:ident => $path:expr),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, EnumIter)]
        pub enum $name {
            $($variant,)*
        }

        impl crate::utils::PropertyPath for $name {
            fn get_property_path(&self) -> &'static str {
                match self {
                    $($name::$variant => concat!($base_path, "/", $path),)*
                }
            }
        }
    };
}

// Macro to simplify asset collection definitions
macro_rules! define_asset_collection {
    ($struct_name:ident, $field_name:ident, $key:tt, $asset_type:ty) => {
        #[derive(AssetCollection, Resource)]
        pub struct $struct_name {
            #[asset(key = $key, collection(mapped, typed))]
            $field_name: HashMap<String, Handle<$asset_type>>,
        }
    };
}

// Macro to simplify adding loading states
macro_rules! add_loading_states {
    ($app:expr, $current_state:ident => $next_state:ident, $collection:ty) => {
        $app.add_loading_state(
            LoadingState::new(LoadingAssetStates::$current_state)
                .continue_to_state(LoadingAssetStates::$next_state)
                .load_collection::<$collection>(),
        );
    };
}

// Macro to simplify registering assets
macro_rules! register_assets {
    ($dynamic_assets:expr, $($key:expr => $type:ty),* $(,)?) => {
        $(
            $dynamic_assets.register_asset(
                $key,
                Box::new(StandardDynamicAsset::Files {
                    paths: <$type>::iter()
                        .map(|t| t.get_property_path().to_string())
                        .collect(),
                }),
            );
        )*
    };
}

pub mod definitions;
pub mod languages;

pub const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/pixel_font.ttf");

// Asset Collections
define_asset_collection!(EffectsCollection, effects, "effects", Image);
define_asset_collection!(EnvironmentCollection, environment, "environment", Image);
define_asset_collection!(InterfacesCollection, interfaces, "interfaces", Image);
define_asset_collection!(MessagesCollection, messages, "properties", PropertiesAsset);
define_asset_collection!(MusicCollection, music, "music", AudioSource);
define_asset_collection!(SoundsCollection, sounds, "sounds", AudioSource);
define_asset_collection!(SplashesCollection, splashes, "splashes", Image);
define_asset_collection!(SpritesCollection, sprites, "sprites", Image);

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<PropertiesAsset>()
            .init_asset_loader::<PropertiesAssetLoader>()
            .add_plugins(LanguagePlugin)
            .add_systems(Startup, setup_assets);
        add_loading_states!(app, MessagesLoading => LanguagesLoading, MessagesCollection);
        add_loading_states!(app, EffectsLoading => EnvironmentLoading, EffectsCollection);
        add_loading_states!(app, EnvironmentLoading => InterfacesLoading, EnvironmentCollection);
        add_loading_states!(app, InterfacesLoading => MusicLoading, InterfacesCollection);
        add_loading_states!(app, MusicLoading => SoundsLoading, MusicCollection);
        add_loading_states!(app, SoundsLoading => SplashesLoading, SoundsCollection);
        add_loading_states!(app, SplashesLoading => SpritesLoading, SplashesCollection);
        add_loading_states!(app, SpritesLoading => SpritesLoading, SpritesCollection);

        let mut assets = app.world_mut().resource_mut::<Assets<Font>>();
        let asset = Font::try_from_bytes(DEFAULT_FONT_DATA.to_vec()).unwrap();
        assets.insert(AssetId::default(), asset).unwrap();
    }
}

// Setup function
fn setup_assets(mut dynamic_assets: ResMut<DynamicAssets>) {
    // Register all asset collections
    register_assets!(
        dynamic_assets,
        "effects" => EffectType,
        "environment" => EnvironmentType,
        "interfaces" => InterfaceType,
        "properties" => MessageType,
        "music" => MusicType,
        "sounds" => SoundType,
        "splashes" => SplashType,
        "sprites" => SpriteType,
    );
}

// Messages
define_asset_type!(MessageType, "messages", {
    Actors => "actors/actors",
    Items => "items/items",
    Journal => "journal/journal",
    Levels => "levels/levels",
    Misc => "misc/misc",
    Plants => "plants/plants",
    Scenes => "scenes/scenes",
    Ui => "ui/ui",
    Windows => "windows/windows",
});

// Effects
define_asset_type!(EffectType, "effects", {
    Effects => "effects.png",
    Fireball => "fireball.png",
    Specks => "specks.png",
    SpellIcons => "spell_icons.png",
    TextIcons => "text_icons.png",
});

// Environment
define_asset_type!(EnvironmentType, "environment", {
    TerrainFeatures => "terrain_features.png",
    VisualGrid => "visual_grid.png",
    WallBlocking => "wall_blocking.png",
    TilesSewers => "tiles_sewers.png",
    TilesPrison => "tiles_prison.png",
    TilesCaves => "tiles_caves.png",
    TilesCity => "tiles_city.png",
    TilesHalls => "tiles_halls.png",
    TilesCavesCrystal => "tiles_caves_crystal.png",
    TilesCavesGnoll => "tiles_caves_gnoll.png",
    WaterSewers => "water0.png",
    WaterPrison => "water1.png",
    WaterCaves => "water2.png",
    WaterCity => "water3.png",
    WaterHalls => "water4.png",
    WeakFloor => "custom_tiles/weak_floor.png",
    SewerBoss => "custom_tiles/sewer_boss.png",
    PrisonQuest => "custom_tiles/prison_quest.png",
    PrisonExit => "custom_tiles/prison_exit.png",
    CavesQuest => "custom_tiles/caves_quest.png",
    CavesBoss => "custom_tiles/caves_boss.png",
    CityQuest => "custom_tiles/city_quest.png",
    CityBoss => "custom_tiles/city_boss.png",
    HallsSp => "custom_tiles/halls_special.png",
});

// Interfaces
define_asset_type!(InterfaceType, "interfaces", {
    ArcsBg => "arcs1.png",
    ArcsFg => "arcs2.png",
    Banners => "banners.png",
    Badges => "badges.png",
    Locked => "locked_badge.png",
    Chrome => "chrome.png",
    Icons => "icons.png",
    Status => "status_pane.png",
    Menu => "menu_pane.png",
    MenuBtn => "menu_button.png",
    Toolbar => "toolbar.png",
    Shadow => "shadow.png",
    Bosshp => "boss_hp.png",
    Surface => "surface.png",
    BuffsSmall => "buffs.png",
    BuffsLarge => "large_buffs.png",
    TalentIcons => "talent_icons.png",
    TalentButton => "talent_button.png",
    HeroIcons => "hero_icons.png",
    RadialMenu => "radial_menu.png",
});

// Music
define_asset_type!(MusicType, "music", {
    Theme1 => "theme_1.ogg",
    Theme2 => "theme_2.ogg",
    ThemeFinale => "theme_finale.ogg",
    Sewers1 => "sewers_1.ogg",
    Sewers2 => "sewers_2.ogg",
    Sewers3 => "sewers_3.ogg",
    SewersTense => "sewers_tense.ogg",
    SewersBoss => "sewers_boss.ogg",
    Prison1 => "prison_1.ogg",
    Prison2 => "prison_2.ogg",
    Prison3 => "prison_3.ogg",
    PrisonTense => "prison_tense.ogg",
    PrisonBoss => "prison_boss.ogg",
    Caves1 => "caves_1.ogg",
    Caves2 => "caves_2.ogg",
    Caves3 => "caves_3.ogg",
    CavesTense => "caves_tense.ogg",
    CavesBoss => "caves_boss.ogg",
    CavesBossFinale => "caves_boss_finale.ogg",
    City1 => "city_1.ogg",
    City2 => "city_2.ogg",
    City3 => "city_3.ogg",
    CityTense => "city_tense.ogg",
    CityBoss => "city_boss.ogg",
    CityBossFinale => "city_boss_finale.ogg",
    Halls1 => "halls_1.ogg",
    Halls2 => "halls_2.ogg",
    Halls3 => "halls_3.ogg",
    HallsTense => "halls_tense.ogg",
    HallsBoss => "halls_boss.ogg",
    HallsBossFinale => "halls_boss_finale.ogg",
});

// Sounds
define_asset_type!(SoundType, "sounds", {
    Click => "click.mp3",
    Badge => "badge.mp3",
    Gold => "gold.mp3",
    Open => "door_open.mp3",
    Unlock => "unlock.mp3",
    Item => "item.mp3",
    Dewdrop => "dewdrop.mp3",
    Step => "step.mp3",
    Water => "water.mp3",
    Grass => "grass.mp3",
    Trample => "trample.mp3",
    Sturdy => "sturdy.mp3",
    Hit => "hit.mp3",
    Miss => "miss.mp3",
    HitSlash => "hit_slash.mp3",
    HitStab => "hit_stab.mp3",
    HitCrush => "hit_crush.mp3",
    HitMagic => "hit_magic.mp3",
    HitStrong => "hit_strong.mp3",
    HitParry => "hit_parry.mp3",
    HitArrow => "hit_arrow.mp3",
    AtkSpiritbow => "atk_spiritbow.mp3",
    AtkCrossbow => "atk_crossbow.mp3",
    HealthWarn => "health_warn.mp3",
    HealthCritical => "health_critical.mp3",
    Descend => "descend.mp3",
    Eat => "eat.mp3",
    Read => "read.mp3",
    Lullaby => "lullaby.mp3",
    Drink => "drink.mp3",
    Shatter => "shatter.mp3",
    Zap => "zap.mp3",
    Lightning => "lightning.mp3",
    Levelup => "levelup.mp3",
    Death => "death.mp3",
    Challenge => "challenge.mp3",
    Cursed => "cursed.mp3",
    Trap => "trap.mp3",
    Evoke => "evoke.mp3",
    Tomb => "tomb.mp3",
    Alert => "alert.mp3",
    Meld => "meld.mp3",
    Boss => "boss.mp3",
    Blast => "blast.mp3",
    Plant => "plant.mp3",
    Ray => "ray.mp3",
    Beacon => "beacon.mp3",
    Teleport => "teleport.mp3",
    Charms => "charms.mp3",
    Mastery => "mastery.mp3",
    Puff => "puff.mp3",
    Rocks => "rocks.mp3",
    Burning => "burning.mp3",
    Falling => "falling.mp3",
    Ghost => "ghost.mp3",
    Secret => "secret.mp3",
    Bones => "bones.mp3",
    Bee => "bee.mp3",
    Degrade => "degrade.mp3",
    Mimic => "mimic.mp3",
    Debuff => "debuff.mp3",
    Chargeup => "chargeup.mp3",
    Gas => "gas.mp3",
    Chains => "chains.mp3",
    Scan => "scan.mp3",
    Sheep => "sheep.mp3",
    Mine => "mine.mp3",
});

// Splashes
define_asset_type!(SplashType, "splashes", {
    Warrior => "warrior.jpg",
    Mage => "mage.jpg",
    Rogue => "rogue.jpg",
    Huntress => "huntress.jpg",
    Duelist => "duelist.jpg",
    Cleric => "cleric.jpg",
    Sewers => "sewers.jpg",
    Prison => "prison.jpg",
    Caves => "caves.jpg",
    City => "city.jpg",
    Halls => "halls.jpg",
    TitleArchs => "title/archs.png",
    TitleBackClusters => "title/back_clusters.png",
    TitleMidMixed => "title/mid_mixed.png",
    TitleFrontSmall => "title/front_small.png",
});

// Sprites
define_asset_type!(SpriteType, "sprites", {
    Items => "items.png",
    ItemIcons => "item_icons.png",
    Warrior => "warrior.png",
    Mage => "mage.png",
    Rogue => "rogue.png",
    Huntress => "huntress.png",
    Duelist => "duelist.png",
    Cleric => "cleric.png",
    Avatars => "avatars.png",
    Pet => "pet.png",
    Amulet => "amulet.png",
    Rat => "rat.png",
    Brute => "brute.png",
    Spinner => "spinner.png",
    Dm300 => "dm300.png",
    Wraith => "wraith.png",
    Undead => "undead.png",
    King => "king.png",
    Piranha => "piranha.png",
    Eye => "eye.png",
    Gnoll => "gnoll.png",
    Crab => "crab.png",
    Goo => "goo.png",
    Swarm => "swarm.png",
    Skeleton => "skeleton.png",
    Shaman => "shaman.png",
    Thief => "thief.png",
    Tengu => "tengu.png",
    Sheep => "sheep.png",
    Keeper => "shopkeeper.png",
    Bat => "bat.png",
    Elemental => "elemental.png",
    Monk => "monk.png",
    Warlock => "warlock.png",
    Golem => "golem.png",
    Statue => "statue.png",
    Succubus => "succubus.png",
    Scorpio => "scorpio.png",
    Fists => "yog_fists.png",
    Yog => "yog.png",
    Larva => "larva.png",
    Ghost => "ghost.png",
    Maker => "wandmaker.png",
    Troll => "blacksmith.png",
    Imp => "demon.png",
    Ratking => "ratking.png",
    Bee => "bee.png",
    Mimic => "mimic.png",
    RotLash => "rot_lasher.png",
    RotHeart => "rot_heart.png",
    Guard => "guard.png",
    Wards => "wards.png",
    Guardian => "guardian.png",
    Slime => "slime.png",
    Snake => "snake.png",
    Necro => "necromancer.png",
    Ghoul => "ghoul.png",
    Ripper => "ripper.png",
    Spawner => "spawner.png",
    Dm100 => "dm100.png",
    Pylon => "pylon.png",
    Dm200 => "dm200.png",
    Lotus => "lotus.png",
    NinjaLog => "ninja_log.png",
    SpiritHawk => "spirit_hawk.png",
    RedSentry => "red_sentry.png",
    CrystalWisp => "crystal_wisp.png",
    CrystalGuardian => "crystal_guardian.png",
    CrystalSpire => "crystal_spire.png",
    GnollGuard => "gnoll_guard.png",
    GnollSapper => "gnoll_sapper.png",
    GnollGeomancer => "gnoll_geomancer.png",
    FungalSpinner => "fungal_spinner.png",
    FungalSentry => "fungal_sentry.png",
    FungalCore => "fungal_core.png",
});
