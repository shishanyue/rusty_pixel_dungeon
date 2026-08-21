//! 画师：把房间集合刻画进 `Level` 地图，对照
//! `core/.../levels/painters/{Painter,RegularPainter,SewerPainter}.java`
//! 与各房间的 `paint`（标准变体见 [`crate::levels::standard`]，
//! 特殊房/密室见 [`crate::levels::special`]）。
//!
//! 二期已移植：水/草刻画（`paintWater`/`paintGrass`，`SewerLevel` 参数 +
//! Feeling 修正）、陷阱地形铺设（`paintTraps`，只铺 `Trap`/`SecretTrap`
//! 地形，行为属后续域）、CHASM/SECRETS/TRAPS 氛围对刻画的修正。
//! 三期加入：房间 `canPlaceWater/Grass/Trap` 落位谓词（水管房禁水、
//! 过火房掩码）、LOCKED/HIDDEN 门类型落地。
//!
//! 仍简化（详见 docs/plans/19、24 实现笔记）：
//! - `mergeRooms`（`RegularPainter` L198-L211、L305-L359）：未移植，相邻标准房恒以门分隔；
//! - SECRETS 氛围的宽松藏门判定（L225-L250）：未移植，恒用"全房可达"严判；
//! - 门、墙、地板、入口/出口与隐藏门（含连通性回退）完整移植。

use bevy::math::{IRect, IVec2};
use rand::{Rng, RngExt, SeedableRng};
use std::collections::VecDeque;

use crate::levels::{
    Feeling, Level,
    builder::{gate, java_round_f32, java_round_f64},
    patch,
    random::{LevelRng, element, float, int, normal_int_range, shuffle},
    rect::SpdRect,
    rooms::{Door, DoorType, Room, RoomKind, set_door},
    special, standard,
    terrain::Terrain,
};

/// SPD `PathFinder.CIRCLE8`（顺时针，从左上开始），以 (dx, dy) 表示。
pub(crate) const CIRCLE8: [IVec2; 8] = [
    IVec2::new(-1, -1),
    IVec2::new(0, -1),
    IVec2::new(1, -1),
    IVec2::new(1, 0),
    IVec2::new(1, 1),
    IVec2::new(0, 1),
    IVec2::new(-1, 1),
    IVec2::new(-1, 0),
];

/// `Level.tunnelTile()`（Level.java L480-L482）：隧道地面按氛围选地形。
fn tunnel_tile(feeling: Feeling) -> Terrain {
    if feeling == Feeling::Chasm {
        Terrain::EmptySp
    } else {
        Terrain::Empty
    }
}

/// `Painter.fill(level, room, value)`：房间**双闭区间足迹**整体填充。
/// Java 里靠 `Room.width()` 覆写（+1）实现；这里显式经 `to_irect` 换算。
pub(crate) fn fill_room(level: &mut Level, room: &Room, terrain: Terrain) {
    level.fill(room.rect.to_irect(), terrain);
}

/// `Painter.fill(level, room, m, value)`（Painter.java L68-L70）：
/// 足迹向内缩 `m` 圈后填充（m=1 即房间内部地板）。
pub(crate) fn fill_room_inset(level: &mut Level, room: &Room, m: i32, terrain: Terrain) {
    let r = room.rect;
    level.fill(
        IRect::new(r.left + m, r.top + m, r.right + 1 - m, r.bottom + 1 - m),
        terrain,
    );
}

/// `Painter.fill(level, x, y, w, h, value)`（Painter.java L54-L62）：
/// 以格数计的 w×h 矩形填充。
pub(crate) fn fill_grid(level: &mut Level, x: i32, y: i32, w: i32, h: i32, terrain: Terrain) {
    level.fill(IRect::new(x, y, x + w, y + h), terrain);
}

/// `Painter.fillEllipse(level, x, y, w, h, value)`（Painter.java L108-L139）：
/// 逐行求椭圆截宽（按 w 奇偶取整到同奇偶）后填充。
pub(crate) fn fill_ellipse(level: &mut Level, x: i32, y: i32, w: i32, h: i32, terrain: Terrain) {
    let rad_h = f64::from(h) / 2.0;
    let rad_w = f64::from(w) / 2.0;
    for i in 0..h {
        // L119：恒取行中心（+0.5）代入椭圆方程解出该行截宽
        let row_y = -rad_h + 0.5 + f64::from(i);
        let mut row_w = 2.0 * ((rad_w * rad_w) * (1.0 - (row_y * row_y) / (rad_h * rad_h))).sqrt();
        // L126-L132：宽度取整到与 w 同奇偶（Math.round 是 Java 语义）
        if w % 2 == 0 {
            row_w = f64::from(java_round_f64(row_w / 2.0)) * 2.0;
        } else {
            row_w = (row_w / 2.0).floor() * 2.0 + 1.0;
        }
        let row_w = row_w as i32;
        fill_grid(level, x + (w - row_w) / 2, y + i, row_w, 1, terrain);
    }
}

