# 26 · 渲染二期：贴图修复 + 拼接 + 迷雾（合并 23 号计划）

**文件所有权**：`src/render/**` + `src/render.rs`。23 号（迷雾）计划并入本域执行。
禁止改：`main.rs`、`Cargo.toml`、`src/actors/**`（22 号并行中，只消费 re-export 的
`Hero`/`GridPos`）、`src/levels/**`（24 号并行中，只消费 `Level` API）、其他共享文件。

## 用户报告的渲染错误（2026-08-14 截图，必须修复）

1. **房间内大块纯黑**：Chasm 地形只铺了基准格 24（图集里近乎全黑），缺
   `stitchChasmTile` 边缘拼接（上邻决定 CHASM_FLOOR/WALL/WATER 变体），
   视觉上像渲染漏洞而非深渊。
2. **墙面上的气泡状错位贴图**：水格只渲染了地形层的透明岸边拼接格
   （`WATER=32` 段），底下没有水面层——SPD 的水面是独立的全图滚动水层
   （`GameScene.java` L247-L266 `SkinnedBlock` + `Level.waterTex()` →
   下水道为 `environment/water0.png`），地形层水格的透明中心露出的应是水面
   而不是黑底。

## 必做目标

1. **水面层**：terrain tilemap 之下（z 更低）加水层——bevy_ecs_tilemap 用
   `water0.png` 切 16px 子格做第二张 tilemap（只铺水格及其邻接，或全图铺
   然后被不透明地形盖住——SPD 是全图铺，性能注释见 GameScene L259），
   滚动动画（L860 `waterOfs -= 5*elapsed`）可做可 TODO。
2. **水岸拼接**：`stitchWaterTile`（DungeonTileSheet.java L143-L170：上岸+1/
   右+2/下+4/左+8）。
3. **深渊拼接**：`chasmStitcheable` + `stitchChasmTile`（L81-L132）。
4. **随机变体**：`tileVariance` 数组（DungeonTileSheet L470-L480 附近，按层
   种子生成）+ `getVisualWithAlts`（L530 附近）——地板/草地的 alt 变体，
   消除大面积重复感。方差种子从 `Level` 取确定性来源（`RunSeed` 在 scenes 域，
   不可依赖；用 Level 的 entrance/exit/尺寸哈希或新增只读方法均可，笔记说明）。
5. **迷雾（23 号计划全部目标并入）**：`VisibilityMap` 资源（`cast_shadow`，
   视距 8 / Dark 2）+ tilemap 三态染色（可见原色/已探索加深/未知全黑）；
   对"场上无 Hero"安全。

## 可选加分（门禁绿灯前提下）

6. Raised 透视墙（`RaisedTerrainTilemap` + `DungeonWallsTilemap` 双层，
   `getTileVisual` 完整移植）——SPD 默认视觉。工作量大，允许留三期。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿（174 基线）。
- 新增测试：水岸/深渊拼接函数的邻接编码对拍（Java 行号 + 手算用例）；
  tileVariance 确定性；三态染色纯函数；FOV 重算（手工铺图：空场圆形对拍
  `rounding_table`、墙后不可见、visited 单调、换层重置、Dark 视距 2）。
- 运行冒烟无 panic/error；截图两类错误消失（水格有水面、深渊有边缘）。

## 进度

- [x] 水面层
- [x] 水岸/深渊拼接
- [x] tileVariance 变体
- [x] 迷雾（VisibilityMap + 染色）
- [x] 测试 + 笔记
- [ ] 可选：Raised 透视墙双层 → 留三期（常量已就位，见笔记）

## 实现笔记（2026-08-14）

### 水面层（tilemap.rs）

- `spawn_water_tilemap`：`water0.png`（32×32 = 2×2 个 16px 子格）全图平铺成
  第二张 tilemap，`WATER_LAYER_Z = -11` 压在地形层（-10）之下。子格索引
  `water_tile_index = (y mod 2)*2 + (x mod 2)`，以关卡左上角为原点，等价于
  `SkinnedBlock` 的世界锚定 UV 平铺（GameScene.java L247-L250）。
- 取舍：照抄 SPD 全图铺（L259 注释"水面无 alpha"、被不透明地形盖住），
  不做邻接裁剪——省一遍邻接扫描，换层重建逻辑与地形层完全同构。
- 地形层水格接 needsRender 语义（DungeonTerrainTilemap.java L114-L117）：
  纯水格（拼接结果 = WATER 基准格，即四邻皆水）设 `TileVisible(false)`
  完全露出水面层；岸边拼接格（WATER+1..15，中心透明边缘不透明）正常渲染。
- 滚动动画（GameScene L860 `waterOfs -= 5*elapsed`）留三期：
  bevy_ecs_tilemap 无逐层 UV 偏移，需自定义材质或滚动贴图本身。
- 贴图句柄经 `WaterTexture` 资源中转（模式照抄一期 `TerrainAtlas`），
  无资产测试注入 `Handle::default()`。

