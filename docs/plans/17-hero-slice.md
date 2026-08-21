# 17 · 英雄可玩竖切计划（M2 收尾 + M4 前置）

**文件所有权**：`src/actors/`（新文件 hero.rs 等；`scheduler/turn/dummy/char_stats/
combat/bestiary` 只消费不改）、`src/actors.rs`（增量）、`src/scenes/in_game.rs`
（所有权移交给你：接英雄生成与调试渲染）。相机跟随归你（`scenes.rs` 的全局相机
实体只许移动 Transform，不许增删）。
禁止改：`main.rs`、`Cargo.toml`、`src/levels/**`（只消费）、`src/render*`（16 号域
并行中）、`src/utils/**`（只消费）、`src/assets*`、`src/states.rs`、`src/dungeon.rs`。

**参考实现**（语义对照，实现允许 M2 简化）：
- `core/.../actors/hero/Hero.java`：`act()`/`move`/`speed` 中与移动相关的最小语义
- `core/.../scenes/GameScene.java` + `windows/CellSelector.java`：输入 → 行动的桥
  （M2 用键盘方向键替代点击，点击寻路是 M3 后续）

## 目标（可玩性第一步：能在生成的地牢里走动）

1. `Hero` 标记组件 + 生成系统：进入 InGame、`Level` 就绪后，在 `level.entrance`
   生成英雄实体（`CharStats` 用 `bestiary` 的战士出生数值、`Actor` 组件进时间轮、
   `HERO_PRIO` 优先级）。
2. 键盘移动：方向键/WASD/小键盘 8 向（SPD 是 8 向移动）。输入只在
   `TurnState::WaitingForInput` 时受理：写入英雄的"待执行动作"，调度系统在英雄
   回合执行——走一步 `spend(1/speed)`（对照 `Char.speed()` 与 `Hero` 移动耗时
   语义），撞墙/solid 不消耗回合。对角步语义照 14 号域笔记（SPD 对角只要求目标格
   passable；但 `Level.adjacent` 判定用切比雪夫距离）。
3. 英雄调试渲染：在 `in_game.rs` 调试视图上加一个显眼方块（或用
   `SpritesCollection` 的 warrior.png 裁一帧，二选一，笔记说明），位置随格子移动
   （平滑插值可选，瞬移可接受）。
4. 相机跟随英雄（简单 lerp 或直接锁定）。
5. 踩到 `Exit` 地形：`dungeon.depth += 1` 并重新生成关卡（英雄回到新层入口）——
   这是"下楼"的最小语义（`Dungeon` 资源可写，这一处豁免 dungeon.rs 禁改：
   你只改 `depth` 字段值，不改 dungeon.rs 文件本身）。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿（不破坏既有
  107 个）。
- 新增集成测试（MinimalPlugins，无渲染）：英雄在入口生成且进时间轮；模拟按键 →
  英雄移动一格且时间轮推进（`TurnClock.now` 增加）；撞墙不动不耗时；踩出口后
  depth+1 且新 `Level` 生成、英雄在新入口。
- 与 16 号域并行工作，坐标约定一致：格 (x,y) → 世界
  `((x-(w-1)/2)*16, ((h-1)/2-y)*16)`。

## 进度

- [x] Hero 生成 + 入时间轮
- [x] 键盘 8 向移动（回合消耗语义）
- [x] 调试渲染 + 相机跟随
- [x] 下楼闭环
- [x] 集成测试

## 实现笔记（交付时状态）

**交付文件**：`src/actors/hero.rs`（新，组件/系统/观察者 + 7 个集成测试）、
`src/actors.rs`（增量：模块声明、re-export、`TurnWheelSet`、插件接线）、
`src/scenes/in_game.rs`（英雄方块、相机跟随/复位、下楼重建、响应式视图与 HUD）。

### act 对接方式（与 11 号域时间轮）

照 `dummy.rs` 模板挂顶层观察者 `hero_act`（`On<ActTurn>`，按 `Hero` 组件过滤）：

- 无 `next_action` ≈ `Hero.act()` 的 `curAction == null` 分支（Hero.java
  L863-881）：置 `TurnState::WaitingForInput` ≈ `ready()` + act 返回 false；
- 有则执行移动并 `spend(1/speed)`；输入系统只在 `WaitingForInput` 受理按键，
  写入动作后置回 `Processing` ≈ `hero.handle()` 后的 `next()` 唤醒。

