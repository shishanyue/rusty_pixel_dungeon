//! 生成期房间纯数据，对照 `core/.../levels/rooms/Room.java` 及其子类
//! （`standard/*`、`connection/TunnelRoom`、`special/*`、`secret/*`）。
//!
//! Java 的房间继承树 → [`RoomKind`] 枚举 + 数据表；房间之间用 `Vec<Room>` 的
//! **索引**互指（不进 ECS，见 docs/plans/10 设计要点 1）。矩形语义见
//! [`crate::levels::rect`] 模块文档（闭区间墙位）。
//! 标准房间的尺寸类别/变体表在 [`crate::levels::standard`]，
//! 特殊房/密室池在 [`crate::levels::special`]。

use bevy::math::IVec2;
use rand::Rng;

use crate::levels::{
    random::{int_range, normal_int_range},
    rect::SpdRect,
    special::{SecretKind, SpecialKind},
    standard::{SizeCategory, StandardVariant},
};

/// 连接方位，对应 Room.java L168-L172 的 `ALL/LEFT/TOP/RIGHT/BOTTOM` 常量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    All,
    Left,
    Top,
    Right,
    Bottom,
}

/// 门类型（`Room.Door.Type`，Room.java L444-L446）。
/// 声明顺序即 Java 枚举序 —— [`Door::set`] 依 `Ord` 只升不降。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DoorType {
    Empty,
    Tunnel,
    Water,
    Regular,
    Unlocked,
    Hidden,
    Barricade,
    Locked,
    Crystal,
    Wall,
}

/// 生成期门（`Room.Door`，Room.java L442-L487）。
/// Java 中两侧房间持同一 `Door` 对象；这里以 `Copy` 值存两份，
/// 经 [`set_door`] 同步写两侧。`typeLocked` 无用例，未移植。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Door {
    pub pos: IVec2,
    pub kind: DoorType,
}

impl Door {
    pub fn new(pos: IVec2) -> Self {
        Self {
            pos,
            kind: DoorType::Empty,
        }
    }

    /// 只升级不降级（`Door.set` L466-L470 的 `compareTo` 语义）。
    pub fn set(&mut self, kind: DoorType) {
        if kind > self.kind {
            self.kind = kind;
        }
    }
}

/// 房间种类。Java 继承树的数据化：
/// `StandardRoom` 子类 → 变体 + 尺寸类别载荷；`SpecialRoom`/`SecretRoom`
/// 子类 → 各自枚举载荷。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomKind {
    /// `StandardRoom` 家族（变体 + `sizeCat`）
    Standard {
        variant: StandardVariant,
        size: SizeCategory,
    },
    /// `EntranceRoom`（Java 里也是 `StandardRoom` 子类，`sizeCat` 恒 NORMAL）
    Entrance,
    /// `ExitRoom`
    Exit,
    /// `TunnelRoom`（SPD 连接房池简化为恒隧道，见 docs/plans/10 笔记 3）
    Tunnel,
    /// `SpecialRoom` 家族（maxConnections 恒 1，挂支路末端）
    Special(SpecialKind),
    /// `SecretRoom` 家族（SpecialRoom 子类；入口门恒 HIDDEN）
    Secret(SecretKind),
}

impl RoomKind {
    /// 一间"普通空标准房"（NORMAL 尺寸 EmptyRoom），测试与占位常用。
    pub const fn empty_standard() -> Self {
        RoomKind::Standard {
            variant: StandardVariant::Empty,
            size: SizeCategory::Normal,
        }
    }

    pub const fn is_standard(self) -> bool {
        matches!(self, RoomKind::Standard { .. })
    }

    pub const fn is_secret(self) -> bool {
        matches!(self, RoomKind::Secret(_))
    }
}

