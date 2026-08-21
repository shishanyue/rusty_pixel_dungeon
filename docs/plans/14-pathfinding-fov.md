# 14 · 寻路与视野域计划（M3 前置）

**文件所有权**：`src/utils.rs`（增量）与 `src/utils/` 目录。
禁止改 `main.rs`/`Cargo.toml`/其他域目录。

**参考实现**：
- `/home/shishanyue/GitHub/shattered-pixel-dungeon/SPD-classes/src/main/java/com/watabou/utils/PathFinder.java`
- `/home/shishanyue/GitHub/shattered-pixel-dungeon/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/mechanics/ShadowCaster.java`
- 消费方（只读参考，理解调用形态）：`actors/mobs/Mob.java` 的 chooseEnemy/getCloser、
  `Level.java` 的 updateFieldOfView

## 目标

两套纯逻辑算法，均以 `&[bool]`（passable / los_blocking）+ `width` 数组形态输入，
不依赖 `Level` 类型与 Bevy：

1. **PathFinder**：`build_distance_map`（SPD 的桶队列 BFS/Dijkstra）、`get_path`、
   `get_step`、`get_step_back`；邻接表 NEIGHBOURS4/8/9 语义照抄（SPD 是 8 向移动，
   注意对角穿墙规则与 Java 完全一致——`canStep` 里对角步要求两侧正交格 passable）。
2. **ShadowCaster**：递归阴影投射 `castShadow`，含 SPD 的圆形半径修正表
   （`rounding` 数组）与 `MAX_DISTANCE` 语义。

## 强制验收

- 纯单测：
  - 手算小地图（≤10×10）的 distance map 与最短路对拍；
  - 对角规则用例（贴墙对角不可走）；
  - FOV：空房间圆形半径边界逐格对拍 rounding 表；柱子遮挡的阴影锥形状；
  - 与 SPD 数值语义差异必须为零（测试注释标 Java 行号）。
- `cargo check/clippy/test` 全绿，不破坏既有 48 个测试。

## 进度

- [x] PathFinder（距离图/取步/回退步）
- [x] ShadowCaster（含 rounding 表）
- [x] 对拍单测

## 实现笔记

**交付**：`src/utils/pathfinder.rs`、`src/utils/shadow_caster.rs`，经 `src/utils.rs`
re-export（`PathFinder`、`cast_shadow`）。新增 14 个单测，期望值全部来自
逐字复刻两份 Java 参考类在本机（OpenJDK 25）的实际输出，注释标注 Java 行号。

**桶队列实现选择**：SPD 的"桶队列"实为等权 BFS 的 FIFO 波前（`int[] queue` +
head/tail 游标）。移植为 `Vec<usize>` 复用缓冲 + 局部 `head` 游标，push 即入队，
遍历序、`dirLR` 首尾裁剪（`PathFinder.java` L231-L232）与出队即断（L225-L227）
逐行对照。Java 静态字段 + `setMapSize` 改为实例结构体 + `new(width, height)`。

**rounding 表来源**：按 `ShadowCaster.java` L35-L46 的公式在 `LazyLock` 中运行时
计算（`min(j, round(i * cos(asin(j / (i + 0.5)))))`）。用 Java 实测全表并硬编码进
`rounding_table_matches_java` 对拍；各值距 .5 舍入边界的最小裕度约 3.4e-3，
libm 实现间 ulp 级差异不可能翻转结果，故 Rust `f64` 复算与 Java 恒等。
`distance == 2` 的补角（L87-L95）在 `cast_shadow` 里取行时应用一次后传入递归，
值恒等，仅省去 Java 每层递归的重复 clone。

**与 Java 的语义差异**（合法输入下均不可观察，详见两文件模块级文档）：

1. 任务书提到的 `canStep` 在 SPD 3.3.8 源码中不存在（全仓库无此符号）。
   真实语义：BFS 对角扩展只要求目标格 passable（L233-L242），两侧正交格全墙
   也允许斜穿（`diagonal_corner_cut_allowed_like_java` 用 Java 输出对拍验证）；
   唯一的"对角限制"是左右边缘 `dirLR` 首尾裁剪防索引回绕（L231-L232，
   `map_edge_trim_prevents_wraparound` 验证）。按真实 Java 语义移植。
2. 防御性差异 A：`find`/`getStep`/`getStepBack` 的下坡扫描（L92-L101 等）Java
   直接索引 `from + dir[i]`，站在首/末行会抛异常；移植跳过越界候选。SPD 关卡
   四周恒为实心墙，不可触发。
3. 防御性差异 B：Java `queue` 定长 `size`，`n == from` 分支（L236、L320）重复
   入队在极端小图上会溢出抛异常；移植用可增长 `Vec`，仅在 Java 本会崩溃的
   输入上有差异。
4. 防御性差异 C：`castShadow` 的 try/catch 越界兜底（L60-L72）移植为显式界
   检查 + 整张 FOV 清空，效果一致（含 `distance` 为负的情形）。

其余零差异：距离图、限距图、最短路平手顺序、`getStepBack` 两种
`canApproachFromPos` 语义（含对 `passable` 的原地改写副作用，`Dungeon.flee`
的重试循环依赖它，故 Rust 签名为 `&mut [bool]`）、FOV 圆形边界、柱后阴影锥、
全包围与 `MAX_DISTANCE` 截断，均以 Java 输出逐格/逐值对拍通过。
