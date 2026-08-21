//! 标准房间的尺寸类别与绘制变体，对照
//! `core/.../levels/rooms/standard/StandardRoom.java` 及各变体子类。
//!
//! 三期移植的下水道变体（`StandardRoom.createRoom` 轮换表中 depth 1-4 权重非零者）：
//! `SewerPipeRoom`（16）、`RingRoom`（8）、`CircleBasinRoom`（4）、
//! `BurnedRoom`（1，仅 depth 2+）、`StripedRoom`（1）。
//! 其余权重非零但未移植的类回退为 [`StandardVariant::Empty`]（清单见
//! docs/plans/24 实现笔记），保证已移植变体的出现率与 Java 一致。

use bevy::math::IVec2;
use rand::Rng;

use crate::levels::{
    painter::{
        draw_inside, draw_line, fill_ellipse_inset, fill_grid, fill_room, fill_room_inset,
        upgrade_doors,
    },
    patch,
    random::{chances, int, int_range},
    rect::SpdRect,
    rooms::{DoorType, Room, RoomKind, set_door},
    terrain::Terrain,
};

/// `StandardRoom.SizeCategory`（StandardRoom.java L36-L51）：
/// `(minDim, maxDim, roomValue)` = NORMAL(4,10,1) / LARGE(10,14,2) / GIANT(14,18,3)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SizeCategory {
    Normal,
    Large,
    Giant,
}

impl SizeCategory {
    /// 枚举序即 Java ordinal（`setSizeCat` 以 ordinal 截断掷点表）。
    pub const ALL: [SizeCategory; 3] = [
        SizeCategory::Normal,
        SizeCategory::Large,
        SizeCategory::Giant,
    ];

    pub const fn min_dim(self) -> i32 {
        match self {
            SizeCategory::Normal => 4,
            SizeCategory::Large => 10,
            SizeCategory::Giant => 14,
        }
    }

    pub const fn max_dim(self) -> i32 {
        match self {
            SizeCategory::Normal => 10,
            SizeCategory::Large => 14,
            SizeCategory::Giant => 18,
        }
    }

    /// `roomValue`：大房间在数量/权重结算中折抵的标准房数
    /// （`sizeFactor`，StandardRoom.java L100-L104）。
    pub const fn room_value(self) -> i32 {
        match self {
            SizeCategory::Normal => 1,
            SizeCategory::Large => 2,
            SizeCategory::Giant => 3,
        }
    }
}

/// 标准房间绘制变体（Java 的 `StandardRoom` 子类 → 数据枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardVariant {
    /// `EmptyRoom`；同时是未移植变体的绘制占位
    Empty,
    /// `SewerPipeRoom`
    SewerPipe,
    /// `RingRoom`
    Ring,
    /// `CircleBasinRoom`
    CircleBasin,
    /// `BurnedRoom`
    Burned,
    /// `StripedRoom`
    Striped,
}

impl StandardVariant {
    /// `sizeCatProbs()` 覆写表（各变体 Java 文件）：
    /// Empty 用基类默认 {1,0,0}（StandardRoom.java L58-L61）。
    pub const fn size_cat_probs(self) -> [f32; 3] {
        match self {
            StandardVariant::Empty => [1.0, 0.0, 0.0],
            StandardVariant::SewerPipe => [3.0, 2.0, 1.0], // SewerPipeRoom.java L49-L52
            StandardVariant::Ring => [9.0, 3.0, 1.0],      // RingRoom.java L42-L45
            StandardVariant::CircleBasin => [0.0, 3.0, 1.0], // CircleBasinRoom.java L38-L41
            StandardVariant::Burned => [4.0, 1.0, 0.0],    // BurnedRoom.java L35-L38
            StandardVariant::Striped => [2.0, 1.0, 0.0],   // StripedRoom.java L33-L36
        }
    }

    /// `minWidth()/minHeight()` 覆写（宽高同表）。
    pub const fn min_dim(self, size: SizeCategory) -> i32 {
        let base = size.min_dim();
        match self {
            // SewerPipeRoom.java L39-L47、RingRoom.java L32-L40：max(7, super)
            StandardVariant::SewerPipe | StandardVariant::Ring => {
                if base > 7 { base } else { 7 }
            }
            // CircleBasinRoom.java L32-L36：minDim + 1（配合奇数尺寸约束）
            StandardVariant::CircleBasin => base + 1,
            StandardVariant::Empty | StandardVariant::Burned | StandardVariant::Striped => base,
        }
    }

    /// `maxWidth()/maxHeight()`：移植集内无覆写，恒 `sizeCat.maxDim`。
    pub const fn max_dim(self, size: SizeCategory) -> i32 {
        size.max_dim()
    }
}

/// 下水道标准房轮换表：`(变体, depth 1 权重, depth 2-4 权重)`。
/// 对照 `StandardRoom.rooms`（L124-L167）与 `chances[1]/chances[2]`（L171-L173）：
/// 前 5 位是下水道区域房 {16,8,8,4,4}，中段 20 位其他区域权重恒 0（省略），
/// 尾 10 位全区域房 depth 1 = {1,0,1,0,1,0,1,1,0,0}、depth 2-4 全 1。
/// 未移植类回退 `Empty` 并保留权重（已移植变体出现率不受影响）。
const SEWER_STANDARD_TABLE: &[(StandardVariant, f32, f32)] = &[
    (StandardVariant::SewerPipe, 16.0, 16.0),
    (StandardVariant::Ring, 8.0, 8.0),
    (StandardVariant::Empty, 8.0, 8.0), // WaterBridgeRoom 未移植
    (StandardVariant::Empty, 4.0, 4.0), // RegionDecoPatchRoom 未移植
    (StandardVariant::CircleBasin, 4.0, 4.0),
    (StandardVariant::Empty, 1.0, 1.0), // PlantsRoom 未移植
    (StandardVariant::Empty, 0.0, 1.0), // AquariumRoom 未移植
    (StandardVariant::Empty, 1.0, 1.0), // PlatformRoom 未移植
    (StandardVariant::Burned, 0.0, 1.0),
    (StandardVariant::Empty, 1.0, 1.0), // FissureRoom 未移植
    (StandardVariant::Empty, 0.0, 1.0), // GrassyGraveRoom 未移植
    (StandardVariant::Striped, 1.0, 1.0),
    (StandardVariant::Empty, 1.0, 1.0), // StudyRoom 未移植
    (StandardVariant::Empty, 0.0, 1.0), // SuspiciousChestRoom 未移植
    (StandardVariant::Empty, 0.0, 1.0), // MinefieldRoom 未移植
];