/// 生成期房间。`neighbours`/`connected` 存**房间索引**；
/// `connected` 保持插入序（Java `LinkedHashMap` 迭代语义）。
#[derive(Debug, Clone)]
pub struct Room {
    pub rect: SpdRect,
    pub kind: RoomKind,
    pub neighbours: Vec<usize>,
    pub connected: Vec<(usize, Option<Door>)>,
    /// `BurnedRoom` 落位后的 Patch 掩码（房内坐标 (w-2)×(h-2)，true 即过火格）：
    /// 后续水/草/陷阱刻画在这些格上被 `canPlace*` 拒绝（BurnedRoom.java L120-L133）。
    pub deco_ban_patch: Option<Vec<bool>>,
}

impl Room {
    /// 新房间矩形为空，待 Builder 摆放时定尺寸/位置。
    pub fn new(kind: RoomKind) -> Self {
        Self {
            rect: SpdRect::default(),
            kind,
            neighbours: Vec::new(),
            connected: Vec::new(),
            deco_ban_patch: None,
        }
    }

    /// 最小格宽：标准房查变体/尺寸类别表（StandardRoom.java L92-L98 + 各覆写）；
    /// Entrance/Exit = max(4, 5) = 5（EntranceRoom.java L44-L52、ExitRoom.java L39-L47）；
    /// `ConnectionRoom` = 3（ConnectionRoom.java L33-L39）；
    /// Special/Secret = 5（SpecialRoom.java L36-L44）。
    pub fn min_width(&self) -> i32 {
        match self.kind {
            RoomKind::Standard { variant, size } => variant.min_dim(size),
            RoomKind::Entrance
            | RoomKind::Exit
            | RoomKind::Special(_)
            | RoomKind::Secret(_) => 5,
            RoomKind::Tunnel => 3,
        }
    }

    /// 最大格宽：标准房 = `sizeCat.maxDim`（移植变体无覆写）；其余恒 10。
    pub fn max_width(&self) -> i32 {
        match self.kind {
            RoomKind::Standard { variant, size } => variant.max_dim(size),
            _ => 10,
        }
    }

    /// 尺寸区间为正方形域：高与宽同表。
    pub fn min_height(&self) -> i32 {
        self.min_width()
    }

    pub fn max_height(&self) -> i32 {
        self.max_width()
    }

    /// 房间格宽 = SPD 单位宽 + 1（Room.java L134-L138：右/下闭合）。
    pub fn width(&self) -> i32 {
        self.rect.width() + 1
    }

    /// 房间格高（Room.java L140-L143）。
    pub fn height(&self) -> i32 {
        self.rect.height() + 1
    }

    pub fn is_entrance(&self) -> bool {
        self.kind == RoomKind::Entrance
    }

    pub fn is_exit(&self) -> bool {
        self.kind == RoomKind::Exit
    }

    /// `sizeFactor()`（StandardRoom.java L100-L104）：大房间在数量/主路径
    /// 结算中折抵的房数；非标准房恒 1。
    pub fn size_factor(&self) -> i32 {
        match self.kind {
            RoomKind::Standard { size, .. } => size.room_value(),
            _ => 1,
        }
    }

    /// Room.java L201-L204：总连接上限 16、每边 4；
    /// Special/Secret 覆写为恒 1（SpecialRoom.java L46-L49）——保证只挂支路末端。
    pub fn max_connections(&self, direction: Side) -> i32 {
        match self.kind {
            RoomKind::Special(_) | RoomKind::Secret(_) => 1,
            _ if direction == Side::All => 16,
            _ => 4,
        }
    }

    /// `StandardRoom.connectionWeight()` = sizeFactor²（StandardRoom.java L113-L115）。
    /// 非标准房无此覆写，按 1 参与（weightRooms 复制 0 份，语义一致）。
    pub fn connection_weight(&self) -> i32 {
        self.size_factor() * self.size_factor()
    }

    /// 变体尺寸约束钩子：`CircleBasinRoom.resize`（L43-L50）宽高不得为偶数格，
    /// 超界即向内收 1。其余房间为纯 `Rect.resize`。
    pub(crate) fn resize(&mut self, w: i32, h: i32) {
        self.rect.resize(w, h);
        if matches!(
            self.kind,
            RoomKind::Standard {
                variant: StandardVariant::CircleBasin,
                ..
            }
        ) {
            if self.width() % 2 == 0 {
                self.rect.right -= 1;
            }
            if self.height() % 2 == 0 {
                self.rect.bottom -= 1;
            }
        }
    }

