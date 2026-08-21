# 10 · 关卡生成域计划

**文件所有权**：`src/levels/`（`terrain.rs`、`levels.rs` 的既有接口只消费不改；
新增文件全归本域）。禁止改 `main.rs`/`Cargo.toml`/其他域目录。

**参考实现**：`shattered-pixel-dungeon/core/.../levels/`
（`rooms/Room.java`、`builders/{Builder,RegularBuilder,LoopBuilder}.java`、
`painters/{Painter,RegularPainter,SewerPainter}.java`、`RegularLevel.java`）。

## 目标（M1 范围）

种子可复现地生成一张**下水道普通层**（SewerLevel, depth 1–4 语义）：
房间集合 → LoopBuilder 摆放 → 连接 → RegularPainter 铺地形 → 产出 `Level`，
入口/出口正确、全图连通。渲染不在本域。

## 设计要点

1. **生成期纯数据**：`Room { rect: IRect, kind: RoomKind, neighbours: Vec<usize>,
   connected: HashMap<usize, Door> }`，用索引不用 Entity——生成是纯函数
   `generate(seed, depth) -> Level`，可脱离 Bevy 单测。
2. **整数矩形语义**：SPD `Rect` 的 right/bottom 是**闭区间墙位**（房间含共享墙）。
   移植时统一换算成 `bevy::math::IRect`（max 开区间），在 `Room::intersect` 与
   邻居判定（重叠段 ≥3 格才可开门，对应 Java `i.width() >= 3`——注意换算差 1）处
   写换算注释 + 对拍单测。
3. **RNG**：rand 0.10 已启用 `chacha` 特性，用 ChaCha 系 RNG `seed_from_u64(seed)`
   贯穿传引用（跨平台/跨版本种子稳定）；SPD `Random.chances/NormalIntRange/IntRange`
   语义先在 `levels/random.rs` 内做等价工具函数。
4. **移植顺序**：Room（含 connect/door 逻辑）→ Builder 基类工具
   （`place_room`/`find_free_space`，SPD Builder.java 静态方法）→ RegularBuilder
   （主路径+分支）→ LoopBuilder → Painter（`paint/fill/drawLine/mergeRooms`）→
   RegularPainter（门刻画/水草刻画简化版）→ `StandardRoom` 尺寸表 +
   `EntranceRoom/ExitRoom/TunnelRoom(EmptyRoom)`。
5. **SPD 角度约定**：Builder 用 0–360 度、12 点钟方向为 0、顺时针增长
   （`Builder.placeRoom` 注释）；`angle_between_rooms` 不能用 `Vec2::angle_to`
   的弧度语义（审计 B 节的既有 bug，勿复制）。

## 验收

- `cargo test -p` 本域测试全绿，至少覆盖：
  - 同种子两次生成逐格相同；
  - 100 个随机种子：入口/出口存在、BFS 全图 passable 连通、无 0 尺寸房间；
  - Rect 换算对拍（构造 SPD 已知用例手算结果）。
- 提供 `Level::debug_ascii() -> String`（调试/测试用字符画）。
- `cargo clippy` 无新告警。

## 进度

- [x] random.rs 工具
- [x] Room/Door
- [x] Builder 工具函数
- [x] RegularBuilder + LoopBuilder
- [x] Painter + SewerPainter(简化)
- [x] generate() 竖切 + 测试

## 实现笔记（M1 完成后追记）

模块布局：`rect.rs`（SPD Rect）→ `random.rs`（Random.java 语义）→ `rooms.rs`
（Room/Door/RoomKind）→ `builder.rs`（Builder 工具 + LoopBuilder）→ `painter.rs`
（RegularPainter 简化 + Sewer decorate）→ `generator.rs`（`generate_level`，
经 `levels.rs` re-export）。全部纯函数、不依赖 Bevy `World`。

### 关键换算

