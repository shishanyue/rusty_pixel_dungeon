# 02 · 里程碑路线图

原则：每个里程碑结束时 `cargo check`/`clippy`/`test` 全绿，游戏可运行到该里程碑的
可见目标。带 ⚡ 的工作项可由多智能体并行（文件所有权互斥）。

## M0 · 地基（本次会话执行，串行）

依赖重建（去 solarborn、Bevy 0.19）、核心类型冻结（Terrain/Level/AppState/资产集合）、
死代码清理、编译恢复绿色。详见 [03-foundation.md](03-foundation.md)。

## M1 · 四域并行 ⚡（本次会话派发）

| 域 | 计划 | 独占目录 | 可见目标 |
| --- | --- | --- | --- |
| 关卡生成 | 10 | `src/levels/**` | 种子可复现地生成下水道关卡，单测验证连通性 |
| 回合调度 | 11 | `src/actors/**` | Actor 时间轮纯逻辑 + 单测（spend/postpone/优先级） |
| i18n 消息 | 12 | `src/assets/**` | `Messages::get("rat.name") → "老鼠"`，语言回退链单测 |
| 场景框架 | 13 | `src/scenes/**` | 启动 → 标题画面（title 四层视差图 + 像素字体） |

## M2 · 集成竖切

把 M1 四域缝合：标题 → 开新游戏 → 生成 1 层 → ASCII/调试渲染 → 英雄占位实体
进入时间轮。协调者执行，主要改 `main.rs` 与跨域胶水。

## M3 · 渲染

bevy_ecs_tilemap 0.19 铺地形图集（tiles_sewers.png 布局对照 SPD `DungeonTileSheet`）、
迷雾(FogOfWar)、镜头跟随、点击寻路（SPD `PathFinder` 移植）。

## M4 · 角色与战斗

Char 数值组件（HP/命中/闪避/护甲）、Hero 四职业起步数值、Rat/Snake/Crab 三种怪、
SPD 命中公式与伤害滚动、死亡与经验。

## M5 · 物品与背包

Item 基架、背包 UI、金币/食物/药水(未鉴定机制)/武器防具穿戴、地面拾取与丢弃。

## M6 · 存档

serde 序列化 Dungeon/Level/actors；评估 moonshine_save 或手写 RON。

## M7 · 音频与打磨

音乐分层播放（SPD Music.java 的 track 系统）、音效表、设置界面（音量/语言）。

## M8 · 内容扩展

按 SPD 区域推进：下水道完整（5 层 + Goo Boss）→ 监狱 → 矿洞 → 城市 → 地狱。
每区域 = 生成器变体 + 怪物表 + 专属机制，结构在 M1–M5 已定型后此阶段是纯内容量产。
