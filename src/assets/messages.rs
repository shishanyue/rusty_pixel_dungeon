//! 消息域：key 归一化、多 bundle 链式查找、miss 哨兵、参数替换与 `Resource Messages`。
//!
//! 对照 SPD `Messages.java`：9 个 bundle 按 `MessageType` 枚举序（即 SPD `prop_files`
//! 顺序）查找，key 查找前全小写，全链 miss 返回哨兵 [`NO_TEXT_FOUND`]；
//! 语言变体（`<cat>_<code>.properties`）排在英文基线（`<cat>.properties`，无后缀）
//! 之前构成回退链。纯函数层不依赖 Bevy `World`，单测直接对拍真实 properties 文件。

use std::{borrow::Cow, collections::HashMap, path::Path, sync::Arc};

use bevy::{asset::io::file::FileAssetReader, prelude::*};
use bevy_asset_loader::prelude::*;
use strum::IntoEnumIterator;

use crate::{
    assets::{
        MessageType, MessagesCollection,
        definitions::{PropertiesAsset, PropertiesAssetLoader},
    },
    setting::Settings,
    states::AppState,
    utils::PropertyPath,
};

/// miss 哨兵，与 SPD `Messages.NO_TEXT_FOUND` 逐字一致
pub const NO_TEXT_FOUND: &str = "!!!NO TEXT FOUND!!!";

// ---------- 纯函数层 ----------

/// key 归一化：SPD 在查找前统一 `toLowerCase(Locale.ENGLISH)`（文件内 key 本身已小写）
pub fn normalize_key(key: &str) -> Cow<'_, str> {
    if key.is_ascii() && !key.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Borrowed(key)
    } else {
        Cow::Owned(key.to_lowercase())
    }
}

/// 沿查找链取第一个命中值
pub fn lookup<'a>(chain: &'a [Arc<HashMap<String, String>>], key: &str) -> Option<&'a str> {
    let key = normalize_key(key);
    chain
        .iter()
        .find_map(|bundle| bundle.get(key.as_ref()).map(String::as_str))
}

/// 沿查找链取值；全链 miss 返回 [`NO_TEXT_FOUND`]（SPD `Messages.get` 语义）
pub fn resolve(chain: &[Arc<HashMap<String, String>>], key: &str) -> String {
    lookup(chain, key).map_or_else(|| NO_TEXT_FOUND.to_owned(), str::to_owned)
}

/// 参数替换。支持两种占位符：
///
/// - `{N}`：libgdx `I18NBundle.format` 风格（计划书 M1 指定）；
/// - printf 子集：`%s`/`%d`/`%f`（顺序消费参数）、`%N$s` 位置参数（1 起）、
///   `%%` 字面百分号、`%.2f` 精度（忽略，参数已由调用方格式化为字符串）——
///   SPD 现版消息文件实际使用该风格（`Messages.format` = `String.format`）。
///
/// 索引越界或无法识别的占位符原样保留（SPD 遇 `IllegalFormatException` 返回原文，
/// 此处按"部分替换、余者保留"处理，便于排查缺参调用）。
pub fn format_args(template: &str, args: &[&str]) -> String {
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len() + 16);
    let mut lit_start = 0usize;
    let mut i = 0usize;
    // 顺序占位符（%s 等）已消费到的参数下标；位置参数不影响它（Java printf 同义）
    let mut next_seq = 0usize;
    while i < bytes.len() {
        let parsed = match bytes[i] {
            b'{' => parse_brace(bytes, i, args),
            b'%' => parse_percent(bytes, i, args, &mut next_seq),
            _ => None,
        };
        if let Some((end, replacement)) = parsed {
            out.push_str(&template[lit_start..i]);
            out.push_str(replacement);
            i = end;
            lit_start = end;
        } else {
            i += 1;
        }
    }
    out.push_str(&template[lit_start..]);
    out
}

/// 自 `from` 起扫描十进制数字，返回（结束偏移, 数值）；无数字或溢出返回 `None`
fn scan_digits(bytes: &[u8], from: usize) -> Option<(usize, usize)> {
    let mut end = from;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == from {
        return None;
    }
    let value = core::str::from_utf8(&bytes[from..end]).ok()?.parse().ok()?;
    Some((end, value))
}

/// `{N}` → `args[N]`；成功返回（占位符结束偏移, 替换文本）
fn parse_brace<'a>(bytes: &[u8], start: usize, args: &'a [&str]) -> Option<(usize, &'a str)> {
    let (digits_end, index) = scan_digits(bytes, start + 1)?;
    if bytes.get(digits_end) != Some(&b'}') {
        return None;
    }
    args.get(index).map(|arg| (digits_end + 1, *arg))
}