/// `Painter.fillEllipse(level, rect, m, value)`（Painter.java L104-L106）：
/// 房间足迹内缩 m 圈的内切椭圆（rect 为 Room 时宽高按格数解释）。
pub(crate) fn fill_ellipse_inset(level: &mut Level, room: &Room, m: i32, terrain: Terrain) {
    fill_ellipse(
        level,
        room.rect.left + m,
        room.rect.top + m,
        room.width() - 2 * m,
        room.height() - 2 * m,
        terrain,
    );
}

/// `Painter.drawInside(level, room, from, n, value)`（Painter.java L164-L186）：
/// 从墙上点 `from` 向房内一步起，沿法向连铺 `n` 格。
pub(crate) fn draw_inside(level: &mut Level, rect: &SpdRect, from: IVec2, n: i32, terrain: Terrain) {
    let step = if from.x == rect.left {
        IVec2::X
    } else if from.x == rect.right {
        IVec2::NEG_X
    } else if from.y == rect.top {
        IVec2::Y
    } else if from.y == rect.bottom {
        IVec2::NEG_Y
    } else {
        // Java 对非墙点保持 (0,0) 步长；本工程无此用例，防御性保留
        IVec2::ZERO
    };
    let mut p = from + step;
    for _ in 0..n {
        level.set_terrain(p, terrain);
        p += step;
    }
}

/// `Painter.drawLine`（Painter.java L76-L98）：浮点步进 + Java 取整。
/// 主轴步长恒 ±1，副轴按比例累积。
pub(crate) fn draw_line(level: &mut Level, from: IVec2, to: IVec2, terrain: Terrain) {
    let mut x = from.x as f32;
    let mut y = from.y as f32;
    let mut dx = (to.x - from.x) as f32;
    let mut dy = (to.y - from.y) as f32;

    let moving_by_x = dx.abs() >= dy.abs();
    if moving_by_x {
        dy /= dx.abs();
        dx /= dx.abs();
    } else {
        dx /= dy.abs();
        dy /= dy.abs();
    }

    level.set_terrain(IVec2::new(java_round_f32(x), java_round_f32(y)), terrain);
    while (moving_by_x && to.x as f32 != x) || (!moving_by_x && to.y as f32 != y) {
        x += dx;
        y += dy;
        level.set_terrain(IVec2::new(java_round_f32(x), java_round_f32(y)), terrain);
    }
}

/// `RegularPainter.paint`（RegularPainter.java L84-L156）；
/// 下水道装饰对照 `SewerPainter.decorate`。房间会被整体平移到正坐标区。
///
/// 水/草/陷阱参数即 `SewerLevel.painter()`（SewerLevel.java L101-L107）：
/// 水 0.30/5、草 0.20/4，WATER/GRASS 氛围分别抬到 0.85/0.80。
pub(crate) fn paint(rng: &mut impl Rng, rooms: &mut [Room], depth: i32, feeling: Feeling) -> Level {
    // SewerLevel.painter()（L101-L107）：构造 painter 时即掷 nTraps
    //（RegularLevel.nTraps L193-L195），先于 paint 内的洗牌消耗随机数
    let water_fill = if feeling == Feeling::Water {
        0.85
    } else {
        0.30
    };
    let grass_fill = if feeling == Feeling::Grass {
        0.80
    } else {
        0.20
    };
    let n_traps = normal_int_range(rng, 2, 3 + depth / 5);

    // L79-L81：feeling == CHASM 时 padding 2
    let padding = if feeling == Feeling::Chasm { 2 } else { 1 };

    // L91-L113：求包围盒 → 平移到 (padding, padding) 起 → 定图尺寸
    let mut left_most = i32::MAX;
    let mut top_most = i32::MAX;
    for room in rooms.iter() {
        left_most = left_most.min(room.rect.left);
        top_most = top_most.min(room.rect.top);
    }
    left_most -= padding;
    top_most -= padding;

    let mut right_most = 0;
    let mut bottom_most = 0;
    for room in rooms.iter_mut() {
        room.shift(-left_most, -top_most);
        right_most = right_most.max(room.rect.right);
        bottom_most = bottom_most.max(room.rect.bottom);
    }
    right_most += padding;
    bottom_most += padding;

    // L113：+1 把闭区间坐标换成尺寸
    let mut level = Level::new((right_most + 1) as usize, (bottom_most + 1) as usize, depth);
    level.feeling = feeling;
    // Level.setSize（Level.java L326）：CHASM 氛围的基底是深渊而非墙
    if feeling == Feeling::Chasm {
        level.fill(
            IRect::new(0, 0, level.width() as i32, level.height() as i32),
            Terrain::Chasm,
        );
    }

    // L122 Random.shuffle(rooms)：Rust 侧房间以索引为身份，改洗"绘制顺序"表，
    // 洗牌算法与 SPD 一致，效果等价（不与 Java 流位对齐）。
    let mut order: Vec<usize> = (0..rooms.len()).collect();
    shuffle(rng, &mut order);

    // L124-L131：逐房定门位 + 刻画。本流水线不产生无连接房间（Java 仅记日志）。
    for &ri in &order {
        place_doors(rng, rooms, ri);
        paint_room(rng, &mut level, rooms, ri);
    }

    // L133
    paint_doors(rng, &mut level, rooms, &order, depth);

    // L135-L153：pushGenerator(Random.Long()) —— 独立子随机流，
    // 使装饰类刻画（水/草/陷阱/贴花）不影响主生成流的后续消耗。
    let mut deco_rng = LevelRng::seed_from_u64(rng.random::<u64>());
    // L139-L149：fill > 0 才刻画（下水道恒真，结构保留）
    if water_fill > 0.0 {
        paint_water(&mut deco_rng, &mut level, rooms, &order, water_fill, 5);
    }
    if grass_fill > 0.0 {
        paint_grass(&mut deco_rng, &mut level, rooms, &order, grass_fill, 4);
    }
    if n_traps > 0 {
        paint_traps(&mut deco_rng, &mut level, rooms, &order, n_traps, depth);
    }
    decorate_sewer(&mut deco_rng, &mut level);

    level
}