- **SPD Rect 闭区间**：`right`/`bottom` 是闭区间墙位，基类 `width() = right-left`
  但 `Room` 覆写为 `+1`。移植保留 `SpdRect`（字段名与 Java 一致）逐行对照，
  写入 `Level`（`IRect` max 开区间）时经 `SpdRect::to_irect()`（right/bottom +1）。
  设计要点 2 中"重叠段 ≥3 格对应 Java `i.width() >= 3`"实为 `>= 2`
  （Room.java L262：SPD 单位宽 2 = 3 格含端点），对拍单测按 Java 源码为准。
- **Java `Math.round`**：`floor(x+0.5)`，负半数与 Rust `round()` 不同
  （Java -2.5→-2，Rust -2.5→-3），实现为 `java_round_f32/f64`（builder.rs）。
- **`GameMath.gate`**：`min > max` 不 panic（返回 min），不能用 `clamp`。
- **角度约定**：0–360 度、12 点钟为 0、顺时针（y 向下）；`angle_between_points`
  返回域 (-180, 180]，按 Java 的 float 斜率除法（垂直得 ±Inf → atan = ±π/2）。
- **层种子**：`seed_for_depth` 对照 `Dungeon.seedForDepth`——以世界种子起流、
  跳过 depth 个 u64、取下一个（M1 恒 branch=0）。

### 确定性

统一 `ChaCha12Rng::seed_from_u64`，显式 `&mut impl Rng` 贯穿传引用。
**只保证本工程内种子确定性，不与 Java `java.util.Random` 位流对齐**（RNG 算法
不同，逐位对齐不可能）；因此凡 Java 有"空耗随机数"处（如 `getDoorCenter` 对
整数和恒假的 `Float()`、`builder()` 的 `Int(2)` 分支），在不影响本工程内部
一致性的前提下省略，且逐处注释。装饰刻画对照 Java `pushGenerator(Random.Long())`
用主流派生的独立子流（painter.rs），保证装饰不影响主生成流。

### 与 SPD 的差异清单（M1 简化，均在代码注释标注 Java 行号）

1. **Builder 只移植 LoopBuilder**：`RegularLevel.builder()` 原为 50% LoopBuilder /
   50% FigureEightBuilder（RegularLevel.java L176-L189）。
2. **标准房恒 `EmptyRoom`、尺寸类别恒 NORMAL**：跳过 `StandardRoom.createRoom()`
   层配比表与 `setSizeCat` 抽取（RegularLevel.java L131-L141）；
   `connectionWeight()` 恒 1（结构保留）。
3. **连接房恒 `TunnelRoom`**：`ConnectionRoom.createRoom()` 的层权重表
   （ConnectionRoom.java L60-L83，下水道 {20,1,0,2,2,1} 本就以 Tunnel 为主）。
4. **房间数**：入口 + 出口 + `4+chances({1,3,1})` 个标准房（SewerLevel.java
   L87-L92 原值）；无 Shop/Special/Secret 房（RegularLevel.java L143-L163）。
5. **Painter 未移植**：`paintWater`（SewerLevel 0.30/5）、`paintGrass`（0.20/4）、
   `paintTraps`、`mergeRooms`（相邻标准房恒以门分隔）；`SewerPainter.decorate`
   已移植，其水沿墙装饰因无水暂不触发（结构保留，M2 补 Patch 元胞自动机即激活）。
6. **入口/出口房**：不生成 `LevelTransition`/引导页（M1 无对应系统），
   仅落 `Terrain::Entrance/Exit` 并回写 `Level.entrance/exit`。
7. **隐藏门**：深度几率（depth/20 封顶）与"藏门不得断图"回退已移植；
   SECRETS feeling 分支与教程门（RegularPainter.java L193-L196、L225-L250、
   L263-L270）未移植。
8. **防御性上限**：`LoopBuilder.build` 闭环塞隧道加 64 次上限、`generate_level`
   整体重试加 512 次上限（Java 均为无限循环，靠运气收敛；上限内行为一致，
   超限 panic 暴露而非死循环）。

### 验收状态

`cargo check --all-targets`、`cargo clippy --all-targets` 零错误零告警；
`cargo test` 72 通过（含验收：同种子逐格一致、120 种子 BFS 连通 + 边界闭合、
Rect 开闭区间手算对拍、seed=42 `debug_ascii` 样例）。
