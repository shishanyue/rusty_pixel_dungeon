# 00 · 现状审计报告

审计对象：`rusty_pixel_dungeon` @ master（约 1700 行 Rust，19 个源文件）。
结论：**项目当前完全无法构建**，且约半数源文件是无法编译的残片或死代码。
需要一次"地基级"修复（见 [03-foundation.md](03-foundation.md)），再按域重写。

## A. 构建阻断（P0）

| # | 问题 | 位置 | 说明 |
| --- | --- | --- | --- |
| A1 | 本地路径依赖不存在 | `Cargo.toml` → `solarborn = { path = "/home/shishanyue/Project/rust/solarborn" }` | 目录 `~/Project` 整个不存在，`cargo metadata` 直接失败。项目经由它间接使用 bevy_kira_audio / bevy_common_assets / thiserror 再导出 |
| A2 | 非法语法 | `src/levels/terrain.rs` | `pub enum Terrain { pub const CHASM: u8 = 0; … }`——enum 体内塞满 const 声明，纯语法错误；且 `Reflect` 未导入 |
| A3 | 引用不存在的类型 | `src/levels.rs` | 使用 `Terrain::WALL`（枚举变体），而 terrain.rs 实际是 u8 常量；`Level` 无 `Default` 却被 `init_resource` |
| A4 | 孤儿模块 | `src/levels.rs` 只声明 `pub mod terrain;` | `builder.rs`、`builder/`、`room.rs`、`room/`、`cave_level.rs` 均未挂进模块树（掩盖了它们自身的编译错误） |
| A5 | 引用不存在的模块 | `src/test.rs` | `assets::messages`、`assets::properties`、`levels::FellingType`（应为 `Feeling` 的笔误）、`levels::cave_level` 均不存在 |
| A6 | 死 proc-macro crate | `crates/macros` | 生成代码引用本项目根本没有的 `Room`/`Door`/`Point` 类型，注释还写着来源是别的项目（rust_dungeon_gen） |

## B. 若挂进模块树也无法编译的文件（P0，当前被 A4 掩盖）

- `levels/builder/loop_builder.rs`：
  - `tunnels = path_tunnels.choose(...)` 对不可变绑定赋值；
  - `self.place_room(&rooms, &prev, room, …)` 与 trait 里 `fn place_room(collision: &mut Query…)`（无 self、单参数）签名完全不符；
  - 引用未定义变量 `to`；`setup_rooms(&mut rooms)` 传的是 `&mut &mut Query`，而签名要 `&mut RoomHelper`。
- `levels/builder/regular_builder.rs`：导入不存在的 `room::RoomImpl`；`rooms_on_main_path -= …` 的 usize 减法可下溢 panic。
- `levels/builder.rs`：`find_neighbors` 的 split_at_mut 借用勉强成立，但 `angle_between_rooms` 用 `Vec2::angle_to`（返回弧度，SPD 语义是角度 0–360）——语义错误。
- `utils/room_helper.rs`：`Query::iter_mut()` 后 `into_inner()` 收集 `&mut RoomCore`——生命周期依赖 Query 借用，设计脆弱（Bevy 0.19 下同样不成立）。

## C. 逻辑缺陷（P1，编译通过也跑不对）

| # | 问题 | 位置 |
| --- | --- | --- |
| C1 | 加载状态链自环：`SpritesLoading => SpritesLoading`，永远到不了 `Loaded`，游戏卡死在加载 | `src/assets.rs` add_loading_states 最后一行 |
| C2 | `LanguageServer` 的 `setup` 系统从未注册，资源永不存在，任何 `Res<LanguageServer>` 都会 panic | `src/assets/languages.rs` |
| C3 | 消息资产路径无扩展名与语言后缀（`messages/actors/actors`），实际文件是 `actors.properties` / `actors_zh.properties`，动态资产注册后加载必失败 | `src/assets.rs` MessageType |
| C4 | 资产集合字段由宏生成为私有且无任何访问器，加载完也读不到 | `src/assets.rs` define_asset_collection! |
| C5 | 9 个串行加载状态（Messages→Languages→Effects→…）无必要且脆弱，bevy_asset_loader 支持单状态多集合 | `src/assets.rs` / `src/states.rs` |
| C6 | `Dungeon::new_level` 全是空 match 臂占位；`init(&Res<Settings>)` 应收 `&Settings` | `src/dungeon.rs` |
| C7 | 用 `AssetId::default()` 覆盖默认字体的 hack 在 0.19 字体系统（FontCollection）下不可靠 | `src/assets.rs` |
| C8 | `RoomCore` 用 f32 `Rect` 表达格子坐标，与 SPD 的整数房间语义不符，后续全链路都会被浮点误差污染 | `src/levels/room.rs` |
| C9 | `weight_rooms` 在遍历时向 `multi_connections` 重复 push（SPD 原意是加权抽样列表，但此实现与 `contains` 组合是 O(n²) 且顺序敏感） | `src/levels/builder/regular_builder.rs` |

## D. 死代码与卫生问题（P2）

- `src/global.rs`：空文件空模块。
- `src/test.rs`：临时试验插件，引用全错。
- `src/utils.rs`：`StaticPropertyPath`、`LevelPropertyPath` 无消费者。
- `src/assets/languages.rs`：`.iter().into_iter()` 冗余；`match_code(lang_type)` 名不副实（按类型不按 code）。
- `Cargo.toml`：`lazy_static`（Rust 1.80+ 应使用 `std::sync::LazyLock`）、`bevy_ecs_tilemap`（尚无任何使用点）。
- `.idea/`、`.vscode/` 入库（编辑器配置，建议 gitignore，保留不动、不扩散）。

## E. 资产盘点（好消息）

`assets/` 与 SPD v3.3.8 资产结构一致且完整：sprites / environment / interfaces /
splashes(+title) / music(ogg) / sounds(mp3) / fonts(pixel_font.ttf) /
messages（9 类 × 27 语言 .properties）/ languages/languages.json。
资产侧不需要任何补齐工作，重写只涉及代码。

## 结论

现存代码中值得保留的只有：资产枚举表（`assets.rs` 的 8 张 define_asset_type 清单，
数据正确）、`PropertiesAssetLoader`（API 已对 0.19 验证兼容）、`LimitedDropType` 枚举、
terrain 常量表的数值与 flags 映射（数据正确，载体错误）。其余按域计划重写。