/// `RegularPainter.paintWater`（L361-L381）：Patch 噪声与房间足迹求交，
/// 只把 `EMPTY` 换成水。足迹先经 `Room.waterPlaceablePoints`
/// （Room.java L309-L318）过滤——`SewerPipeRoom` 恒否、`BurnedRoom` 掩码。
fn paint_water(
    rng: &mut impl Rng,
    level: &mut Level,
    rooms: &[Room],
    order: &[usize],
    fill: f32,
    smoothness: i32,
) {
    let lake = patch::generate(rng, level.width(), level.height(), fill, smoothness, true);

    for &ri in order {
        for p in rooms[ri].rect.points() {
            if !rooms[ri].can_place_water(p) {
                continue;
            }
            let i = level.index(p);
            if lake[i] && level.map()[i] == Terrain::Empty {
                level.set_terrain(p, Terrain::Water);
            }
        }
    }
}

/// `RegularPainter.paintGrass`（L383-L422）：Patch 噪声选格后，
/// 按 8 邻域内补丁浓度决定高草/矮草（浓度越高越可能是高草）。
/// L409 的 heaps/mobs 检查在生成期恒空（物品/怪物在后续域才落地），未移植。
fn paint_grass(
    rng: &mut impl Rng,
    level: &mut Level,
    rooms: &[Room],
    order: &[usize],
    fill: f32,
    smoothness: i32,
) {
    let grass = patch::generate(rng, level.width(), level.height(), fill, smoothness, true);

    // L386-L403：收集顺序 = 房间（洗牌后）序 × 足迹点序，决定后续 Float 消耗序；
    // 足迹先经 grassPlaceablePoints 过滤（BurnedRoom 过火格禁草）
    let mut grass_cells: Vec<usize> = Vec::new();
    for &ri in order {
        for p in rooms[ri].rect.points() {
            if !rooms[ri].can_place_grass(p) {
                continue;
            }
            let i = level.index(p);
            if grass[i] && level.map()[i] == Terrain::Empty {
                grass_cells.push(i);
            }
        }
    }

    // L405-L421：count = 自身 + 8 邻域内补丁格数，Float() < count/12 则高草。
    // 草格必为房间内部（外圈是墙/门），i ± w ± 1 不会越界。
    let w = level.width() as isize;
    let neighbours8: [isize; 8] = [-w - 1, -w, -w + 1, -1, 1, w - 1, w, w + 1];
    for &i in &grass_cells {
        let mut count = 1;
        for &n in &neighbours8 {
            if grass[(i as isize + n) as usize] {
                count += 1;
            }
        }
        let terrain = if float(rng) < count as f32 / 12.0 {
            Terrain::HighGrass
        } else {
            Terrain::Grass
        };
        level.set_terrain(level.pos_of(i), terrain);
    }
}

/// 下水道陷阱权重与 `avoidsHallways` 落位属性对照表
/// （`SewerLevel.trapClasses`/`trapChances`，SewerLevel.java L119-L137；
/// avoidsHallways 见各 trap 类构造块）。陷阱**行为**属后续域，
/// 此处抽类型只为忠实消耗随机数并取落位属性。
///
/// depth 1：仅 WornDart（avoidsHallways=true）。
const SEWER_TRAPS_DEPTH1: ([f32; 1], [bool; 1]) = ([1.0], [true]);
/// depth 2+：Chilling/Shocking/Toxic/WornDart ×4、Alarm/Ooze ×2、
/// Confusion/Flock/Summoning/Teleportation/Gateway ×1；
/// 其中 `WornDart` 与 `Gateway` 回避走廊。
const SEWER_TRAPS: ([f32; 11], [bool; 11]) = (
    [
        4.0, 4.0, 4.0, 4.0, //
        2.0, 2.0, //
        1.0, 1.0, 1.0, 1.0, 1.0,
    ],
    [
        false, false, false, true, //
        false, false, //
        false, false, false, false, true,
    ],
);

