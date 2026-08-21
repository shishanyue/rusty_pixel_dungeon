# 01 · 目标架构：Java OOP → Rust ECS 映射

## 总原则

1. **忠实玩法，不忠实结构**。SPD 的数值、生成算法、回合语义逐一对照移植；
   Java 的继承树（`Actor → Char → Mob → Rat`）不搬进 Rust，改用组件组合。
2. **纯逻辑与 Bevy 解耦**。生成算法、回合推进、战斗结算写成不依赖 `World` 的纯函数/
   纯结构体，Bevy 系统只做薄适配层——这是可单测性的根。
3. **数据表驱动**。SPD 大量 switch-by-depth/kind 的逻辑改成 const 表 + 纯函数。

## 分层

```text
┌─ scenes/     场景与 UI（Bevy UI，观察者驱动）
├─ render/     地图/精灵渲染（bevy_ecs_tilemap，M3 起）
├─ actors/     回合调度、英雄、怪物、Buff（ECS 组件 + 纯逻辑核）
├─ items/      物品、背包、使用效果（M4 起）
├─ levels/     关卡数据 + 生成流水线（纯逻辑，零 Bevy 渲染依赖）
├─ assets/     资产集合、.properties/JSON 加载器、i18n
├─ dungeon.rs  一局游戏的全局状态（深度/分支/挑战/限量掉落）
└─ states.rs   AppState: Loading → Title → InGame
```

## Java → Rust 对应表（核心）

| SPD (Java) | Rusty (Rust/Bevy) | 说明 |
| --- | --- | --- |
| `Dungeon`（静态字段） | `Resource Dungeon` | depth/branch/challenges/LimitedDrops |
| `Actor` 静态时间轮 | `Resource TurnScheduler` + `Component Actor{time,prio}` | 见 11 号计划 |
| `Level`（抽象类+子类） | `struct Level`（数据）+ `LevelKind`（枚举）+ 生成流水线 | 行为差异用数据表 |
| `Terrain`（int 常量） | `#[repr(u8)] enum Terrain` + `const fn flags()` | num_enum 双向转换 |
| `Room` 继承树 | `RoomKind` 枚举 + `Room{ rect, connections }` 结构 | 生成期纯数据，不进 ECS |
| `Builder`（LoopBuilder 等） | `trait Builder`（纯逻辑，输入输出 `Vec<Room>`） | 生成期不碰 Entity |
| `Painter` | `trait Painter`（写 `Level.map`） | |
| `Messages`（I18NBundle） | `Resource Messages`（HashMap 链式回退） | 见 12 号计划 |
| `PixelScene/GameScene` | `AppState` + 各 `ScenePlugin` | 见 13 号计划 |
| `Char/Mob/Hero` | `Char` 组件 + `Hero`/`Mob` 标记 + 数值组件 | M4 |
| Bundle 存档 | serde + RON/JSON（后续评估 moonshine_save） | M6 |

## 关键接口（M0 冻结，各域消费）

```rust
// levels/terrain.rs
#[repr(u8)] pub enum Terrain { Chasm=0, Empty=1, …, HeroLockedDoor=38 }
impl Terrain { pub const fn flags(self) -> TerrainFlags; pub fn discover(self) -> Terrain; }

// levels.rs
pub struct Level { pub map: Vec<Terrain>, pub width/height: usize, pub depth: i32,
                   pub feeling: Feeling, pub entrance/exit: IVec2, … }
// 坐标约定：IVec2 格子坐标，bevy::math::IRect（max 开区间）；index = y*width + x

// states.rs
pub enum AppState { #[default] Loading, Title, InGame }

// dungeon.rs
pub fn level_kind(depth: i32, branch: i32) -> LevelKind;  // SPD Dungeon.newLevel 的表化

// assets.rs：8 个资产集合（单一 Loading 状态加载），枚举键 → Handle 取用
```

## 确定性

- 地牢生成必须可种子复现（SPD 的 `Dungeon.seed` 语义）。生成流水线只允许使用
  显式传入的 `&mut impl Rng`，禁止 `rand::random`/`rand::rng()` 全局源。
- 回合逻辑同理：一切随机经由 `Dungeon` 持有的 RNG。
