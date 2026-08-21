# 19 · 关卡生成二期计划（水/草/氛围/第二种构建器）

**文件所有权**：`src/levels/**` 与 `src/levels.rs`（一期已交付，所有权移交本域）。
禁止改 `main.rs`/`Cargo.toml`/其他域目录。16/17 号域并行中，它们只读消费
`Level` 公共 API——**不许破坏 API 签名**（`generate_level`、`map()/terrain()/
passable/entrance/exit/debug_ascii`），新增可以。

**参考实现**（core/.../levels/）：
- `painters/RegularPainter.java`：`paintWater`/`paintGrass`/`paintTraps` 与
  Feeling 修正；`Patch.java`：细胞自动机噪声填充（水草的形状来源）
- `builders/FigureEightBuilder.java`：与 LoopBuilder 二选一的主构建器
  （`RegularLevel.builder()` 的 50/50 掷点，一期跳过了那次 Int(2)——恢复它）
- `RegularLevel.java`：`nTraps()`（陷阱数量表）、水草填充率参数
- `SewerLevel.java`：下水道的 `waterFill/grassFill` 参数
- `Level.java` `create()`：Feeling 掷点表（WATER/GRASS/DARK/…概率）

## 目标

1. `Patch` 噪声移植（对拍 Java：同种子同形状，至少统计学断言 + 一个钉死用例）。
2. `paint_water`/`paint_grass`：按 SewerLevel 参数与 Feeling 修正填充
   `Terrain::Water/Grass/HighGrass`（RegularPainter 语义，含 Feeling.Water/
   Grass 的填充率加成）。
3. Feeling 掷点进 `generate_level`（对照 Level.create 的概率表；`Level.feeling`
   已有字段）。
4. `FigureEightBuilder` 移植 + 恢复 `builder()` 的 50/50 掷点。
5. 陷阱地形铺设：`nTraps()` 数量表 + `Terrain::SecretTrap/Trap` 落位
   （只铺地形，陷阱行为是后续域）。
6. 更新既有确定性/连通性测试（RNG 消耗变化会改变同种子出图，属预期；
   连通性百种子测试必须依旧全绿——水草不挡路，`HighGrass` 是 passable）。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿。
- 新增测试：Patch 对拍；水/草出现率在参数邻域内（统计断言，固定种子）；
  Feeling 掷点分布；FigureEight 与 Loop 两分支同种子确定性；
  百种子连通性含新地形仍绿。
- `debug_ascii` 扩展水（~）草（"）等符号。

## 进度

- [x] Patch 噪声（`levels/patch.rs`：钉死用例 + 填充率/聚簇统计断言）
- [x] 水/草刻画 + Feeling（`paint_water`/`paint_grass` + `roll_feeling` 掷点表）
- [x] FigureEightBuilder（`builder.rs` 共用件重构 + 50/50 掷点恢复）
- [x] 陷阱地形（`nTraps()` + `paint_traps` 落位，TRAPS 5 倍加铺）
- [x] 测试更新（levels:: 域 44 项全绿；全量 174 项全绿；check/clippy 本域零告警）

## 实现笔记（二期）

### RNG 消耗顺序（每层一条流，`seed_for_depth` 派生）

1. **Feeling 掷点**（`Level.create` L255-L292）：depth>1 时 `Int(14)`，
   0-6 → CHASM/WATER/GRASS/DARK/LARGE/TRAPS/SECRETS，其余 None。
   L222-L253 的限量物资掷点属物品域，未移植、不消耗随机数。
2. **builder 选择**（`RegularLevel.builder` L176-L189）：`Int(2)` 五五开。
   Loop 分支消耗两次 Float（intensity ∈ [0,0.65)、offset ∈ [0,0.5)）；
   FigureEight 分支只消耗一次 Float（intensity ∈ [0.3,0.8)，offset 字面量 0）。
   一期恒用 LoopBuilder 且跳过该 Int(2)，本期恢复 → 同种子出图全变，
   钉死测试已按新图重钉。