/// printf 子集；成功返回（占位符结束偏移, 替换文本）
fn parse_percent<'a>(
    bytes: &[u8],
    start: usize,
    args: &'a [&str],
    next_seq: &mut usize,
) -> Option<(usize, &'a str)> {
    let mut i = start + 1;
    if bytes.get(i) == Some(&b'%') {
        return Some((i + 1, "%"));
    }
    // 位置参数 `N$`（数字后无 `$` 即宽度语法，SPD 文件不用，原样保留）
    let mut positional = None;
    if let Some((digits_end, value)) = scan_digits(bytes, i) {
        if bytes.get(digits_end) != Some(&b'$') {
            return None;
        }
        positional = Some(value.checked_sub(1)?);
        i = digits_end + 1;
    }
    // 精度 `.N`（如 %.2f），替换时忽略
    if bytes.get(i) == Some(&b'.') {
        let (digits_end, _) = scan_digits(bytes, i + 1)?;
        i = digits_end;
    }
    if !matches!(bytes.get(i), Some(b's' | b'd' | b'f')) {
        return None;
    }
    let index = match positional {
        Some(p) => p,
        None => {
            let seq = *next_seq;
            // 参数不足时不消费顺序下标，占位符原样保留
            if seq >= args.len() {
                return None;
            }
            *next_seq = seq + 1;
            seq
        }
    };
    args.get(index).map(|arg| (i + 1, *arg))
}

/// 英文基线路径 + 语言代码 → 变体路径（语言代码后缀规则见 `assets/messages/` 文件名）：
/// `messages/actors/actors.properties` + `zh` → `messages/actors/actors_zh.properties`
pub fn variant_path(base_path: &str, code: &str) -> String {
    let stem = base_path.strip_suffix(".properties").unwrap_or(base_path);
    format!("{stem}_{code}.properties")
}

/// 按语言代码列出应注册的变体路径，并按磁盘存在性分成（存在, 缺失）两组；
/// `en` 即英文基线本身，无变体。部分语言可能缺个别分类文件，缺失组由调用方
/// 告警后跳过（注册不存在的路径会让 loading state 永久卡死）。
pub fn split_variant_paths(assets_root: &Path, code: &str) -> (Vec<String>, Vec<String>) {
    if code == "en" {
        return (Vec::new(), Vec::new());
    }
    let mut existing = Vec::new();
    let mut missing = Vec::new();
    for category in MessageType::iter() {
        let path = variant_path(category.get_property_path(), code);
        if assets_root.join(&path).is_file() {
            existing.push(path);
        } else {
            missing.push(path);
        }
    }
    (existing, missing)
}

// ---------- Bevy 层 ----------

/// 把消息文件注册进 `"properties"` 动态资产（key 与 `MessagesCollection` 的
/// `#[asset(key = "properties")]` 一致）：英文基线 9 个文件必注册；非 `en` 语言
/// 追加磁盘上存在的 `<cat>_<code>.properties`，缺失分类回退英文基线并告警。
/// 必须在 `PreStartup`（Loading 状态 `OnEnter` 之前）执行。
fn register_message_dynamic_assets(
    mut dynamic_assets: ResMut<DynamicAssets>,
    settings: Res<Settings>,
) {
    let mut paths: Vec<String> = MessageType::iter()
        .map(|t| t.get_property_path().to_owned())
        .collect();

    let assets_root = FileAssetReader::get_base_path().join("assets");
    let (existing, missing) = split_variant_paths(&assets_root, &settings.local_code);
    for path in &missing {
        warn!(
            "语言 {} 缺少消息文件 {path}，该分类回退英文基线",
            settings.local_code
        );
    }
    paths.extend(existing);

    dynamic_assets.register_asset(
        "properties",
        Box::new(StandardDynamicAsset::Files { paths }),
    );
}

/// 消息查找资源。查找链 = [语言变体 bundle…, 英文基线 bundle…]，
/// 两段内部均按 `MessageType` 枚举序。装载完成后由 `FromWorld` 构建
/// （`finally_init_resource` 收尾步骤，模式同 `LanguageServer`）。
#[derive(Debug, Resource)]
pub struct Messages {
    chain: Vec<Arc<HashMap<String, String>>>,
}