### 拼接（tile_sheet.rs）

- `stitch_water_tile`/`water_stitcheable`：DungeonTileSheet.java L142-L170。
  编码上岸+1/右+2/下+4/左+8；门族全算岸（L150），墙/深渊/水不算；
  REGION_DECO_ALT 仅 depth>20 算岸（L154-L158）。
- `stitch_chasm_tile`：L81-L132。上邻地形选 CHASM_FLOOR/FLOOR_SP/WALL/WATER，
  表外地形与 NULL（地图上边缘）落默认 CHASM（SparseArray 默认值，L131）；
  REGION_DECO_ALT 按深度特判（L124-L130）。上邻取法照抄
  DungeonTerrainTilemap.java L55 的严格大于（`pos > mapWidth`），保留
  (0,1) 差一 quirk（边界恒墙不可观察）。
- 主分派 `tile_visual_flat` = getTileVisual 的 flat=true 路径
  （DungeonTerrainTilemap.java L42-L106）：directVisuals 直查（带变体）→
  水拼接 → 深渊拼接 → directFlatVisuals（带变体）。越界邻居经
  `Level::terrain` 读为墙（SPD 关卡边界恒实心，语义一致且对手工小图安全）。

### tileVariance（tile_sheet.rs）

- `tile_variance(seed, size)`：SplitMix64 每格一个 [0,100) 字节
  （setupVariance，L474-L483）。Java 用 watabou Random；方差纯视觉、
  无需与 Java 逐值对拍，SplitMix64 零依赖、跨平台字节稳定。
- 种子 `variance_seed(level)`：Java 的 `Dungeon.seedCurDepth()`（GameScene
  L271）依赖 RunSeed（scenes 域，不可依赖），改混合 `Level` 确定性只读特征
  （depth/width/height/entrance/exit）。同一关卡重进画面稳定、换层即变；
  同特征两图共享方差序列，纯视觉可接受。
- `visual_with_alts`（L530-L537）：方差 ≥95 且有稀有变体 → 稀有
  （L519-L528）；≥50 且有普通变体 → 普通（L486-L516）；拼接格不吃变体。

### 迷雾（visibility.rs，23 号计划并入）

- `VisibilityMap` 资源：`visible`（= heroFOV）+ `visited`（只增，换层清零）。
  公开 API：`state_at(cell) -> CellVisibility`、`is_visible(cell)`（供后续
  怪物遮蔽标记）、`visible()/visited()` 只读切片。
- 重算 `recompute_visibility`：`Level` 变更边沿（覆盖式换层也触发）→
  重置+重算；英雄 `Changed<GridPos>`（含 Added）→ 重算。`cast_shadow` +
  视距 8（Level.java L157）/`Feeling::Dark` 2（任务书取 DARKNESS 挑战数值；
  Java 的 Dark feeling 实为 round(5*8/8)=5，挑战系统落地后按 SPD 语义校正）。
  visited 并入 = Dungeon.observe（Dungeon.java L931 `visited |= heroFOV` +
  L935-L938 紧邻 9 格恒记已探索）。
- 无 Hero / 坐标越界安全：全图不可见（Level.java L1342-L1344 盲分支语义），
  visited 保留——22 号域并行改英雄不会拉爆渲染。
- 染色：三态纯函数 `fog_color`（可见白 / 已探索 0.45 灰 / 未知黑）写
  `TileColor`，水面+地形两层同染。tile 出生时 spawn 侧烘焙初色（同帧
  Commands 未冲刷，`apply_fog` 看不到新 tile）；后续 `apply_fog` 增量刷新
  （仅 `VisibilityMap` 变更帧运行、仅颜色实际变化才写，避免无谓标脏 chunk）。
- 未知格用 TileColor 全黑而非 `TileVisible(false)`：TileVisible 已被
  needsRender 语义占用（纯水格），三态统一走颜色、语义单一。

### 换层重建（tilemap.rs / render.rs）

- bevy 0.19 覆盖式 `insert_resource` 只更新 changed tick、不重置 added
  tick；下楼路径直接覆盖插入 `Level`，按 `resource_added` 监听会漏换层 →
  全链用 `resource_exists_and_changed`，spawn 前防御性清 `LevelRenderRoot`，
  同帧 remove+insert 丢边沿也不残留双份。

### 三期遗留

- Raised 透视墙双层（RaisedTerrainTilemap + DungeonWallsTilemap）：
  图集常量已全部就位并有数值对拍测试（tile_sheet.rs 的 RAISED_*/WALLS_*/
  *_OVERHANG 段），`getRaisedWallTile`/`stitchInternalWallTile`/
  `stitchWallOverhangTile` 算法与第三层 tilemap 未接。
- 水面滚动动画（自定义材质或滚动贴图）。
- FogOfWar 半格平滑迷雾网格；MAPPED 态（魔法测绘卷轴，来源未实现）。
