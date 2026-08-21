# 23 · 视野与迷雾计划（M3 第二阶段）

**文件所有权**：`src/render/**` + `src/render.rs`（增量）。
禁止改：`main.rs`、`Cargo.toml`、`src/actors/**`（22 号域并行中，只消费其公开的
`Hero`/`GridPos`）、`src/levels/**`、`src/scenes/**`、`src/utils/**`（只消费
`cast_shadow`）、其他共享文件。

**参考实现**：
- `core/.../levels/Level.java`：`updateFieldOfView`（英雄视距、`viewDistance`、
  盲/心灵视觉等修正——基础版只做距离 + `ShadowCaster`）、`visited[]` 语义
- `core/.../scenes/GameScene.java` + `tiles/FogOfWar.java`：三态迷雾
  （可见/已探索/未知）的视觉语义（FogOfWar 的平滑网格不移植，用 TileColor 三态）
- `SewerLevel` 视距：正常 8（`Level.viewDistance`），Dark feeling 降为 2
  （`Dungeon.newLevel` 中设置——检查 Java 实际位置并对照）

## 目标

1. `VisibilityMap` 资源（`src/render/visibility.rs`）：`visible: Vec<bool>` +
   `visited: Vec<bool>`，尺寸随 `Level`；英雄 `GridPos` 变化（`Changed<GridPos>`
   + `resource_exists`）时用 `utils::cast_shadow`（`los_blocking` 数组）重算
   visible，并把 visible 并进 visited。换层（`Level` 替换）时重建。
2. 地形 tilemap 三态染色：visible = 原色；visited 不可见 = 加深
   （`TileColor` 乘 0.45 左右）；未探索 = 全黑（或 `TileVisible(false)`，
   查 bevy_ecs_tilemap 0.19 取舍并在笔记说明）。只染你自己的 terrain tilemap。
3. 视距：正常 8，`Feeling::Dark` = 2（Level.feeling 已有；对照 Java 行号）。
4. 公开 API：`VisibilityMap` pub + `is_visible(pos)`，供下波协调者做
   "怪物标记遮蔽"（本波不跨域接线，22 号域并行中）。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿。
- 新增测试：`VisibilityMap` 重算正确（空场半径对拍 `rounding` 表；墙后不可见；
  visited 单调增长；换层重置）；Dark feeling 视距 2；TileColor 三态映射纯函数
  单测。集成测试用 MinimalPlugins + 手工插入 Level/模拟 GridPos 变更
  （22 号域并行中，不 import 其 Hero 组件的话可以自建同名测试组件？不行——
  必须消费 actors 公开的 `Hero`/`GridPos` 真类型，它们已在 `actors.rs`
  re-export，稳定可用）。

## 进度

已并入 26 号渲染二期执行完成（实现细节见
`docs/plans/26-render-phase2.md` 实现笔记·迷雾一节）。

- [x] VisibilityMap + 重算（`src/render/visibility.rs`，`cast_shadow` +
      Level 变更/`Changed<GridPos>` 双触发；公开 `is_visible(cell)`）
- [x] 三态染色（`fog_color` 纯函数写 `TileColor`，水面+地形两层同染；
      未知格用全黑而非 `TileVisible(false)`——后者已被水格 needsRender 占用）
- [x] Dark 视距（8 / Dark 2，Java 行号注释在常量处）
- [x] 测试（空场圆形对拍 rounding 表、墙后阴影、visited 单调、换层重置、
      Dark 视距、无英雄安全，均手工铺图）
