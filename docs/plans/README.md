# Rusty Pixel Dungeon 重构计划总纲

本目录维护 Shattered Pixel Dungeon（下称 SPD，Java 参考实现位于
`/home/shishanyue/GitHub/shattered-pixel-dungeon`，版本 v3.3.8）的 Rust/Bevy 重写计划。
所有参与开发的智能体与人工贡献者以本目录为唯一事实来源。

## 文档索引

| 文档 | 内容 | 状态 |
| --- | --- | --- |
| [00-audit.md](00-audit.md) | 现状审计报告（问题清单与结论） | ✅ 已完成 |
| [01-architecture.md](01-architecture.md) | 目标架构：Java OOP → Rust ECS 映射 | ✅ 已定稿 |
| [02-roadmap.md](02-roadmap.md) | 里程碑路线图 M0–M8 | ✅ 已定稿 |
| [03-foundation.md](03-foundation.md) | M0 地基：依赖重建 + 编译恢复 | ✅ 已完成（全绿） |
| [10-level-generation.md](10-level-generation.md) | 关卡生成域（房间/构建器/画师） | ✅ 已交付（32 测试，百种子连通性验证） |
| [11-turn-scheduler.md](11-turn-scheduler.md) | 回合调度域（Actor 时间轮） | ✅ 已交付（48 测试全绿） |
| [12-i18n-messages.md](12-i18n-messages.md) | 本地化与消息域 | ✅ 已交付（含编码缺陷修复） |
| [13-scenes-ui.md](13-scenes-ui.md) | 场景与 UI 框架域 | ✅ 已交付（标题画面 + 状态闭环） |
| [14-pathfinding-fov.md](14-pathfinding-fov.md) | 寻路与视野域（M3 前置） | ✅ 已交付（14 测试，OpenJDK 基准对拍） |
| [16-tilemap-render.md](16-tilemap-render.md) | 图集渲染域（M3） | ✅ 已交付（39 地形平面映射，拼接二阶段 TODO） |
| [17-hero-slice.md](17-hero-slice.md) | 英雄可玩竖切（M2 收尾 + M4 前置） | ✅ 已交付（8 向移动/回合消耗/下楼闭环） |
| [18-audio-music.md](18-audio-music.md) | 音乐音频域（M7 提前独立项） | ✅ 已交付（状态驱动 BGM，防叠播） |
| [19-levelgen-phase2.md](19-levelgen-phase2.md) | 关卡生成二期（水/草/氛围/FigureEight） | ✅ 已交付（44 测试，Patch 噪声对拍） |
| [21-items-data.md](21-items-data.md) | 物品数据域一期（掉落表/鉴定底座） | ✅ 已交付（166 种物品表 + deck 掉落语义） |
| [22-mobs-combat.md](22-mobs-combat.md) | 怪物 AI 与战斗接线（M4 主体） | ✅ 已交付（三态 AI + 撞击战斗 + 经验升级） |
| [23-fov-fog.md](23-fov-fog.md) | 视野与迷雾（M3 二阶段） | ✅ 已交付（并入 26 号完成） |
| [24-levelgen-phase3.md](24-levelgen-phase3.md) | 关卡生成三期（房间变体/特殊房） | ✅ 已交付（5 变体 + 特殊/密室框架，70 测试） |
| [25-char-sprites-data.md](25-char-sprites-data.md) | 角色精灵动画数据（纯数据层） | ✅ 已交付（四套动画表 + PNG 头对拍） |
| [26-render-phase2.md](26-render-phase2.md) | 渲染二期：贴图修复+拼接+迷雾 | ✅ 已交付（截图两类错误已修复） |
| [15-char-combat-core.md](15-char-combat-core.md) | 角色战斗纯核（M4 前置） | ✅ 已交付（20 测试，公式对拍 Java 行号） |
| [20-backlog.md](20-backlog.md) | 后续领域待办（渲染/战斗/物品/存档等） | 📋 规划 |
| [90-quality.md](90-quality.md) | 质量门禁与测试策略 | ✅ 已定稿 |

## 协作约定（多智能体并行）

1. **领域隔离**：每个域计划声明其独占的源码目录（见各计划"文件所有权"节）。
   智能体只能改动自己域内文件，禁止改 `src/main.rs`、`Cargo.toml`（由协调者统一集成）。
2. **接口先行**：跨域类型（`Terrain`、`Level`、`AppState`、资产集合）由 M0 地基固定，
   域内代码只消费不修改；需要变更接口时在计划文档中记录提案，由协调者裁决。
3. **绿色基线**：任何改动后 `cargo check` 必须通过；带 `cargo test` 的域必须全绿。
4. **状态回写**：域完成后更新本 README 状态列与对应计划文档的"进度"节。

## 一次性决策记录（ADR 摘要）

- **Bevy 0.19**：原项目锁 0.18 且依赖已消失的本地库 `solarborn`（硬阻断）。
  本机完整 0.19 生态（`~/GitHub/bevy` 等本地源码 + 技能文档）→ 直接升级 0.19，
  移除 `solarborn`，改为直连官方 crates。
- **音频**：改用 Bevy 内置 `bevy::audio`（vorbis 默认 + mp3 特性），移除 kira 间接依赖。
- **JSON 资产**：自写 ~40 行 `AssetLoader` 取代 `bevy_common_assets`（少一个版本耦合）。
- **随机数**：`rand 0.10`（`SmallRng` 起步）；地牢种子确定性方案在关卡生成域内落地。
- **资产加载**：9 个串行加载状态合并为单一 `AppState::Loading`（bevy_asset_loader
  0.27 单状态多集合），入口状态机简化为 `Loading → Title → InGame`。
- **proc-macro crate 移除**：`crates/macros` 为旧项目遗留死代码（引用不存在类型），删除。