    /// `Room.setSize()`（L82-L84 → L104-L118）：三角分布随机尺寸。
    /// L113-L115：房间右/下边闭合，`resize` 参数 = 格数 - 1。宽先高后（RNG 消耗序）。
    pub fn set_size(&mut self, rng: &mut impl Rng) {
        let w = normal_int_range(rng, self.min_width(), self.max_width());
        let h = normal_int_range(rng, self.min_height(), self.max_height());
        self.resize(w - 1, h - 1);
    }

    /// `Room.setSizeWithLimit(w, h)`（L90-L102）：`w`/`h` 为可用**格数**上限。
    pub fn set_size_with_limit(&mut self, rng: &mut impl Rng, w: i32, h: i32) -> bool {
        if w < self.min_width() || h < self.min_height() {
            return false;
        }
        self.set_size(rng);
        if self.width() > w || self.height() > h {
            self.resize(self.width().min(w) - 1, self.height().min(h) - 1);
        }
        true
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.rect.set_pos(x, y);
    }

    pub fn shift(&mut self, dx: i32, dy: i32) {
        self.rect.shift(dx, dy);
    }

    /// `Room.random(m)`（L149-L152）：距边 ≥ m 格的随机内部点。
    pub fn random_point(&self, rng: &mut impl Rng, m: i32) -> IVec2 {
        IVec2::new(
            int_range(rng, self.rect.left + m, self.rect.right - m),
            int_range(rng, self.rect.top + m, self.rect.bottom - m),
        )
    }

    /// `Room.inside(Point)`（L154-L157）：严格在 1 格边圈以内。
    pub fn inside(&self, p: IVec2) -> bool {
        p.x > self.rect.left && p.y > self.rect.top && p.x < self.rect.right && p.y < self.rect.bottom
    }

    /// `Room.canConnect(Point)`（L206-L210）：门必须落在恰好一条边上
    /// （异或排除四角与内部/外部点）。
    /// `SewerPipeRoom.canConnect`（SewerPipeRoom.java L59-L63）额外拒绝
    /// 紧邻四角的墙位（水管房门内 2 格是水道起点，需要余量）。
    pub fn can_connect_point(&self, p: IVec2) -> bool {
        let r = &self.rect;
        let base = (p.x == r.left || p.x == r.right) != (p.y == r.top || p.y == r.bottom);
        match self.kind {
            RoomKind::Standard {
                variant: StandardVariant::SewerPipe,
                ..
            } => {
                base && ((p.x > r.left + 1 && p.x < r.right - 1)
                    || (p.y > r.top + 1 && p.y < r.bottom - 1))
            }
            _ => base,
        }
    }

    /// `Room.canPlaceWater`：`SewerPipeRoom` 恒否（L215-L218，水道自绘）；
    /// `BurnedRoom` 过火格否（L120-L123）；其余恒真。
    pub fn can_place_water(&self, p: IVec2) -> bool {
        match self.kind {
            RoomKind::Standard {
                variant: StandardVariant::SewerPipe,
                ..
            } => false,
            _ => self.deco_allowed(p),
        }
    }

    /// `Room.canPlaceGrass`：`BurnedRoom` 过火格否（L125-L128）；其余恒真。
    pub fn can_place_grass(&self, p: IVec2) -> bool {
        self.deco_allowed(p)
    }

    /// `Room.canPlaceTrap`：`BurnedRoom` 过火格否（L130-L133）；其余恒真。
    /// `EntranceRoom` 的 1 层禁陷阱（EntranceRoom.java L69-L75）在 painter
    /// 侧判定（需要 depth）。
    pub fn can_place_trap(&self, p: IVec2) -> bool {
        self.deco_allowed(p)
    }

    /// `BurnedRoom.canPlace*` 的公共体：`!inside(p) || !patch[...]`。
    fn deco_allowed(&self, p: IVec2) -> bool {
        let Some(patch) = &self.deco_ban_patch else {
            return true;
        };
        if !self.inside(p) {
            return true;
        }
        let w = self.width() - 2;
        let idx = ((p.x - self.rect.left - 1) + (p.y - self.rect.top - 1) * w) as usize;
        !patch[idx]
    }

