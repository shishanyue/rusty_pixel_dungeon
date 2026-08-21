//! 关卡生成入口：`generate_level(seed, depth)`。
//!
//! 一条 SPD 下水道普通层（depth 1–4 语义）流水线：
//! Feeling 掷点（`Level.create`）→ 房间集合（`RegularLevel.initRooms` +
//! `SewerLevel.standardRooms/specialRooms` + 密室预算）→
//! `LoopBuilder`/`FigureEightBuilder` 五五开摆放/连接 →
//! `RegularPainter`(下水道参数) 刻画水/草/陷阱 → [`Level`]。
//! 纯函数、不依赖 Bevy `World`，同种子同深度输出逐格一致。
//!
//! # run 级池子的重放
//!
//! 特殊房队列与密室预算是**一局**的跨层状态（Java 静态字段，见
//! [`crate::levels::special`] 模块文档）。为保持纯函数签名，生成第 `depth`
//! 层前先把 1..depth-1 各层的 [`init_rooms`] 在各自层流上空跑一遍、
//! 只推进池子（`build`/`paint` 不触碰池子，无需重放）。

use rand::{Rng, RngExt, SeedableRng};

use crate::levels::{
    Feeling, Level,
    builder::{FigureEightBuilder, LoopBuilder},
    painter,
    random::{LevelRng, chances, float_range, int, shuffle},
    rooms::{Room, RoomKind},
    special::{RunPools, sewer_special_rooms_count},
    standard::{roll_size_category, roll_standard_variant},
};

/// 单次 `builder.build` 的失败重试上限。Java 是无限 `do{}while`
/// （RegularLevel.java L112-L118）；实测每次成功率很高，512 次全败只可能是
/// 逻辑坏死，直接 panic 暴露而非死循环。
const MAX_BUILD_ATTEMPTS: u32 = 512;

/// `RegularLevel.builder()` 二选一的结果（L176-L189），测试用于分支断言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuilderKind {
    Loop,
    FigureEight,
}

enum AnyBuilder {
    Loop(LoopBuilder),
    FigureEight(FigureEightBuilder),
}

impl AnyBuilder {
    fn build(&mut self, rng: &mut impl Rng, rooms: &mut Vec<Room>) -> bool {
        match self {
            AnyBuilder::Loop(b) => b.build(rng, rooms),
            AnyBuilder::FigureEight(b) => b.build(rng, rooms),
        }
    }

    fn kind(&self) -> BuilderKind {
        match self {
            AnyBuilder::Loop(_) => BuilderKind::Loop,
            AnyBuilder::FigureEight(_) => BuilderKind::FigureEight,
        }
    }
}

/// 生成一层下水道普通层。同 `(seed, depth)` 结果逐格确定。
///
/// 对照 `Level.create`（Level.java L215-L317）→ `RegularLevel.build`
/// （RegularLevel.java L104-L122）。
pub fn generate_level(seed: u64, depth: i32) -> Level {
    generate_level_impl(seed, depth).0
}

/// [`generate_level`] 的内部版本，额外返回 builder 分支供测试断言。
pub(crate) fn generate_level_impl(seed: u64, depth: i32) -> (Level, BuilderKind) {
    let mut pools = run_pools_before_depth(seed, depth);

    let mut rng = LevelRng::seed_from_u64(seed_for_depth(seed, depth));

    // Level.create L255-L292：Feeling 掷点先于 build()。
    // L222-L253 的限量物资掷点属物品域（未移植，不消耗随机数）。
    let feeling = roll_feeling(&mut rng, depth);

    let mut builder = roll_builder(&mut rng);

    // RegularLevel.build L109-L110：initRooms 只做一次、洗牌一次；重试仅重跑 builder
    let template = init_rooms(&mut rng, feeling, depth, &mut pools);

    for _ in 0..MAX_BUILD_ATTEMPTS {
        // L112-L117：每次尝试用初始房间列表的干净副本（连接/矩形全空）
        let mut rooms = template.clone();
        if builder.build(&mut rng, &mut rooms) {
            // L120：painter().paint(...)，水/草/陷阱参数与 nTraps 掷点在 painter 内
            return (
                painter::paint(&mut rng, &mut rooms, depth, feeling),
                builder.kind(),
            );
        }
    }
    panic!("关卡生成 {MAX_BUILD_ATTEMPTS} 次尝试全部失败：seed={seed} depth={depth}");
}

