# 21 · 物品数据域一期计划（M5 前置，纯数据层）

**文件所有权**：新建 `src/items.rs` + `src/items/` 目录。`main.rs` 的
`pub mod items;` 声明由协调者合流时添加——你不碰 `main.rs`（纯数据层暂无插件）。
禁止改 `Cargo.toml`/其他域目录。

**参考实现**（core/.../items/）：
- `Item.java`：核心字段语义（quantity/level/cursed/levelKnown/cursedKnown、
  stackable、价格）
- `Generator.java`：**掉落表核心**——`Category` 枚举（prob 权重 + 各类物品
  class 数组及其内部权重 `probs`）、`random()/randomUsingDefaults()` 的
  抽取语义（含 `deck` 洗牌机制 categoryProbs 的 Float 消耗）
- `ItemStatusHandler.java` + `potions/Potion.java`、`scrolls/Scroll.java`：
  未鉴定外观 ↔ 种类的洗牌绑定（每局一次、可种子复现）
- `Gold.java`、`food/Food.java`：最简单的两类实数据

## 目标（纯逻辑 + 数据表，不接背包/UI/掉落钩子——那是 M5 集成）

1. `ItemKind` 体系：枚举 + 每类的静态数据表（金币/食物/药水 13 种/卷轴 13 种/
   武器分层/护甲 5 阶——一期只需要种类与基础字段，效果留空 TODO）。
2. `Item` 实例结构：kind + quantity + level + cursed + identified 位。
3. `Generator` 移植：Category 权重表（Java 行号注释）、`random(category, rng)`、
   deck 洗牌语义（`categoryProbs` 递减补充机制照抄）。
4. `ItemStatusHandler` 等价物：药水颜色/卷轴符文的外观洗牌
   （`shuffle(rng)` 确定性、双射、已鉴定集合查询）。
5. 随机源显式 `&mut impl Rng`（项目确定性纪律）。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警（注意：模块暂时不挂进
  `main.rs`，用 `cargo test --lib` 无法编译到你——所以**你需要在
  `src/main.rs` 加一行 `pub mod items;`**，这一行是唯一豁免，其他内容不动）。
- `cargo test` 全绿；新增测试：Category 权重表与 Java 逐值对拍（行号）；
  固定种子抽取序列钉死；deck 递减补充语义；外观洗牌双射 + 同种子一致；
  Item 堆叠/等级语义。

## 进度

- [x] ItemKind + 数据表（`src/items/kinds.rs`）
- [x] Item 实例（`src/items/item.rs`）
- [x] Generator 掉落表（`src/items/generator.rs`）
- [x] 外观洗牌（鉴定系统底座，`src/items/identification.rs`）
- [x] 测试（36 个，`cargo test items::` 全绿；含权重逐值对拍/固定种子钉死/
  deck 递减补充/洗牌双射）

## 实现笔记（一期交付）

### 模块结构

- `src/items.rs` 入口 re-export；`src/items/random.rs` 为 SPD `Random.java`
  语义子集（`float`/`int`/`int_range`/`chances` + `ItemRng = ChaCha12`），
  按域边界独立实现，不 import `levels::random`。
- 全部纯 struct/enum，不依赖 Bevy `World`；`Dungeon.depth` 全局改为显式
  `depth` 参数（仅金币数额 `Gold.java` L91 与 `floor_set = depth / 5` 使用）。

### deck 机制细节（`Generator.java` 逐行对照）

- **类目层**：`categoryProbs`（L621-L623）= 两副 35 张类目牌
  （first/second prob，L222-L252）；每抽扣 1（L682），`chances` 和 ≤ 0 时
  **不消耗随机数**直接换副补满（L677-L681；`Random.java` L184-L186 先判和）。