impl Messages {
    /// 取消息文本；key 大小写不敏感，全链 miss 返回 [`NO_TEXT_FOUND`] 哨兵
    pub fn get(&self, key: &str) -> String {
        resolve(&self.chain, key)
    }

    /// 取消息并做参数替换；miss 时返回哨兵且不做替换（SPD 同义）
    pub fn format(&self, key: &str, args: &[&str]) -> String {
        lookup(&self.chain, key).map_or_else(
            || NO_TEXT_FOUND.to_owned(),
            |value| format_args(value, args),
        )
    }
}

impl FromWorld for Messages {
    fn from_world(world: &mut World) -> Self {
        let local_code = world.resource::<Settings>().local_code.clone();
        let collection = world.resource::<MessagesCollection>();
        let assets = world.resource::<Assets<PropertiesAsset>>();

        let bundle = |path: &str| {
            assets
                .get(&collection.get_by_path(path))
                .expect("MessagesCollection 装载完成后资产必然存在")
                .properties
                .clone()
        };

        let mut chain = Vec::new();
        if local_code != "en" {
            for category in MessageType::iter() {
                let path = variant_path(category.get_property_path(), &local_code);
                // 注册阶段已按磁盘存在性过滤，缺失的分类直接走英文基线段
                if collection.contains(&path) {
                    chain.push(bundle(&path));
                }
            }
        }
        for category in MessageType::iter() {
            chain.push(bundle(category.get_property_path()));
        }
        Self { chain }
    }
}

pub struct MessagesPlugin;