新增公开系统集 `TurnWheelSet`（`actors.rs` 注册 `process_turns` 时打标签，
`turn.rs` 未改）：英雄生成/输入排其前、场景域下楼排其后，"按键 → 行动 →
换层"一帧内定序完成。英雄生成不用 `OnEnter` 而是 Update 条件系统
（`Level` 存在且无英雄 → 在 `level.entrance` 生成），进场与下楼重生走同一
路径。actors 域系统全部以资源存在性（`Level`/`ButtonInput`）做门卫而非
`in_state`，`turn.rs` 既有的无状态测试环境不受影响。

**空轮语义**：`TurnState` 默认 `Processing`，空轮时 `process_turns` 选不出
行动者立即返回（无空转），Loading/Title 无需处理；`OnExit(InGame)` 复位
TurnClock/TurnState/DescendRequest（`Actor.clear()` 语义，Actor.java
L160-168），防止挂起状态残留导致再入卡死。

### 速度 / 对角语义

- 移动耗时 `TICK / CharStats::base_speed`（`getCloser` L1832/L1863 基础
  delay=1 + `spend(delay/speed)`；`Char.speed()` L775-788 的 Buff/护甲乘子
  属 M4，M2 即基础值，战士 1.0 → 每步 1 回合）。
- 撞墙/solid：目标格不可走 → 不动不耗时、保持待输入（`actMove` 失败分支
  L989-992 的 `ready()`）。合法性判定 = `is_inside && passable[dst]`，
  与 `getCloser` 寻路底图一致（L1809）；对角步只要求**目标格** passable
  （SPD `PathFinder` BFS 真实语义，14 号域笔记第 1 条），不查两侧正交格。
- 8 向键位：方向键/WASD/小键盘 8462 四正向，QEZC/小键盘 7913 四对角；
  同帧多键向量合成后钳到 [-1,1]（W+D = 右上，对冲抵消）。

### 下楼实现选择

英雄踩上 `Terrain::Exit`（M2 简化为"走上即下楼"，SPD 原版是站上后另发
`HeroAction.LvlTransition` 走过场）→ act 观察者直写 `DescendRequest` 资源
并挂起时间轮——用直写资源而非事件/Commands，因观察者跑在 `process_turns`
独占 World 期间，命令冲刷时机跨帧不定，直写立即可见。场景域 `descend`
系统（`after(TurnWheelSet)`，同帧）：`dungeon.depth += 1` → despawn 英雄 →
TurnClock/TurnState 清零（对照 `Dungeon.newLevel` L297-300 换层前
`Actor.clear()`；SPD `fixTime` 保留的分数回合余量 M2 舍弃，场上唯一 actor
是英雄，不可观察）→ 熵种子生成新层整体替换 `Level`/`RunSeed`。

**HUD 二选一**：取"响应式"而非整场景重建——方块视图与 HUD 文本分别以
`resource_exists_and_changed::<Level>` / `<RunSeed>` 为条件重建/重写
（`RunSeed` 每层整体替换，天然是"深度/种子已换"的变更信号）。

### 渲染/相机取舍

- 英雄形象二选一取纯色方块（`Sprite::from_color` 亮黄、z=1 压地块），
  不裁 warrior.png——16 号渲染域并行持有图集渲染，竖切不碰 `render/`。
  方块组件按需补挂在英雄逻辑实体上，每帧随 `GridPos` 瞬移（插值留 M3）。
- 格 → 世界坐标抽 `in_game::grid_to_world`（(x,y) →
  ((x-(w-1)/2)·16, ((h-1)/2-y)·16)），铺地块/英雄/相机三处共用，与 16 号域
  `cell_center_world` 约定一致。
- 相机每帧直接锁定英雄（无 lerp）；退出 `InGame` 时复位到原点（Title 星空
  以原点为中心）。全局相机实体只改 Transform，未增删。

### 已知留白（有意不做）

- Esc 回 Title 再进 InGame 不重置 `Dungeon.depth`（新一局的 `Dungeon.init`
  归 Title"进入地牢"流程/协调者裁决，本域只加深度）。
- `LockedExit`/`UnlockedExit` 不触发下楼（SOLID 不可走/boss 层语义，M4+）；
  Chasm 坠落、高草践踏、水声等 `Hero.move`（L2286-2308）副作用均属 M4。
