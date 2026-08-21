# 16 · 图集渲染域计划（M3 第一阶段）

**文件所有权**：新建 `src/render.rs` + `src/render/` 目录（模块声明与插件注册由
协调者合流时接入 `main.rs`，你自己不碰 `main.rs`）。
禁止改 `Cargo.toml`（`bevy_ecs_tilemap = "0.19"` 已由协调者加好）、其他域目录。
**不碰相机**（相机跟随归 17 号英雄域）。

**参考实现**：
- SPD：`core/.../tiles/DungeonTileSheet.java`（地形 → 图集索引的完整映射，含
  墙体拼接/水岸拼接规则）、`tiles/DungeonTerrainTilemap.java`（查表入口）
- 图集：`assets/environment/tiles_sewers.png`（16×16 网格）
- 本地 bevy_ecs_tilemap 0.19 源码与示例：`~/GitHub/bevy_ecs_tilemap/`
- 技能参考：`~/.claude/skills/bevy_ecs_tilemap/SKILL.md`

## 目标

给定 `Level` 资源，渲染一张真实图集的地形 tilemap（下水道 tileset），
替代当前 `scenes/in_game.rs` 的彩色方块调试视图（替换动作由协调者做，你提供能力）：

1. `RenderLevelPlugin`：监听 `Level` 资源插入/移除（`resource_added` 等运行条件），
   生成/销毁 tilemap 实体（挂 `DespawnOnExit(AppState::InGame)` 双保险）。
2. 地形 → 图集索引：移植 `DungeonTileSheet` 的映射。M3 第一阶段允许**平面映射**
   （每种地形固定一个索引：地板/墙/门/入口/出口/水/草…），墙体拼接
   （raised 透视墙）与水岸 stitching 作为第二阶段 TODO 记录在笔记里，但索引常量表
   必须现在就照抄 Java（含拼接变体的常量，注明行号）。
3. 坐标约定必须与调试视图一致：格 (x,y) → 世界
   `((x-(w-1)/2)*16, ((h-1)/2-y)*16)`（y 翻转、地图中心在原点、格边长 16）。
4. 视觉栅格对齐：`ImagePlugin::default_nearest` 已全局开启，无需处理。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿（不破坏
  既有 107 个）；新增测试：地形→索引映射表抽查（对照 Java 行号）、tilemap 实体
  随 `Level` 资源插入/移除的集成测试（MinimalPlugins 可行则做，不可行在笔记说明
  并给出替代验证）。
- 提供一个 `#[cfg(test)]` 之外可调用的公共入口（插件 + 必要组件），协调者能一行接入。

## 进度

- [x] 图集索引常量表（对照 DungeonTileSheet）
- [x] RenderLevelPlugin（生成/销毁）
- [x] 测试 + 笔记

## 实现笔记（M3 第一阶段交付）

### 文件

- `src/render.rs`：`RenderLevelPlugin` 入口 + `cache_terrain_atlas` 系统 + 集成测试。
- `src/render/tile_sheet.rs`：`DungeonTileSheet.java` 常量表全量照抄 +
  `flat_tile_index`（平面映射，覆盖全部 39 种 Terrain）。
- `src/render/tilemap.rs`：坐标换算、`TerrainAtlas`/`TerrainTilemap`、
  `spawn_terrain_tilemap` 构建函数与 spawn/despawn 系统。

### main.rs 接入（协调者授权的唯一豁免，已执行）

```rust
pub mod render;                  // 模块声明块，levels 之后
render::RenderLevelPlugin,       // 第二个 add_plugins 元组内（DefaultPlugins 之后）
```

注意：`RenderLevelPlugin` 必须晚于 `DefaultPlugins` 注册——它以
`is_plugin_added::<RenderPlugin>()` 判断是否挂 `TilemapPlugin`
（无渲染的 MinimalPlugins 测试环境挂了会 panic：`TilemapPlugin::build`
在 render feature 且未开 atlas 时直接取 `RenderApp` 子应用）。

### 索引常量表来源（`DungeonTileSheet.java`，v3.3.8）

- `xy(x,y) = (x-1) + 16*(y-1)`：L34-L39；图集 256×256 = 16×16 格。
- Floor 段 L50-L71（GROUND=0）；Chasm 段 L73-L78（CHASM=24，含 4 个上邻拼接格）；
  Water 段 L139（WATER=32，后 15 格为四邻岸位掩码 +1/+2/+4/+8，L162-L170）；
  Flat 段 L181-L216（FLAT_WALLS=48、FLAT_OTHER=64）；
  Raised 下层 L222-L316（RAISED_WALLS=80、RAISED_DOORS=112、RAISED_OTHER=120）；
  Raised 上层 L324-L408（WALLS_INTERNAL=144、WALLS_OVERHANG=192、
  DOOR_OVERHANG=224、OTHER_OVERHANG=232）。