/// `StandardRoom.createRoom()`（L190-L192）：按深度权重表抽变体。
/// 仅下水道表（depth 1 与 2+ 两行）；其他区域是四期范围。
/// Java 构造器的 `{ setSizeCat(); }` 初始化掷点（L54）结果必被 `initRooms`
/// 的显式 `setSizeCat(standards-i)` 覆盖，本移植不与 Java 流位对齐，省略。
pub(crate) fn roll_standard_variant(rng: &mut impl Rng, depth: i32) -> StandardVariant {
    let weights: Vec<f32> = SEWER_STANDARD_TABLE
        .iter()
        .map(|&(_, w1, w2)| if depth == 1 { w1 } else { w2 })
        .collect();
    let i = chances(rng, &weights).expect("轮换表权重和恒正");
    SEWER_STANDARD_TABLE[i].0
}

/// `StandardRoom.setSizeCat(0, maxRoomValue-1)`（L63-L90）：
/// 把 ordinal 超过上限的类别权重清零后掷点；全零返回 `None`
/// （调用方按 Java 的 do-while 重抽变体，RegularLevel.initRooms L136-L138）。
pub(crate) fn roll_size_category(
    rng: &mut impl Rng,
    variant: StandardVariant,
    max_room_value: i32,
) -> Option<SizeCategory> {
    let mut probs = variant.size_cat_probs();
    let max_ordinal = max_room_value - 1;
    for (i, p) in probs.iter_mut().enumerate() {
        if i as i32 > max_ordinal {
            *p = 0.0;
        }
    }
    chances(rng, &probs).map(|i| SizeCategory::ALL[i])
}

/// 标准房间 paint 分发（Java 各子类 `paint(Level)` 虚调用）。
pub(crate) fn paint_standard(
    rng: &mut impl Rng,
    level: &mut crate::levels::Level,
    rooms: &mut [Room],
    ri: usize,
    variant: StandardVariant,
) {
    match variant {
        // EmptyRoom.paint（EmptyRoom.java L30-L38）
        StandardVariant::Empty => {
            fill_room(level, &rooms[ri], Terrain::Wall);
            fill_room_inset(level, &rooms[ri], 1, Terrain::Empty);
            upgrade_doors(rooms, ri, DoorType::Regular);
        }
        StandardVariant::SewerPipe => paint_sewer_pipe(rng, level, rooms, ri),
        StandardVariant::Ring => paint_ring(rng, level, rooms, ri),
        StandardVariant::CircleBasin => paint_circle_basin(rng, level, rooms, ri),
        StandardVariant::Burned => paint_burned(rng, level, rooms, ri),
        StandardVariant::Striped => paint_striped(rng, level, rooms, ri),
    }
}

/// `Room.center()`（Room.java L159-L163）：几何中心；SPD 单位宽/高为奇数
/// （即格数为偶数）时随机偏置 1 —— 每个奇数轴消耗一次 `Int(2)`。
pub(crate) fn room_center(rng: &mut impl Rng, rect: &SpdRect) -> IVec2 {
    IVec2::new(
        (rect.left + rect.right) / 2
            + if (rect.right - rect.left) % 2 == 1 {
                int(rng, 2)
            } else {
                0
            },
        (rect.top + rect.bottom) / 2
            + if (rect.bottom - rect.top) % 2 == 1 {
                int(rng, 2)
            } else {
                0
            },
    )
}

// ---------------------------------------------------------------------------
// SewerPipeRoom（SewerPipeRoom.java）
// ---------------------------------------------------------------------------

/// 幻影门重掷上限。Java 是无上限 do-while（L122-L138），合法墙位必然存在、
/// 期望次数个位数；超限说明几何坏死，panic 暴露。
const PIPE_PHANTOM_DOOR_ATTEMPTS: u32 = 1024;

