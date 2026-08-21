//! 关卡构建器，对照 `core/.../levels/builders/{Builder,RegularBuilder,LoopBuilder}.java`。
//!
//! # SPD 角度约定（沉默陷阱）
//!
//! Builder 全程用 **0–360 度、12 点钟方向为 0、顺时针增长**（Builder.java L145、L160-L161）。
//! 千万不要换成 `Vec2::angle_to` 的弧度/x 轴基准/逆时针语义。
//! y 轴向下（屏幕坐标），因此 90° 指向 +x（3 点钟）、180° 指向 +y（6 点钟）。

use bevy::math::{IVec2, Vec2};
use rand::Rng;

use crate::levels::{
    random::{chances, element, float, float_range, int, shuffle},
    rect::SpdRect,
    rooms::{Room, RoomKind, Side, add_neighbour, clear_connections, connect},
};

/// Builder.java L143 的 `A`：度/弧度换算系数。
const DEG_PER_RAD: f64 = 180.0 / std::f64::consts::PI;

/// Java `Math.round(float)` 语义：`floor(x + 0.5)`。
/// 与 Rust `round()`（半数远离零）在负半数处不同（Java：-2.5 → -2；Rust：-3），
/// 必须按 Java 语义实现。
pub(crate) fn java_round_f32(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// Java `Math.round(double)` 语义（同上，双精度版）。
pub(crate) fn java_round_f64(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

/// `GameMath.gate(min, value, max)`（GameMath.java L37-L44）。
/// 与 `clamp` 不同：`min > max` 时不 panic，而是返回 `min`。
pub(crate) fn gate(min: i32, value: i32, max: i32) -> i32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// `Builder.findNeighbours`（Builder.java L43-L50）：全房间两两建邻接。
pub(crate) fn find_neighbours(rooms: &mut [Room]) {
    for i in 0..rooms.len() {
        for j in (i + 1)..rooms.len() {
            add_neighbour(rooms, i, j);
        }
    }
}

/// `Builder.angleBetweenRooms`（L146-L150）：两房中心连线的 SPD 角度。
pub(crate) fn angle_between_rooms(rooms: &[Room], from: usize, to: usize) -> f32 {
    let f = rooms[from].rect;
    let t = rooms[to].rect;
    angle_between_points(
        Vec2::new(
            (f.left + f.right) as f32 / 2.0,
            (f.top + f.bottom) as f32 / 2.0,
        ),
        Vec2::new(
            (t.left + t.right) as f32 / 2.0,
            (t.top + t.bottom) as f32 / 2.0,
        ),
    )
}

/// `Builder.angleBetweenPoints`（L152-L158）：返回域 (-180, 180]。
/// 正上 0°、正右 90°、正下 180°、正左 -90°（调用方按需 +360 归一）。
pub(crate) fn angle_between_points(from: Vec2, to: Vec2) -> f32 {
    // Java 按 float 除法求斜率：垂直时得 ±Inf，atan(±Inf) = ±π/2，语义自洽
    let m = f64::from((to.y - from.y) / (to.x - from.x));
    let mut angle = (DEG_PER_RAD * (m.atan() + std::f64::consts::FRAC_PI_2)) as f32;
    if from.x > to.x {
        angle -= 180.0;
    }
    angle
}

/// `Builder.findFreeSpace`（L53-L141）：从 `start` 向四周扩展出不与 `collision`
/// 中任何房间相交的最大矩形（SPD 闭区间语义）。
///
/// 注意：`inside`/`cur_diff` 跨房间累积不重置是 Java 原版行为（L75-L76 声明于
/// 循环外），必须原样保留。
pub(crate) fn find_free_space(
    rng: &mut impl Rng,
    start: IVec2,
    rooms: &[Room],
    collision: &[usize],
    max_size: i32,
) -> SpdRect {
    let mut space = SpdRect::new(
        start.x - max_size,
        start.y - max_size,
        start.x + max_size,
        start.y + max_size,
    );

    // Java：浅拷贝 collision 列表后迭代删减
    let mut colliding: Vec<usize> = collision.to_vec();
    loop {
        // 剔除空房间与当前已不相交的房间（L61-L70）
        colliding.retain(|&ri| {
            let room = &rooms[ri].rect;
            !(room.is_empty()
                || space.left.max(room.left) >= space.right.min(room.right)
                || space.top.max(room.top) >= space.bottom.min(room.bottom))
        });

        // 找与 start 最近的相交房间（L72-L105）
        let mut closest_room: Option<usize> = None;
        let mut closest_diff = i32::MAX;
        let mut inside = true;
        let mut cur_diff = 0;
        for &ri in &colliding {
            let cur = &rooms[ri].rect;

            if start.x <= cur.left {
                inside = false;
                cur_diff += cur.left - start.x;
            } else if start.x >= cur.right {
                inside = false;
                cur_diff += start.x - cur.right;
            }

            if start.y <= cur.top {
                inside = false;
                cur_diff += cur.top - start.y;
            } else if start.y >= cur.bottom {
                inside = false;
                cur_diff += start.y - cur.bottom;
            }

            if inside {
                // start 在某房间内部：退化为单点
                return SpdRect::new(start.x, start.y, start.x, start.y);
            }

            if cur_diff < closest_diff {
                closest_diff = cur_diff;
                closest_room = Some(ri);
            }
        }

        // 向损失最小的方向收缩 space（L107-L135）
        if let Some(ci) = closest_room {
            let cr = rooms[ci].rect;

            let mut w_diff = i32::MAX;
            if cr.left >= start.x {
                w_diff = (space.right - cr.left) * (space.height() + 1);
            } else if cr.right <= start.x {
                w_diff = (cr.right - space.left) * (space.height() + 1);
            }

            let mut h_diff = i32::MAX;
            if cr.top >= start.y {
                h_diff = (space.bottom - cr.top) * (space.width() + 1);
            } else if cr.bottom <= start.y {
                h_diff = (cr.bottom - space.top) * (space.width() + 1);
            }

            if w_diff < h_diff || (w_diff == h_diff && int(rng, 2) == 0) {
                if cr.left >= start.x && cr.left < space.right {
                    space.right = cr.left;
                }
                if cr.right <= start.x && cr.right > space.left {
                    space.left = cr.right;
                }
            } else {
                if cr.top >= start.y && cr.top < space.bottom {
                    space.bottom = cr.top;
                }
                if cr.bottom <= start.y && cr.bottom > space.top {
                    space.top = cr.bottom;
                }
            }
            if let Some(pos) = colliding.iter().position(|&x| x == ci) {
                colliding.remove(pos);
            }
        } else {
            colliding.clear();
        }

        if colliding.is_empty() {
            break;
        }
    }

    space
}

/// 摆放时相对 `prev` 的离开边（Builder.placeRoom 内的 `direction` 局部变量）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceDir {
    Top,
    Bottom,
    Right,
    Left,
}

/// `Builder.placeRoom`（L164-L257）：沿给定角度把 `next` 摆到 `prev` 旁并连接。
/// 成功返回两房中心的实际夹角；失败返回 `None`（Java 用 -1 哨兵，Rust 用 Option
/// 避免与合法负角撞值）。
pub(crate) fn place_room(
    rng: &mut impl Rng,
    rooms: &mut [Room],
    collision: &[usize],
    prev_i: usize,
    next_i: usize,
    angle: f32,
) -> Option<f32> {
    // L167-L170：角度归一化到 [0, 360)
    let mut angle = angle % 360.0;
    if angle < 0.0 {
        angle += 360.0;
    }

    let prev = rooms[prev_i].rect;
    let prev_center = Vec2::new(
        (prev.left + prev.right) as f32 / 2.0,
        (prev.top + prev.bottom) as f32 / 2.0,
    );

    // L174-L176：直线 y = mx + b。SPD 角 0 在 12 点钟，故先 +π/2 旋转到数学角
    let m = (f64::from(angle) / DEG_PER_RAD + std::f64::consts::FRAC_PI_2).tan();
    let b = f64::from(prev_center.y) - m * f64::from(prev_center.x);

    // L178-L197：判定直线离开 prev 的边与出发点
    let (direction, mut start) = if m.abs() >= 1.0 {
        if !(90.0..=270.0).contains(&angle) {
            (
                PlaceDir::Top,
                IVec2::new(java_round_f64((f64::from(prev.top) - b) / m), prev.top),
            )
        } else {
            (
                PlaceDir::Bottom,
                IVec2::new(
                    java_round_f64((f64::from(prev.bottom) - b) / m),
                    prev.bottom,
                ),
            )
        }
    } else if angle < 180.0 {
        (
            PlaceDir::Right,
            IVec2::new(prev.right, java_round_f64(m * f64::from(prev.right) + b)),
        )
    } else {
        (
            PlaceDir::Left,
            IVec2::new(prev.left, java_round_f64(m * f64::from(prev.left) + b)),
        )
    };

    // L199-L204：出发点夹到可开门区间（避开四角）
    if matches!(direction, PlaceDir::Top | PlaceDir::Bottom) {
        start.x = gate(prev.left + 1, start.x, prev.right - 1);
    } else {
        start.y = gate(prev.top + 1, start.y, prev.bottom - 1);
    }

    // L206-L210：空间检查与尺寸设定（width()+1：SPD 单位宽 → 可用格数）
    let max_dim = rooms[next_i].max_width().max(rooms[next_i].max_height());
    let space = find_free_space(rng, start, rooms, collision, max_dim);
    if !rooms[next_i].set_size_with_limit(rng, space.width() + 1, space.height() + 1) {
        return None;
    }

    // L212-L234：由直线方程和已知尺寸求理想中心并落位。
    // Java 中 targetCenter 为 float、直线代入为 double，此处逐一对应。
    let next_w = rooms[next_i].width();
    let next_h = rooms[next_i].height();
    match direction {
        PlaceDir::Top => {
            let target_y = prev.top as f32 - (next_h - 1) as f32 / 2.0;
            let target_x = ((f64::from(target_y) - b) / m) as f32;
            rooms[next_i].set_pos(
                java_round_f32(target_x - (next_w - 1) as f32 / 2.0),
                prev.top - (next_h - 1),
            );
        }
        PlaceDir::Bottom => {
            let target_y = prev.bottom as f32 + (next_h - 1) as f32 / 2.0;
            let target_x = ((f64::from(target_y) - b) / m) as f32;
            rooms[next_i].set_pos(
                java_round_f32(target_x - (next_w - 1) as f32 / 2.0),
                prev.bottom,
            );
        }
        PlaceDir::Right => {
            let target_x = prev.right as f32 + (next_w - 1) as f32 / 2.0;
            let target_y = (m * f64::from(target_x) + b) as f32;
            rooms[next_i].set_pos(
                prev.right,
                java_round_f32(target_y - (next_h - 1) as f32 / 2.0),
            );
        }
        PlaceDir::Left => {
            let target_x = prev.left as f32 - (next_w - 1) as f32 / 2.0;
            let target_y = (m * f64::from(target_x) + b) as f32;
            rooms[next_i].set_pos(
                prev.left - (next_w - 1),
                java_round_f32(target_y - (next_h - 1) as f32 / 2.0),
            );
        }
    }

    // L236-L249：保证与 prev 有可开门重叠（边角至少让出 2），并压回可用空间
    if matches!(direction, PlaceDir::Top | PlaceDir::Bottom) {
        let next = rooms[next_i].rect;
        if next.right < prev.left + 2 {
            rooms[next_i].shift(prev.left + 2 - next.right, 0);
        } else if next.left > prev.right - 2 {
            rooms[next_i].shift(prev.right - 2 - next.left, 0);
        }
        let next = rooms[next_i].rect;
        if next.right > space.right {
            rooms[next_i].shift(space.right - next.right, 0);
        } else if next.left < space.left {
            rooms[next_i].shift(space.left - next.left, 0);
        }
    } else {
        let next = rooms[next_i].rect;
        if next.bottom < prev.top + 2 {
            rooms[next_i].shift(0, prev.top + 2 - next.bottom);
        } else if next.top > prev.bottom - 2 {
            rooms[next_i].shift(0, prev.bottom - 2 - next.top);
        }
        let next = rooms[next_i].rect;
        if next.bottom > space.bottom {
            rooms[next_i].shift(0, space.bottom - next.bottom);
        } else if next.top < space.top {
            rooms[next_i].shift(0, space.top - next.top);
        }
    }

    // L251-L256
    if connect(rooms, next_i, prev_i) {
        Some(angle_between_rooms(rooms, prev_i, next_i))
    } else {
        None
    }
}

/// `RegularBuilder.setupRooms` 的分类结果。
struct RoomSetup {
    entrance: Option<usize>,
    exit: Option<usize>,
    main_path: Vec<usize>,
    multi_connections: Vec<usize>,
    single_connections: Vec<usize>,
}

/// `RegularBuilder.setupRooms`（RegularBuilder.java L89-L130）。
/// Shop 分类（L103-L104）M1 未移植。
fn setup_rooms(
    rng: &mut impl Rng,
    rooms: &mut [Room],
    path_length: f32,
    path_len_jitter_chances: &[f32],
) -> RoomSetup {
    for room in rooms.iter_mut() {
        room.rect.set_empty();
    }

    let mut entrance = None;
    let mut exit = None;
    let mut multi = Vec::new();
    let mut single = Vec::new();
    for (i, room) in rooms.iter().enumerate() {
        if room.is_entrance() {
            entrance = Some(i);
        } else if room.is_exit() {
            exit = Some(i);
        } else if room.max_connections(Side::All) > 1 {
            multi.push(i);
        } else {
            single.push(i);
        }
    }

    // L112-L117：大房间按 connectionWeight 复制加权（更可能进主环）
    // → 洗牌 → LinkedHashSet 保序去重 → 再洗牌
    weight_rooms(rooms, &mut multi);
    shuffle(rng, &mut multi);
    dedup_preserve_order(&mut multi);
    shuffle(rng, &mut multi);

    // L119：主路径房间数 = 多连接房数 × pathLength + 权重表加成
    //（jitter {0,0,0,1} 恒 +3；chances 失败对应 Java 的 -1）
    let mut on_main_path = (multi.len() as f32 * path_length) as i32
        + chances(rng, path_len_jitter_chances).map_or(-1, |i| i as i32);

    let mut main_path = Vec::new();
    while on_main_path > 0 && !multi.is_empty() {
        let r = multi.remove(0);
        // L122-L127：StandardRoom 按 sizeFactor 扣减（LARGE=2、GIANT=3），
        // 非标准房扣 1（size_factor 对其恒 1，合并写法）
        on_main_path -= rooms[r].size_factor();
        main_path.push(r);
    }

    RoomSetup {
        entrance,
        exit,
        main_path,
        multi_connections: multi,
        single_connections: single,
    }
}

/// `RegularBuilder.weightRooms`（L134-L141）：标准房按 `connectionWeight`
/// （= sizeFactor²，LARGE ×4、GIANT ×9）复制加权。
fn weight_rooms(rooms: &[Room], list: &mut Vec<usize>) {
    for ri in list.clone() {
        // Java 判 instanceof StandardRoom；Entrance/Exit 也是 StandardRoom 子类
        // （权重恒 1 → 0 份复制），Special/Secret/Tunnel 不是（跳过 ≡ 0 份复制）
        if rooms[ri].kind != RoomKind::Tunnel {
            for _ in 1..rooms[ri].connection_weight() {
                list.push(ri);
            }
        }
    }
}

/// Java `LinkedHashSet` 去重语义：保留首次出现顺序。
fn dedup_preserve_order(list: &mut Vec<usize>) {
    let mut seen = Vec::new();
    list.retain(|&x| {
        if seen.contains(&x) {
            false
        } else {
            seen.push(x);
            true
        }
    });
}

/// `RegularBuilder` 的路径/分支公共参数（RegularBuilder.java L49-L75 默认值），
/// `LoopBuilder` 与 `FigureEightBuilder` 共用。
#[derive(Debug, Clone, Copy)]
struct RegularParams {
    /// L50：主路径占多连接房的比例
    path_length: f32,
    /// L52：主路径长度加成权重表（{0,0,0,1} 恒 +3）
    path_len_jitter_chances: [f32; 4],
    /// L61：主路径隧道数权重表
    path_tunnel_chances: [f32; 3],
    /// L62：支路隧道数权重表
    branch_tunnel_chances: [f32; 3],
    /// L71：邻接房额外连门几率（两侧各判一次 → 实际约 51%）
    extra_connection_chance: f32,
}

impl Default for RegularParams {
    fn default() -> Self {
        Self {
            path_length: 0.25,
            path_len_jitter_chances: [0.0, 0.0, 0.0, 1.0],
            path_tunnel_chances: [2.0, 2.0, 1.0],
            branch_tunnel_chances: [1.0, 1.0, 0.0],
            extra_connection_chance: 0.30,
        }
    }
}

/// 环形曲线参数与方程，`LoopBuilder` 与 `FigureEightBuilder` 共用
/// （LoopBuilder.java L36-L68 与 FigureEightBuilder.java L33-L67 逐式相同）。
#[derive(Debug, Clone, Copy)]
struct LoopShape {
    exponent: i32,
    intensity: f32,
    offset: f32,
}

impl LoopShape {
    /// `setLoopShape` 的取模语义（LoopBuilder.java L49-L54）。
    fn new(exponent: i32, intensity: f32, offset: f32) -> Self {
        Self {
            exponent: exponent.abs(),
            intensity: intensity % 1.0,
            offset: offset % 0.5,
        }
    }

    /// `targetAngle`（LoopBuilder.java L56-L62）。
    fn target_angle(&self, percent_along: f32) -> f32 {
        let p = f64::from(percent_along + self.offset);
        let intensity = f64::from(self.intensity);
        360.0
            * ((intensity * self.curve_equation(p) + (1.0 - intensity) * p - f64::from(self.offset))
                as f32)
    }

    /// `curveEquation`（LoopBuilder.java L64-L68）。
    fn curve_equation(&self, x: f64) -> f64 {
        4f64.powi(2 * self.exponent) * ((x % 0.5) - 0.25).powi(2 * self.exponent + 1)
            + 0.25
            + 0.5 * (2.0 * x).floor()
    }
}

/// 主路径/支路隧道数抽取：权重逐次扣减、抽空（Java 返回 -1）时整表重置再抽
/// （LoopBuilder.java L92-L98、FigureEightBuilder.java L119-L124、
/// RegularBuilder.java L169-L174 的共用模式）。
fn draw_tunnel_count(rng: &mut impl Rng, current: &mut [f32; 3], base: [f32; 3]) -> usize {
    let mut n = chances(rng, current);
    if n.is_none() {
        *current = base;
        n = chances(rng, current);
    }
    let n = n.expect("重置后权重和必为正");
    current[n] -= 1.0;
    n
}

/// 房间列表的矩形中心均值（LoopBuilder.java L144-L150、
/// FigureEightBuilder.java L220-L234 的求环心）。
fn rect_centroid(rooms: &[Room], list: &[usize]) -> Vec2 {
    let mut center = Vec2::ZERO;
    for &r in list {
        let rect = rooms[r].rect;
        center.x += (rect.left + rect.right) as f32 / 2.0;
        center.y += (rect.top + rect.bottom) as f32 / 2.0;
    }
    center / list.len() as f32
}

/// 抽 5 个随机角、取最指向 `center` 的那个（LoopBuilder.java L177-L194 与
/// FigureEightBuilder.java L262-L286 的共用逻辑）。Java 对空中心兜底纯随机角
/// （RegularBuilder.java L243-L245）；两处调用点的中心均已就绪，兜底未移植。
fn branch_angle_toward_center(rng: &mut impl Rng, rooms: &[Room], r: usize, center: Vec2) -> f32 {
    let rect = rooms[r].rect;
    let from = Vec2::new(
        (rect.left + rect.right) as f32 / 2.0,
        (rect.top + rect.bottom) as f32 / 2.0,
    );
    let mut to_center = angle_between_points(from, center);
    if to_center < 0.0 {
        to_center += 360.0;
    }

    let mut curr_angle = float_range(rng, 0.0, 360.0);
    for _ in 0..4 {
        let new_angle = float_range(rng, 0.0, 360.0);
        if (to_center - new_angle).abs() < (to_center - curr_angle).abs() {
            curr_angle = new_angle;
        }
    }
    curr_angle
}

/// `Builder` 各实现收尾的邻接补连（LoopBuilder.java L162-L171、
/// FigureEightBuilder.java L248-L257 逐行相同）。
fn add_extra_connections(rng: &mut impl Rng, rooms: &mut [Room], chance: f32) {
    find_neighbours(rooms);
    for r in 0..rooms.len() {
        let neighbours = rooms[r].neighbours.clone();
        for n in neighbours {
            if !rooms[r].connected_contains(n) && float(rng) < chance {
                connect(rooms, r, n);
            }
        }
    }
}

/// `RegularBuilder.createBranches`（RegularBuilder.java L145-L241）。
/// `roomsToBranch` 中的每个房间经 0-2 段隧道挂到 `branchable` 上；
/// `branch_angle` 是各构建器的 `randomBranchAngle` 覆写（按环心选角）。
fn create_branches<R: Rng>(
    rng: &mut R,
    rooms: &mut Vec<Room>,
    branchable: &mut Vec<usize>,
    rooms_to_branch: &[usize],
    branch_tunnel_chances: [f32; 3],
    mut branch_angle: impl FnMut(&mut R, &[Room], usize) -> f32,
) -> bool {
    let mut i = 0;
    let mut failed_branch_attempts = 0;
    let mut connection_chances = branch_tunnel_chances;
    while i < rooms_to_branch.len() {
        if failed_branch_attempts > 100 {
            return false; // L157-L159
        }
        let r = rooms_to_branch[i];

        // L165-L167：密室的分支起点不得是隧道房（藏门必须开在实体房间墙上），
        // do-while 重抽。Java 另将密室支路的连接房换成 MazeConnectionRoom
        // （L177）——连接房池简化为恒 Tunnel，未移植（见 docs/plans/24 笔记）
        let mut curr = *element(rng, branchable);
        while rooms[r].kind.is_secret() && rooms[curr].kind == RoomKind::Tunnel {
            curr = *element(rng, branchable);
        }

        // L169-L174：抽支路隧道数并扣减权重
        let connecting_rooms =
            draw_tunnel_count(rng, &mut connection_chances, branch_tunnel_chances);

        // 本支路新隧道全部追加在 rooms 尾部；失败时 truncate 等价 Java 的逐个 remove
        let base_len = rooms.len();
        let mut branch_tunnels: Vec<usize> = Vec::new();
        for _ in 0..connecting_rooms {
            // ConnectionRoom.createRoom() 简化为恒 TunnelRoom（L177，见 docs/plans/10 笔记 3）
            rooms.push(Room::new(RoomKind::Tunnel));
            let t = rooms.len() - 1;

            // L179-L183：最多 3 次尝试
            let mut placed = false;
            for _ in 0..3 {
                let angle = branch_angle(rng, rooms, curr);
                let all: Vec<usize> = (0..rooms.len()).collect();
                if place_room(rng, rooms, &all, curr, t, angle).is_some() {
                    placed = true;
                    break;
                }
            }

            if !placed {
                // L185-L192：整条支路回滚
                clear_connections(rooms, t);
                for &c in &branch_tunnels {
                    clear_connections(rooms, c);
                }
                rooms.truncate(base_len);
                branch_tunnels.clear();
                break;
            }
            branch_tunnels.push(t);
            curr = t;
        }

        // L201-L204
        if branch_tunnels.len() != connecting_rooms {
            failed_branch_attempts += 1;
            continue;
        }

        // L206-L211：目标房间最多 10 次尝试
        let mut placed = false;
        for _ in 0..10 {
            let angle = branch_angle(rng, rooms, curr);
            let all: Vec<usize> = (0..rooms.len()).collect();
            if place_room(rng, rooms, &all, curr, r, angle).is_some() {
                placed = true;
                break;
            }
        }
        if !placed {
            // L213-L222：r 的矩形保持"已挪动"状态（与 Java 一致，只回滚图连接与隧道）
            clear_connections(rooms, r);
            for &t in &branch_tunnels {
                clear_connections(rooms, t);
            }
            rooms.truncate(base_len);
            failed_branch_attempts += 1;
            continue;
        }

        // L224-L235：隧道 2/3 概率、目标房 1/3 概率加入可分支列表
        for &t in &branch_tunnels {
            if int(rng, 3) <= 1 {
                branchable.push(t);
            }
        }
        if rooms[r].max_connections(Side::All) > 1 && int(rng, 3) == 0 {
            // StandardRoom 按 connectionWeight 加权复制（恒 1 次）
            for _ in 0..rooms[r].connection_weight() {
                branchable.push(r);
            }
        }

        i += 1;
    }

    true
}

/// `LoopBuilder`（LoopBuilder.java L32-L195）：以一条主环为骨架的构建器。
pub struct LoopBuilder {
    shape: LoopShape,
    params: RegularParams,
}

impl LoopBuilder {
    /// 构造并设置环形状（`setLoopShape` L49-L54 的取模语义）。
    pub fn new(exponent: i32, intensity: f32, offset: f32) -> Self {
        Self {
            shape: LoopShape::new(exponent, intensity, offset),
            params: RegularParams::default(),
        }
    }

    /// `LoopBuilder.build`（L72-L174）。成功返回 true；失败返回 false
    /// 由调用方整体重试（对应 Java 返回 null）。
    /// 隧道房会追加进 `rooms` 尾部；失败时整个 `rooms` 由调用方丢弃。
    pub fn build(&mut self, rng: &mut impl Rng, rooms: &mut Vec<Room>) -> bool {
        let setup = setup_rooms(
            rng,
            rooms,
            self.params.path_length,
            &self.params.path_len_jitter_chances,
        );
        let Some(entrance) = setup.entrance else {
            return false; // L77-L79
        };

        // L81-L82
        rooms[entrance].set_size(rng);
        rooms[entrance].set_pos(0, 0);

        let start_angle = float_range(rng, 0.0, 360.0); // L84

        // L86-L87：入口在环首，出口插在环中点
        let mut main_path = setup.main_path;
        main_path.insert(0, entrance);
        if let Some(exit) = setup.exit {
            // Java `(size()+1)/2`，非负整数下即 div_ceil
            main_path.insert(main_path.len().div_ceil(2), exit);
        }

        // L89-L104：主环 = 主路径房间 + 按权重表穿插的隧道房。
        // ConnectionRoom.createRoom() 的层配比表（ConnectionRoom.java L60-L83）
        // 简化为恒 TunnelRoom（下水道层权重本就以 Tunnel 为主：{20,1,0,2,2,1}）。
        let mut loop_rooms: Vec<usize> = Vec::new();
        let mut path_tunnels = self.params.path_tunnel_chances;
        for &r in &main_path {
            loop_rooms.push(r);
            let tunnels =
                draw_tunnel_count(rng, &mut path_tunnels, self.params.path_tunnel_chances);
            for _ in 0..tunnels {
                rooms.push(Room::new(RoomKind::Tunnel));
                loop_rooms.push(rooms.len() - 1);
            }
        }

        // L106-L119：沿目标角依次摆放主环
        let mut prev = entrance;
        for i in 1..loop_rooms.len() {
            let r = loop_rooms[i];
            let target = start_angle + self.shape.target_angle(i as f32 / loop_rooms.len() as f32);
            let all: Vec<usize> = (0..rooms.len()).collect();
            if place_room(rng, rooms, &all, prev, r, target).is_none() {
                return false; // L115-L118：原版注释承认这里靠运气，失败整体重试
            }
            prev = r;
        }

        // L121-L132：闭合主环——塞隧道直到 prev 能连回入口。
        // Java 无迭代上限（依赖 placeRoom 失败兜底）；这里加防御性上限避免病态死循环。
        let mut closing_attempts = 0;
        while !connect(rooms, prev, entrance) {
            closing_attempts += 1;
            if closing_attempts > 64 {
                return false;
            }
            rooms.push(Room::new(RoomKind::Tunnel));
            let c = rooms.len() - 1;
            let angle = angle_between_rooms(rooms, prev, entrance);
            // 注意 Java 此处碰撞列表传的是 loop 而非全量 rooms
            if place_room(rng, rooms, &loop_rooms, prev, c, angle).is_none() {
                return false;
            }
            loop_rooms.push(c);
            prev = c;
        }

        // L134-L142：shop 摆放未移植（无 ShopRoom）

        // L144-L150：环几何中心
        let center = rect_centroid(rooms, &loop_rooms);

        // L152-L160：其余房间挂分支
        let mut branchable = loop_rooms.clone();
        let mut rooms_to_branch = setup.multi_connections.clone();
        rooms_to_branch.extend_from_slice(&setup.single_connections);
        weight_rooms(rooms, &mut branchable);
        if !create_branches(
            rng,
            rooms,
            &mut branchable,
            &rooms_to_branch,
            self.params.branch_tunnel_chances,
            // LoopBuilder.randomBranchAngle（L177-L194）：指向唯一环心
            |rng, rooms, r| branch_angle_toward_center(rng, rooms, r, center),
        ) {
            return false;
        }

        // L162-L171
        add_extra_connections(rng, rooms, self.params.extra_connection_chance);

        true
    }
}

/// `FigureEightBuilder`（FigureEightBuilder.java L31-L288）：以一间"地标房"
/// 为交点的双环（8 字形）构建器，与 `LoopBuilder` 五五开二选一
/// （RegularLevel.builder L176-L189）。
pub struct FigureEightBuilder {
    shape: LoopShape,
    params: RegularParams,
}

impl FigureEightBuilder {
    /// 构造并设置环形状（`setLoopShape` FigureEightBuilder.java L48-L53，
    /// 取模语义与 `LoopBuilder` 相同）。
    pub fn new(exponent: i32, intensity: f32, offset: f32) -> Self {
        Self {
            shape: LoopShape::new(exponent, intensity, offset),
            params: RegularParams::default(),
        }
    }

    /// `FigureEightBuilder.build`（L79-L260）。成功返回 true；失败由调用方整体重试。
    pub fn build(&mut self, rng: &mut impl Rng, rooms: &mut Vec<Room>) -> bool {
        let setup = setup_rooms(
            rng,
            rooms,
            self.params.path_length,
            &self.params.path_len_jitter_chances,
        );
        let Some(entrance) = setup.entrance else {
            return false;
        };
        let mut main_path = setup.main_path;
        let mut multi = setup.multi_connections;

        // L83-L95：选地标房——主路径中 maxConnections ≥ 4 且 min 面积最大者
        //（本工程房间同规格，即首个），再从 multi 拉一间补占掉的主路径名额。
        // setLandmarkRoom 预设（L71-L74）仅 boss 层使用，未移植。
        let mut landmark: Option<usize> = None;
        for &r in &main_path {
            if rooms[r].max_connections(Side::All) >= 4
                && landmark.is_none_or(|l| {
                    rooms[l].min_width() * rooms[l].min_height()
                        < rooms[r].min_width() * rooms[r].min_height()
                })
            {
                landmark = Some(r);
            }
        }
        if !multi.is_empty() {
            main_path.push(multi.remove(0));
        }
        // 主路径空时 Java 会在 L151 上 NPE；这里防御性失败交由整体重试
        let Some(landmark) = landmark else {
            return false;
        };
        // L96-L97：地标从两张表中摘除（每表至多出现一次）
        main_path.retain(|&r| r != landmark);
        multi.retain(|&r| r != landmark);

        let start_angle = float_range(rng, 0.0, 360.0); // L99

        // L101-L102：主路径对半分到两环；奇数时多出的那间随机归属
        let mut rooms_on_first_loop = main_path.len() / 2;
        if main_path.len() % 2 == 1 {
            rooms_on_first_loop += int(rng, 2) as usize;
        }

        // L104-L111：第一环 = 地标开头 + 前一半主路径，入口插在环中点
        let mut rooms_to_loop = main_path;
        let mut first_temp: Vec<usize> = vec![landmark];
        for _ in 0..rooms_on_first_loop {
            first_temp.push(rooms_to_loop.remove(0));
        }
        // Java `(size()+1)/2`，非负整数下即 div_ceil
        first_temp.insert(first_temp.len().div_ceil(2), entrance);

        // L113-L129：按权重表穿插隧道房（权重两环连续扣减，不重置）
        let mut path_tunnels = self.params.path_tunnel_chances;
        let mut first_loop: Vec<usize> = Vec::new();
        for &r in &first_temp {
            first_loop.push(r);
            let tunnels =
                draw_tunnel_count(rng, &mut path_tunnels, self.params.path_tunnel_chances);
            for _ in 0..tunnels {
                rooms.push(Room::new(RoomKind::Tunnel));
                first_loop.push(rooms.len() - 1);
            }
        }

        // L130-L133：第二环 = 地标开头 + 其余主路径，出口插在环中点
        let mut second_temp: Vec<usize> = vec![landmark];
        second_temp.extend_from_slice(&rooms_to_loop);
        if let Some(exit) = setup.exit {
            second_temp.insert(second_temp.len().div_ceil(2), exit);
        }

        // L135-L149
        let mut second_loop: Vec<usize> = Vec::new();
        for &r in &second_temp {
            second_loop.push(r);
            let tunnels =
                draw_tunnel_count(rng, &mut path_tunnels, self.params.path_tunnel_chances);
            for _ in 0..tunnels {
                rooms.push(Room::new(RoomKind::Tunnel));
                second_loop.push(rooms.len() - 1);
            }
        }

        // L151-L152：地标定尺寸并落原点（LoopBuilder 里这个角色是入口房）
        rooms[landmark].set_size(rng);
        rooms[landmark].set_pos(0, 0);

        // L154-L167：沿目标角摆放第一环
        let mut prev = landmark;
        for i in 1..first_loop.len() {
            let r = first_loop[i];
            let target = start_angle + self.shape.target_angle(i as f32 / first_loop.len() as f32);
            let all: Vec<usize> = (0..rooms.len()).collect();
            if place_room(rng, rooms, &all, prev, r, target).is_none() {
                return false; // L163-L166：原版注释承认这里靠运气，失败整体重试
            }
            prev = r;
        }

        // L169-L180：闭合第一环——塞隧道直到 prev 连回地标。
        // 与 LoopBuilder 不同：Java 此处碰撞列表传全量 rooms。
        // 防御性上限同 LoopBuilder（Java 无上限）。
        let mut closing_attempts = 0;
        while !connect(rooms, prev, landmark) {
            closing_attempts += 1;
            if closing_attempts > 64 {
                return false;
            }
            rooms.push(Room::new(RoomKind::Tunnel));
            let c = rooms.len() - 1;
            let angle = angle_between_rooms(rooms, prev, landmark);
            let all: Vec<usize> = (0..rooms.len()).collect();
            if place_room(rng, rooms, &all, prev, c, angle).is_none() {
                return false;
            }
            first_loop.push(c);
            prev = c;
        }

        // L182-L195：第二环从地标出发、起始角反转 180° 摆放
        prev = landmark;
        let start_angle = start_angle + 180.0; // L183
        for i in 1..second_loop.len() {
            let r = second_loop[i];
            let target = start_angle + self.shape.target_angle(i as f32 / second_loop.len() as f32);
            let all: Vec<usize> = (0..rooms.len()).collect();
            if place_room(rng, rooms, &all, prev, r, target).is_none() {
                return false;
            }
            prev = r;
        }

        // L197-L208：闭合第二环
        let mut closing_attempts = 0;
        while !connect(rooms, prev, landmark) {
            closing_attempts += 1;
            if closing_attempts > 64 {
                return false;
            }
            rooms.push(Room::new(RoomKind::Tunnel));
            let c = rooms.len() - 1;
            let angle = angle_between_rooms(rooms, prev, landmark);
            let all: Vec<usize> = (0..rooms.len()).collect();
            if place_room(rng, rooms, &all, prev, c, angle).is_none() {
                return false;
            }
            second_loop.push(c);
            prev = c;
        }

        // L210-L218：shop 摆放未移植（无 ShopRoom）

        // L220-L234：两环各自的几何中心
        let first_center = rect_centroid(rooms, &first_loop);
        let second_center = rect_centroid(rooms, &second_loop);

        // L236-L246：两环全部房间可挂分支；地标在两环各出现一次，摘除一份
        let mut branchable: Vec<usize> = first_loop
            .iter()
            .chain(second_loop.iter())
            .copied()
            .collect();
        if let Some(pos) = branchable.iter().position(|&x| x == landmark) {
            branchable.remove(pos);
        }
        let mut rooms_to_branch = multi;
        rooms_to_branch.extend_from_slice(&setup.single_connections);
        weight_rooms(rooms, &mut branchable);
        if !create_branches(
            rng,
            rooms,
            &mut branchable,
            &rooms_to_branch,
            self.params.branch_tunnel_chances,
            // FigureEightBuilder.randomBranchAngle（L262-L286）：按所在环选环心，
            // 不在第一环的（含后加入 branchable 的支路隧道）一律用第二环心
            |rng, rooms, r| {
                let center = if first_loop.contains(&r) {
                    first_center
                } else {
                    second_center
                };
                branch_angle_toward_center(rng, rooms, r, center)
            },
        ) {
            return false;
        }

        // L248-L257
        add_extra_connections(rng, rooms, self.params.extra_connection_chance);

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::random::LevelRng;
    use rand::SeedableRng;

    /// SPD 角度约定对拍：正上 0、正右 90、正下 180、正左 -90。
    #[test]
    fn angle_convention_matches_spd() {
        let o = Vec2::new(10.0, 10.0);
        let up = angle_between_points(o, Vec2::new(10.0, 5.0));
        let right = angle_between_points(o, Vec2::new(15.0, 10.0));
        let down = angle_between_points(o, Vec2::new(10.0, 15.0));
        let left = angle_between_points(o, Vec2::new(5.0, 10.0));
        assert!((up - 0.0).abs() < 1e-3, "正上应为 0°，得 {up}");
        assert!((right - 90.0).abs() < 1e-3, "正右应为 90°，得 {right}");
        assert!((down - 180.0).abs() < 1e-3, "正下应为 180°，得 {down}");
        assert!((left + 90.0).abs() < 1e-3, "正左应为 -90°，得 {left}");
        // 45° 斜向（y 向下为正，右上 = 45°）
        let diag = angle_between_points(o, Vec2::new(15.0, 5.0));
        assert!((diag - 45.0).abs() < 1e-3, "右上应为 45°，得 {diag}");
    }

    #[test]
    fn java_round_differs_from_rust_on_negative_halves() {
        assert_eq!(java_round_f32(2.5), 3);
        assert_eq!(java_round_f32(-2.5), -2, "Java Math.round(-2.5) == -2");
        assert_eq!(java_round_f64(-0.5), 0);
        assert_eq!(gate(3, 1, 7), 3);
        assert_eq!(gate(3, 9, 7), 7);
        assert_eq!(gate(3, 5, 7), 5);
    }

    #[test]
    fn find_free_space_unbounded_and_clipped() {
        let mut rng = LevelRng::seed_from_u64(11);
        // 无碰撞：完整 ±max 方块
        let rooms: Vec<Room> = Vec::new();
        let space = find_free_space(&mut rng, IVec2::new(0, 0), &rooms, &[], 8);
        assert_eq!(space, SpdRect::new(-8, -8, 8, 8));

        // start 位于房间上边缘（模拟从该房向上摆放）：space 被压到房间上方
        let mut rooms = vec![Room::new(RoomKind::empty_standard())];
        rooms[0].rect = SpdRect::new(-5, 0, 5, 6);
        let start = IVec2::new(0, 0);
        let space = find_free_space(&mut rng, start, &rooms, &[0], 8);
        assert_eq!(space.bottom, 0, "space 底边应贴住房间顶边");
        assert_eq!(space.top, -8);
    }

    /// placeRoom 语义冒烟：90°（正右）时 next 与 prev 共享 prev.right 墙列且连通。
    #[test]
    fn place_room_at_90_degrees_lands_to_the_right() {
        let mut rng = LevelRng::seed_from_u64(5);
        for _ in 0..50 {
            let mut rooms = vec![
                Room::new(RoomKind::empty_standard()),
                Room::new(RoomKind::empty_standard()),
            ];
            rooms[0].rect = SpdRect::new(0, 0, 6, 6);
            let all = [0usize, 1];
            let angle = place_room(&mut rng, &mut rooms, &all, 0, 1, 90.0)
                .expect("空场地上 90° 摆放必成功");
            assert!((angle - 90.0).abs() < 45.0, "实际角应接近 90°，得 {angle}");
            assert_eq!(
                rooms[1].rect.left, 6,
                "next.left 应贴上 prev.right（共享墙）"
            );
            assert!(rooms[0].connected_contains(1));
            assert!(rooms[1].connected_contains(0));
        }
    }
}