    pub fn connected_contains(&self, other: usize) -> bool {
        self.connected.iter().any(|&(n, _)| n == other)
    }

    /// 与 `other` 之间已就位的门（连接存在但门未定时返回 `None`）。
    pub fn door_to(&self, other: usize) -> Option<Door> {
        self.connected
            .iter()
            .find(|&&(n, _)| n == other)
            .and_then(|&(_, d)| d)
    }
}

/// `Room.addNeigbour`（L256-L268）：交叠为一条 SPD 单位长 ≥2 的直线
/// （即共享墙段 ≥3 格，含两端角格）时互为邻居。
pub(crate) fn add_neighbour(rooms: &mut [Room], a: usize, b: usize) -> bool {
    if rooms[a].neighbours.contains(&b) {
        return true;
    }
    let i = rooms[a].rect.intersect(&rooms[b].rect);
    if (i.width() == 0 && i.height() >= 2) || (i.height() == 0 && i.width() >= 2) {
        rooms[a].neighbours.push(b);
        rooms[b].neighbours.push(a);
        return true;
    }
    false
}

/// `Room.connect`（L270-L279）：先确保邻居关系，再做几何/方向校验，
/// 成功后双向登记连接（门待 Painter 的 `place_doors` 决定）。
pub(crate) fn connect(rooms: &mut [Room], a: usize, b: usize) -> bool {
    if (rooms[a].neighbours.contains(&b) || add_neighbour(rooms, a, b))
        && !rooms[a].connected_contains(b)
        && can_connect_rooms(rooms, a, b)
    {
        rooms[a].connected.push((b, None));
        rooms[b].connected.push((a, None));
        return true;
    }
    false
}

/// `Room.canConnect(Room)`（L218-L245）。
pub(crate) fn can_connect_rooms(rooms: &[Room], a: usize, b: usize) -> bool {
    let ra = &rooms[a];
    let rb = &rooms[b];
    // 入口与出口不允许直连（L219-L222）
    if (ra.is_exit() && rb.is_entrance()) || (ra.is_entrance() && rb.is_exit()) {
        return false;
    }

    let i = ra.rect.intersect(&rb.rect);
    let found = i
        .points()
        .any(|p| ra.can_connect_point(p) && rb.can_connect_point(p));
    if !found {
        return false;
    }

    if i.width() == 0 && i.left == ra.rect.left {
        can_connect_dir(rooms, a, Side::Left) && can_connect_dir(rooms, b, Side::Right)
    } else if i.height() == 0 && i.top == ra.rect.top {
        can_connect_dir(rooms, a, Side::Top) && can_connect_dir(rooms, b, Side::Bottom)
    } else if i.width() == 0 && i.right == ra.rect.right {
        can_connect_dir(rooms, a, Side::Right) && can_connect_dir(rooms, b, Side::Left)
    } else if i.height() == 0 && i.bottom == ra.rect.bottom {
        can_connect_dir(rooms, a, Side::Bottom) && can_connect_dir(rooms, b, Side::Top)
    } else {
        false
    }
}

/// `Room.canConnect(int direction)`（L213-L215）。
fn can_connect_dir(rooms: &[Room], r: usize, direction: Side) -> bool {
    rem_connections(rooms, r, direction) > 0
}

/// `Room.remConnections`（L196-L199）。
fn rem_connections(rooms: &[Room], r: usize, direction: Side) -> i32 {
    if cur_connections(rooms, r, Side::All) >= rooms[r].max_connections(Side::All) {
        0
    } else {
        rooms[r].max_connections(direction) - cur_connections(rooms, r, direction)
    }
}

