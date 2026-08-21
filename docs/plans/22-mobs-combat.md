# 22 · 怪物 AI 与战斗接线计划（M4 主体）

**文件所有权**：`src/actors/**` + `src/actors.rs`（增量）+ `src/scenes/in_game.rs`
（怪物调试标记与 HUD 扩展）。`scheduler/turn/dummy/char_stats/combat/bestiary/hero`
尽量只消费；hero.rs 允许小幅增量（撞击攻击入口），改动必须在笔记列明。
禁止改：`main.rs`、`Cargo.toml`、`src/levels/**`、`src/render/**`（23 号域并行中）、
`src/utils/**`（只消费 `PathFinder`/`cast_shadow`）、其他共享文件。

**参考实现**（core/.../actors/）：
- `mobs/Mob.java`：`act()` 主流程、`SLEEPING/WANDERING/HUNTING` 内部类状态机、
  `chooseEnemy`、`getCloser`（用 `PathFinder`）、`attack` 触发条件、EXP 掉落
- `Char.java`：`attack()` 结算流程（命中 → damageRoll → drRoll → 扣血）
- `hero/Hero.java`：撞击攻击（走向怪物格 = 攻击）、`earnExp`/`lvlUp`
- `levels/RegularLevel.java`：`nMobs()`、`createMobs`、`randomRespawnCell`
- `Bestiary`/`SewerLevel` 的按深度怪物轮换表（Rat/Snake/Crab 三种已够 1-4 层，
  Swarm/Slime 未入库记 TODO）

## 目标

1. 怪物生成：进入新层时按 `nMobs(depth)` 与轮换表生成怪物实体
   （`CharStats` 来自 bestiary + `Actor` 进时间轮 + `GridPos` + AI 状态组件），
   出生格远离入口（`randomRespawnCell` 语义：passable、无占用、不与英雄相邻）。
2. AI 状态机（手写 enum，不引外部库）：Sleeping（视野内见英雄且过警觉判定 →
   醒）、Wandering（随机溜达/朝目标走）、Hunting（`PathFinder.get_step` 追击，
   邻格则攻击）。怪物视野用 `utils::cast_shadow`（各自 FOV，SPD 语义）。
   简化点（记笔记）：无 FLEEING、无 alerted 传播细节。
3. 战斗结算：`combat::hit/damage_roll/dr_roll` 接线——英雄走向怪物格 = 撞击攻击；
   怪物 Hunting 邻格 = 攻击英雄。伤害、闪避消息先 `info!` 日志（战斗日志 UI 后续）。
4. 死亡：怪物 HP≤0 → 移出时间轮与场景 + 英雄 `earn_exp`（升级曲线 bestiary 已有，
   升级回满血 + `info!`）。英雄 HP≤0 → `warn!` + 回 Title（GameOver 场景后续）。
5. 单格占用：移动目标格有 Char 时不可走（英雄→怪物格 = 攻击；怪物→占用格 =
   绕行/等待，对照 `Dungeon.findPath` 的 passable 修正）。
6. 展示：怪物在 `in_game.rs` 加调试色块（Rat 棕/Snake 青/Crab 红橙，z 与英雄同层；
   真精灵是 25 号域数据 + 下波接线）；HUD 加英雄 HP 行；本波不做迷雾联动
   （23 号域并行中，标记遮蔽由协调者下波接线）。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿（174 基线
  不许破坏）。
- 新增集成测试（无渲染）：按深度生成数量正确且不占入口；睡怪不动，英雄进视野
  后转 Hunting 并逼近；邻格攻击造成期望范围伤害（固定种子）；怪死 → EXP 入账 +
  实体清理；英雄死 → 回 Title；占用格不可走入（攻击替代）。
- 时间轮公平性：怪物与英雄 spend 语义正确（Crab speed=2 每回合走两步的表现，
  有测试）。

## 进度

- [x] 生成与轮换表
- [x] AI 状态机
- [x] 战斗接线 + 死亡/EXP
- [x] 占用与撞击攻击
- [x] 调试标记 + HUD
- [x] 集成测试

## 实现笔记

### 模块布局

- `actors/melee.rs`：攻击结算纯核 `resolve_melee`（`Char.attack` 剥除 Buff/物品
  乘子），掷值顺序钉死 hit(2 掷) → drRoll(2 掷) → damageRoll(2 掷)，未命中
  只耗命中掷。英雄撞击与怪物攻击共用，单测对拍顺序/值域/Miss 不耗流。