/// `RegularLevel.builder()`（L176-L189）：Int(2) 五五开，曲线参数照抄 L178-L186。
fn roll_builder(rng: &mut impl Rng) -> AnyBuilder {
    if int(rng, 2) == 0 {
        let intensity = float_range(rng, 0.0, 0.65);
        let offset = float_range(rng, 0.0, 0.50);
        AnyBuilder::Loop(LoopBuilder::new(2, intensity, offset))
    } else {
        // L183-L186：FigureEight 的 offset 是字面量 0，只消耗一次 Float
        let intensity = float_range(rng, 0.3, 0.8);
        AnyBuilder::FigureEight(FigureEightBuilder::new(2, intensity, 0.0))
    }
}

/// 把 run 池推进到"即将生成第 `depth` 层"的状态：
/// `Dungeon.init` 在 `seed+1` 流上 initForRun，随后每下潜一层，
/// 该层的 initRooms 从**该层的层流**消耗掷点并轮换池子。
/// 逐层空跑 [`init_rooms`] 即可精确复现（模板丢弃，只留池子副作用）。
fn run_pools_before_depth(seed: u64, depth: i32) -> RunPools {
    let mut pools = RunPools::init_for_run(seed);
    for d in 1..depth {
        let mut replay = LevelRng::seed_from_u64(seed_for_depth(seed, d));
        let feeling = roll_feeling(&mut replay, d);
        let _ = roll_builder(&mut replay);
        let _ = init_rooms(&mut replay, feeling, d, &mut pools);
    }
    pools
}

/// `Level.create` 的氛围掷点表（Level.java L255-L292）：仅 depth > 1；
/// `Int(14)` 的 0-6 号各 ~7.15%，其余 50% 无氛围。
///
/// 未移植的分支内效果：L270 DARK 视距（英雄/渲染域）、L274 LARGE 补给
/// （物品域）、L282-L290 default 分支的两次饰品覆写 Float（无饰品系统，
/// 不消耗随机数）。
pub(crate) fn roll_feeling(rng: &mut impl Rng, depth: i32) -> Feeling {
    if depth <= 1 {
        return Feeling::None;
    }
    match int(rng, 14) {
        0 => Feeling::Chasm,
        1 => Feeling::Water,
        2 => Feeling::Grass,
        3 => Feeling::Dark,
        4 => Feeling::Large,
        5 => Feeling::Traps,
        6 => Feeling::Secrets,
        _ => Feeling::None,
    }
}

/// `Dungeon.seedForDepth`（Dungeon.java L418-L431）：从世界种子推每层种子——
/// 以 seed 起一条流、跳过 depth 个值、取下一个（恒 branch=0）。
fn seed_for_depth(seed: u64, depth: i32) -> u64 {
    let mut rng = LevelRng::seed_from_u64(seed);
    let mut result = rng.random::<u64>();
    for _ in 0..depth.max(0) {
        result = rng.random::<u64>();
    }
    result
}

/// 标准房变体重抽上限。Java 是无上限 do-while（RegularLevel.java L136-L138）；
/// 权重表里恒有 NORMAL 档为正的变体，期望重抽次数 < 2，超限即逻辑坏死。
const MAX_VARIANT_REROLLS: u32 = 4096;

