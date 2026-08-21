# 24 · 关卡生成三期计划（标准房间变体 + 特殊/密室框架）

**文件所有权**：`src/levels/**` + `src/levels.rs`。
禁止改：`main.rs`/`Cargo.toml`/其他域目录。22/23 号域并行消费 `Level` 公共 API，
**签名与语义不得变**（新增可以）；RNG 消耗顺序变化导致出图变化属预期
（更新自己的钉死测试）。

**参考实现**（core/.../levels/rooms/）：
- `standard/StandardRoom.java`：`sizeCat` 掷点表（NORMAL/LARGE/GIANT 概率随
  深度）、`SizeCategory` 语义
- `standard/` 下水道会出的变体：`SewerPipeRoom`、`RingRoom`、`SegmentedRoom`、
  `GrassyGraveRoom`、`CaveRoom`（按 `SewerLevel`/`StandardRoom.createRoom` 的
  实际轮换表选 3-5 个可行的先移植，其余记清单）
- `special/SpecialRoom.java`：`initForRun`/`SpecialRoomPool` 框架 +
  `RatKingRoom` 或 `GardenRoom` 里挑 1 个简单的做样板
- `secret/SecretRoom.java`：密室计数表 + `SecretGardenRoom` 样板
- `RegularLevel.initRooms`：special/secret 数量与插入位置（一期简化处恢复）

## 目标

1. `StandardRoom` 尺寸类别掷点（深度加权表，Java 行号）+ 房间"绘制变体"框架
   （`RoomKind::Standard` 细分或新枚举，笔记说明取舍）。
2. 移植 3-5 个下水道标准房间变体的 `paint`（对照各自 Java；实现不了的水管房
   曲线细节允许保真降级并记录）。
3. 特殊房间框架：`initRooms` 恢复 special 数量语义，先接 1 个样板房
   （上锁门 + 内容占位）；密室同理 1 个样板。
4. 门类型丰富化：`Door.Type`（REGULAR/TUNNEL/HIDDEN/LOCKED）在 painter 里的
   完整落地（一期已有基础，补 LOCKED 与钥匙占位 TODO）。
5. `debug_ascii` 符号扩展（锁门 L、特殊房间边界可辨识）。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿。
- 新增测试：尺寸类别分布（多种子统计）；每个新房间变体的独立 paint 单测
  （小 rect 内地形计数/形状断言）；含特殊/密室的百种子连通性依旧全绿
  （锁门算 passable？——对照 SPD flags：LOCKED_DOOR 是 SOLID，连通性测试
  的可达性判定需要相应调整为"钥匙可达"或排除锁门房，笔记说明）。

## 进度

- [x] 尺寸类别掷点
- [x] 标准房间变体 ×5
- [x] 特殊/密室框架 + 样板
- [x] 门类型完善
- [x] 测试

---

## 实现笔记（三期完工）

改动文件：`src/levels/standard.rs`（新增）、`src/levels/special.rs`（新增）、
`src/levels/rooms.rs`（重写扩展）、`src/levels/painter.rs`、`src/levels/builder.rs`、
`src/levels/generator.rs`、`src/levels.rs`、本文件。
`terrain.rs` 的 `Terrain`/flags 冻结接口未动；`Level` 公共 API 只增未改。

### 1. 尺寸类别与 sizeFactor

- `SizeCategory`（StandardRoom.java L36-L51）：NORMAL(4,10,1)/LARGE(10,14,2)/
  GIANT(14,18,3)，`roll_size_category` 移植 `setSizeCat(0, maxRoomValue-1)`
  （L63-L90）：超预算档权重清零，全零返回 `None` 由调用方重抽变体
  （对应 `RegularLevel.initRooms` L136-L138 的 do-while，重抽上限 4096 防死循环）。
- **省略的流位**：Java `StandardRoom` 构造器实例初始化块的 `setSizeCat()`（L54）
  掷点结果必被 `initRooms` 的显式 `setSizeCat(standards-i)` 覆盖，本移植不掷这一次。
  与 Java 逐 roll 对齐从二期起就不是目标（`ChaCha12` ≠ Java `Random`），只保证自身确定性。
- sizeFactor 同步三处：`initRooms` 预算扣减（`i += room_value`）、
  `RegularBuilder.setupRooms` 主路径配额扣减、`weightRooms` 连接权重
  （标准房按 `size.room_value()` 次数重复进 `branchable`，Entrance/Exit 也算标准房）。

### 2. 标准变体（5 个）与降级点

轮换表 `SEWER_STANDARD_TABLE` 对照 `StandardRoom.rooms`/`chances`（L124-L173）：
下水道区域房 {16,8,8,4,4} + 全区域房尾段，未移植类回退 `StandardVariant::Empty`
**但保留权重**——已移植变体出现率与 Java 一致，Empty 房承接其余概率质量。