- `actors/mob.rs`：`Mob` 组件（kind + wander_target）、`ActorRng` 资源
  （ChaCha12，actors 域回合逻辑统一随机源，种子在应用边界取熵、测试覆写）、
  `MobSpawnRequest` 一次性生成请求、`n_mobs`/`mob_rotation`/`exp_for_kill`
  纯函数、`spawn_mobs` 系统、`chebyshev`（`Level.distance` 语义，邻格 = 1
  含对角）。
- `actors/ai.rs`：`AiState` 三态枚举 + `mob_act` 观察者（顶层收 `ActTurn`
  按 `Mob` 组件过滤，dummy.rs 分发模式）；`step_towards`（邻格直踏否则
  `PathFinder.get_step`）、`random_destination`（30 次采样封顶）。
- 接线：`ActorsPlugin` 注册 `ActorRng`/`spawn_mobs`（`Level` + `MobSpawnRequest`
  同时存在才触发，排在 `spawn_hero` 后）与 `mob_act` 观察者；
  `scenes/in_game.rs` 的 `insert_fresh_level` 进层/下楼时插 `MobSpawnRequest`
  （对应 SPD `Level.create → createMobs`），退出场景移除。

### 生成（对照 RegularLevel/MobSpawner）

- `n_mobs`：首层固定 8（L221-L222，不掷随机）；其余 `3 + depth%5 + Int(3)`，
  Large 氛围 ×1.33 向上取整（L212-L215）。重生冷却（`mobLimit` 的护符分支）
  未移植。
- 轮换表 `mob_rotation`（`standardMobRotation` L71-L97）：图鉴只有
  Rat/Snake/Crab（15 号域），未入库怪按最近角色替换——TODO（图鉴扩充后还原）：
  Gnoll→Rat、Swarm→Snake、Slime→Crab。替换后：d1 [鼠3 蛇1]、d2 [鼠4 蛇1]、
  d3 [鼠4 蛇2 蟹1]、d4 [鼠1 蛇1 蟹4]；抽取照 `Level.createMob` 的
  "耗尽重灌 + 洗牌"（L508-L516，SPD 前向 Fisher-Yates）。
- 出生格约束（任务书钉死的简化交集）：passable、非入口/出口、与英雄切比雪夫
  距离 > 1、单格单怪（采样不放回）。SPD 的"入口 FOV + 8 步步行禁区"
  （L235-L252）与房间驱动落位未移植——生成域房间数据不出 `Level` API，
  改为全图筛选随机采样。
- 入轮语义照 `Actor.add`：入场 `time = clock.now`、`MOB_PRIO`；出生即
  `Sleeping`（Mob.java L118）；挂 `DespawnOnExit(InGame)`。

### AI 状态机简化清单（相对 Mob.java 内部类）

- 只保留 SLEEPING/WANDERING/HUNTING 三态：无 FLEEING/INVESTIGATING/PASSIVE，
  无 `alerted` 跨怪传播（Swarm Intelligence）与 `recentlyAttackedBy` 换目标。
- 警觉掷值（睡眠 L1118 `1/(dist+stealth)`、游荡 L1170）简化为
  "英雄进自身 FOV 即察觉"；惊醒仍花 1 回合（`TIME_TO_WAKE_UP`）。
- 游荡察觉是唯一零耗时转移（对照 `noticeEnemy` 不 spend、时间轮同刻重选），
  实现为同一次 act 内直落 Hunting 分支，满足 `process_turns` 活锁保护。
- Hunting 丢失视野持续追击英雄当前格（原文回落 WANDERING + 记忆最后目击点
  L1252-L1260）；目标不可达（被堵）等待 1 回合再试（`handleUnreachableTarget`
  的换目标简化）。
- 视距固定 8（`Char.viewDistance` 默认值）；黑暗氛围减视距归 26 号渲染/迷雾域。
  各怪行动时即算即弃 FOV（SPD 由 `Level.updateFieldOfView` 维护每怪缓存）。
- 占用底图：任何存活 Char 所占格视为不可走（SPD `findPassable` 只排除自身
  FOV 内可见者，简化为全排）；自身格靠 `PathFinder` 的起点旁路不受影响。

### 战斗接线与死亡

- 英雄撞击：`hero_act` 移动分支前置检查——目标格有存活怪即改攻击
  （`handle` L1904-1910；键盘单步目标恒邻格，`canAttack` 恒真），
  `spend(attackDelay=1)`，不位移。
