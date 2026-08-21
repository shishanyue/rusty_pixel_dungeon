# 11 · 回合调度域计划

**文件所有权**：`src/actors/`。禁止改 `main.rs`/`Cargo.toml`/其他域目录
（`src/actors.rs` 模块入口由地基预挂，可改其内容）。

**参考实现**：`shattered-pixel-dungeon/core/.../actors/Actor.java`。

## SPD 语义（必须保真）

- 全局时钟 `now: f32`；每个 Actor 有 `time`（下次行动时刻）与 `actPriority`
  （tie-break，数值大者先行：VFX > Hero > 默认…见 Actor.java 常量）。
- `process()` 循环：取 `time` 最小者（同 time 比 priority），推进 `now = actor.time`，
  调 `act()`；`act()` 返回 false 表示等待外部输入（英雄），循环挂起。
- `spend(t)` 把 `time += t`（受冰冻/加速修饰的在 Char 层，M4 再说）；
  `postpone(t)` 设 `time = max(time, now + t)`；`TICK = 1.0`。
- 时间精度：SPD 对 `time` 做 `Math.round(time*1000)/1000` 防浮点漂移——保留该细节。

## ECS 设计

- 纯逻辑核 `scheduler.rs`：`TurnClock { now: f32 }` + 排序选择函数
  （输入 `(id, time, priority)` 列表，输出下一个行动者），零 Bevy 依赖，单测在此层。
- Bevy 适配：`Component Actor { time: f32, priority: i32 }`；
  `Resource TurnState { Waiting(输入等待) | Processing }`；
  `process_turns` 系统在 `Update` 内循环推进，直到英雄待输入或帧预算耗尽
  （SPD 也有每帧限额逻辑）。行动分发用 `EntityEvent`（0.19 观察者 `On<E>`），
  M1 先用一个 `DummyActor`（固定 spend(TICK)）验证轮转。
- **注意 0.19**：缓冲事件是 `Message`/`MessageReader`；观察者事件是 `Event`/`On<E>`，
  不要按 0.18 记忆写。

## 验收

- 单测：三个不同 time/priority 的 actor 轮转顺序与 SPD 语义一致；spend/postpone
  边界（相同 time 比 priority、postpone 不回退）；千步推进 `now` 无漂移
  （0.001 取整生效）。
- Bevy 集成测试（`App::update` 无渲染跑若干帧）：DummyActor × 3 轮转计数正确，
  `TurnState::Waiting` 时不推进。

## 进度

- [x] scheduler.rs 纯逻辑 + 单测
- [x] Bevy 组件/资源/系统适配
- [x] DummyActor 集成测试

## 实现笔记（M1 交付）

文件：`src/actors.rs`（入口/Plugin/re-export）、`src/actors/scheduler.rs`（纯核）、
`src/actors/turn.rs`（Bevy 适配）、`src/actors/dummy.rs`（哑元行为）。

### priority 常量表（Actor.java L48-53，tie 时数值大者先行，L55）

| 常量 | 值 | 说明 |
| --- | --- | --- |
| `VFX_PRIO` | 100 | 视觉特效最先 |
| `HERO_PRIO` | 0 | 正值在英雄前、负值在后 |
| `BLOB_PRIO` | -10 | 英雄后、怪物前 |
| `MOB_PRIO` | -20 | |
| `BUFF_PRIO` | -30 | 一回合内最后的常规类别 |
| `DEFAULT_PRIO` | -100 | Java 私有 `DEFAULT`，兜底 |

time 与 priority 完全平手时 Java 遍历 `HashSet` 顺序不定（L260 严格 `>` 保留先
遍历者）；本移植钉死为迭代序（Bevy 即同原型内生成序），属确定性增强。

### 取整细节（Actor.java L63-67 / L82-86）

`|time % 1| < 0.001` 时 `round()`。注意这是**单侧**判定：只吸收略高于整数的值
（5.0004 → 5.0），略低者（2.9999998）原样保留——已用测试钉死，勿"修正"。实测
千次 spend(1/3) 终值误差 1e-5（无取整为 7.7e-4 且线性增长），但瞬时误差可达
~2e-3 且大量级下 f32 ulp 逼近阈值，Java 完全相同——SPD 靠 `fixTime()` 定期拉回
小量级，该函数留待 M4 随 GameScene 对应物一起移植（同批：`spendToWhole`/
`clearTime`/`delayChar`，M1 无调用方，未提前实现）。与 Java 唯一已知差异：
`Math.round(float)` 返回 int 会把 `time ≈ f32::MAX`（失活）饱和到 `i32::MAX`，
Rust `f32::round` 保值（失活者保持失活，更合理，SPD 从不依赖该饱和）。

### 分发方案取舍

选 **`EntityEvent` + 观察者**：`process_turns`（独占 `&mut World` 系统）循环内
`world.trigger(ActTurn { entity })` 同步分发，行为域各自 `add_observer` 并按标记
组件过滤（`dummy.rs` 即模板）；"act() 返回 false" ≙ 观察者置
`TurnState::WaitingForInput`，输入侧置回 `Processing` ≙ `next()` + 唤醒。取舍：
观察者对 M4 新行为零侵入、可拿全量世界副作用；代价是调度系统独占 World（回合
推进本就全局串行，无并行损失）。弃用 trait 对象组件方案：act 需要任意世界副作
用，迭代查询时拿不到 `&mut World`，只能回传命令枚举导致接口膨胀且重回 OOP 风
格。另有两处 SPD 没有的防御：单帧行动预算 `MAX_ACTS_PER_UPDATE = 100`（SPD 的
actor 线程可自旋，Bevy 在帧内循环必须封顶）与活锁保护（行动后未花时间/未挂起/
未离场即断帧并告警；Java 同场景是 actor 线程死循环）。