/// `RegularPainter.paintTraps`（L424-L495）：只铺 `Terrain::Trap/SecretTrap`
/// 地形。无饰品系统 → `revealHiddenTrapChance` 恒 0（L465-L466），
/// 可见陷阱只来自 TRAPS 氛围的 4 倍加铺（L469、L483-L489）。
fn paint_traps(
    rng: &mut impl Rng,
    level: &mut Level,
    rooms: &[Room],
    order: &[usize],
    n_traps: i32,
    depth: i32,
) {
    // L425-L442：备选格 = 房间足迹内仍为 EMPTY 的格子，
    // 先经 trapPlaceablePoints 过滤（BurnedRoom 过火格禁陷阱）。
    // EntranceRoom 在 1 层禁陷阱（EntranceRoom.java L69-L75）。
    let mut valid_cells: Vec<usize> = Vec::new();
    for &ri in order {
        if depth == 1 && rooms[ri].kind == RoomKind::Entrance {
            continue;
        }
        for p in rooms[ri].rect.points() {
            if !rooms[ri].can_place_trap(p) {
                continue;
            }
            let i = level.index(p);
            if level.map()[i] == Terrain::Empty {
                valid_cells.push(i);
            }
        }
    }

    // L444-L445：每 5 个备选格至多一个陷阱（Java 在 L463 重复同句，无额外效果）
    let n_traps = n_traps.min(valid_cells.len() as i32 / 5);

    // L447-L460：回避走廊的备选表——上下至少一侧可通 且 左右至少一侧可通。
    // Java 借 passable 数组临时重算（L450-L453）；本工程 passable 缓存
    // 随 set_terrain 增量维护，值恒一致。PathFinder.CIRCLE4 = {-w, +1, +w, -1}。
    let mut valid_non_hallways: Vec<usize> = Vec::new();
    for &i in &valid_cells {
        let p = level.pos_of(i);
        let passable_at = |d: IVec2| level.passable[level.index(p + d)];
        if (passable_at(IVec2::NEG_Y) || passable_at(IVec2::Y))
            && (passable_at(IVec2::X) || passable_at(IVec2::NEG_X))
        {
            valid_non_hallways.push(i);
        }
    }

    // L469：TRAPS 氛围铺 5 倍陷阱，超出 nTraps 的部分全部可见
    let total = if level.feeling == Feeling::Traps {
        5 * n_traps
    } else {
        n_traps
    };
    for i in 0..total {
        // L471：抽陷阱类型（行为域未开工，仅取 avoidsHallways 落位属性）
        let (weights, avoids_hallways): (&[f32], &[bool]) = if depth == 1 {
            (&SEWER_TRAPS_DEPTH1.0, &SEWER_TRAPS_DEPTH1.1)
        } else {
            (&SEWER_TRAPS.0, &SEWER_TRAPS.1)
        };
        let class = chances_index(rng, weights);

        // L473-L481：回避走廊的陷阱优先落非走廊格；按值移除（一次出现）
        let pos = if avoids_hallways[class] && !valid_non_hallways.is_empty() {
            *element(rng, &valid_non_hallways)
        } else {
            *element(rng, &valid_cells)
        };
        if let Some(k) = valid_cells.iter().position(|&c| c == pos) {
            valid_cells.remove(k);
        }
        if let Some(k) = valid_non_hallways.iter().position(|&c| c == pos) {
            valid_non_hallways.remove(k);
        }

        // L483-L493：revealInc 恒 0 → 前 nTraps 个隐藏，加铺部分可见
        let terrain = if i >= n_traps {
            Terrain::Trap
        } else {
            Terrain::SecretTrap
        };
        level.set_terrain(level.pos_of(pos), terrain);
    }
}

/// `Random.chances` 的必中版：权重表全正（本文件的陷阱表），直接解包。
fn chances_index(rng: &mut impl Rng, weights: &[f32]) -> usize {
    crate::levels::random::chances(rng, weights).expect("权重和恒正")
}

/// `RegularPainter.placeDoors`（L160-L184）：为 `ri` 的每条无门连接
/// 在共享墙段上随机选一个双方都可开门的格子。
fn place_doors(rng: &mut impl Rng, rooms: &mut [Room], ri: usize) {
    let others: Vec<usize> = rooms[ri].connected.iter().map(|&(n, _)| n).collect();
    for n in others {
        if rooms[ri].door_to(n).is_some() {
            continue;
        }
        let intersect = rooms[ri].rect.intersect(&rooms[n].rect);
        let door_spots: Vec<IVec2> = intersect
            .points()
            .filter(|&p| rooms[ri].can_connect_point(p) && rooms[n].can_connect_point(p))
            .collect();
        // connect() 成功时必存在门位，且此后房间只被整体平移，几何关系不变；
        // 空门位意味着上游逻辑错误（Java 此处报异常后继续，最终同样崩溃）。
        assert!(
            !door_spots.is_empty(),
            "无处开门：房间 {ri} 与 {n} 的交界无合法门位"
        );
        let pos = *element(rng, &door_spots);
        set_door(rooms, ri, n, Door::new(pos));
    }
}

