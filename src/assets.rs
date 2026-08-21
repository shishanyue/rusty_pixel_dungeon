//! 资产层：8 个动态资产集合 + 字体集合，在单一 `AppState::Loading` 内全部装载完成。
//!
//! 资产枚举表与 SPD `Assets.java` 逐项对照；键 = 资产完整路径（`MapKey for String`）。

use bevy::{audio::AudioSource, platform::collections::HashMap, prelude::*};
use bevy_asset_loader::prelude::*;
use strum::{EnumIter, IntoEnumIterator};

use crate::{
    assets::{definitions::PropertiesAsset, languages::LanguagePlugin, messages::MessagesPlugin},
    states::AppState,
    utils::PropertyPath,
};

pub mod definitions;
pub mod languages;
pub mod messages;

/// 定义资产枚举与其文件路径（PropertyPath 返回完整资产路径）
macro_rules! define_asset_type {
    ($name:ident, $base_path:expr, { $($variant:ident => $path:expr),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, EnumIter)]
        pub enum $name {
            $($variant,)*
        }

        impl $crate::utils::PropertyPath for $name {
            fn get_property_path(&self) -> &'static str {
                match self {
                    $($name::$variant => concat!($base_path, "/", $path),)*
                }
            }
        }
    };
}

/// 定义动态资产集合：路径键 → Handle，并生成按枚举取用的公有 API
macro_rules! define_asset_collection {
    ($struct_name:ident, $field_name:ident, $key:tt, $asset_type:ty, $handle_type:ty) => {
        #[derive(AssetCollection, Resource)]
        pub struct $struct_name {
            #[asset(key = $key, collection(mapped, typed))]
            $field_name: HashMap<String, Handle<$handle_type>>,
        }

        impl $struct_name {
            /// 按枚举键取句柄；集合装载完成后枚举必然命中，miss 即为编程错误
            pub fn get(&self, key: $asset_type) -> Handle<$handle_type> {
                self.get_by_path(key.get_property_path())
            }

            /// 按资产路径取句柄（供语言变体等运行期拼接的路径使用）
            pub fn get_by_path(&self, path: &str) -> Handle<$handle_type> {
                self.$field_name
                    .get(path)
                    .unwrap_or_else(|| panic!("资产未注册: {path}"))
                    .clone()
            }

            pub fn contains(&self, path: &str) -> bool {
                self.$field_name.contains_key(path)
            }
        }
    };
}

// 资产集合（key 与 register_dynamic_assets 中的注册键一致）
define_asset_collection!(EffectsCollection, effects, "effects", EffectType, Image);
define_asset_collection!(
    EnvironmentCollection,
    environment,
    "environment",
    EnvironmentType,
    Image
);
define_asset_collection!(
    InterfacesCollection,
    interfaces,
    "interfaces",
    InterfaceType,
    Image
);
define_asset_collection!(
    MessagesCollection,
    messages,
    "properties",
    MessageType,
    PropertiesAsset
);
define_asset_collection!(MusicCollection, music, "music", MusicType, AudioSource);
define_asset_collection!(SoundsCollection, sounds, "sounds", SoundType, AudioSource);
define_asset_collection!(SplashesCollection, splashes, "splashes", SplashType, Image);
define_asset_collection!(SpritesCollection, sprites, "sprites", SpriteType, Image);

/// 字体（静态路径，无需动态注册）
#[derive(AssetCollection, Resource)]
pub struct FontAssets {
    #[asset(path = "fonts/pixel_font.ttf")]
    pub pixel: Handle<Font>,
}

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app
            // 动态资产注册必须早于 Loading 状态的 OnEnter（首帧 StateTransition）
            .add_systems(PreStartup, register_dynamic_assets)
            .add_loading_state(
                LoadingState::new(AppState::Loading)
                    .continue_to_state(AppState::Title)
                    .load_collection::<EffectsCollection>()
                    .load_collection::<EnvironmentCollection>()
                    .load_collection::<InterfacesCollection>()
                    .load_collection::<MusicCollection>()
                    .load_collection::<SoundsCollection>()
                    .load_collection::<SplashesCollection>()
                    .load_collection::<SpritesCollection>()
                    .load_collection::<FontAssets>(),
            )
            // 两个子插件用 configure_loading_state 挂到上面的 Loading 状态，
            // 必须在 add_loading_state 之后注册
            .add_plugins((LanguagePlugin, MessagesPlugin));
    }
}