- 怪物攻击：Hunting 且切比雪夫 1（含对角，`canAttack` L477-L479）→
  `resolve_melee` + `spend(attackDelay)`；命中/闪避/伤害均 `info!`。
- 死亡即时性（17 号域同款结论）：观察者内 `Commands` 冲刷在 `process_turns`
  独占期之后，故击杀先直写"HP=0 + `Actor.time = f32::MAX`（≈ `diactivate`）"
  保证本帧不再被选中，`despawn` 走延迟命令帧末生效。
- 英雄死亡：`warn!` + `TurnState::WaitingForInput`（挂起时间轮）+
  `NextState(Title)`（GameOver 场景属后续里程碑）；实体清理靠
  `DespawnOnExit`，时间轮复位靠既有 `reset_turn_wheel`。
- EXP：`exp_for_kill`（`Mob.destroy` L853：英雄等级 > `maxLvl` 不给）；
  `Hero::earn_exp` 升级曲线经 bestiary，**升级回满血为任务书钉死简化**
  （SPD `updateHT` L265-L267 仅把 HT 增量补进当前 HP）。

### 与 SPD 的其他差异（值恒等或延后）

- `defenseProc`/`attackProc` 与全部 Buff/物品乘子未移植（裸基线恒等变换）。
- 怪物间不互相攻击（`chooseEnemy` 的仇恨/魅惑分支未移植，敌人恒为英雄）。
- 重生（`respawner`）未移植：每层生成一次，杀完即清场。

### hero.rs 增量改动清单

1. `Hero` 组件加 `lvl`/`exp` 字段（L217-L218），`Default` 改手写（lvl=1）。
2. 新增 `Hero::earn_exp`（L1967-L2065 骨架：入账 + while 升级循环 + 满级清零，
   升级重算 HT/命中/闪避并回满血）。
3. `hero_act` 移动分支前插撞击攻击：目标格有存活怪 → `resolve_melee` →
   击杀则失活 + despawn + `earn_exp`（新增 `mobs` 查询、`ActorRng`、
   `Commands` 参数；英雄查询加 `Without<Mob>` 保证与怪物查询不相交）。
4. 测试 `install_boxed_level` 换图时顺带清怪（进场真实关卡现在会生成怪物），
   保持既有英雄测试的无怪确定性环境；新增 `earn_exp_levels_up_and_heals_to_full`
   纯逻辑对拍。

### 展示（scenes 域增量）

- `in_game.rs`：`sync_mob_markers`（模式照抄 `sync_hero_marker`，Rat 棕
  `srgb(0.62,0.4,0.22)` / Snake 青 `srgb(0.2,0.85,0.75)` / Crab 红橙
  `srgb(0.95,0.4,0.15)`，z=1 与英雄同层——单格占用保证不重叠）；HUD 第二行
  `HP x/y  Lv n  EXP a/b`（`HpHudText` 标记 + `Changed<CharStats>/Changed<Hero>`
  响应式刷新）；根节点改纵向 flex。`text.rs` 加 `HUD_HP_LABEL`/`HUD_LVL_LABEL`/
  `HUD_EXP_LABEL`。
- 迷雾遮蔽未做（26 号渲染域并行，协调者下波接线）。

### 新增测试清单

- `melee.rs`：掷值顺序对拍 / Miss 不耗随机流 / 值域 + 同种子确定性。
- `mob.rs`：轮换表对拍（含替换映射）/ `n_mobs` 公式（首层恒 8、Large ×1.33）/
  EXP 门槛 / 切比雪夫语义 / 生成集成验收（8 只 = 6 鼠 2 蛇、格约束、
  入轮组件、出生沉睡、请求被消费）。
- `ai.rs` 集成（无渲染手工铺图 + 固定种子）：睡怪视野外静止 → 进 FOV 惊醒
  （耗 1 回合）→ 逐回合逼近至邻格；邻格攻击伤害域 [0,4] 且 12 回合至少一中；
  撞击攻击不位移不顶开、耗时 1；杀怪 EXP 入账 + 实体消失；英雄死回 Title
  （实体清理 + 时间轮复位）；追猎怪绕行被占格（惰性路障）全程单格占用；
  Crab 每英雄回合两步、邻格后改一击。
- `hero.rs`：`earn_exp` 连升/回满血/不足不升。
