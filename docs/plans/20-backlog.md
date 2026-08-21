# 20 · 后续领域待办（M2+）

按里程碑排队，M1 四域合流后逐个立项（届时从本文件拆出独立计划文档）。

## M2 · 集成竖切（协调者）

- [x] 标题"进入地牢" → 熵种子 + `generate_level(seed, dungeon.depth)` → `Level`
  资源插入 → InGame 彩色方块调试渲染 + HUD（seed/depth）→ Esc 返回并清理
  （2026-08-13，含集成测试 `in_game_creates_and_tears_down_level`）。
- [x] 英雄进时间轮 + 8 向回合制移动 + 下楼闭环（17 号域交付，2026-08-13）。
- [x] 真实图集地形渲染（16 号域交付）；状态驱动 BGM（18 号域交付）。
- [x] 移除调试方块视图（协调者，2026-08-13）。
- [x] Esc 重进重置新一局（协调者：`OnEnter(InGame)` 调 `Dungeon.init` + 测试）。
- [x] 怪物标记的迷雾遮蔽（协调者：`sync_mob_markers` 接 `VisibilityMap`，
  对照 SPD `heroFOV[mob.pos]` 语义，2026-08-14）。
- [ ] HUD 文案接 12 号域 `Messages`（目前硬编码英文常量）。
- [ ] 英雄/怪物真精灵动画接线（25 号域数据表已就绪，替换调试色块）。
- [ ] Raised 透视墙三期（26 号域图集常量已就位，算法待接）。
- [ ] 水面滚动动画（26 号域留 TODO）。
- [ ] 锁门钥匙投放（24 号域特殊房已上锁，钥匙归物品域接线）。

## M3 · 渲染域

- `bevy_ecs_tilemap = "0.19"`（届时协调者加依赖）。
- SPD `DungeonTileSheet.java` 的图集索引表移植（tiles_sewers.png 16×16 网格）；
  水面动画（water0.png 滚动 UV）、墙体拼接（wall stitching）规则。
- FogOfWar：SPD `fog.png` 混合模式的 tilemap 等价物；`ShadowCaster` 视野算法移植。
- 镜头：跟随英雄 + 像素完美缩放（`Camera2d` + 整数缩放）。
- 输入：点击格子 → `PathFinder.getPath`（SPD `utils/PathFinder.java` 的
  Dijkstra 距离图先移植到 `src/utils/`）。

## M4 · 角色与战斗域

- `Char` 数值组件族：HP/ATK skill/DEF skill/armor/speed；SPD 命中公式
  （`Char.hit()` 的 attack/defense roll）。
- Hero：四职业初始数值（`HeroClass.java`）、升级曲线（`Hero.lvlUp`）。
- Mob AI 状态机：SLEEPING/HUNTING/WANDERING/FLEEING（`mobs/Mob.java` 内部类）——
  评估直接手写 enum 状态机（不引 bevy_hsm，除非复杂度证明必要）。
- 首批怪：Rat、Snake、Crab（下水道），生成表 `Bestiary` 对应逻辑。
- Buff 框架：挂 Char 的时限组件 + 每回合 tick（对齐 Actor 时间轮）。

## M5 · 物品域

- `Item` 数据模型（SPD 继承树 → `ItemKind` 枚举 + 组件），背包 `Belongings`。
- 未鉴定机制（药水/卷轴的 handle 洗牌，`Generator.java` 掉落表）。
- 物品栏 UI（toolbar/inventory 窗口，对照 `windows/WndBag.java`）。

## M6 · 存档域

- `Dungeon`/`Level`/actors 的 serde 序列化；GamesInProgress 槽位语义。
- 候选：手写 RON vs moonshine_save（本地有 0.19 适配源码），做一次 spike 再定。

## M7 · 音频与设置域

- 音乐轨道系统（`Music.java` 分层播放/无缝切换）、音效节流。
- 设置界面：语言运行时切换（12 号域遗留 TODO）、音量、缩放。

## M8 · 内容量产

区域推进顺序：下水道(含 Goo) → 监狱(Tengu) → 矿洞(DM-300) → 城市(Dwarf King)
→ 地狱(Yog)。特殊房间（special/secret rooms）、陷阱表、植物、炼金按区域穿插。

## 技术债登记簿

| 项 | 来源 | 处置 |
| --- | --- | --- |
| I18N 性别/复数选择语法 `{0,choice,...}` | 12 号域 M1 简化 | M7 前补 |
| 运行时语言切换需重载消息集合 | 12 号域 | M7 设置界面一并做 |
| 标题视差滚动 | 13 号域 M1 静态 | M3 渲染域顺手做 |
| ~~SmallRng 跨版本种子稳定性~~ | 10 号域 | 已解决：rand 0.10 `chacha` 特性，ChaCha 种子跨平台稳定 |
| `.idea/`/`.vscode/` 入库 | 审计 D | 与用户确认后移入 gitignore |