3. **initRooms**（一次性，重试不重掷）：标准房数 `4 + chances({1,3,1})`；
   LARGE 氛围恒 6 再 ×1.5 向上取整 = 9（`RegularLevel.initRooms` L129-L133 +
   `SewerLevel.standardRooms` L87-L92）。随后洗牌一次。
4. **builder.build**（可重试）：隧穿数/分支角/额外连接等，见 10 号计划一期笔记；
   FigureEight 与 Loop 共享 `RegularParams`/`LoopShape`/`create_branches` 等共用件。
5. **painter.paint**：`nTraps = NormalIntRange(2, 3 + depth/5)` 在洗绘制顺序**之前**
   掷（对应 Java 在 `SewerLevel.painter()` 构造期传参）→ 洗牌 → 逐房
   `place_doors`/`paint_room` → `paint_doors`。
6. **装饰子流**（`RegularPainter.paint` L135-L153 `pushGenerator(Random.Long())`）：
   主流掷一个 u64 作种子开新流，水/草/陷阱/贴花全部在子流消耗——三期在装饰后
   加内容不会再漂移主流。

### 与 Java 的差异（二期新增部分）

- **Patch**（`levels/patch.rs` vs `Patch.java`）：算法逐行对照（含 `forceFillRate`
  的精确洗牌填充与 5 轮细胞自动机），但 RNG 引擎不同（ChaCha12 vs Java Random），
  同种子形状与 Java 不对拍，以自钉用例 + 统计断言（填充率精确性、聚簇性）验收。
- **水/草落位**：`Room.waterPlaceablePoints`/`grassPlaceablePoints` 对本工程
  四类房间（入口/出口/标准/连接）恒为全足迹（`canPlaceWater/Grass` 无覆写），
  故直接以房间矩形与 Patch 求交；只替换 `EMPTY`。高草规则照抄
  `RegularPainter.paintGrass` L397-L422：邻草计数决定 HighGrass 概率。
- **陷阱**：只铺 `Terrain::Trap/SecretTrap` 地形，行为域未开工。陷阱"类型"仅用于
  `avoidsHallways` 落位属性（种类权重表照抄 `SewerLevel.trapClasses/trapChances`，
  depth 1 与 2+ 两套）。`TrapMechanism.revealHiddenTrapChance()` 依赖饰品系统，
  未移植 → revealInc 恒 0，可见陷阱只来自 TRAPS 氛围 5 倍加铺中超出 nTraps 的部分。
- **Feeling 未移植的分支内效果**：DARK 的视距压缩（英雄/渲染域）、LARGE 的补给
  加成（物品域）、default 分支的两次饰品覆写 Float（无饰品系统，不消耗随机数）。
- **SECRETS 氛围**：藏门率 depth 曲线值抬到 0.3（`RegularPainter.paintDoors`），
  但"藏门若断图则回退为普通门"的严判会压低实际密门数——统计上仍显著更多
  （测试按相对倍数断言）。
- **连通性口径**：`Terrain::Trap` 是 AVOID（寻路回避但英雄可踩），百种子连通性
  测试的可走谓词 = `passable || Trap`。
- **debug_ascii 符号集**：水 `~`、草/犁草 `"`、高草 `!`、明陷阱 `^`、暗陷阱 `,`、
  深渊 ` `（空格）；一期符号（`#.+X​E`）不变。

### 三期仍缺

- 特殊房间（`RegularLevel.initRooms` L146-L156：SpecialRoom 配比与其 chances 掷点）、
  商店房（L143-L144）、密室（L158-L163）。
- 陷阱行为域（触发/效果）；水草的战斗语义（灭火/隐身等）归各自域。
- 标准房间的尺寸类别掷点（`StandardRoom.setSizeCat`）与层配比表
  （一期简化为恒 EmptyRoom，见 10 号计划笔记）。
- 饰品系统相关：`revealHiddenTrapChance`、Feeling default 分支的两次 Float。