/// 把 `ri` 的全部门升级为 `kind`（各房间 `paint` 末尾的
/// `for door in connected.values() { door.set(...) }` 段）。
pub(crate) fn upgrade_doors(rooms: &mut [Room], ri: usize, kind: DoorType) {
    let others: Vec<usize> = rooms[ri].connected.iter().map(|&(n, _)| n).collect();
    for n in others {
        if let Some(mut door) = rooms[ri].door_to(n) {
            door.set(kind);
            set_door(rooms, ri, n, door);
        }
    }
}

/// 房间刻画分发（Java 的 `Room.paint(Level)` 虚调用）。
fn paint_room(rng: &mut impl Rng, level: &mut Level, rooms: &mut [Room], ri: usize) {
    match rooms[ri].kind {
        // 标准房间各变体（standard.rs）
        RoomKind::Standard { variant, .. } => {
            standard::paint_standard(rng, level, rooms, ri, variant);
        }
        // 特殊房/密室样板（special.rs）
        RoomKind::Special(kind) => special::paint_special(rng, level, rooms, ri, kind),
        RoomKind::Secret(kind) => special::paint_secret(rng, level, rooms, ri, kind),
        // EntranceRoom.paint（EntranceRoom.java L77-L100；guide 页与 transitions 未移植）
        RoomKind::Entrance => {
            fill_room(level, &rooms[ri], Terrain::Wall);
            fill_room_inset(level, &rooms[ri], 1, Terrain::Empty);
            upgrade_doors(rooms, ri, DoorType::Regular);
            // L86-L89：random(2) 取距墙 ≥2 的内部点（无怪物需回避，单次抽取）
            let pos = rooms[ri].random_point(rng, 2);
            level.set_terrain(pos, Terrain::Entrance);
            level.entrance = pos;
        }
        // ExitRoom.paint（ExitRoom.java L54-L66）
        RoomKind::Exit => {
            fill_room(level, &rooms[ri], Terrain::Wall);
            fill_room_inset(level, &rooms[ri], 1, Terrain::Empty);
            upgrade_doors(rooms, ri, DoorType::Regular);
            let pos = rooms[ri].random_point(rng, 2);
            level.set_terrain(pos, Terrain::Exit);
            level.exit = pos;
        }
        RoomKind::Tunnel => paint_tunnel(rng, level, rooms, ri),
    }
}

/// `TunnelRoom.paint`（TunnelRoom.java L36-L96）：从每扇门向内一步，
/// 先走向连接域所在列/行、再拐向连接域，画出直角走廊。
fn paint_tunnel(rng: &mut impl Rng, level: &mut Level, rooms: &mut [Room], ri: usize) {
    let floor = tunnel_tile(level.feeling);
    let rect = rooms[ri].rect;
    let doors: Vec<IVec2> = rooms[ri]
        .connected
        .iter()
        .map(|&(_, d)| d.expect("place_doors 已为所有连接就位门").pos)
        .collect();

    // getDoorCenter（L107-L122）：门坐标和先截断再整除。
    // Java L116-L117 的 `Random.Float() < doorCenter.x % 1` 对整数和恒为假、
    // 仅空耗 2 次 Float()；本移植不与 Java 流位对齐，予以省略。
    let sum = doors
        .iter()
        .fold(IVec2::ZERO, |acc, d| IVec2::new(acc.x + d.x, acc.y + d.y));
    let count = doors.len() as i32;
    let cx = gate(rect.left + 1, sum.x / count, rect.right - 1);
    let cy = gate(rect.top + 1, sum.y / count, rect.bottom - 1);
    // getConnectionSpace（L98-L104）：单格连接域，闭区间语义（left==right）
    let c = SpdRect::new(cx, cy, cx, cy);

    for door in &doors {
        // L44-L52：起点 = 门向房内一步
        let mut start = *door;
        if start.x == rect.left {
            start.x += 1;
        } else if start.y == rect.top {
            start.y += 1;
        } else if start.x == rect.right {
            start.x -= 1;
        } else if start.y == rect.bottom {
            start.y -= 1;
        }

        // L54-L63
        let right_shift = if start.x < c.left {
            c.left - start.x
        } else if start.x > c.right {
            c.right - start.x
        } else {
            0
        };
        let down_shift = if start.y < c.top {
            c.top - start.y
        } else if start.y > c.bottom {
            c.bottom - start.y
        } else {
            0
        };

        // L65-L74：总是先向房间内侧走
        let (mid, end) = if door.x == rect.left || door.x == rect.right {
            let mid = IVec2::new(start.x + right_shift, start.y);
            (mid, IVec2::new(mid.x, mid.y + down_shift))
        } else {
            let mid = IVec2::new(start.x, start.y + down_shift);
            (mid, IVec2::new(mid.x + right_shift, mid.y))
        };

        draw_line(level, start, mid, floor);
        draw_line(level, mid, end, floor);
    }

    // L80-L91：大隧道房多门时随机补一格对角，避免形状过于呆板
    if rooms[ri].width() >= 7 && rooms[ri].height() >= 7 && doors.len() >= 4 && c.square() == 0 {
        let cell = IVec2::new(c.left, c.top);
        let ofs = (2 * int(rng, 4)) as usize;
        // 先确认不会凭空多出一截走廊（两侧斜邻均已是隧道地面）
        if level.terrain(cell + CIRCLE8[(ofs + 7) % 8]) == floor
            && level.terrain(cell + CIRCLE8[(ofs + 1) % 8]) == floor
        {
            level.set_terrain(cell + CIRCLE8[ofs], floor);
        }
    }

    // L93-L95
    upgrade_doors(rooms, ri, DoorType::Tunnel);
}