/// `SewerPipeRoom.paint`（L66-L205）：全房填墙后以水道连接各门，
/// 再把水旁的墙蚀空为走道。
fn paint_sewer_pipe(
    rng: &mut impl Rng,
    level: &mut crate::levels::Level,
    rooms: &mut [Room],
    ri: usize,
) {
    let rect = rooms[ri].rect;
    fill_room(level, &rooms[ri], Terrain::Wall);

    let doors: Vec<IVec2> = rooms[ri]
        .connected
        .iter()
        .map(|&(_, d)| d.expect("place_doors 已就位").pos)
        .collect();
    let RoomKind::Standard { size, .. } = rooms[ri].kind else {
        unreachable!("paint_sewer_pipe 只接标准房")
    };

    // getConnectionSpace（L207-L213）：单门用 center()，多门用 getDoorCenter()
    let c = if doors.len() <= 1 {
        room_center(rng, &rect)
    } else {
        pipe_door_center(&rect, &doors)
    };

    // L73-L112：1 门，或标准尺寸 2 门 —— 经中心点直角水道
    if doors.len() == 1 || (doors.len() == 2 && size == SizeCategory::Normal) {
        for door in &doors {
            // L81-L85：起点 = 门向房内两步
            let mut start = *door;
            if start.x == rect.left {
                start.x += 2;
            } else if start.y == rect.top {
                start.y += 2;
            } else if start.x == rect.right {
                start.x -= 2;
            } else if start.y == rect.bottom {
                start.y -= 2;
            }

            // L87-L96：连接域是单格（getConnectionSpace L212 left==right），
            // Java 的三分支（< left / > right / 域内）合并后即坐标差
            let right_shift = c.x - start.x;
            let down_shift = c.y - start.y;

            // L98-L107：总是先向房间内侧走
            let (mid, end) = if door.x == rect.left || door.x == rect.right {
                let mid = IVec2::new(start.x + right_shift, start.y);
                (mid, IVec2::new(mid.x, mid.y + down_shift))
            } else {
                let mid = IVec2::new(start.x, start.y + down_shift);
                (mid, IVec2::new(mid.x + right_shift, mid.y))
            };

            draw_line(level, start, mid, Terrain::Water);
            draw_line(level, mid, end, Terrain::Water);
        }
    } else {
        // L113-L178：多门（或大房 2 门）——贪心最近点对连管
        let mut door_points = doors.clone();

        // L116-L140：恰 2 门时补一扇"幻影门"，保证大管道房有最小开敞空间
        if door_points.len() == 2 {
            let mut attempts = 0;
            let p = loop {
                attempts += 1;
                assert!(
                    attempts <= PIPE_PHANTOM_DOOR_ATTEMPTS,
                    "SewerPipeRoom 幻影门重掷超限"
                );
                let p = if int(rng, 2) == 0 {
                    IVec2::new(
                        if int(rng, 2) == 0 {
                            rect.left
                        } else {
                            rect.right
                        },
                        int_range(rng, rect.top + 2, rect.bottom - 2),
                    )
                } else {
                    IVec2::new(
                        int_range(rng, rect.left + 2, rect.right - 2),
                        if int(rng, 2) == 0 {
                            rect.top
                        } else {
                            rect.bottom
                        },
                    )
                };
                if doors.iter().all(|d| d.x != p.x && d.y != p.y) {
                    break p;
                }
            };
            door_points.push(p);
        }

        // L142-L155：各门向内两步的落点（注意判序 y 先于 x，与单门分支不同——照抄）
        let mut points_to_fill: Vec<IVec2> = door_points
            .iter()
            .map(|door| {
                let mut p = *door;
                if p.y == rect.top {
                    p.y += 2;
                } else if p.y == rect.bottom {
                    p.y -= 2;
                } else if p.x == rect.left {
                    p.x += 2;
                } else {
                    p.x -= 2;
                }
                p
            })
            .collect();

        // L157-L177：贪心取全局最近 (已连, 未连) 点对连管
        let mut points_filled = vec![points_to_fill.remove(0)];
        while !points_to_fill.is_empty() {
            let mut shortest = i32::MAX;
            let mut from = points_filled[0];
            let mut to_idx = 0;
            for &f in &points_filled {
                for (ti, &t) in points_to_fill.iter().enumerate() {
                    let dist = pipe_distance_between(&rect, f, t);
                    if dist < shortest {
                        shortest = dist;
                        from = f;
                        to_idx = ti;
                    }
                }
            }
            let to = points_to_fill.remove(to_idx);
            pipe_fill_between(level, &rect, from, to, Terrain::Water);
            points_filled.push(to);
        }
    }

    // L180-L189：水道旁的墙蚀空成走道
    for p in rect.points() {
        if level.terrain(p) == Terrain::Water {
            for d in crate::levels::painter::CIRCLE8 {
                if level.terrain(p + d) == Terrain::Wall {
                    level.set_terrain(p + d, Terrain::Empty);
                }
            }
        }
    }

    // L191-L204：与相邻管道房的门做 3×3 开敞 + 过门水线，门型 WATER；其余 REGULAR
    let connected: Vec<(usize, IVec2)> = rooms[ri]
        .connected
        .iter()
        .map(|&(n, d)| (n, d.expect("place_doors 已就位").pos))
        .collect();
    for (n, door) in connected {
        let neighbour_is_pipe = matches!(
            rooms[n].kind,
            RoomKind::Standard {
                variant: StandardVariant::SewerPipe,
                ..
            }
        );
        if neighbour_is_pipe {
            fill_grid(level, door.x - 1, door.y - 1, 3, 3, Terrain::Empty);
            if door.x == rect.left || door.x == rect.right {
                fill_grid(level, door.x - 1, door.y, 3, 1, Terrain::Water);
            } else {
                fill_grid(level, door.x, door.y - 1, 1, 3, Terrain::Water);
            }
            let mut d = rooms[ri].door_to(n).expect("门已就位");
            d.set(DoorType::Water);
            set_door(rooms, ri, n, d);
        } else {
            let mut d = rooms[ri].door_to(n).expect("门已就位");
            d.set(DoorType::Regular);
            set_door(rooms, ri, n, d);
        }
    }
}

/// `getDoorCenter`（L221-L236）：门坐标均值（整数截断），gate 到 ±2 边距。
/// Java L230-L231 的 `Random.Float() < doorCenter.x % 1` 对整数和恒假、
/// 仅空耗 2 次 `Float()`；本移植不与 Java 流位对齐，予以省略（同 `TunnelRoom`）。
fn pipe_door_center(rect: &SpdRect, doors: &[IVec2]) -> IVec2 {
    let sum = doors.iter().fold(IVec2::ZERO, |a, d| a + *d);
    let count = doors.len() as i32;
    IVec2::new(
        crate::levels::builder::gate(rect.left + 2, sum.x / count, rect.right - 2),
        crate::levels::builder::gate(rect.top + 2, sum.y / count, rect.bottom - 2),
    )
}

/// `spaceBetween`（L238-L240）：两坐标间的空格数。
fn space_between(a: i32, b: i32) -> i32 {
    (a - b).abs() - 1
}

/// `distanceBetweenPoints`（L242-L260）：沿房间内圈的路径距离。
fn pipe_distance_between(rect: &SpdRect, a: IVec2, b: IVec2) -> i32 {
    // 同侧
    if ((a.x == rect.left + 2 || a.x == rect.right - 2) && a.y == b.y)
        || ((a.y == rect.top + 2 || a.y == rect.bottom - 2) && a.x == b.x)
    {
        return space_between(a.x, b.x).max(space_between(a.y, b.y));
    }
    // 否则沿左右取近边 + 沿上下取近边，重叠扣 1
    (space_between(rect.left, a.x) + space_between(rect.left, b.x))
        .min(space_between(rect.right, a.x) + space_between(rect.right, b.x))
        + (space_between(rect.top, a.y) + space_between(rect.top, b.y))
            .min(space_between(rect.bottom, a.y) + space_between(rect.bottom, b.y))
        - 1
}