/// `Room.curConnections`（L179-L194）：按共享墙所在边归类计数。
fn cur_connections(rooms: &[Room], r: usize, direction: Side) -> i32 {
    if direction == Side::All {
        return rooms[r].connected.len() as i32;
    }
    let rect = rooms[r].rect;
    let mut total = 0;
    for &(n, _) in &rooms[r].connected {
        let i = rect.intersect(&rooms[n].rect);
        let on_side = match direction {
            Side::Left => i.width() == 0 && i.left == rect.left,
            Side::Top => i.height() == 0 && i.top == rect.top,
            Side::Right => i.width() == 0 && i.right == rect.right,
            Side::Bottom => i.height() == 0 && i.bottom == rect.bottom,
            Side::All => unreachable!("上方已提前返回"),
        };
        if on_side {
            total += 1;
        }
    }
    total
}

/// `Room.clearConnections`（L281-L290）：双向摘除图边。
pub(crate) fn clear_connections(rooms: &mut [Room], r: usize) {
    let neighbours = std::mem::take(&mut rooms[r].neighbours);
    for n in neighbours {
        rooms[n].neighbours.retain(|&x| x != r);
    }
    let connected = std::mem::take(&mut rooms[r].connected);
    for (n, _) in connected {
        rooms[n].connected.retain(|&(x, _)| x != r);
    }
}