/// `RegularPainter.paintDoors`（L186-L303）的简化版：
/// - `mergeRooms`（L198-L211）未移植，恒刻门；
/// - SECRETS 氛围的宽松藏门判定（L225-L250，"只须不完全孤立"）与教程隐藏门
///   （L263-L270）未移植——SECRETS 层仍用"全房可达"严判，藏门略少于 Java，
///   但入口→出口的明门连通性因此恒成立（详见 docs/plans/19 实现笔记）；
/// - SECRETS 氛围把藏门几率向 50% 拉拢（L193-L196）已移植；
/// - 隐藏门的连通性回退（L219-L224、L251-L256）完整保留。
fn paint_doors(
    rng: &mut impl Rng,
    level: &mut Level,
    rooms: &mut [Room],
    order: &[usize],
    depth: i32,
) {
    // L188-L192：隐藏门几率从 2 层的 2/20 线性升到 20 层封顶；1 层恒 0
    let mut hidden_door_chance = if depth > 1 {
        (depth as f32 / 20.0).min(1.0)
    } else {
        0.0
    };
    // L193-L196：SECRETS 氛围把几率向 50% 拉拢
    if level.feeling == Feeling::Secrets {
        hidden_door_chance = (0.5 + hidden_door_chance) / 2.0;
    }

    for &ri in order {
        let others: Vec<usize> = rooms[ri].connected.iter().map(|&(n, _)| n).collect();
        for n in others {
            let mut door = rooms[ri]
                .door_to(n)
                .expect("place_doors 已为所有连接就位门");

            // 每对连接会被两侧各处理一次；REGULAR 只在首次处理时转换（Java 同）
            if door.kind == DoorType::Regular {
                // Java 无论几率是否为 0 都消耗一次 Float()
                if float(rng) < hidden_door_chance {
                    door.kind = DoorType::Hidden;
                    set_door(rooms, ri, n, door);
                    // L219-L224：藏门后 n 必须仍可达（沿非隐藏门 BFS），否则回退普通门
                    if room_distances(rooms, ri)[n] == usize::MAX {
                        door.kind = DoorType::Unlocked;
                        set_door(rooms, ri, n, door);
                    }
                } else {
                    door.kind = DoorType::Unlocked;
                    set_door(rooms, ri, n, door);
                }
                door = rooms[ri].door_to(n).expect("门刚写回");
            }

            // L272-L300：门类型 → 地形
            let terrain = match door.kind {
                DoorType::Empty => Terrain::Empty,
                DoorType::Tunnel => tunnel_tile(level.feeling),
                DoorType::Water => Terrain::Water,
                // REGULAR 已在上方全部转换；防御性归入普通门
                DoorType::Regular | DoorType::Unlocked => Terrain::Door,
                DoorType::Hidden => Terrain::SecretDoor,
                DoorType::Barricade => Terrain::Barricade,
                DoorType::Locked => Terrain::LockedDoor,
                DoorType::Crystal => Terrain::CrystalDoor,
                DoorType::Wall => Terrain::Wall,
            };
            level.set_terrain(door.pos, terrain);
        }
    }
}

/// `Graph.buildDistanceMap` 的房间图等价 BFS。边 = `Room.edges()`
/// （Room.java L407-L419）：只有 EMPTY/TUNNEL/UNLOCKED/REGULAR 门算连通。
fn room_distances(rooms: &[Room], focus: usize) -> Vec<usize> {
    let mut distances = vec![usize::MAX; rooms.len()];
    distances[focus] = 0;
    let mut queue = VecDeque::from([focus]);
    while let Some(r) = queue.pop_front() {
        for &(n, door) in &rooms[r].connected {
            let passable_edge = door.is_some_and(|d| {
                matches!(
                    d.kind,
                    DoorType::Empty | DoorType::Tunnel | DoorType::Unlocked | DoorType::Regular
                )
            });
            if passable_edge && distances[n] == usize::MAX {
                distances[n] = distances[r] + 1;
                queue.push_back(n);
            }
        }
    }
    distances
}