/// `fillBetweenPoints`（L262-L320）：两点间取最短管道填充。
fn pipe_fill_between(
    level: &mut crate::levels::Level,
    rect: &SpdRect,
    from: IVec2,
    to: IVec2,
    floor: Terrain,
) {
    // 同侧：一次矩形填充（L267-L277）
    if ((from.x == rect.left + 2 || from.x == rect.right - 2) && from.x == to.x)
        || ((from.y == rect.top + 2 || from.y == rect.bottom - 2) && from.y == to.y)
    {
        fill_grid(
            level,
            from.x.min(to.x),
            from.y.min(to.y),
            space_between(from.x, to.x) + 2,
            space_between(from.y, to.y) + 2,
            floor,
        );
        return;
    }

    // 邻侧：经共享内圈角折线（L279-L295）
    let corners = [
        IVec2::new(rect.left + 2, rect.top + 2),
        IVec2::new(rect.right - 2, rect.top + 2),
        IVec2::new(rect.right - 2, rect.bottom - 2),
        IVec2::new(rect.left + 2, rect.bottom - 2),
    ];
    for c in corners {
        if (c.x == from.x || c.y == from.y) && (c.x == to.x || c.y == to.y) {
            draw_line(level, from, c, floor);
            draw_line(level, c, to, floor);
            return;
        }
    }

    // 对侧：取较近的一条侧边中点中转，化归两次邻侧（L297-L319）
    let side = if from.y == rect.top + 2 || from.y == rect.bottom - 2 {
        if space_between(rect.left, from.x) + space_between(rect.left, to.x)
            <= space_between(rect.right, from.x) + space_between(rect.right, to.x)
        {
            IVec2::new(rect.left + 2, rect.top + (rect.height() + 1) / 2)
        } else {
            IVec2::new(rect.right - 2, rect.top + (rect.height() + 1) / 2)
        }
    } else if space_between(rect.top, from.y) + space_between(rect.top, to.y)
        <= space_between(rect.bottom, from.y) + space_between(rect.bottom, to.y)
    {
        IVec2::new(rect.left + (rect.width() + 1) / 2, rect.top + 2)
    } else {
        IVec2::new(rect.left + (rect.width() + 1) / 2, rect.bottom - 2)
    };
    pipe_fill_between(level, rect, from, side, floor);
    pipe_fill_between(level, rect, side, to, floor);
}

// ---------------------------------------------------------------------------
// RingRoom（RingRoom.java）
// ---------------------------------------------------------------------------

/// `RingRoom.paint`（L47-L96）：环形走道 + 实心内芯；
/// 大房（minDim ≥ 10）内芯换装饰地形并开一扇内门通往中心。
/// 降级：Java L81 `placeCenterDetail` 在中心掉落奖励物品（物品域未开工），
/// 中心 `EMPTY_SP` 地形保留作占位。
fn paint_ring(rng: &mut impl Rng, level: &mut crate::levels::Level, rooms: &mut [Room], ri: usize) {
    let rect = rooms[ri].rect;
    fill_room(level, &rooms[ri], Terrain::Wall);
    fill_room_inset(level, &rooms[ri], 1, Terrain::Empty);

    let min_dim = (rect.width() + 1).min(rect.height() + 1);
    // L53：(int)Math.floor(0.2f*(minDim+3))
    let passage_width = (0.2f32 * (min_dim + 3) as f32).floor() as i32;
    fill_room_inset(level, &rooms[ri], passage_width + 1, Terrain::Wall);

    if min_dim >= 10 {
        // L57：centerDecoTiles() = REGION_DECO_ALT
        fill_room_inset(level, &rooms[ri], passage_width + 2, Terrain::RegionDecoAlt);
        let mut center = room_center(rng, &rect);
        let mut x_dir = 0;
        let mut y_dir = 0;

        // L61-L78：内门尽量开在离外门更远的一侧
        if int(rng, 2) == 0 {
            let mid = (rect.left + rect.right) as f32 / 2.0;
            x_dir = if (center.x as f32) < mid {
                1
            } else if (center.x as f32) > mid {
                -1
            } else if int(rng, 2) == 0 {
                1
            } else {
                -1
            };
        } else {
            let mid = (rect.top + rect.bottom) as f32 / 2.0;
            y_dir = if (center.y as f32) < mid {
                1
            } else if (center.y as f32) > mid {
                -1
            } else if int(rng, 2) == 0 {
                1
            } else {
                -1
            };
        }

        // L80-L81：中心特殊地板；placeCenterDetail 的奖励掉落属物品域（TODO）
        level.set_terrain(center, Terrain::EmptySp);

        // L83-L90：向选定方向铺路直到内墙，把内墙凿成门
        center.x += x_dir;
        center.y += y_dir;
        while level.terrain(center) != Terrain::Wall {
            level.set_terrain(center, Terrain::EmptySp);
            center.x += x_dir;
            center.y += y_dir;
        }
        level.set_terrain(center, Terrain::Door);
    }

    upgrade_doors(rooms, ri, DoorType::Regular);
}

// ---------------------------------------------------------------------------
// CircleBasinRoom（CircleBasinRoom.java）
// ---------------------------------------------------------------------------