/// 同步写两侧共享门（Java 两房间持同一 `Door` 引用，改一处即两处）。
pub(crate) fn set_door(rooms: &mut [Room], a: usize, b: usize, door: Door) {
    if let Some(entry) = rooms[a].connected.iter_mut().find(|(n, _)| *n == b) {
        entry.1 = Some(door);
    }
    if let Some(entry) = rooms[b].connected.iter_mut().find(|(n, _)| *n == a) {
        entry.1 = Some(door);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::random::LevelRng;
    use rand::SeedableRng;

    fn room_at(kind: RoomKind, left: i32, top: i32, right: i32, bottom: i32) -> Room {
        let mut r = Room::new(kind);
        r.rect = SpdRect::new(left, top, right, bottom);
        r
    }

    fn std_room(left: i32, top: i32, right: i32, bottom: i32) -> Room {
        room_at(RoomKind::empty_standard(), left, top, right, bottom)
    }

    /// 验收用例：邻居判定手算对拍。共享墙段 ≥3 格（SPD 单位 ≥2）才算邻居。
    #[test]
    fn neighbour_needs_three_shared_cells() {
        // A(0,0,5,5) 与 B(5,0,10,4)：共享 x=5 列 y∈[0,4]，5 格 → 邻居
        let mut rooms = vec![
            std_room(0, 0, 5, 5),
            std_room(5, 0, 10, 4),
            // C(5,4,10,6)：与 A 共享 x=5 列 y∈[4,5]，仅 2 格（SPD 高 1）→ 非邻居
            std_room(5, 4, 10, 6),
        ];
        assert!(add_neighbour(&mut rooms, 0, 1));
        assert!(!add_neighbour(&mut rooms, 0, 2));
        assert_eq!(rooms[0].neighbours, vec![1]);
        assert_eq!(rooms[1].neighbours, vec![0]);
        assert!(rooms[2].neighbours.is_empty());
    }

    /// 门位异或规则：四角与内部点不可开门，边中段可以；
    /// 水管房额外拒绝紧邻四角的墙位（SewerPipeRoom.java L59-L63）。
    #[test]
    fn door_points_exclude_corners() {
        let room = std_room(0, 0, 5, 5);
        assert!(!room.can_connect_point(IVec2::new(0, 0)), "角");
        assert!(!room.can_connect_point(IVec2::new(5, 5)), "角");
        assert!(!room.can_connect_point(IVec2::new(2, 2)), "内部");
        assert!(room.can_connect_point(IVec2::new(0, 2)), "左边中段");
        assert!(room.can_connect_point(IVec2::new(3, 5)), "下边中段");

        let pipe = room_at(
            RoomKind::Standard {
                variant: StandardVariant::SewerPipe,
                size: SizeCategory::Normal,
            },
            0,
            0,
            6,
            6,
        );
        assert!(!pipe.can_connect_point(IVec2::new(1, 0)), "紧邻角的墙位");
        assert!(!pipe.can_connect_point(IVec2::new(0, 5)), "紧邻角的墙位");
        assert!(pipe.can_connect_point(IVec2::new(3, 0)), "边中段");
        assert!(pipe.can_connect_point(IVec2::new(0, 3)), "边中段");
    }

    /// A(0,0,5,5) 与 C(5,3,10,8) 共享 3 格墙段：唯一门位是中间格 (5,4)。
    #[test]
    fn shared_three_cells_leave_single_door_spot() {
        let a = std_room(0, 0, 5, 5);
        let c = std_room(5, 3, 10, 8);
        let i = a.rect.intersect(&c.rect);
        assert_eq!(i, SpdRect::new(5, 3, 5, 5));
        let spots: Vec<IVec2> = i
            .points()
            .filter(|&p| a.can_connect_point(p) && c.can_connect_point(p))
            .collect();
        assert_eq!(spots, vec![IVec2::new(5, 4)]);
    }

    #[test]
    fn connect_links_both_sides_and_bans_entrance_exit() {
        let mut rooms = vec![
            room_at(RoomKind::Entrance, 0, 0, 5, 5),
            room_at(RoomKind::Exit, 5, 0, 10, 5),
            std_room(0, 5, 5, 10),
        ];
        // 入口-出口几何上可连但被禁止
        assert!(!connect(&mut rooms, 0, 1));
        assert!(rooms[0].connected.is_empty());
        // 入口-标准房正常连接，双向登记、门未定
        assert!(connect(&mut rooms, 0, 2));
        assert!(rooms[0].connected_contains(2));
        assert!(rooms[2].connected_contains(0));
        assert_eq!(rooms[0].door_to(2), None);
        // 重复连接失败（Java connect 对已连房间返回 false）
        assert!(!connect(&mut rooms, 0, 2));

        // 门写入后两侧同步可见
        let door = Door::new(IVec2::new(2, 5));
        set_door(&mut rooms, 0, 2, door);
        assert_eq!(rooms[0].door_to(2), Some(door));
        assert_eq!(rooms[2].door_to(0), Some(door));

        clear_connections(&mut rooms, 0);
        assert!(rooms[0].connected.is_empty());
        assert!(!rooms[2].connected_contains(0));
        assert!(!rooms[2].neighbours.contains(&0));
    }

    /// Special/Secret 房 maxConnections 恒 1：一条连接后拒绝第二条。
    #[test]
    fn special_rooms_cap_at_one_connection() {
        let mut rooms = vec![
            room_at(RoomKind::Special(SpecialKind::Garden), 0, 0, 5, 5),
            std_room(5, 0, 10, 5),
            std_room(0, 5, 5, 10),
        ];
        assert_eq!(rooms[0].max_connections(Side::All), 1);
        assert_eq!(rooms[0].max_connections(Side::Left), 1);
        assert!(connect(&mut rooms, 0, 1));
        // 第二条连接因 remConnections == 0 被拒
        assert!(!connect(&mut rooms, 0, 2));
        assert_eq!(rooms[0].connected.len(), 1);

        let secret = Room::new(RoomKind::Secret(SecretKind::Garden));
        assert_eq!(secret.max_connections(Side::All), 1);
        assert!(secret.kind.is_secret());
    }

    #[test]
    fn door_type_only_upgrades() {
        let mut door = Door::new(IVec2::ZERO);
        assert_eq!(door.kind, DoorType::Empty);
        door.set(DoorType::Tunnel);
        door.set(DoorType::Regular);
        assert_eq!(door.kind, DoorType::Regular);
        // 降级请求被忽略
        door.set(DoorType::Tunnel);
        assert_eq!(door.kind, DoorType::Regular);
        door.set(DoorType::Hidden);
        assert_eq!(door.kind, DoorType::Hidden);
        // LOCKED 高于 HIDDEN（Java 枚举序）
        door.set(DoorType::Locked);
        assert_eq!(door.kind, DoorType::Locked);
    }

    #[test]
    fn set_size_respects_kind_table() {
        let mut rng = LevelRng::seed_from_u64(7);
        for kind in [
            RoomKind::empty_standard(),
            RoomKind::Entrance,
            RoomKind::Exit,
            RoomKind::Tunnel,
            RoomKind::Special(SpecialKind::Garden),
            RoomKind::Secret(SecretKind::Garden),
        ] {
            for _ in 0..200 {
                let mut room = Room::new(kind);
                room.set_size(&mut rng);
                assert!(room.width() >= room.min_width() && room.width() <= room.max_width());
                assert!(room.height() >= room.min_height() && room.height() <= room.max_height());
            }
        }
        // setSizeWithLimit：限制低于最小尺寸时失败，否则收缩到限制内
        let mut room = Room::new(RoomKind::Entrance);
        assert!(!room.set_size_with_limit(&mut rng, 4, 8), "入口最小 5 格");
        assert!(room.set_size_with_limit(&mut rng, 6, 6));
        assert!(room.width() <= 6 && room.height() <= 6);
        assert!(room.width() >= 5 && room.height() >= 5);
    }

    /// 尺寸类别改变标准房的尺寸域与权重（StandardRoom.java L92-L115）。
    #[test]
    fn size_category_drives_dims_and_weights() {
        let mut rng = LevelRng::seed_from_u64(11);
        for (size, min, max, value) in [
            (SizeCategory::Normal, 4, 10, 1),
            (SizeCategory::Large, 10, 14, 2),
            (SizeCategory::Giant, 14, 18, 3),
        ] {
            let mut room = Room::new(RoomKind::Standard {
                variant: StandardVariant::Empty,
                size,
            });
            assert_eq!(room.min_width(), min);
            assert_eq!(room.max_width(), max);
            assert_eq!(room.size_factor(), value);
            assert_eq!(room.connection_weight(), value * value);
            for _ in 0..50 {
                room.set_size(&mut rng);
                assert!(room.width() >= min && room.width() <= max);
            }
        }
        // 变体最小尺寸覆写：水管/环形 ≥7；圆盆 minDim+1 且宽高恒奇
        let pipe = Room::new(RoomKind::Standard {
            variant: StandardVariant::SewerPipe,
            size: SizeCategory::Normal,
        });
        assert_eq!(pipe.min_width(), 7);
        let mut basin = Room::new(RoomKind::Standard {
            variant: StandardVariant::CircleBasin,
            size: SizeCategory::Large,
        });
        assert_eq!(basin.min_width(), 11);
        for _ in 0..100 {
            basin.set_size(&mut rng);
            assert_eq!(basin.width() % 2, 1, "圆盆房宽必须为奇数格");
            assert_eq!(basin.height() % 2, 1, "圆盆房高必须为奇数格");
            assert!(basin.width() >= 11 && basin.width() <= 14);
        }
    }

    /// `BurnedRoom` 的落位掩码：过火格拒绝水/草/陷阱，边墙与未过火格放行。
    #[test]
    fn deco_ban_patch_blocks_placement() {
        let mut room = room_at(
            RoomKind::Standard {
                variant: StandardVariant::Burned,
                size: SizeCategory::Normal,
            },
            0,
            0,
            5,
            5,
        );
        // 内部 4×4，仅 (1,1)（房内坐标 (0,0)）过火
        let mut patch = vec![false; 16];
        patch[0] = true;
        room.deco_ban_patch = Some(patch);

        assert!(!room.can_place_water(IVec2::new(1, 1)), "过火格禁水");
        assert!(!room.can_place_grass(IVec2::new(1, 1)));
        assert!(!room.can_place_trap(IVec2::new(1, 1)));
        assert!(room.can_place_water(IVec2::new(2, 1)), "未过火格放行");
        assert!(room.can_place_water(IVec2::new(0, 3)), "边墙不属 inside，放行");

        // 水管房整房禁水（SewerPipeRoom.java L215-L218）
        let pipe = room_at(
            RoomKind::Standard {
                variant: StandardVariant::SewerPipe,
                size: SizeCategory::Normal,
            },
            0,
            0,
            6,
            6,
        );
        assert!(!pipe.can_place_water(IVec2::new(3, 3)));
        assert!(pipe.can_place_grass(IVec2::new(3, 3)), "禁水不禁草");
    }
}