/// `RegularLevel.initRooms`（L124-L166）与 `SewerLevel.standardRooms`（L87-L92）、
/// `SewerLevel.specialRooms`（L94-L99）、`SecretRoom.secretsForFloor` 的合体。
/// Shop（L143-L144）未移植：下水道 1-4 层无商店（`Dungeon.shopOnLevel` 是
/// depth 6/11/16/21）。
fn init_rooms(rng: &mut impl Rng, feeling: Feeling, depth: i32, pools: &mut RunPools) -> Vec<Room> {
    let mut rooms = vec![Room::new(RoomKind::Entrance), Room::new(RoomKind::Exit)];

    // SewerLevel.standardRooms：forceMax（=LARGE）恒 6 不掷点（L89），
    // 否则 4 + chances({1,3,1}) ∈ [4,6]，均值 5（L91）。
    let mut standards = if feeling == Feeling::Large {
        6
    } else {
        4 + chances(rng, &[1.0, 3.0, 1.0]).unwrap_or(0) as i32
    };
    // RegularLevel.initRooms L129-L133：LARGE 氛围再乘 1.5 向上取整（6 → 9）
    if feeling == Feeling::Large {
        standards = (standards as f32 * 1.5).ceil() as i32;
    }

    // L134-L141：变体掷点 + 尺寸类别掷点；剩余预算 standards-i 截断尺寸表，
    // 全零（如预算 1 时抽到无 NORMAL 档的 CircleBasin）则重抽变体。
    // 大房间按 sizeFactor 折抵多个标准房名额。
    // Java 构造器的隐式 setSizeCat()（StandardRoom.java L54）结果必被显式
    // 覆盖，本移植不与 Java 流位对齐，省略该次消耗。
    let mut i = 0;
    while i < standards {
        let mut rerolls = 0;
        let (variant, size) = loop {
            rerolls += 1;
            assert!(rerolls <= MAX_VARIANT_REROLLS, "标准房变体重抽超限");
            let variant = roll_standard_variant(rng, depth);
            if let Some(size) = roll_size_category(rng, variant, standards - i) {
                break (variant, size);
            }
        };
        i += size.room_value();
        rooms.push(Room::new(RoomKind::Standard { variant, size }));
    }

    // L146-L156：特殊房数量（LARGE 氛围 +1），本层池克隆后逐个抽取。
    // 移植面只有 1 种特殊房：本层池耗尽时静默截断
    // （Java 池 19 种耗不尽，无此分支；见 docs/plans/24 实现笔记）。
    let mut specials = sewer_special_rooms_count(rng, feeling == Feeling::Large);
    if feeling == Feeling::Large {
        specials += 1;
    }
    pools.init_for_floor();
    for _ in 0..specials {
        if let Some(kind) = pools.create_special(rng) {
            rooms.push(Room::new(RoomKind::Special(kind)));
        }
    }

    // L158-L163：密室数量 = 域预算均摊（SECRETS 氛围 +1，不走预算）
    let mut secrets = pools.secrets_for_floor(rng, depth);
    if feeling == Feeling::Secrets {
        secrets += 1;
    }
    for _ in 0..secrets {
        rooms.push(Room::new(RoomKind::Secret(pools.create_secret(rng))));
    }

    // RegularLevel.build L110
    shuffle(rng, &mut rooms);
    rooms
}