| 变体 | Java | 降级点 |
| --- | --- | --- |
| `SewerPipe`(16) | SewerPipeRoom | `getDoorCenter` L230-L231 的 `Float() < int%1` 恒假掷点省略（不对齐流位）；幻影门 do-while 加 4096 上限 |
| `Ring`(8) | RingRoom | 完整移植（含大房内芯装饰 + 内门） |
| `CircleBasin`(4) | CircleBasinRoom | 完整移植（椭圆 + 深渊 + 栈桥 + Patch 蓄水） |
| `Burned`(0/1) | BurnedRoom | 火焰陷阱种类简化为 `TRAP/SECRET_TRAP/INACTIVE_TRAP` 地形（陷阱实体是四期+）；过火格存 `deco_ban_patch` 拒绝水/草/陷阱落位 |
| `Striped`(1) | StripedRoom | 完整移植（条纹/同心环两模式） |

未移植回退 Empty 的类：WaterBridgeRoom、RegionDecoPatchRoom、PlantsRoom、
AquariumRoom、PlatformRoom、FissureRoom、GrassyGraveRoom、StudyRoom、
SuspiciousChestRoom、MinefieldRoom（多数依赖物品/植物/刷怪系统）。

### 3. 特殊房/密室框架与池子简化

- `RunPools`（special.rs）对应 Java `SpecialRoom`/`SecretRoom` 的静态字段：
  `runSpecials`（用过轮换队尾）、`floorSpecials`（层内克隆池，用过即删）、
  `runSecrets`（不缩水轮换）、`regionSecretsThisRun`（各域预算 [2,2.25,2.5,2.75,3]
  整数保底 + 小数掷点）。`create_*` 的 {6,3,1} 队首偏好照搬（L176-L178/L96-L98）。
- **池子简化**：已移植池仅 `Garden`/`SecretGarden` 各 1 种（Java 19/12 种）。
  Java 特殊房池耗不尽，本工程在 `initRooms` 把 special 数量截断到
  `min(掷点数, 池大小)` = 1——每层恰 1 间特殊房（Java 下水道 1-2 间）。
  PitRoom/Laboratory/Shop 前置分支未移植（下水道 1-4 层 Shop 恒 false）。
- **run 池重放**：`generate_level(seed, depth)` 保持纯函数——
  `run_pools_before_depth` 用 `seed+1` 起 run 流重建池子，再按 depth 1..n-1
  逐层重放 `initRooms` 消耗（feeling/builder/变体/special/secret 掷点序完全一致）。
  新增测试 `run_pool_replay_is_order_independent` 钉死"跳层生成 ≡ 顺序下潜"。
- 样板房：`GardenRoom`（SpecialRoom 池，LOCKED 门 + 草地 + 高草圈；
  蜜罐/种子/植物实体记 TODO 占位为地形）；`SecretGardenRoom`（HIDDEN 门 +
  高草 Patch；植物实体同 TODO）。钥匙投放是物品域四期 TODO
  （`upgrade_entrance_door` 处有标注）。

### 4. 门类型与不挡路保证

- `DoorType`：`Empty < Tunnel < Water < Regular < Unlocked < Hidden < Barricade
  < Locked < Crystal` 只升不降（`Door.setType` 语义）；painter 落地全部对应地形。
- 特殊房/密室 `maxConnections ≡ 1`（SpecialRoom.java L46-L49）→ 只能当支路叶子；
  `createBranches` 里密室挂点跳过隧道房（RegularBuilder.java L286-L292）。
  锁门/密门地形是 SOLID 非 passable，百种子连通性测试的 BFS 天然不穿——
  主路径连通即自动验证"特殊房不挡路"；另断言每层锁门恰 1 扇且紧邻可达地板
  （钥匙送达后开得了）。

### 5. debug_ascii 符号集（三期新增）

`L` 锁门（LOCKED_DOOR/HERO_LOCKED/CRYSTAL 并入）、`S` 密门、
`B` 路障/书架、`&` 雕像与区域装饰（STATUE/REGION_DECO/ALT）、
`EMPTY_SP`/`EMBERS`/`INACTIVE_TRAP` 等可通行地形归 `.`。
完整映射见 `src/levels.rs::debug_ascii` 与 `debug_ascii_maps_phase3_terrains` 测试。

### 6. 测试与钉死

70 个 levels 域测试全绿（全仓 251）。三期新增：变体/尺寸掷点分布（±6σ 带）、
5 变体各自 paint 单测、`RunPools` 全套单测、special/secret 数量约束
（每层 1 特殊房、depth 1 无密室、depths 2-4 合计 = 域预算 2 + SECRETS 氛围加成）、
密门数 ≥ 密室数且不挡主路、GIANT 房实际出现、run 池重放序无关、
seed=42 钉死重钉（含恰 1 扇 `L`）。水草率带宽因花园/条纹房结构性草地放宽到
0.45（实测 ≈0.33，注释留档）。

### 7. 四期候选

- 陷阱实体系统接 `BurnedRoom` 的火焰陷阱与 `paint_traps` 的陷阱种类表。
- 物品域：花园蜜罐/种子、锁门钥匙（`IRON_KEY`）投放、密室战利品。
- 更多变体：WaterBridgeRoom（水桥）、AquariumRoom（水族）、PitRoom 前置链。
- 其他区域（监狱+）的轮换表与 `StandardRoom.createRoom` 全表化。
- 连接房池（TunnelRoom 之外的 PerimeterRoom/RingTunnelRoom 等）。