- 平面映射 = `directVisuals`（L415-L438）∪ `directFlatVisuals`（L441-L465）
  ∪ {Water→WATER, Chasm→CHASM 基准格}，恰好覆盖全部 39 种 Terrain，无遗漏。
- Java 里矿区滚石/水晶共用格位（L211-L216、L311-L316、L397-L402 常量同值），照抄。

### 锚点/坐标换算（有测试对拍）

- 关卡格 y 向下（行 0 在上），`TilePos` y 向上 → `tile_pos_for_cell` 翻转 `ty = h-1-y`。
- 方形地图整图 AABB 为 `min=(-8,-8)`、`max=((w-1)*16+8, (h-1)*16+8)`
  （bevy_ecs_tilemap `chunk_aabb`），`TilemapAnchor::Center` 平移 `-(max+min)/2 =
  (-(w-1)*8, -(h-1)*8)`（`src/anchor.rs` L68）→ `TilePos(tx,ty)` 中心 =
  `((tx-(w-1)/2)*16, (ty-(h-1)/2)*16)`；代入 `ty=h-1-y` 即调试视图公式
  `((x-(w-1)/2)*16, ((h-1)/2-y)*16)`。因此 tilemap `Transform` 只抬 z（-10，
  `TERRAIN_LAYER_Z`），x/y 留原点。等价性由
  `render::tilemap::tests::anchor_center_matches_debug_view_formula`
  调 `center_in_world` 逐点对拍（奇/偶尺寸；所有值是 8 的整数倍，f32 精确）。

### 结构与语义要点

- tile 实体挂为 tilemap 根实体子级（`ChildOf`）：despawn 根实体级联清 tile，
  `DespawnOnExit(AppState::InGame)` 只需标根实体（双保险）。
- 图集句柄经 `TerrainAtlas` 资源中转（`cache_terrain_atlas` 从
  `EnvironmentCollection` 取 `TilesSewers` 一次性缓存，run_if
  `resource_exists::<EnvironmentCollection>`——与项目既有测试模式一致，
  无资产环境整条链静默跳过）。这层中转让集成测试能注入
  `TerrainAtlas(Handle::default())` 走完整插件路径（bevy 0.19 的
  `Handle::default()` 为合法弱句柄），不必构造私有字段的资产集合。
- bevy 0.19 `insert_resource` 原地覆盖**不会**重置 added tick：换层必须
  remove + insert（`in_game.rs` teardown/setup 即此流程）。spawn 系统内
  仍先防御性清旧实体，同帧 remove+insert 丢失 `resource_removed` 边沿
  也不会残留双份（有测试覆盖）。
- MinimalPlugins 集成测试可直接 spawn `TilemapBundle`（组件纯数据；
  `SyncToRenderWorld` 无 hook、默认材质句柄由渲染插件在真实环境注册），
  无需降级为"只测纯函数"。

### 第二阶段 TODO（拼接/变体，常量已就位）

1. 水岸 stitching：`stitchWaterTile`（L162-L170）+ `waterStitcheable`
   （L143-L160，REGION_DECO_ALT 按深度特判）。
2. 深渊上边缘：`stitchChasmTile`（L123-L132）+ `chasmStitcheable`（L81-L121，
   REGION_DECO_ALT 按深度特判）。
3. raised 透视墙（非 flat 模式）：`getRaisedWallTile`（L250-L265）、
   `getRaisedDoorTile`（L276-L284）、`stitchInternalWallTile`（L329-L342）、
   `stitchWallOverhangTile`（L355-L371）——需引入上/下双层 tilemap
   （参照 `DungeonWallsTilemap.java`）与 `wallStitcheable`/`doorTile` 判定。
4. 随机变体：`setupVariance`（L474-L483，按 `Dungeon.seedCurDepth` 播种）+
   `getVisualWithAlts`（L530-L537，common 50%/rare 5%），需接 `Dungeon` 的
   确定性 RNG 纪律。
5. 查表入口整体语义对照 `DungeonTerrainTilemap.getTileVisual`
   （`DungeonTerrainTilemap.java` L42-L106）。
6. 分区域图集（prison/caves/…）：按深度换 `TerrainAtlas`（改写
   `cache_terrain_atlas` 即可）；水面动画贴图（`water0.png`）是独立图层。