- **类目内层**：deck 类目抽取走**类目私有流**（L711-L727）——`fullReset`
  时从外部流取 `seed`（L632），每次抽取从 `seed` 重建流、跳过 `dropped` 个
  `Long`（L713）再 `chances`；抽空在**同一条私有流**上重置补牌再抽
  （L717-L720）；完成后 `dropped += 1`。因此类目内序列只取决于
  `seed + dropped + probs`，与外部随机时序无关（有测试钉死）。
- 药水/卷轴的两副内牌在每次重置时交替（`reset` L645-L654）；
  `randomUsingDefaults` 用两副之和 `defaultProbsTotal`（L602-L609）。
- `random()` 抽中 SEED 走 `randomUsingDefaults`（L684-L688），不动 SEED 的
  deck 状态（种子主要掉落源是草丛）。
- 神器：抽空**不重置**返回 `None`，调用方回退发戒指（L706-L709/L855-L879）；
  `dropped` 无论是否抽中都自增（L866-L869 在 -1 检查之前）。
- 普通药水/卷轴经 `random(cat)` 生成时有一次 exotic 升格判定
  （L729-L737）：一期无异域水晶，概率恒 0（`ExoticCrystals.java` L48-L57），
  但**消耗一个 `Float` 的时序照抄**，保证随机流布局与 Java 一致。
- 实例掷取（`roll_item`）：武器/护甲/投掷 +0/+1/+2 = 75/20/5%、诅咒 30% 走
  `Random.Long()` 播种的独立流（`Weapon.java` L419-L449、`Armor.java`
  L654-L684）；法杖/戒指 +0/+1/+2 = 66.67/26.67/6.67%、诅咒 30% 直接当前流
  （`Wand.java` L546-L566、`Ring.java` L259-L278）；神器恒 +0 诅咒 30%
  （`Artifact.java` L218-L226）；金币数量 `IntRange(30+10d, 60+20d)`
  （`Gold.java` L89-L93）。

### 外观表来源（行号）

- 药水 12 色：`Potion.java` L90-L105 `colors` 表键序（crimson…ivory）。
- 卷轴 12 符文：`Scroll.java` L73-L88 `runes` 表键序（KAUNAN…TIWAZ）。
- 洗牌语义：`ItemStatusHandler.java` L42-L62——按种类序
  `Random.Int(剩余数)` 取外观并**保序移除**（`ArrayList.remove` 语义，
  影响洗牌结果，已照抄）；`known` 集合查询对应 L187-L215。
- 外观 → 精灵图编号（`ItemSpriteSheet.*`）属渲染域，一期只建枚举。

### 与 Java 的已知差异

- 随机数**位流**不与 `java.util.Random` 对齐（本工程 ChaCha12 自洽确定），
  但消耗结构（何时取几个数、走哪条流）逐行照抄。
- `Generator.java` L450 上游笔误（`WEP_T3.probs` 初始化为 `WEP_T1` 的表）
  未复刻：`fullReset` 后不可观测。
- `undoDrop` L664 的 `cls.isAssignableFrom(cat.superClass)` 前置过滤疑似
  写反（对具体物品类恒 false），按内层 `cls == cat.classes[i]`（L667）的
  意图实现。
- 药水/卷轴实际各 **12** 种（本计划文档旧文案"13 种"系笔误；法杖为 13）。

### 一期未覆盖（效果域 / M4-M5）

- 使用效果全部（饮用/阅读/进食/施放）、食物饱食度、法杖充能
  （`Wand.java` L558 `curCharges += n`）、附魔/铭刻（`Weapon.java`
  L442-L444、`Armor.java` L677-L679）、exotic 变体替换表、
  饰品概率乘数（`ParchmentScrap`/`ExoticCrystals` 等级）、
  投掷武器 `setID`/耐久共享（`MissileWeapon.java` L185-L187）、
  价格的诅咒/等级/附魔修正（`MeleeWeapon.java` L395-L407 等，现为
  基准价 × 数量）、背包/掉落堆/商店、存档（`storeInBundle` 系列）。