/// `CircleBasinRoom.paint`（L72-L116）：椭圆盆地 + 深渊环 + 十字栈桥，
/// Patch 噪声在空地上蓄水（水上沿的墙换装饰墙）。
/// `PatchRoom` 参数（L52-L70）：fill 0.5、clustering 5、`ensurePath`/`cleanEdges` 均假
/// —— `setupPatch` 直接退化为一次 `Patch.generate`（PatchRoom.java L86-L88）。
fn paint_circle_basin(
    rng: &mut impl Rng,
    level: &mut crate::levels::Level,
    rooms: &mut [Room],
    ri: usize,
) {
    let rect = rooms[ri].rect;
    let (w, h) = (rect.width() + 1, rect.height() + 1);
    fill_room(level, &rooms[ri], Terrain::Wall);
    fill_ellipse_inset(level, &rooms[ri], 1, Terrain::Empty);

    // L78-L85：门 REGULAR，并从门向内凿出通往椭圆的通道
    let connected: Vec<(usize, IVec2)> = rooms[ri]
        .connected
        .iter()
        .map(|&(n, d)| (n, d.expect("place_doors 已就位").pos))
        .collect();
    for (n, door) in &connected {
        let mut d = rooms[ri].door_to(*n).expect("门已就位");
        d.set(DoorType::Regular);
        set_door(rooms, ri, *n, d);
        let steps = if door.x == rect.left || door.x == rect.right {
            w / 2
        } else {
            h / 2
        };
        draw_inside(level, &rect, *door, steps, Terrain::Empty);
    }

    fill_ellipse_inset(level, &rooms[ri], 3, Terrain::Chasm);

    // L89-L95：十字栈桥
    draw_line(
        level,
        IVec2::new(rect.left + w / 2, rect.top + 3),
        IVec2::new(rect.left + w / 2, rect.bottom - 3),
        Terrain::EmptySp,
    );
    draw_line(
        level,
        IVec2::new(rect.left + 3, rect.top + h / 2),
        IVec2::new(rect.right - 3, rect.top + h / 2),
        Terrain::EmptySp,
    );

    // L97-L101：特大房中心 3×3 平台 + 中央石柱。
    // 尺寸恒奇（resize 覆写）→ center() 不消耗随机数
    if w > 11 || h > 11 {
        let center = room_center(rng, &rect);
        fill_grid(level, center.x - 1, center.y - 1, 3, 3, Terrain::EmptySp);
        level.set_terrain(center, Terrain::Wall);
    }

    // L103-L114：Patch 蓄水（只替换 EMPTY），水上沿的墙换 WALL_DECO
    let patch = patch::generate(rng, (w - 2) as usize, (h - 2) as usize, 0.5, 5, true);
    for y in (rect.top + 1)..rect.bottom {
        for x in (rect.left + 1)..rect.right {
            let p = IVec2::new(x, y);
            let idx = ((x - rect.left - 1) + (y - rect.top - 1) * (w - 2)) as usize;
            if level.terrain(p) == Terrain::Empty && patch[idx] {
                level.set_terrain(p, Terrain::Water);
                let above = IVec2::new(x, y - 1);
                if level.terrain(above) == Terrain::Wall {
                    level.set_terrain(above, Terrain::WallDeco);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// BurnedRoom（BurnedRoom.java）
// ---------------------------------------------------------------------------

/// `BurnedRoom.paint`（L68-L118）：Patch 噪声铺过火痕迹——
/// 空地/余烬/明陷阱/暗陷阱/失效陷阱各 1/5。
/// 降级：`level.setTrap(BurningTrap)` 属陷阱行为域（只铺地形，TODO）；
/// `TrapMechanism.revealHiddenTrapChance()` 无饰品系统恒 0（同 paintTraps）。
fn paint_burned(
    rng: &mut impl Rng,
    level: &mut crate::levels::Level,
    rooms: &mut [Room],
    ri: usize,
) {
    let rect = rooms[ri].rect;
    let (w, h) = (rect.width() + 1, rect.height() + 1);
    fill_room(level, &rooms[ri], Terrain::Wall);
    fill_room_inset(level, &rooms[ri], 1, Terrain::Empty);
    upgrade_doors(rooms, ri, DoorType::Regular);

    // L46-L51：8×8 以上每格宽/高扣 3% 填充率
    let fill = (1.48 - (w + h) as f32 * 0.03).min(1.0);
    // PatchRoom.setupPatch（L86-L88）：ensurePath=false → 单次生成；cleanEdges=false
    let patch = patch::generate(rng, (w - 2) as usize, (h - 2) as usize, fill, 2, true);

    // L78-L117：行优先逐格掷 Int(5)
    for y in (rect.top + 1)..rect.bottom {
        for x in (rect.left + 1)..rect.right {
            let idx = ((x - rect.left - 1) + (y - rect.top - 1) * (w - 2)) as usize;
            if !patch[idx] {
                continue;
            }
            let terrain = match int(rng, 5) {
                1 => Terrain::Embers,
                2 => Terrain::Trap,
                // revealInc 恒 0 → case 3 恒暗陷阱
                3 => Terrain::SecretTrap,
                4 => Terrain::InactiveTrap,
                _ => Terrain::Empty,
            };
            level.set_terrain(IVec2::new(x, y), terrain);
        }
    }

    // canPlaceWater/Grass/Trap（L120-L133）：patch 覆盖格禁止后续水/草/陷阱刻画
    rooms[ri].deco_ban_patch = Some(patch);
}

// ---------------------------------------------------------------------------
// StripedRoom（StripedRoom.java）
// ---------------------------------------------------------------------------

/// `StripedRoom.paint`（L48-L73）：NORMAL 为特殊地板 + 高草条纹；
/// LARGE 为同心环交替。GIANT 掷不出（probs {2,1,0}），Java 无对应分支。
fn paint_striped(
    rng: &mut impl Rng,
    level: &mut crate::levels::Level,
    rooms: &mut [Room],
    ri: usize,
) {
    let rect = rooms[ri].rect;
    let (w, h) = (rect.width() + 1, rect.height() + 1);
    let RoomKind::Standard { size, .. } = rooms[ri].kind else {
        unreachable!("paint_striped 只接标准房")
    };
    fill_room(level, &rooms[ri], Terrain::Wall);
    upgrade_doors(rooms, ri, DoorType::Regular);

    match size {
        SizeCategory::Normal => {
            fill_room_inset(level, &rooms[ri], 1, Terrain::EmptySp);
            if w > h || (w == h && int(rng, 2) == 0) {
                let mut x = rect.left + 2;
                while x < rect.right {
                    fill_grid(level, x, rect.top + 1, 1, h - 2, Terrain::HighGrass);
                    x += 2;
                }
            } else {
                let mut y = rect.top + 2;
                while y < rect.bottom {
                    fill_grid(level, rect.left + 1, y, w - 2, 1, Terrain::HighGrass);
                    y += 2;
                }
            }
        }
        SizeCategory::Large => {
            let layers = (w.min(h) - 1) / 2;
            for i in 1..=layers {
                let terrain = if i % 2 == 1 {
                    Terrain::EmptySp
                } else {
                    Terrain::HighGrass
                };
                fill_room_inset(level, &rooms[ri], i, terrain);
            }
        }
        // sizeCatProbs {2,1,0} 掷不出 GIANT；对照 Java 无分支（内部保持全墙）
        SizeCategory::Giant => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::{Level, random::LevelRng, rooms::Door, rooms::connect};
    use rand::SeedableRng;

    fn rng(seed: u64) -> LevelRng {
        LevelRng::seed_from_u64(seed)
    }

    fn std_room(variant: StandardVariant, size: SizeCategory, rect: SpdRect) -> Room {
        let mut room = Room::new(RoomKind::Standard { variant, size });
        room.rect = rect;
        room
    }

    /// 计数某地形在关卡中出现的次数。
    fn count(level: &Level, t: Terrain) -> usize {
        level.map().iter().filter(|&&x| x == t).count()
    }

    /// 造一对已连接房间并手工放门（绕过 painter 的 `place_doors`）。
    fn link(rooms: &mut [Room], a: usize, b: usize, door: IVec2) {
        assert!(connect(rooms, a, b), "测试几何必须可连");
        set_door(rooms, a, b, Door::new(door));
    }

    /// 变体掷点分布：depth 1 权重表（管道 16/45 最常见，Burned 掷不出）、
    /// depth 2 表（Burned 可出现）。
    #[test]
    fn variant_roll_follows_rotation_table() {
        let mut r = rng(1);
        const N: usize = 45_000;
        let mut pipe = 0;
        let mut ring = 0;
        let mut burned_d1 = 0;
        for _ in 0..N {
            match roll_standard_variant(&mut r, 1) {
                StandardVariant::SewerPipe => pipe += 1,
                StandardVariant::Ring => ring += 1,
                StandardVariant::Burned => burned_d1 += 1,
                _ => {}
            }
        }
        // depth 1 权重和 45：管道 16/45 = 16000，环形 8/45 = 8000（±6σ 带）
        assert_eq!(burned_d1, 0, "BurnedRoom 在 depth 1 权重为 0");
        assert!((15200..=16800).contains(&pipe), "管道房应约 16/45，得 {pipe}");
        assert!((7300..=8700).contains(&ring), "环形房应约 8/45，得 {ring}");

        let burned_d2 = (0..N)
            .filter(|_| roll_standard_variant(&mut r, 2) == StandardVariant::Burned)
            .count();
        // depth 2 权重和 50：Burned 1/50 = 900（±6σ 带宽约 ±180）
        assert!((650..=1150).contains(&burned_d2), "Burned 应约 1/50，得 {burned_d2}");
    }

    /// 尺寸类别掷点：Empty 恒 NORMAL；SewerPipe {3,2,1} 三档都出；
    /// 预算截断（maxRoomValue）把高档清零，全零返回 None。
    #[test]
    fn size_category_roll_and_truncation() {
        let mut r = rng(2);
        for _ in 0..100 {
            assert_eq!(
                roll_size_category(&mut r, StandardVariant::Empty, 3),
                Some(SizeCategory::Normal)
            );
        }

        let mut seen = [0usize; 3];
        for _ in 0..6000 {
            let cat = roll_size_category(&mut r, StandardVariant::SewerPipe, 3).unwrap();
            seen[cat as usize] += 1;
        }
        // {3,2,1}：期望 3000/2000/1000，全部按 ±6σ 放宽
        assert!((2700..=3300).contains(&seen[0]), "NORMAL {seen:?}");
        assert!((1700..=2300).contains(&seen[1]), "LARGE {seen:?}");
        assert!((800..=1200).contains(&seen[2]), "GIANT {seen:?}");

        // 预算 2 → GIANT 清零；预算 1 → 只剩 NORMAL
        for _ in 0..200 {
            let cat = roll_size_category(&mut r, StandardVariant::SewerPipe, 2).unwrap();
            assert_ne!(cat, SizeCategory::Giant);
            assert_eq!(
                roll_size_category(&mut r, StandardVariant::SewerPipe, 1),
                Some(SizeCategory::Normal)
            );
        }
        // CircleBasin {0,3,1}：预算 1 全零 → None（调用方重抽变体）
        assert_eq!(roll_size_category(&mut r, StandardVariant::CircleBasin, 1), None);
    }

    /// EmptyRoom.paint：墙圈 + 全空内部。
    #[test]
    fn paint_empty_room() {
        let mut level = Level::new(12, 12, 1);
        let mut rooms = vec![
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(2, 2, 8, 8),
            ),
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(8, 2, 11, 8),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(8, 5));
        paint_standard(&mut rng(3), &mut level, &mut rooms, 0, StandardVariant::Empty);

        assert_eq!(count(&level, Terrain::Empty), 5 * 5, "内部 5×5 全空");
        assert_eq!(level.terrain(IVec2::new(2, 2)), Terrain::Wall);
        assert_eq!(rooms[0].door_to(1).unwrap().kind, DoorType::Regular);
    }

    /// `SewerPipeRoom` 单门：直角水道 + 蚀墙走道，非管道邻居门型 REGULAR。
    #[test]
    fn paint_sewer_pipe_single_door() {
        let mut level = Level::new(20, 14, 1);
        let mut rooms = vec![
            std_room(
                StandardVariant::SewerPipe,
                SizeCategory::Normal,
                SpdRect::new(2, 2, 10, 10),
            ),
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(10, 2, 16, 10),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(10, 6));
        paint_standard(
            &mut rng(4),
            &mut level,
            &mut rooms,
            0,
            StandardVariant::SewerPipe,
        );

        // 门内两步 (8,6) → 中心 (6,6)：三格水线
        for x in 6..=8 {
            assert_eq!(level.terrain(IVec2::new(x, 6)), Terrain::Water, "x={x}");
        }
        assert_eq!(count(&level, Terrain::Water), 3);
        // 水道 8 邻域的墙全部蚀空：x∈[5,9] × y∈[5,7] 除水外 12 格 EMPTY
        assert_eq!(count(&level, Terrain::Empty), 12);
        assert_eq!(level.terrain(IVec2::new(5, 5)), Terrain::Empty);
        assert_eq!(level.terrain(IVec2::new(9, 7)), Terrain::Empty);
        // 外圈墙未被蚀穿；非管道邻居 → REGULAR 门
        assert_eq!(level.terrain(IVec2::new(10, 6)), Terrain::Wall, "门位地形由 paint_doors 决定");
        assert_eq!(rooms[0].door_to(1).unwrap().kind, DoorType::Regular);
        // 管道房整房禁水刻画
        assert!(!rooms[0].can_place_water(IVec2::new(5, 5)));
    }

    /// `SewerPipeRoom` 与管道邻居：门旁 3×3 开敞 + 过门水线，门型 WATER。
    #[test]
    fn paint_sewer_pipe_neighbour_opens_shared_door() {
        let mut level = Level::new(24, 14, 1);
        let mut rooms = vec![
            std_room(
                StandardVariant::SewerPipe,
                SizeCategory::Normal,
                SpdRect::new(2, 2, 10, 10),
            ),
            std_room(
                StandardVariant::SewerPipe,
                SizeCategory::Normal,
                SpdRect::new(10, 2, 18, 10),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(10, 6));
        paint_standard(
            &mut rng(5),
            &mut level,
            &mut rooms,
            0,
            StandardVariant::SewerPipe,
        );

        assert_eq!(rooms[0].door_to(1).unwrap().kind, DoorType::Water);
        assert_eq!(rooms[1].door_to(0).unwrap().kind, DoorType::Water, "门对象两侧同步");
        // 过门水线（横穿墙位）；3×3 开敞的角格是 EMPTY
        for x in 9..=11 {
            assert_eq!(level.terrain(IVec2::new(x, 6)), Terrain::Water, "x={x}");
        }
        assert_eq!(level.terrain(IVec2::new(9, 5)), Terrain::Empty);
        assert_eq!(level.terrain(IVec2::new(11, 7)), Terrain::Empty);
    }

    /// `RingRoom` 最小尺寸（7×7）：环形走道 + 单格实心内芯，无内门。
    #[test]
    fn paint_ring_small() {
        let mut level = Level::new(12, 12, 1);
        let mut rooms = vec![
            std_room(
                StandardVariant::Ring,
                SizeCategory::Normal,
                SpdRect::new(1, 1, 7, 7),
            ),
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(7, 1, 10, 7),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(7, 4));
        paint_standard(&mut rng(6), &mut level, &mut rooms, 0, StandardVariant::Ring);

        // minDim 7 → passage 2 → inset 3 填墙 = 单格 (4,4)
        assert_eq!(level.terrain(IVec2::new(4, 4)), Terrain::Wall, "内芯");
        assert_eq!(count(&level, Terrain::Empty), 5 * 5 - 1, "环形走道");
        assert_eq!(count(&level, Terrain::Door), 0, "小环无内门");
        assert_eq!(rooms[0].door_to(1).unwrap().kind, DoorType::Regular);
    }

    /// `RingRoom` 大房（11×11）：装饰内芯 + 中心特殊地板 + 一扇内门。
    #[test]
    fn paint_ring_large_opens_inner_door() {
        let mut level = Level::new(14, 14, 1);
        let mut rooms = vec![
            std_room(
                StandardVariant::Ring,
                SizeCategory::Large,
                SpdRect::new(1, 1, 11, 11),
            ),
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(11, 1, 13, 11),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(11, 6));
        paint_standard(&mut rng(7), &mut level, &mut rooms, 0, StandardVariant::Ring);

        // minDim 11 → passage 2：内墙圈 [4,8]²，装饰芯 [5,7]²（3×3）
        // 中心 (6,6) EMPTY_SP + 通往内墙的 1 格 EMPTY_SP 路 + 内墙上 1 扇门
        assert_eq!(level.terrain(IVec2::new(6, 6)), Terrain::EmptySp, "中心占位");
        assert_eq!(count(&level, Terrain::EmptySp), 2, "中心 + 一格路");
        assert_eq!(count(&level, Terrain::Door), 1, "恰一扇内门");
        assert_eq!(count(&level, Terrain::RegionDecoAlt), 7, "3×3 芯减中心减路");
        assert_eq!(rooms[0].door_to(1).unwrap().kind, DoorType::Regular);
    }

    /// CircleBasinRoom（11×11）：椭圆盆地 + 深渊 + 十字栈桥，门 REGULAR。
    #[test]
    fn paint_circle_basin_structure() {
        let mut level = Level::new(14, 14, 1);
        let mut rooms = vec![
            std_room(
                StandardVariant::CircleBasin,
                SizeCategory::Large,
                SpdRect::new(1, 1, 11, 11),
            ),
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(11, 1, 13, 11),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(11, 6));
        paint_standard(
            &mut rng(8),
            &mut level,
            &mut rooms,
            0,
            StandardVariant::CircleBasin,
        );

        // 椭圆外的足迹四角保持墙
        for p in [IVec2::new(2, 2), IVec2::new(10, 2), IVec2::new(2, 10), IVec2::new(10, 10)] {
            assert_eq!(level.terrain(p), Terrain::Wall, "{p:?} 椭圆外");
        }
        assert!(count(&level, Terrain::Chasm) > 0, "盆地深渊");
        // 十字栈桥：竖桥 x=6 y∈[4,8]、横桥 y=6 x∈[4,8]
        for d in 4..=8 {
            assert_eq!(level.terrain(IVec2::new(6, d)), Terrain::EmptySp, "竖桥 y={d}");
            assert_eq!(level.terrain(IVec2::new(d, 6)), Terrain::EmptySp, "横桥 x={d}");
        }
        assert_eq!(rooms[0].door_to(1).unwrap().kind, DoorType::Regular);
        // 11×11 不触发 >11 的中心平台（恰在阈值上）
        assert_ne!(level.terrain(IVec2::new(6, 6)), Terrain::Wall);
    }

    /// BurnedRoom（8×8，fill=1.0 → 全内部过火）：地形五选一 + 落位掩码。
    #[test]
    fn paint_burned_covers_interior_with_scorch_set() {
        let mut level = Level::new(12, 12, 2);
        let mut rooms = vec![
            std_room(
                StandardVariant::Burned,
                SizeCategory::Normal,
                SpdRect::new(1, 1, 8, 8),
            ),
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(8, 1, 11, 8),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(8, 4));
        paint_standard(&mut rng(9), &mut level, &mut rooms, 0, StandardVariant::Burned);

        // 8×8 格：fill = min(1, 1.48-0.48) = 1.0 → 36 个内部格全过火
        let patch = rooms[0].deco_ban_patch.as_ref().expect("过火掩码已存");
        assert_eq!(patch.len(), 36);
        assert!(patch.iter().all(|&b| b), "fill 1.0 应全为过火格");

        let mut in_set = 0;
        for y in 2..8 {
            for x in 2..8 {
                let t = level.terrain(IVec2::new(x, y));
                assert!(
                    matches!(
                        t,
                        Terrain::Empty
                            | Terrain::Embers
                            | Terrain::Trap
                            | Terrain::SecretTrap
                            | Terrain::InactiveTrap
                    ),
                    "({x},{y}) 过火地形集之外：{t:?}"
                );
                in_set += 1;
            }
        }
        assert_eq!(in_set, 36);
        // seed 9 下五种地形都应出现（36 次 Int(5) 全缺某类概率 ~0.03%）
        for t in [
            Terrain::Embers,
            Terrain::Trap,
            Terrain::SecretTrap,
            Terrain::InactiveTrap,
        ] {
            assert!(count(&level, t) > 0, "{t:?} 应出现");
        }
        // 过火格禁止后续水/草/陷阱
        assert!(!rooms[0].can_place_water(IVec2::new(4, 4)));
        assert!(!rooms[0].can_place_trap(IVec2::new(4, 4)));
        assert_eq!(rooms[0].door_to(1).unwrap().kind, DoorType::Regular);
    }

    /// `StripedRoom` NORMAL（9×5 横向）：奇数列高草条纹、余为特殊地板。
    #[test]
    fn paint_striped_normal_stripes() {
        let mut level = Level::new(13, 9, 1);
        let mut rooms = vec![
            std_room(
                StandardVariant::Striped,
                SizeCategory::Normal,
                SpdRect::new(1, 1, 9, 5),
            ),
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(9, 1, 12, 5),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(9, 3));
        paint_standard(&mut rng(10), &mut level, &mut rooms, 0, StandardVariant::Striped);

        // w 9 > h 5 → 竖条纹 x ∈ {3,5,7}，每条 h-2 = 3 格
        assert_eq!(count(&level, Terrain::HighGrass), 9);
        assert_eq!(count(&level, Terrain::EmptySp), 7 * 3 - 9);
        for x in [3, 5, 7] {
            for y in 2..=4 {
                assert_eq!(level.terrain(IVec2::new(x, y)), Terrain::HighGrass, "({x},{y})");
            }
        }
        assert_eq!(level.terrain(IVec2::new(2, 2)), Terrain::EmptySp);
    }

    /// `StripedRoom` LARGE（11×11）：同心环交替（奇圈特殊地板、偶圈高草）。
    #[test]
    fn paint_striped_large_concentric_rings() {
        let mut level = Level::new(14, 14, 1);
        let mut rooms = vec![
            std_room(
                StandardVariant::Striped,
                SizeCategory::Large,
                SpdRect::new(1, 1, 11, 11),
            ),
            std_room(
                StandardVariant::Empty,
                SizeCategory::Normal,
                SpdRect::new(11, 1, 13, 11),
            ),
        ];
        link(&mut rooms, 0, 1, IVec2::new(11, 6));
        paint_standard(&mut rng(11), &mut level, &mut rooms, 0, StandardVariant::Striped);

        // 由外向内：圈 1 EMPTY_SP、圈 2 HIGH_GRASS、圈 3 EMPTY_SP、
        // 圈 4 HIGH_GRASS、中心格 EMPTY_SP
        for (p, expect) in [
            (IVec2::new(2, 2), Terrain::EmptySp),
            (IVec2::new(3, 3), Terrain::HighGrass),
            (IVec2::new(4, 4), Terrain::EmptySp),
            (IVec2::new(5, 5), Terrain::HighGrass),
            (IVec2::new(6, 6), Terrain::EmptySp),
            (IVec2::new(3, 6), Terrain::HighGrass),
            (IVec2::new(6, 2), Terrain::EmptySp),
        ] {
            assert_eq!(level.terrain(p), expect, "{p:?}");
        }
    }

    /// `room_center`：偶数格尺寸不掷点；奇数格（SPD 单位奇）掷 `Int(2)` 偏置。
    #[test]
    fn room_center_consumes_rng_only_on_odd_spd_dims() {
        // 7×7 格（SPD 宽 6 偶）→ 恒 (4,4)，不消耗随机数
        // （双生 RNG 对照：r1 过 room_center 后下一浮点应与未动的 r0 相同）
        let rect = SpdRect::new(1, 1, 7, 7);
        let mut r0 = rng(12);
        let mut r1 = rng(12);
        assert_eq!(room_center(&mut r1, &rect), IVec2::new(4, 4));
        assert_eq!(
            crate::levels::random::float(&mut r1),
            crate::levels::random::float(&mut r0),
            "流位未动"
        );

        // 8×7 格（SPD 宽 7 奇）→ x 有 0/1 偏置，两个值都会出现
        let rect = SpdRect::new(0, 0, 7, 6);
        let mut seen = [false; 2];
        let mut r2 = rng(13);
        for _ in 0..64 {
            let c = room_center(&mut r2, &rect);
            assert!(c.x == 3 || c.x == 4);
            assert_eq!(c.y, 3);
            seen[(c.x - 3) as usize] = true;
        }
        assert!(seen[0] && seen[1]);
    }
}
