# 15 · 角色与战斗纯核计划（M4 前置）

**文件所有权**：`src/actors.rs`（增量，不破坏调度域已交付内容）与 `src/actors/` 新文件。
调度域（11 号）已交付并移交所有权；`scheduler.rs`/`turn.rs`/`dummy.rs` 只许消费，
除非有充分理由并在笔记中说明。禁止改 `main.rs`/`Cargo.toml`/其他域目录。

**参考实现**（core/.../actors/ 下）：
- `Char.java`：`hit()` 命中公式、`attackSkill/defenseSkill`、`damageRoll`、`drRoll`、
  `attackDelay`、`speed`
- `hero/Hero.java` + `hero/HeroClass.java`：初始数值（HP/STR）、升级曲线
  `lvlUp`、命中/闪避随等级成长
- `mobs/Mob.java`（数值相关部分）+ `mobs/{Rat,Snake,Crab}.java`：三种下水道怪的
  完整数值表（HP/EXP/maxLvl/攻防/伤害域/护甲）

## 目标（纯逻辑，不接输入/AI/渲染——那些是 M4 集成的事）

1. `src/actors/char_stats.rs`：`CharStats` 组件（hp/ht/attack_skill/defense_skill/
   damage_range/armor_range/speed/…），Bevy `Component` 但逻辑函数全部纯化。
2. `src/actors/combat.rs`：`hit(attacker, defender, acc_multi, rng)`、`damage_roll`、
   `dr_roll` ——公式与 Java 逐行对照（注意 SPD 的 `Random.Float()` 半开区间语义与
   命中公式里 attacker/defender roll 的具体形态）。
3. `src/actors/bestiary.rs`：Hero 四职业初始数值表 + Rat/Snake/Crab 数值表
   （const fn / const 表，来源行号注释）。
4. 随机源显式传参（`&mut impl Rng`），与项目确定性纪律一致。

## 强制验收

- 纯单测：
  - 固定种子下 hit/damage/dr 的取值与手算 Java 语义一致（测试注释标行号与手算过程）；
  - 命中公式边界：acc=0、evasion=0、acc_multi 缩放；
  - 数值表抽查（Rat HP=8 等，以 Java 源为准逐项断言）。
- `cargo check/clippy/test` 全绿，不破坏既有 48 个测试。

## 进度

- [x] CharStats 组件（`char_stats.rs`：`CharStats`/`StatRange` + `is_alive`/`take_damage` 纯方法）
- [x] 命中/伤害/护甲公式（`combat.rs`：`hit`/`hit_with_skills`/`damage_roll`/`dr_roll` + SPD 随机等价工具）
- [x] Hero/三怪数值表（`bestiary.rs`：四职业出生数值与成长曲线 const fn、Rat/Snake/Crab const 表）
- [x] 对拍单测（20 个：脚本化随机源手算对拍、边界、数值表逐项断言；`cargo test actors::` 34 全绿）

## 实现笔记（交付时状态）

### 随机语义等价方案

- 随机源显式传 `&mut impl Rng`（rand 0.10），生成器与 Java `java.util.Random`
  不同，**不做位流对齐**；对齐的是公式形态：掷值区间开闭、掷值次数与顺序、
  乘数施加位置、`(int)` 向零截断、f32 运算序（Java `float` 算术同为
  IEEE-754，给定相同 `[0,1)` 掷值序列时结果逐位一致）。
- `Random.Float()`（Random.java L77-79）↔ rand 0.10 f32 `StandardUniform`：
  两者同构（24 位分辨率 `k/2^24`，可取 0 永不取 1）。测试
  `word_to_f32_mapping_is_pinned` 钉住 rand 的 `(u32 >> 8) / 2^24` 映射，
  rand 升级若变实现会先在该测试报警。
- `Random.NormalIntRange(min, max)`（L138-140）：闭区间三角分布，照抄
  `(int)((f1 + f2) * span / 2)` 运算序；**恒消耗两次** `Float()`（即使
  `min == max`，区别于 `Random.Int(max <= 0)` 的零消耗）。