/// 测试用：复现 [`generate_level_impl`] 在 build 之前的全部掷点，
/// 返回该层的氛围与洗牌后的初始房间模板（不执行摆放/刻画）。
#[cfg(test)]
pub(crate) fn level_template(seed: u64, depth: i32) -> (Feeling, Vec<Room>) {
    let mut pools = run_pools_before_depth(seed, depth);
    let mut rng = LevelRng::seed_from_u64(seed_for_depth(seed, depth));
    let feeling = roll_feeling(&mut rng, depth);
    let _ = roll_builder(&mut rng);
    let template = init_rooms(&mut rng, feeling, depth, &mut pools);
    (feeling, template)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::terrain::Terrain;
    use bevy::math::IVec2;
    use std::collections::VecDeque;

    /// 英雄可通行谓词：`passable` 或明陷阱。SPD 中 `Terrain.TRAP` 是 AVOID——
    /// 寻路回避但英雄可踩（暗陷阱 `SecretTrap` 本就 PASSABLE），
    /// 连通性以"能走到"为准。
    fn walkable(level: &Level, i: usize) -> bool {
        level.passable[i] || level.map()[i] == Terrain::Trap
    }

    /// 自 `from` 起按 [`walkable`] 4 邻接 BFS 泛洪，返回可达标记表。
    /// 锁门/密门是 SOLID（非 passable）——泛洪不会穿过它们。
    fn bfs_flood(level: &Level, from: IVec2) -> Vec<bool> {
        let mut seen = vec![false; level.size()];
        if !level.is_inside(from) {
            return seen;
        }
        let mut queue = VecDeque::from([from]);
        seen[level.index(from)] = true;
        while let Some(p) = queue.pop_front() {
            for d in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let n = p + d;
                if level.is_inside(n) {
                    let i = level.index(n);
                    if walkable(level, i) && !seen[i] {
                        seen[i] = true;
                        queue.push_back(n);
                    }
                }
            }
        }
        seen
    }

    /// 入口→出口，按 [`walkable`] 4 邻接 BFS。
    fn bfs_reachable(level: &Level, from: IVec2, to: IVec2) -> bool {
        level.is_inside(to) && bfs_flood(level, from)[level.index(to)]
    }

    /// 验收 a)：同一 seed 两次生成逐格相同。
    #[test]
    fn same_seed_generates_identical_levels() {
        for seed in [0u64, 42, 1337, u64::MAX] {
            let a = generate_level(seed, 1);
            let b = generate_level(seed, 1);
            assert_eq!(a.width(), b.width(), "seed {seed}");
            assert_eq!(a.height(), b.height(), "seed {seed}");
            assert_eq!(a.entrance, b.entrance, "seed {seed}");
            assert_eq!(a.exit, b.exit, "seed {seed}");
            assert_eq!(a.map(), b.map(), "seed {seed}：地图必须逐格一致");
            assert_eq!(a.passable, b.passable, "seed {seed}");
        }
        // 不同深度走不同的层种子，几乎必然不同图
        let d1 = generate_level(42, 1);
        let d2 = generate_level(42, 2);
        assert!(
            d1.map() != d2.map() || d1.width() != d2.width(),
            "不同深度应产出不同关卡"
        );
    }

    /// 验收 b)：≥100 随机种子——入口/出口存在且 passable、
    /// 入口 BFS（4 邻接，走 [`walkable`]）可达出口、无越界写入 panic、边界闭合。
    /// 三期起：锁门/密门是 SOLID 不可通行，[`walkable`] 天然不穿——
    /// 主路径连通即自动验证"特殊房/密室永远挂支路、不挡入口→出口"。
    /// 另断言：每层恰一扇锁门（花园样板必出），且锁门本身从入口可及
    /// （贴着可达地板，钥匙送达后开得了门）。
    #[test]
    fn hundred_seeds_produce_valid_connected_levels() {
        let mut secret_doors_total = 0usize;
        for seed in 0..120u64 {
            let depth = (seed % 4) as i32 + 1; // 覆盖下水道 1-4 层语义
            let level = generate_level(seed, depth);

            assert_eq!(
                level.terrain(level.entrance),
                Terrain::Entrance,
                "seed {seed}：入口地形"
            );
            assert_eq!(
                level.terrain(level.exit),
                Terrain::Exit,
                "seed {seed}：出口地形"
            );
            assert!(
                level.passable[level.index(level.entrance)],
                "seed {seed}：入口应 passable"
            );
            assert!(
                level.passable[level.index(level.exit)],
                "seed {seed}：出口应 passable"
            );
            let reached = bfs_flood(&level, level.entrance);
            assert!(
                reached[level.index(level.exit)],
                "seed {seed} depth {depth}：入口应能走到出口（不穿锁门/密门）\n{}",
                level.debug_ascii()
            );

            // 特殊房恰 1 间（下水道配额 1-2 被移植池截断为 1）→ 恰 1 扇锁门；
            // 锁门必须紧邻主图可达地板（不挡路但可开）
            let locked: Vec<usize> = (0..level.size())
                .filter(|&i| level.map()[i] == Terrain::LockedDoor)
                .collect();
            assert_eq!(locked.len(), 1, "seed {seed}：应恰有一扇锁门");
            let door_pos = level.pos_of(locked[0]);
            assert!(
                [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y]
                    .iter()
                    .any(|&d| level.is_inside(door_pos + d)
                        && reached[level.index(door_pos + d)]),
                "seed {seed}：锁门 {door_pos:?} 应从入口侧可及\n{}",
                level.debug_ascii()
            );

            secret_doors_total += level
                .map()
                .iter()
                .filter(|&&t| t == Terrain::SecretDoor)
                .count();

            // padding 保证最外一圈永远是实心墙（生成期无越界写入的旁证）
            let (w, h) = (level.width() as i32, level.height() as i32);
            for x in 0..w {
                assert!(
                    !level.passable[level.index(IVec2::new(x, 0))],
                    "seed {seed}"
                );
                assert!(
                    !level.passable[level.index(IVec2::new(x, h - 1))],
                    "seed {seed}"
                );
            }
            for y in 0..h {
                assert!(
                    !level.passable[level.index(IVec2::new(0, y))],
                    "seed {seed}"
                );
                assert!(
                    !level.passable[level.index(IVec2::new(w - 1, y))],
                    "seed {seed}"
                );
            }
        }
        // 密室藏门 + 随机藏门：120 层聚合必然存在
        assert!(
            secret_doors_total > 0,
            "语料内应存在密门，得 {secret_doors_total}"
        );
    }

    /// 验收 d)：seed=42 的字符画样例（`cargo test -- --nocapture` 可见）。
    /// 三期重新钉死：RNG 消耗序变化后同种子出图不同属预期，期望据新图核对。
    #[test]
    fn debug_ascii_seed_42() {
        let level = generate_level(42, 1);
        let art = level.debug_ascii();
        println!(
            "seed=42 depth=1（{}×{}）：\n{art}",
            level.width(),
            level.height()
        );

        assert_eq!(art.matches('E').count(), 1, "恰一个入口");
        assert_eq!(art.matches('X').count(), 1, "恰一个出口");
        assert!(art.contains('+'), "至少一扇门");
        assert_eq!(art.matches('L').count(), 1, "恰一扇锁门（花园样板）");
        assert!(art.contains('#') && art.contains('.'));
        assert!(art.contains('~'), "seed=42 应有水（0.30 填充率下几乎必然）");
        assert!(
            art.contains('"') || art.contains('!'),
            "seed=42 应有草（0.20 填充率下几乎必然，且花园自带草地）"
        );
        // 行数 = 高度，每行长度 = 宽度
        let lines: Vec<&str> = art.lines().collect();
        assert_eq!(lines.len(), level.height());
        assert!(lines.iter().all(|l| l.chars().count() == level.width()));
    }

    /// 二期验收：builder 五五开的两个分支都会出现，且各自同种子逐格确定。
    #[test]
    fn both_builder_branches_occur_and_are_deterministic() {
        let mut seen_loop = None;
        let mut seen_fig8 = None;
        for seed in 0..64u64 {
            let (_, kind) = generate_level_impl(seed, 1);
            match kind {
                BuilderKind::Loop if seen_loop.is_none() => seen_loop = Some(seed),
                BuilderKind::FigureEight if seen_fig8.is_none() => seen_fig8 = Some(seed),
                _ => {}
            }
            if seen_loop.is_some() && seen_fig8.is_some() {
                break;
            }
        }
        // Int(2) 五五开，64 个种子内两分支都缺席的概率 ~2^-63
        let seen_loop = seen_loop.expect("应存在 LoopBuilder 分支的种子");
        let seen_fig8 = seen_fig8.expect("应存在 FigureEightBuilder 分支的种子");

        for (seed, kind) in [
            (seen_loop, BuilderKind::Loop),
            (seen_fig8, BuilderKind::FigureEight),
        ] {
            let (a, ka) = generate_level_impl(seed, 1);
            let (b, kb) = generate_level_impl(seed, 1);
            assert_eq!(ka, kind, "seed {seed}");
            assert_eq!(kb, kind, "seed {seed}");
            assert_eq!(
                a.map(),
                b.map(),
                "seed {seed}（{kind:?}）：地图必须逐格一致"
            );
            assert_eq!(a.entrance, b.entrance, "seed {seed}");
            assert_eq!(a.exit, b.exit, "seed {seed}");
        }
    }

    /// 二期验收：Feeling 掷点分布对拍 Level.create 概率表（Level.java L255-L292）
    /// ——depth>1 时 7 种氛围各 1/14、无氛围 1/2；depth 1 恒无氛围。
    #[test]
    fn feeling_roll_distribution_matches_table() {
        use crate::levels::random::LevelRng;
        use rand::SeedableRng;
        use std::collections::HashMap;

        let mut rng = LevelRng::seed_from_u64(2024);
        assert_eq!(roll_feeling(&mut rng, 1), Feeling::None, "1 层恒无氛围");

        const N: usize = 14_000;
        let mut counts: HashMap<Feeling, usize> = HashMap::new();
        for _ in 0..N {
            *counts.entry(roll_feeling(&mut rng, 2)).or_default() += 1;
        }
        for feeling in [
            Feeling::Chasm,
            Feeling::Water,
            Feeling::Grass,
            Feeling::Dark,
            Feeling::Large,
            Feeling::Traps,
            Feeling::Secrets,
        ] {
            let n = counts.get(&feeling).copied().unwrap_or(0);
            // 期望 1000（1/14），±200 已是 ~6.5σ
            assert!(
                (800..=1200).contains(&n),
                "{feeling:?} 出现 {n} 次，应接近 {}",
                N / 14
            );
        }
        let none = counts.get(&Feeling::None).copied().unwrap_or(0);
        assert!(
            (6600..=7400).contains(&none),
            "None 出现 {none} 次，应接近 {}",
            N / 2
        );
    }

    /// 生成后仍属"刻画前地板"的地形集合（水/草/陷阱/贴花只覆盖 EMPTY）。
    fn was_empty_at_paint(t: Terrain) -> bool {
        matches!(
            t,
            Terrain::Empty
                | Terrain::EmptyDeco
                | Terrain::Water
                | Terrain::Grass
                | Terrain::HighGrass
                | Terrain::Trap
                | Terrain::SecretTrap
        )
    }

    /// 二期验收：无氛围层（depth 1）的水/草出现率落在 `SewerLevel` 参数邻域
    /// （水 0.30、草 0.20，`SewerLevel.java` L104-L105）。Patch 强制全图填充率
    /// 精确，与房间足迹求交后仍有波动，故聚合 30 个种子后取带宽 ±0.10。
    /// 三期起花园（每层必出）/密园/条纹房自带结构性草地，草率上带放宽到
    /// 0.45（30 种子实测 ≈0.33）；管道房水线并入水率，原带已覆盖。
    #[test]
    fn water_and_grass_rates_near_sewer_params() {
        let mut floor_total = 0usize; // 刻画前的 EMPTY 总数
        let mut water_total = 0usize;
        let mut grass_total = 0usize; // 矮草 + 高草
        let mut high_total = 0usize;
        let mut trap_total = 0usize;

        for seed in 0..30u64 {
            let level = generate_level(seed, 1);
            for &t in level.map() {
                if was_empty_at_paint(t) {
                    floor_total += 1;
                }
                match t {
                    Terrain::Water => water_total += 1,
                    Terrain::Grass => grass_total += 1,
                    Terrain::HighGrass => {
                        grass_total += 1;
                        high_total += 1;
                    }
                    Terrain::Trap | Terrain::SecretTrap => trap_total += 1,
                    _ => {}
                }
            }
        }

        let water_rate = water_total as f32 / floor_total as f32;
        assert!(
            (0.20..=0.40).contains(&water_rate),
            "水占地板率应近 0.30，得 {water_rate}"
        );
        // 草只刻在水没占走的地板上：分母扣掉水
        let grass_rate = grass_total as f32 / (floor_total - water_total) as f32;
        assert!(
            (0.10..=0.45).contains(&grass_rate),
            "草占非水地板率应近 0.20 + 花园/条纹房结构草，得 {grass_rate}"
        );
        // RegularPainter L405-L407：0.20/4 参数下高草占比常见 ~60%，全域 8.3%-75%
        let high_rate = high_total as f32 / grass_total as f32;
        assert!(
            (0.20..=0.80).contains(&high_rate),
            "高草占草比应在常见带内，得 {high_rate}"
        );
        // depth 1 每层 2-3 个陷阱（NormalIntRange(2,3)），聚合后必有
        assert!(trap_total >= 30, "30 层聚合陷阱数应 ≥30，得 {trap_total}");
    }

    /// 二期验收：Feeling 对刻画的修正生效——
    /// WATER/GRASS 氛围抬填充率（SewerLevel.java L104-L105）、
    /// TRAPS 氛围 5 倍陷阱且加铺部分可见（RegularPainter.java L469、L483-L489）、
    /// CHASM 氛围基底为深渊（Level.java L326）、
    /// SECRETS 氛围藏门几率向 50% 拉拢（RegularPainter.java L193-L196）、
    /// LARGE 氛围房间更多图更大（RegularLevel.java L129-L133）。
    #[test]
    fn feeling_modifiers_change_painting() {
        // 按氛围聚合的 (水率分子/分母, 草率分子/分母, 密门数, 图面积, 层数)
        #[derive(Default)]
        struct Agg {
            floor: usize,
            water: usize,
            grass: usize,
            secret_doors: usize,
            area: usize,
            levels: usize,
        }
        let mut none = Agg::default();
        let mut water = Agg::default();
        let mut grass = Agg::default();
        let mut secrets = Agg::default();
        let mut large = Agg::default();
        let mut traps_seen = 0u32;
        let mut chasm_seen = 0u32;

        for seed in 0..400u64 {
            let level = generate_level(seed, 2);
            let agg = match level.feeling {
                Feeling::None => &mut none,
                Feeling::Water => &mut water,
                Feeling::Grass => &mut grass,
                Feeling::Secrets => &mut secrets,
                Feeling::Large => &mut large,
                Feeling::Traps => {
                    // paintTraps 部分：前 nTraps 个隐藏、加铺 4 倍全部可见（4:1）。
                    // 三期起 BurnedRoom 会额外铺近似对半的明/暗陷阱，
                    // 恒等式退化为不等式：明 > 暗 且暗 ≥ 2（种子固定，结果确定）。
                    let hidden = level
                        .map()
                        .iter()
                        .filter(|&&t| t == Terrain::SecretTrap)
                        .count();
                    let visible = level.map().iter().filter(|&&t| t == Terrain::Trap).count();
                    assert!(hidden >= 2, "seed {seed}：TRAPS 层暗陷阱应 ≥2");
                    assert!(
                        visible > hidden,
                        "seed {seed}：TRAPS 层明陷阱应多于暗陷阱（{visible} vs {hidden}）"
                    );
                    traps_seen += 1;
                    continue;
                }
                Feeling::Chasm => {
                    // 基底是深渊而非墙（角格必未被房间覆盖）
                    assert_eq!(
                        level.terrain(IVec2::ZERO),
                        Terrain::Chasm,
                        "seed {seed}：CHASM 层基底应为深渊"
                    );
                    chasm_seen += 1;
                    continue;
                }
                Feeling::Dark => continue, // 视距修正属英雄/渲染域，生成期无可断言项
            };
            agg.levels += 1;
            agg.area += level.size();
            for &t in level.map() {
                if was_empty_at_paint(t) {
                    agg.floor += 1;
                }
                match t {
                    Terrain::Water => agg.water += 1,
                    Terrain::Grass | Terrain::HighGrass => agg.grass += 1,
                    Terrain::SecretDoor => agg.secret_doors += 1,
                    _ => {}
                }
            }
        }

        // 400 个种子每氛围期望 ~28 个；任一桶空说明掷点接线坏了
        for (name, agg) in [
            ("None", &none),
            ("WATER", &water),
            ("GRASS", &grass),
            ("SECRETS", &secrets),
            ("LARGE", &large),
        ] {
            assert!(agg.levels > 0, "{name} 氛围在 400 种子内应至少出现一次");
        }
        assert!(traps_seen > 0 && chasm_seen > 0);

        let water_rate = |a: &Agg| a.water as f32 / a.floor as f32;
        assert!(
            water_rate(&water) > water_rate(&none) + 0.2,
            "WATER 氛围（0.85）应显著高于无氛围（0.30）：{} vs {}",
            water_rate(&water),
            water_rate(&none)
        );
        let grass_rate = |a: &Agg| a.grass as f32 / (a.floor - a.water) as f32;
        assert!(
            grass_rate(&grass) > grass_rate(&none) + 0.2,
            "GRASS 氛围（0.80）应显著高于无氛围（0.20）：{} vs {}",
            grass_rate(&grass),
            grass_rate(&none)
        );
        // depth 2 基础藏门率 0.1，SECRETS 拉到 0.3。"藏门断图即回退"的严判
        // 会压低两侧实现值（实测 ~1.0 vs ~2.0），按相对倍数断言；
        // 种子固定 → 全部聚合值确定，无随机波动。
        let secret_mean = |a: &Agg| a.secret_doors as f32 / a.levels as f32;
        assert!(
            secret_mean(&secrets) > secret_mean(&none) * 1.5,
            "SECRETS 氛围密门均数应明显更多：{} vs {}",
            secret_mean(&secrets),
            secret_mean(&none)
        );
        // LARGE：9 个标准房 vs 4-6 个，平均图面积应更大
        let area_mean = |a: &Agg| a.area as f32 / a.levels as f32;
        assert!(
            area_mean(&large) > area_mean(&none),
            "LARGE 氛围平均图面积应更大：{} vs {}",
            area_mean(&large),
            area_mean(&none)
        );
    }

    /// 三期验收：尺寸类别分布——NORMAL 最常见、LARGE 次之、GIANT 存在；
    /// sizeFactor 折抵让模板总房值落在 standards 预算带内（预算 + 至多 2 超额）。
    #[test]
    fn size_categories_distribute_with_depth_weights() {
        let mut counts = [0usize; 3];
        for seed in 0..200u64 {
            let (feeling, template) = level_template(seed, 2);
            let mut value = 0;
            for room in &template {
                if let RoomKind::Standard { size, .. } = room.kind {
                    counts[size as usize] += 1;
                    value += size.room_value();
                }
            }
            // SewerLevel.standardRooms：4-6（LARGE 氛围 9）；
            // while i < standards 的最后一间最多超预算 room_value-1 = 2
            let budget = if feeling == Feeling::Large { 9 } else { 6 };
            assert!(
                (4..=budget + 2).contains(&value),
                "seed {seed}：标准房总值 {value} 超出预算带（≤{}）",
                budget + 2
            );
        }
        let [normal, large, giant] = counts;
        assert!(
            normal > large && large > giant,
            "NORMAL({normal}) > LARGE({large}) > GIANT({giant}) 的次序应成立"
        );
        assert!(giant > 0, "GIANT 应实际出现（200 种子聚合）");
    }

    /// 三期验收：LARGE/GIANT 房间在成图里真实存在——
    /// 找一个模板含 GIANT 的种子，生成后图幅必须容得下 ≥15 格的房间足迹。
    #[test]
    fn large_rooms_survive_into_painted_levels() {
        use crate::levels::standard::SizeCategory;

        let mut checked = 0;
        for seed in 0..64u64 {
            let (_, template) = level_template(seed, 2);
            let has_giant = template.iter().any(|r| {
                matches!(
                    r.kind,
                    RoomKind::Standard {
                        size: SizeCategory::Giant,
                        ..
                    }
                )
            });
            if !has_giant {
                continue;
            }
            let level = generate_level(seed, 2);
            // GIANT 最小 15×15 格 + 两侧 padding：任一轴必然 ≥ 17
            assert!(
                level.width().max(level.height()) >= 17,
                "seed {seed}：含 GIANT 的层图幅过小（{}×{}）",
                level.width(),
                level.height()
            );
            checked += 1;
        }
        assert!(checked > 0, "64 种子内应至少出现一个 GIANT 模板");
    }

    /// 三期验收：特殊房/密室数量语义——
    /// 每层特殊房恰 1（配额 1-3 被移植池截断）；depth 1 无密室；
    /// depths 2-4 密室合计 = 下水道域预算 2 + SECRETS 氛围层加成。
    #[test]
    fn special_and_secret_counts_follow_budget() {
        let count_special = |rooms: &[Room]| {
            rooms
                .iter()
                .filter(|r| matches!(r.kind, RoomKind::Special(_)))
                .count()
        };
        let count_secret = |rooms: &[Room]| {
            rooms
                .iter()
                .filter(|r| matches!(r.kind, RoomKind::Secret(_)))
                .count()
        };

        for seed in 0..100u64 {
            let (_, t1) = level_template(seed, 1);
            assert_eq!(count_special(&t1), 1, "seed {seed}：depth 1 特殊房");
            assert_eq!(count_secret(&t1), 0, "seed {seed}：depth 1 恒无密室");

            let mut secrets = 0;
            let mut bonus = 0;
            for depth in 2..=4 {
                let (feeling, t) = level_template(seed, depth);
                assert_eq!(count_special(&t), 1, "seed {seed} depth {depth}");
                if feeling == Feeling::Secrets {
                    bonus += 1;
                }
                secrets += count_secret(&t);
            }
            assert_eq!(
                secrets,
                2 + bonus,
                "seed {seed}：depths 2-4 密室合计应为域预算 2 + SECRETS 加成 {bonus}"
            );
        }
    }

    /// 三期验收：密室成图后带隐藏门（`SECRET_DOOR`），且藏在支路上——
    /// 挑一个 depth 内模板含密室的种子，验证图中密门数 ≥ 模板密室数，
    /// 且 BFS（不穿密门）依旧入口→出口连通。
    #[test]
    fn secret_rooms_paint_hidden_doors_off_path() {
        let mut verified = 0;
        for seed in 0..80u64 {
            for depth in 2..=4 {
                let (_, template) = level_template(seed, depth);
                let secret_rooms = template
                    .iter()
                    .filter(|r| r.kind.is_secret())
                    .count();
                if secret_rooms == 0 {
                    continue;
                }
                let level = generate_level(seed, depth);
                let secret_doors = level
                    .map()
                    .iter()
                    .filter(|&&t| t == Terrain::SecretDoor)
                    .count();
                assert!(
                    secret_doors >= secret_rooms,
                    "seed {seed} depth {depth}：密室 {secret_rooms} 间应有对应密门，图中仅 {secret_doors}"
                );
                assert!(
                    bfs_reachable(&level, level.entrance, level.exit),
                    "seed {seed} depth {depth}：密门不得挡主路径"
                );
                verified += 1;
            }
        }
        assert!(verified >= 10, "语料应包含足量带密室的层，得 {verified}");
    }

    /// 三期验收：run 池重放的确定性——同种子任意深度多次生成结果一致，
    /// 且逐层顺序生成与"直接跳到 depth 3"生成的第 3 层逐格相同。
    #[test]
    fn run_pool_replay_is_order_independent() {
        for seed in [7u64, 42, 1234] {
            // 先隔空生成 depth 3，再按 1→2→3 顺序生成，末层必须一致
            let direct = generate_level(seed, 3);
            let _ = generate_level(seed, 1);
            let _ = generate_level(seed, 2);
            let sequential = generate_level(seed, 3);
            assert_eq!(direct.map(), sequential.map(), "seed {seed}：重放应与顺序下潜一致");
            assert_eq!(direct.entrance, sequential.entrance);
            assert_eq!(direct.exit, sequential.exit);
        }
    }
}