/// `SewerPainter.decorate`（SewerPainter.java L34-L73）。
/// 前两段依赖水体（M1 水刻画 TODO），条件短路后自然不触发但结构保留。
fn decorate_sewer(rng: &mut impl Rng, level: &mut Level) {
    let w = level.width();
    let len = level.size();

    for i in 0..w {
        if level.map()[i] == Terrain::Wall
            && level.map()[i + w] == Terrain::Water
            && int(rng, 4) == 0
        {
            level.set_terrain(level.pos_of(i), Terrain::WallDeco);
        }
    }

    for i in w..len - w {
        if level.map()[i] == Terrain::Wall
            && level.map()[i - w] == Terrain::Wall
            && level.map()[i + w] == Terrain::Water
            && int(rng, 2) == 0
        {
            level.set_terrain(level.pos_of(i), Terrain::WallDeco);
        }
    }

    for i in (w + 1)..(len - w - 1) {
        if level.map()[i] == Terrain::Empty {
            let count = usize::from(level.map()[i + 1] == Terrain::Wall)
                + usize::from(level.map()[i - 1] == Terrain::Wall)
                + usize::from(level.map()[i + w] == Terrain::Wall)
                + usize::from(level.map()[i - w] == Terrain::Wall);
            if int(rng, 16) < (count * count) as i32 {
                level.set_terrain(level.pos_of(i), Terrain::EmptyDeco);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// drawLine 对拍：水平线、对角线各画一次，落点与 Java 步进一致。
    #[test]
    fn draw_line_covers_expected_cells() {
        let mut level = Level::new(10, 10, 1);
        draw_line(
            &mut level,
            IVec2::new(1, 1),
            IVec2::new(5, 1),
            Terrain::Empty,
        );
        for x in 1..=5 {
            assert_eq!(level.terrain(IVec2::new(x, 1)), Terrain::Empty);
        }
        assert_eq!(level.terrain(IVec2::new(6, 1)), Terrain::Wall);

        // 斜率 1 的对角线逐格落在对角线上
        let mut level = Level::new(10, 10, 1);
        draw_line(
            &mut level,
            IVec2::new(2, 2),
            IVec2::new(6, 6),
            Terrain::Empty,
        );
        for d in 2..=6 {
            assert_eq!(level.terrain(IVec2::new(d, d)), Terrain::Empty);
        }

        // 单点线（from == to）只写一格、不发散
        let mut level = Level::new(10, 10, 1);
        draw_line(
            &mut level,
            IVec2::new(3, 3),
            IVec2::new(3, 3),
            Terrain::Empty,
        );
        assert_eq!(level.terrain(IVec2::new(3, 3)), Terrain::Empty);
        assert_eq!(
            level.map().iter().filter(|&&t| t == Terrain::Empty).count(),
            1
        );
    }

    /// 房间填充遵守双闭区间足迹（墙圈 + 内部地板）。
    #[test]
    fn fill_room_paints_walls_and_floor() {
        let mut level = Level::new(12, 12, 1);
        let mut room = Room::new(RoomKind::empty_standard());
        room.rect = SpdRect::new(2, 2, 7, 7);
        fill_room(&mut level, &room, Terrain::Wall);
        fill_room_inset(&mut level, &room, 1, Terrain::Empty);

        // 四角与边界是墙
        for p in [
            IVec2::new(2, 2),
            IVec2::new(7, 7),
            IVec2::new(2, 7),
            IVec2::new(7, 2),
            IVec2::new(4, 2),
            IVec2::new(2, 4),
        ] {
            assert_eq!(level.terrain(p), Terrain::Wall, "{p:?} 应为墙");
        }
        // 内部是地板
        for p in [IVec2::new(3, 3), IVec2::new(6, 6), IVec2::new(4, 5)] {
            assert_eq!(level.terrain(p), Terrain::Empty, "{p:?} 应为地板");
        }
        // 足迹外未被触碰（初始即墙，这里查右侧一列）
        assert_eq!(level.terrain(IVec2::new(8, 4)), Terrain::Wall);
    }

    /// 隐藏门回退：仅一条连接时藏门会断图，必须回退为普通门。
    #[test]
    fn hidden_door_falls_back_when_it_would_disconnect() {
        use crate::levels::rooms::connect;

        // 两个房间一条连接：隐藏它必然使 n 不可达 → 恒回退 Unlocked
        for seed in 0..32 {
            let mut rooms = vec![
                Room::new(RoomKind::empty_standard()),
                Room::new(RoomKind::empty_standard()),
            ];
            rooms[0].rect = SpdRect::new(0, 0, 5, 5);
            rooms[1].rect = SpdRect::new(5, 0, 10, 5);
            assert!(connect(&mut rooms, 0, 1));
            set_door(&mut rooms, 0, 1, Door::new(IVec2::new(5, 2)));
            let mut door = rooms[0].door_to(1).unwrap();
            door.set(DoorType::Regular);
            set_door(&mut rooms, 0, 1, door);

            let mut level = Level::new(12, 8, 20); // depth 20 → 隐藏门几率 100%
            let order = [0usize, 1];
            let mut rng = LevelRng::seed_from_u64(seed);
            paint_doors(&mut rng, &mut level, &mut rooms, &order, 20);

            assert_eq!(rooms[0].door_to(1).unwrap().kind, DoorType::Unlocked);
            assert_eq!(level.terrain(IVec2::new(5, 2)), Terrain::Door);
        }
    }

    /// 搭一间已刻好墙/地板的标准房（含四周墙、内部 EMPTY）。
    fn painted_room(level: &mut Level, left: i32, top: i32, right: i32, bottom: i32) -> Room {
        let mut room = Room::new(RoomKind::empty_standard());
        room.rect = SpdRect::new(left, top, right, bottom);
        fill_room(level, &room, Terrain::Wall);
        fill_room_inset(level, &room, 1, Terrain::Empty);
        room
    }

    /// paintWater 语义：只把房间足迹内的 EMPTY 换成水，墙/门等一概不动。
    /// fill=1.0 时 Patch 强制全图为真，房间内部应全部成水。
    #[test]
    fn paint_water_replaces_only_empty() {
        let mut level = Level::new(14, 12, 1);
        let room = painted_room(&mut level, 2, 2, 9, 9);
        level.set_terrain(IVec2::new(5, 2), Terrain::Door); // 足迹上的门不许被水覆盖

        let rooms = vec![room];
        let mut rng = LevelRng::seed_from_u64(3);
        paint_water(&mut rng, &mut level, &rooms, &[0], 1.0, 5);

        for p in rooms[0].rect.points() {
            let expect = match (p.x, p.y) {
                (5, 2) => Terrain::Door,
                (x, y) if x == 2 || x == 9 || y == 2 || y == 9 => Terrain::Wall,
                _ => Terrain::Water,
            };
            assert_eq!(level.terrain(p), expect, "{p:?}");
        }
        // 足迹外（全图其余）仍是初始墙
        assert_eq!(level.terrain(IVec2::new(11, 5)), Terrain::Wall);
    }

    /// paintTraps 语义：数量被备选格 /5 钳制；无 TRAPS 氛围全部是暗陷阱；
    /// TRAPS 氛围总数 5 倍、加铺部分可见（明:暗 = 4:1）。
    #[test]
    fn paint_traps_clamps_and_reveals_by_feeling() {
        let count_traps = |level: &Level| {
            let hidden = level
                .map()
                .iter()
                .filter(|&&t| t == Terrain::SecretTrap)
                .count();
            let visible = level.map().iter().filter(|&&t| t == Terrain::Trap).count();
            (hidden, visible)
        };

        // 6×6 内部 = 36 个 EMPTY → nTraps 钳到 36/5 = 7
        let mut level = Level::new(12, 12, 2);
        let rooms = vec![painted_room(&mut level, 2, 2, 9, 9)];
        let mut rng = LevelRng::seed_from_u64(11);
        paint_traps(&mut rng, &mut level, &rooms, &[0], 99, 2);
        let (hidden, visible) = count_traps(&level);
        assert_eq!((hidden, visible), (7, 0), "数量钳制 + 无氛围全暗");

        // TRAPS 氛围：nTraps=2 → 总数 10，明 8 暗 2
        let mut level = Level::new(12, 12, 2);
        level.feeling = Feeling::Traps;
        let rooms = vec![painted_room(&mut level, 2, 2, 9, 9)];
        let mut rng = LevelRng::seed_from_u64(11);
        paint_traps(&mut rng, &mut level, &rooms, &[0], 2, 2);
        let (hidden, visible) = count_traps(&level);
        assert_eq!((hidden, visible), (2, 8), "TRAPS 氛围 5 倍且加铺可见");

        // 1 层入口房禁陷阱（EntranceRoom.java L69-L75）：唯一房间是入口时无处可铺
        let mut level = Level::new(12, 12, 1);
        let mut entrance_room = Room::new(RoomKind::Entrance);
        entrance_room.rect = SpdRect::new(2, 2, 9, 9);
        fill_room(&mut level, &entrance_room, Terrain::Wall);
        fill_room_inset(&mut level, &entrance_room, 1, Terrain::Empty);
        let rooms = vec![entrance_room];
        let mut rng = LevelRng::seed_from_u64(11);
        paint_traps(&mut rng, &mut level, &rooms, &[0], 3, 1);
        assert_eq!(count_traps(&level), (0, 0), "1 层入口房不铺陷阱");
    }
}