impl Plugin for MessagesPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<PropertiesAsset>()
            .init_asset_loader::<PropertiesAssetLoader>()
            // 动态资产注册必须早于 Loading 状态的 OnEnter（首帧 StateTransition）
            .add_systems(PreStartup, register_message_dynamic_assets)
            .configure_loading_state(
                LoadingStateConfig::new(AppState::Loading)
                    .load_collection::<MessagesCollection>()
                    .finally_init_resource::<Messages>(),
            );
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::assets::{definitions::parse_properties, languages};

    fn assets_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
    }

    /// java-properties 直接解析真实文件（不经 Bevy App）
    fn load_bundle(rel: &str) -> Arc<HashMap<String, String>> {
        let bytes = fs::read(assets_root().join(rel)).unwrap();
        Arc::new(parse_properties(&bytes).unwrap())
    }

    /// 真实 key 在 en 基线下取值
    #[test]
    fn real_key_resolves_in_english() {
        let chain = vec![load_bundle("messages/actors/actors.properties")];
        assert_eq!(resolve(&chain, "actors.mobs.rat.name"), "marsupial rat");
    }

    /// zh 链（变体在前）：真实 key 取中文值
    #[test]
    fn real_key_resolves_in_chinese() {
        let chain = vec![
            load_bundle("messages/actors/actors_zh.properties"),
            load_bundle("messages/actors/actors.properties"),
        ];
        assert_eq!(resolve(&chain, "actors.mobs.rat.name"), "啮齿小鼠");
    }

    /// 多 bundle 顺序：靠前的 bundle 优先，前段 miss 时向后回退
    #[test]
    fn chain_respects_bundle_order() {
        let first = Arc::new(HashMap::from([(
            "shared.key".to_owned(),
            "第一".to_owned(),
        )]));
        let second = Arc::new(HashMap::from([
            ("shared.key".to_owned(), "第二".to_owned()),
            ("only.second".to_owned(), "仅第二".to_owned()),
        ]));
        let chain = vec![first, second];
        assert_eq!(resolve(&chain, "shared.key"), "第一");
        assert_eq!(resolve(&chain, "only.second"), "仅第二");
    }

    /// miss 哨兵与 SPD `Messages.NO_TEXT_FOUND` 逐字一致
    #[test]
    fn miss_returns_sentinel() {
        let chain = vec![load_bundle("messages/actors/actors.properties")];
        assert_eq!(resolve(&chain, "no.such.key"), "!!!NO TEXT FOUND!!!");
        assert_eq!(resolve(&[], "anything"), NO_TEXT_FOUND);
    }

    /// key 大小写归一（SPD 查找前 toLowerCase）
    #[test]
    fn key_is_case_normalized() {
        let chain = vec![load_bundle("messages/actors/actors.properties")];
        assert_eq!(resolve(&chain, "Actors.Mobs.Rat.NAME"), "marsupial rat");
        assert_eq!(normalize_key("ACTORS.Mobs.rat"), "actors.mobs.rat");
        assert!(matches!(normalize_key("already.lower"), Cow::Borrowed(_)));
    }

    /// `{N}` 替换（libgdx 风格，计划 M1 指定）：乱序、重复、越界保留
    #[test]
    fn format_replaces_brace_placeholders() {
        assert_eq!(
            format_args("{0} hits {1}!", &["rat", "hero"]),
            "rat hits hero!"
        );
        assert_eq!(
            format_args("{1}被{0}击中，{1}倒下", &["鼠", "你"]),
            "你被鼠击中，你倒下"
        );
        assert_eq!(format_args("{2} {x} {0}", &["a"]), "{2} {x} a");
        assert_eq!(format_args("no placeholder", &["a"]), "no placeholder");
    }

    /// printf 子集替换（SPD 现版文件实际风格：%s/%d/%N$s/%%/%.2f）
    #[test]
    fn format_replaces_printf_placeholders() {
        assert_eq!(
            format_args("%s takes %d damage", &["rat", "3"]),
            "rat takes 3 damage"
        );
        assert_eq!(format_args("%2$s prefers %1$s", &["a", "b"]), "b prefers a");
        assert_eq!(format_args("100%% pure", &[]), "100% pure");
        assert_eq!(format_args("charge: %.2f", &["1.50"]), "charge: 1.50");
        // 参数不足：已匹配的照替，缺参占位符原样保留
        assert_eq!(format_args("%s and %s", &["only"]), "only and %s");
        // 位置参数不影响顺序参数的消费（Java printf 同义）
        assert_eq!(format_args("%s %1$s %s", &["a", "b"]), "a a b");
    }

    /// 真实 en 模板对拍参数替换
    #[test]
    fn format_works_on_real_template() {
        let chain = vec![load_bundle("messages/actors/actors.properties")];
        let template = resolve(&chain, "actors.buffs.adrenaline.desc");
        let formatted = format_args(&template, &["5"]);
        assert!(
            formatted.contains("Turns remaining: 5."),
            "实际输出: {formatted}"
        );
        assert!(!formatted.contains("%s"));
    }

    /// 变体路径拼接规则（en 无后缀即基线；zh-hant 这类带连字符的代码原样后缀）
    #[test]
    fn variant_path_appends_language_code() {
        assert_eq!(
            variant_path("messages/actors/actors.properties", "zh"),
            "messages/actors/actors_zh.properties"
        );
        assert_eq!(
            variant_path("messages/ui/ui.properties", "zh-hant"),
            "messages/ui/ui_zh-hant.properties"
        );
    }

    /// 变体注册的磁盘过滤：zh 九类齐全；未收录代码全缺失；en 无变体
    #[test]
    fn split_variant_paths_filters_by_disk() {
        let root = assets_root();

        let (existing, missing) = split_variant_paths(&root, "zh");
        assert_eq!(existing.len(), 9);
        assert!(missing.is_empty());

        let (existing, missing) = split_variant_paths(&root, "xx");
        assert!(existing.is_empty());
        assert_eq!(missing.len(), 9);

        let (existing, missing) = split_variant_paths(&root, "en");
        assert!(existing.is_empty() && missing.is_empty());
    }

    /// 集成：无窗口 App（MinimalPlugins + AssetPlugin）装载后构建 Messages 资源，
    /// zh 查找链生效；顺带验证 `LanguageServer` 的 code → 枚举映射
    #[test]
    fn messages_resource_builds_after_loading() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            AssetPlugin::default(),
        ))
        .init_state::<AppState>()
        .insert_resource(Settings {
            local_code: String::from("zh"),
            ..default()
        })
        .add_loading_state(LoadingState::new(AppState::Loading).continue_to_state(AppState::Title))
        // configure_loading_state 必须在 add_loading_state 之后
        .add_plugins((languages::LanguagePlugin, MessagesPlugin));

        let mut frames = 0;
        while !app.world().contains_resource::<Messages>() {
            app.update();
            frames += 1;
            assert!(frames < 5000, "装载超时：Messages 资源未构建");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let messages = app.world().resource::<Messages>();
        assert_eq!(messages.get("actors.mobs.rat.name"), "啮齿小鼠");
        assert_eq!(messages.get("no.such.key"), NO_TEXT_FOUND);

        let server = app.world().resource::<languages::LanguageServer>();
        assert_eq!(
            server.get_by_code("zh").expect("zh 已收录").language_type,
            languages::LanguageType::ChiSmpl
        );
    }
}
