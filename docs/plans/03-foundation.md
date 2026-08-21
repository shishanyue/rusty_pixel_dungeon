# 03 · M0 地基计划（依赖重建 + 编译恢复）

执行者：协调者本人（串行，因为全是共享文件）。完成标准：`cargo check` 与
`cargo clippy` 零错误，`cargo test` 通过，`cargo run` 能开窗并停在 Title 状态。

## 1. 依赖重建（Cargo.toml）

- 删除：`solarborn`（不存在）、`macros`（死代码）、`lazy_static`（用 LazyLock/const 替代）、
  `bevy_ecs_tilemap`（M3 再加，避免未使用依赖）、`bevy_asset_loader 0.25`。
- 升级/新增：
  - `bevy = "0.19"`，特性：默认 + `jpeg`（splash 是 .jpg）+ `mp3`（音效是 .mp3；ogg 走默认 vorbis）
  - `bevy_asset_loader = "0.27"`（配 0.19）
  - `serde_json`（自写 JSON 资产加载器用）
  - 保留：`thiserror`、`anyhow`、`serde`、`bitflags`、`rand 0.10`（+`small_rng`）、
    `java-properties`、`num_enum`、`strum`
- 删除 `[workspace]`（crates/ 目录移除后无成员）；重新生成 `Cargo.lock`。

## 2. 状态机（states.rs）

`LoadingAssetStates`（9 态串行）→ `AppState { #[default] Loading, Title, InGame }`。
动态资产注册放 `PreStartup`，早于首帧 `StateTransition` 进入 Loading，时序安全。

## 3. 资产层（assets/）

- 单一 LoadingState 装载全部 9 个集合，`continue_to_state(Title)`。
- `define_asset_collection!` 宏生成 **公有** 取用方法：
  `fn get(&self, key: impl PropertyPath) -> Handle<T>`（按枚举路径查 HashMap）。
- 修复 C1 自环、C3 消息路径（补 `.properties` 扩展名，M0 只注册英文基线文件，
  多语言由 12 号计划接管）、C7 字体 hack（改为 `FontAssets` 集合显式加载）。
- `definitions.rs` 的 `PropertiesAssetLoader` 保留（0.19 API 已验证兼容），
  `thiserror` 改直连。
- languages.json 用自写 `LanguagesAssetLoader`（serde_json）加载；
  `LanguageServer` 改为 12 号计划的职责范围，M0 只保证类型编译。

## 4. 地形与关卡核心（levels/）

- `terrain.rs` 重写：`#[repr(u8)] enum Terrain`（num_enum `IntoPrimitive/TryFromPrimitive`），
  flags 用 `const fn flags(self)` + bitflags const `union`，保留审计确认过的数值映射；
  `discover()` 移到 `impl Terrain`。附单测（数值往返、flags 抽查、discover）。
- `levels.rs` 重写：`Level` 结构（map/宽高/entrance/exit/feeling/flag 缓存），
  坐标统一 `bevy::math::IRect`（max 开区间）；`fill`/`set_terrain`/`terrain` API；
  删除自制 IRect。附单测（fill 边界、越界防护、flags 缓存一致性）。
- 删除孤儿残片：`builder.rs`、`builder/`、`room.rs`、`room/`、`cave_level.rs`、
  `utils/room_helper.rs`（由 10 号计划按 SPD 语义重写，f32 Rect 的 C8 缺陷不再继承）。

## 5. Dungeon（dungeon.rs）

- `level_kind(depth, branch) -> LevelKind` 纯函数落地（SPD `Dungeon.newLevel()`
  的深度→关卡类型映射表化），替换空 match 占位；附单测。
- `LimitedDrops` 保留，`count()` 的裸 `unwrap` 改为 `unwrap_or(0)` 语义。
- `init(&Settings)` 修签名。

## 6. 清理

删除 `crates/`、`src/test.rs`、`src/global.rs`、`utils.rs` 中无消费者的 trait。
`main.rs` 预挂四个域模块占位（`actors`/`scenes` 空插件），M1 智能体只填目录内文件，
不碰共享文件。

## 进度

- [x] 计划定稿
- [x] 依赖重建
- [x] 状态机 + 资产层
- [x] terrain/levels 重写 + 单测
- [x] dungeon 表化 + 单测
- [x] 清理 + main.rs 域占位
- [x] check/clippy/test 全绿（2026-08-13：clippy 零告警，12 测试通过，
      运行冒烟成功进入 Title、全部资产装载成功）

## 执行中发现并修复的额外问题

1. `rand 0.10` 移除了 `small_rng` 特性 → 改用内置 `chacha` 特性（种子确定性更好）。
2. `configure_loading_state` 必须在 `add_loading_state` **之后**调用，否则运行时
   panic（LanguagePlugin 注册顺序，冒烟测试抓到）。
3. 资产实际文件是 `fireball-short.png`/`fireball-tall.png`（SPD 运行时拼接后缀），
   枚举原来登记的 `fireball.png` 不存在 → 拆成两个枚举项，并新增
   `all_registered_asset_paths_exist` 守护单测（403 个路径全量校验）。