- 手算对拍用测试本地的 `ScriptedRng`（按脚本出 u32 字），可逐位复算；
  另有 ChaCha12 固定种子确定性 + 多种子值域/端点可达性测试。
- 与 `levels::random` 的工具**有意重复**：文件所有权是硬边界且两域并行开发，
  公式与对拍测试必须留在本域；utils 级统一留给协调者裁决。

### 公式形态（Char.java 行号）

```text
hit（L624-690 纯核）：
  defStat >= 1_000_000 → miss     // L643-645，无限闪避优先、零掷值
  acuStat >= 1_000_000 → hit      // L646-649，零掷值
  acuRoll = Float(acuStat) * accMulti   // L651 掷值 + L665 乘数（先掷后乘）
  defRoll = Float(defStat)              // L667，掷序固定先攻后守
  hit ⇔ acuRoll >= defRoll              // L683，平手判中（acc=0 vs eva=0 → 中）
damage_roll = NormalIntRange(damage_range)   // Rat L54-57 / Snake L48-51 / Crab L45-48 / 徒手英雄 RingOfForce L105
dr_roll     = NormalIntRange(armor_range)    // Rat L64-67 / Crab L55-58 / Snake 与裸英雄 (0,0)
```

### 与 Java 的差异清单

1. **Buff/天赋/饰品乘子未移植**（M4）：hit 的 Bless/Hex/Daze/冠军怪/登顶/
   雪貂草（L652-681）、偷袭必中（L633-635）、`speed()` 与 `attackDelay()`
   的全部修饰因子。`CharStats` 承载的是已折算基础值。
2. **随机流掷数差异**（取值分布不变，仅流位置不同，M4 接入对应系统时补掷）：
   - Java `Char.drRoll`（L706-712）恒含 Barkskin 项 `NormalIntRange(0, 0)`
     （无 Buff 时仍消耗两掷，Barkskin.java L119-125）；纯核 `dr_roll` 不掷该项。
   - Java `Hero.heroDamageIntRange`（Hero.java L699-705）先掷一次 `Float()`
     与三叶草概率比较（无饰品时恒不触发）；纯核 `damage_roll` 不掷该次。
3. **动态折算入口未移植**：`Hero.attackSkill/defenseSkill` 的 `max(1, round(…))`
   下限（L555-557/L604）与 `Mob.defenseSkill` 被偷袭/麻痹归 0（L698-705）
   属装备/AI 域；表函数返回的裸值在基线下与折算值相等（已注释说明）。
4. **职业枚举只含四经典职业**：SPD 现版本另有 DUELIST/CLERIC
   （HeroClass.java L91-92），裸数值与四职业相同（init 只发装备），
   M4 物品域接入时再开放枚举项。
5. `hit` 的 `magic` 参数未保留：Java 布尔参数在函数体内无用（仅便捷重载
   L620-622 用它选 accMulti = 2），纯核以 `acc_multi = 2.0` 表达魔法命中。

### 数值表来源（逐项行号见 `bestiary.rs` 注释与测试断言）

| 条目 | HP/HT | 命中 | 闪避 | 伤害域 | 护甲域 | 速度 | EXP | maxLvl |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Rat（Rat.java L33-67） | 8 | 8 | 2 | 1-4 | 0-1 | 1 | 1 | 5 |
| Snake（Snake.java L35-56） | 4 | 10 | 25 | 1-4 | 0-0 | 1 | 2 | 7 |
| Crab（Crab.java L31-58） | 15 | 12 | 5 | 1-7 | 0-4 | 2 | 4 | 9 |
| Hero ×4（Hero.java L199-252） | 20 | 10 | 5 | 1-2（徒手 STR10） | 0-0 | 1 | — | — |

成长曲线（Hero.java）：命中 `10+(lvl-1)`（L213/L2032）、闪避 `5+(lvl-1)`
（L214/L2033）、`HT = 20+5*(lvl-1)`（L257）、升级经验 `5+lvl*5`（L2071-2073）、
`MAX_LEVEL = 30`（L199）、`STARTING_STR = 10`（L201）。