fn register_dynamic_assets(mut dynamic_assets: ResMut<DynamicAssets>) {
    fn register<T: PropertyPath + IntoEnumIterator>(assets: &mut DynamicAssets, key: &str) {
        assets.register_asset(
            key,
            Box::new(StandardDynamicAsset::Files {
                paths: T::iter()
                    .map(|t| t.get_property_path().to_string())
                    .collect(),
            }),
        );
    }

    register::<EffectType>(&mut dynamic_assets, "effects");
    register::<EnvironmentType>(&mut dynamic_assets, "environment");
    register::<InterfaceType>(&mut dynamic_assets, "interfaces");
    // "properties"（消息文件）由 messages::register_message_dynamic_assets 注册：
    // 除英文基线外还要按 Settings.local_code 追加语言变体
    register::<MusicType>(&mut dynamic_assets, "music");
    register::<SoundType>(&mut dynamic_assets, "sounds");
    register::<SplashType>(&mut dynamic_assets, "splashes");
    register::<SpriteType>(&mut dynamic_assets, "sprites");
}

// 消息（英文基线；语言后缀变体在 messages.rs 按 Settings.local_code 运行期追加注册）
define_asset_type!(MessageType, "messages", {
    Actors => "actors/actors.properties",
    Items => "items/items.properties",
    Journal => "journal/journal.properties",
    Levels => "levels/levels.properties",
    Misc => "misc/misc.properties",
    Plants => "plants/plants.properties",
    Scenes => "scenes/scenes.properties",
    Ui => "ui/ui.properties",
    Windows => "windows/windows.properties",
});

// 特效
// SPD 的 FIREBALL 常量是 "fireball.png"，但真实文件是 -short/-tall 两份
// （Fireball.java 运行时替换后缀），此处直接枚举真实文件
define_asset_type!(EffectType, "effects", {
    Effects => "effects.png",
    FireballShort => "fireball-short.png",
    FireballTall => "fireball-tall.png",
    Specks => "specks.png",
    SpellIcons => "spell_icons.png",
    TextIcons => "text_icons.png",
});

// 环境
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

// 界面
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

// 音乐
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

// 音效
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

// 闪屏
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

// 精灵
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 所有枚举登记的资产路径必须真实存在（防文件名漂移，如 fireball-short/tall）
    #[test]
    fn all_registered_asset_paths_exist() {
        fn check<T: PropertyPath + IntoEnumIterator + std::fmt::Debug>(missing: &mut Vec<String>) {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
            for entry in T::iter() {
                let path = root.join(entry.get_property_path());
                if !path.is_file() {
                    missing.push(format!("{entry:?} → {}", path.display()));
                }
            }
        }

        let mut missing = Vec::new();
        check::<EffectType>(&mut missing);
        check::<EnvironmentType>(&mut missing);
        check::<InterfaceType>(&mut missing);
        check::<MessageType>(&mut missing);
        check::<MusicType>(&mut missing);
        check::<SoundType>(&mut missing);
        check::<SplashType>(&mut missing);
        check::<SpriteType>(&mut missing);
        assert!(missing.is_empty(), "缺失资产:\n{}", missing.join("\n"));
    }

    /// 字体文件存在（FontAssets 用静态路径，不走枚举）
    #[test]
    fn font_asset_exists() {
        let font = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/pixel_font.ttf");
        assert!(font.is_file());
    }
}
