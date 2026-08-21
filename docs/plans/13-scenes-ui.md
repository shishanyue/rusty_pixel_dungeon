# 13 · 场景与 UI 框架域计划

**文件所有权**：`src/scenes/`（模块入口 `src/scenes.rs` 由地基预挂，可改其内容）。
禁止改 `main.rs`/`Cargo.toml`/其他域目录。

**参考实现**：`shattered-pixel-dungeon/core/.../scenes/TitleScene.java`、
`ui/Archs.java`、SPD 标题的四层视差星空（`splashes/title/{archs,back_clusters,
mid_mixed,front_small}.png`）+ `interfaces/banners.png` 的游戏 Logo 区块。

## 目标（M1 范围）

1. **场景骨架**：`ScenePlugin` 模式——每个场景一个插件：
   `OnEnter(state)` 生成、实体挂 `DespawnOnExit(state)`（0.19 提供的状态作用域实体，
   查 `~/GitHub/bevy/examples/state/states.rs` 确认当前推荐写法）。
2. **TitleScene**：
   - `Camera2d`；背景四层图（先静态摆放，视差滚动做 TODO）；
   - banners.png 里的标题区块（用 `Sprite::from_atlas_image`/`TextureAtlasLayout`
     裁剪，SPD `BannerSprites.get(Type.PIXEL_DUNGEON)` 的源矩形照抄）；
   - 按钮列：`进入地牢`（→ `AppState::InGame` 占位）、`退出`；
     0.19 UI：`Node` + `px()/percent()` 辅助函数 + `On<Pointer<Click>>` 观察者，
     字体用地基 `FontAssets` 的 pixel_font。
3. **InGame 占位场景**：黑底 + 一行文字（后续 M2 接关卡渲染），验证状态切换闭环。
4. 资产取用走地基的集合 `get(枚举)` API（`SplashType::TitleArchs` 等），
   不要手写路径字符串。

## 约束

- 文本先硬编码英文占位并集中放常量表，M2 接 12 号域的 `Messages`（勿自造 i18n）。
- 不写音乐播放（M7），不碰 `bevy_ecs_tilemap`（M3）。

## 验收

- `cargo run`：Loading → Title 显示四层背景 + Logo + 可点按钮；点击进入 InGame
  占位场景；`Esc` 从 InGame 返回 Title。
- 无 panic、无 0.19 弃用告警；`cargo clippy` 无新告警。

## 进度

- [x] 场景骨架（插件模式 + 状态作用域实体）
- [x] TitleScene 背景/Logo/按钮
- [x] InGame 占位 + 状态闭环

## 实现笔记（M1 交付）

- **模块结构**：`scenes.rs`（`ScenesPlugin`：全局 `Camera2d` Startup 生成一次 +
  `ClearColor` 黑底 + 集成测试）、`scenes/title.rs`、`scenes/in_game.rs`、
  `scenes/text.rs`（英文文案常量表，M2 换 `Messages`）。
- **状态作用域 API**：选用 0.19 的 `DespawnOnExit(状态)` 组件（`bevy_state::state_scoped`，
  `init_state` 时自动注册清理系统，无需在 `AppState` 派生上加属性）；
  不写 `OnExit` 手动清理。备选的 `DespawnOnEnter`/`DisableOnExit` 未用。
- **横幅源矩形**：`banners.png`（512x256）按 `BannerSprites.java` 裁剪，
  桌面横屏取 `TITLE_LAND = uvRect(0, 100, 240, 157)`（240x57，2 倍显示 480x114），
  用 `ImageNode.rect` 一次性裁剪（未建 `TextureAtlasLayout`，单帧无必要）；
  竖版 `TITLE_PORT = (0,0,139,100)`，发光层 `TITLE_GLOW_*` 需加法混合，M1 未做。
- **四层背景帧网格**（SPD `TextureFilm` 行主序，`Sprite.rect` 裁剪）：
  `archs.png` 1024x256 帧 333x100（3 列 6 帧）、`back_clusters.png` 512x512 帧
  450x250（1 列 2 帧）、`mid_mixed.png` 2048x1024 帧 273x242（7 列有效 24 帧）、
  `front_small.png` 1024x512 帧 112x116（9 列有效 20 帧）。
  缩放基准 = 窗口高/450；拱门层整屏平铺（行距 95、横向搭接 9），浮动三层用
  确定性的静态摆放常量表（替代 SPD 运行期随机），亮度经 `Sprite.color` 乘算
  （0.55/0.85/1.0）。TODO(M2)：视差滚动（SPD `SCROLL_SPEED=15`，逐层 1.33 倍递增）
  与底部渐暗遮罩。
- **可测试性**：资产装饰系统挂 `run_if(resource_exists::<集合>)`——生产环境
  Loading 完成后恒为真，仅为 `MinimalPlugins` 无资产集成测试留跳过路径；
  场景根实体（`TitleUiRoot`/`InGameRoot`）无资产依赖，测试据此断言
  Loading→Title→InGame→Title 闭环的生成与清理、相机全局唯一、Esc 返回。
