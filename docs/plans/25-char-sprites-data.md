# 25 · 角色精灵动画数据计划（纯数据层，下波接线）

**文件所有权**：新建 `src/sprites.rs` + `src/sprites/` 目录；`main.rs` 唯一豁免
一行 `pub mod sprites;`。禁止改其他一切共享文件与他域目录。

**参考实现**：
- `SPD-classes/.../noosa/MovieClip.java` + `TextureFilm.java`：帧网格与动画剪辑
  （fps、frames、looped）的数据语义
- `core/.../sprites/CharSprite.java`：idle/run/attack/die 四剪辑约定
- `core/.../sprites/{HeroSprite,RatSprite,SnakeSprite,CrabSprite}.java`：
  各自的 `texture(...)` 帧尺寸与 `frames(...)` 索引序列（逐个照抄，行号注释）
- 图集：`assets/sprites/{warrior,rat,snake,crab}.png`（`SpriteType` 枚举已有）

## 目标（纯数据 + 换算，不做 Bevy 动画系统——那是下波接线）

1. `FrameGrid`：贴图尺寸 + 帧宽高 → 帧索引/UV 矩形换算（`TextureFilm` 等价，
   行主序与 16 号域 `tile_sheet` 的既有约定一致）。
2. `AnimClip`：fps + 帧序列 + looped；`CharAnimSet { idle, run, attack, die }`。
3. 数据表：Warrior（HeroSprite 帧布局：注意英雄图集含六职业行/装备变体行，
   只取战士基础行）、Rat、Snake、Crab 四套 `CharAnimSet`（Java 行号）。
4. 提供 `bevy_sprite` 可直接消费的换算：帧索引 → `TextureAtlasLayout` 的
   `URect`（或裁剪 `Sprite.rect`），单测对拍手算像素矩形。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿。
- 新增测试：帧网格换算（首帧/末帧/换行边界的像素矩形手算对拍）；四套动画表
  的帧数/ fps 与 Java 逐值断言（行号注释）；剪辑循环语义。

## 进度

- [x] FrameGrid/AnimClip 结构
- [x] 四套动画数据表
- [x] UV 换算 + 测试

## 实现笔记（2026-08）

**交付物**：`src/sprites.rs`（入口 + `CharSpriteSheet` 聚合）、
`src/sprites/film.rs`（`FrameGrid`）、`src/sprites/clip.rs`
（`AnimClip`/`CharAnimSet`）、`src/sprites/tables.rs`（四套表 +
`CharSpriteKind` + `hero_tier_grid`）；`main.rs` 豁免行 `pub mod sprites;`。
15 个 `sprites::` 测试全绿。

**英雄图集行布局**（勘误：不是"六职业行"共一张图）：六职业各有一张
256×128 的**同构**图集（`HeroClass.spritesheet()` L296-L311；正因同构，
Java `tiers()` L170 固定拿 ROGUE 贴图切行条）。单张图集内是**护甲阶行条**：
`tiers()`（`HeroSprite.java` L168-L175）按"全宽 × 15px"切行，行号 =
`Hero.tier()`（`Hero.java` L454-L462）：0 无甲、1-5 护甲阶、6 职业甲
（128/15 = 8 条，第 8 条闲置）；`updateArmor()` L73 再在行条内按 12×15
切出 21 帧（0-20：四剪辑用 0-15，operate 16-17、fly 18、read 19-20）。
战士基础行 = tier 0，通用换算封装在 `hero_tier_grid(tier)`——换甲/换职业
只是换行条/换贴图句柄，网格结构不变。

**非满行处理**：照抄 `TextureFilm` 整除截断语义（L62-L63），右/下余量
不成帧——rat 256×64@16×15 → 16×4 帧（底部 4px 弃）；snake 256×16@12×11
→ 21×1 帧（右 4px、底 5px 弃）；英雄行条 256@12 → 21 帧（右 4px 弃）。
帧矩形为整数像素 `URect`（min 含/max 不含，图像坐标 y 向下），归一化
UV 交给 bevy 内部，`frame_rect` 越界索引显式 panic（Java 是 null→NPE）。
贴图尺寸 Java 运行期读取、Rust 侧为常量，靠测试解析 PNG IHDR 头对拍防漂移。

**下波接线建议**：
1. 图集句柄经 `CharSpriteKind::sprite_type()` → `SpritesCollection::get`；
   `FrameGrid::atlas_layout()` 产出 `TextureAtlasLayout`（帧索引即 atlas
   索引），或轻量路线直接 `sprite_rect(frame)` 写 `Sprite.rect`。
2. 播放系统照抄 `MovieClip.updateAnimation`（L57-L91）：计一个
   `frame_timer`，超过 `AnimClip::frame_delay()` 推帧；`looped` 播完回 0，
   否则停末帧并发完成事件（对应 `listener.onComplete`，attack 播完回
   idle 的约定见 `CharSprite.onComplete` L841-L862）。
3. die 不可被打断（`CharSprite.play` 覆写 L136-L142）；朝向翻转是
   `flipHorizontal`（`turnTo` L284-L292），对应 `Sprite.flip_x`。
4. operate/zap/fly/read 等扩展剪辑帧序已在 Java 源就位（`HeroSprite`
   L87-L96），需要时在 `CharAnimSet` 外补挂即可，帧网格无需改动。
