//! SPD `PathFinder` 移植：FIFO 波前（桶队列退化为单桶步进的 BFS）距离图与取步逻辑。
//! 逐行对照 `SPD-classes/src/main/java/com/watabou/utils/PathFinder.java`（SPD 3.3.8），
//! 注释中的行号均指该文件。
//!
//! # 形态约定
//!
//! - 地图为线性数组，`index = y * width + x`；`passable` 由调用方以 `&[bool]` 传入。
//! - Java 的静态字段 + `setMapSize` 改为实例结构体 + [`PathFinder::new`]，
//!   一层地牢建一个实例复用内部缓冲。
//! - Java 的 `-1` / `null` 返回值改为 `Option`。
//!
//! # 与 Java 的语义差异（仅防御性，合法输入下不可观察）
//!
//! 1. `find`/`getStep`/`getStepBack` 的"单步下坡"扫描（L92-L101、L121-L127、L190-L199）
//!    直接索引 `from + dir[i]`，当格子位于首行/末行时会越界抛
//!    `ArrayIndexOutOfBoundsException`；本移植跳过越界候选。SPD 关卡四周恒为
//!    实心墙、角色不可能站在边界行，故不可触发。
//! 2. Java 的 `queue` 是定长 `size` 数组；`buildDistanceMap` 的 `n == from` 分支
//!    （L236、L320）可将 `from` 重复入队，极端拥挤的小图上会溢出抛异常。
//!    本移植用可增长的 `Vec`，在 Java 本会崩溃的输入上正常返回。
//! 3. 横向"回绕"读取与 Java 一致保留：下坡扫描用的 `dir`（L66）不做边缘裁剪，
//!    只有 BFS 扩展用的 `dirLR`（L67）按 L231-L232 裁剪首尾三项。

use std::collections::VecDeque;

/// SPD `PathFinder.find` 返回的路径（Java `LinkedList<Integer>`，L414-L416）：
/// 不含起点、含终点，队首为第一步。
pub type Path = VecDeque<usize>;

/// 距离图中"不可达"的哨兵值（Java `Integer.MAX_VALUE`，L63-L64）。
pub const UNREACHABLE: i32 = i32::MAX;

/// SPD 寻路器。持有距离图与复用缓冲，尺寸固定于构造时（对照 `setMapSize`）。
pub struct PathFinder {
    width: usize,
    size: usize,

    /// 最近一次 `build_*` 调用产出的距离图（Java 公有静态 `distance`，L29）。
    distance: Vec<i32>,
    goals: Vec<bool>,
    queue: Vec<usize>,
    queued: Vec<bool>,

    /// 下坡扫描邻接序（L66）。
    dir: [isize; 8],
    /// BFS 扩展邻接序（L67）：前三项含 -1（左列）、后三项含 +1（右列），
    /// 便于在地图左右边缘裁剪防止索引回绕。
    dir_lr: [isize; 8],

    /// 4 邻接偏移，数组访问序（L69）。
    pub neighbours4: [isize; 4],
    /// 8 邻接偏移，数组访问序（L70）。
    pub neighbours8: [isize; 8],
    /// 9 邻接偏移（含 0 自身），数组访问序（L71）。
    pub neighbours9: [isize; 9],
    /// 4 邻接偏移，顺时针序（L73）。
    pub circle4: [isize; 4],
    /// 8 邻接偏移，顺时针序（L74）。
    pub circle8: [isize; 8],
}

impl PathFinder {
    /// 对照 `setMapSize`（L53-L75）。
    pub fn new(width: usize, height: usize) -> Self {
        assert!(width > 0 && height > 0, "地图尺寸必须为正");
        let size = width * height;
        let w = width as isize;
        Self {
            width,
            size,
            distance: vec![UNREACHABLE; size],
            goals: vec![false; size],
            queue: Vec::with_capacity(size),
            queued: vec![false; size],
            dir: [-1, 1, -w, w, -w - 1, -w + 1, w - 1, w + 1],
            dir_lr: [-1 - w, -1, -1 + w, -w, w, 1 - w, 1, 1 + w],
            neighbours4: [-w, -1, 1, w],
            neighbours8: [-w - 1, -w, -w + 1, -1, 1, w - 1, w, w + 1],
            neighbours9: [-w - 1, -w, -w + 1, -1, 0, 1, w - 1, w, w + 1],
            circle4: [-w, 1, w, -1],
            circle8: [-w - 1, -w, -w + 1, 1, w + 1, w, w - 1, -1],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// 最近一次构图的距离图；不可达格为 [`UNREACHABLE`]。
    pub fn distance(&self) -> &[i32] {
        &self.distance
    }

    /// 对照 `find`（L77-L107）：`from` 到 `to` 的最短路。
    /// 返回路径不含 `from`、含 `to`；不可达或 `from == to` 时为 `None`（Java `null`）。
    pub fn get_path(&mut self, from: usize, to: usize, passable: &[bool]) -> Option<Path> {
        assert_eq!(passable.len(), self.size, "passable 长度必须等于地图尺寸");

        if !self.build_distance_map_from_to(from, to, passable) {
            return None;
        }

        let mut result = Path::new();
        let mut s = from;

        // 从起点沿距离图一路下坡直到终点（L86-L104 的 do-while）
        loop {
            s = self.downhill_step(s);
            result.push_back(s);
            if s == to {
                break;
            }
        }

        Some(result)
    }

    /// 对照 `getStep`（L109-L130）：朝 `to` 走一步。
    /// 不可达或 `from == to` 时为 `None`（Java `-1`）；注意与 Java 相同，
    /// 极端情形下返回值可能等于 `from`（周围无更近格）。
    pub fn get_step(&mut self, from: usize, to: usize, passable: &[bool]) -> Option<usize> {
        assert_eq!(passable.len(), self.size, "passable 长度必须等于地图尺寸");

        if !self.build_distance_map_from_to(from, to, passable) {
            return None;
        }

        Some(self.downhill_step(from))
    }

    /// 对照 `getStepBack`（L132-L202）：从 `cur` 逃离 `from`，
    /// 以"比当前再远 `lookahead` 格"为目标取一步；无处可逃时 `None`（Java `-1`）。
    ///
    /// `passable` 与 Java 一致按可变引用传入：`can_approach_from_pos == false`
    /// （恐惧/惊惧）时会把比 `cur` 更接近 `from` 的格子原地改为不可通行
    /// （L162-L163），`Dungeon.flee` 的重试循环依赖这一副作用。
    pub fn get_step_back(
        &mut self,
        cur: usize,
        from: usize,
        lookahead: i32,
        passable: &mut [bool],
        can_approach_from_pos: bool,
    ) -> Option<usize> {
        assert_eq!(passable.len(), self.size, "passable 长度必须等于地图尺寸");

        let mut d = self.build_escape_distance_map(cur, from, lookahead, passable);
        if d == 0 {
            return None;
        }

        if !can_approach_from_pos {
            // 不能接近逃离点：把朝向它的格子标记为不可走，并据此收缩目标距离（L137-L175）
            let dir_lr = self.dir_lr;
            let mut new_d = self.distance[cur];
            self.queued.fill(false);

            self.queue.clear();
            let mut head = 0;
            self.queue.push(cur);
            self.queued[cur] = true;

            while head < self.queue.len() {
                let step = self.queue[head];
                head += 1;

                if self.distance[step] > new_d {
                    new_d = self.distance[step];
                }

                let start = if step.is_multiple_of(self.width) {
                    3
                } else {
                    0
                };
                let end = if (step + 1).is_multiple_of(self.width) {
                    3
                } else {
                    0
                };
                for &off in &dir_lr[start..dir_lr.len() - end] {
                    let n = step as isize + off;
                    if n >= 0 && (n as usize) < self.size && passable[n as usize] {
                        let n = n as usize;
                        if self.distance[n] < self.distance[cur] {
                            passable[n] = false;
                        } else if self.distance[n] >= self.distance[step] && !self.queued[n] {
                            // 入队
                            self.queue.push(n);
                            self.queued[n] = true;
                        }
                    }
                }
            }

            d = new_d.min(d);
        }

        // 以"距离恰为 d 的格子"为目标集（L177-L179）
        for (goal, &dist) in self.goals.iter_mut().zip(&self.distance) {
            *goal = dist == d;
        }
        if !self.build_distance_map_to_goals(cur, passable) {
            return None;
        }

        Some(self.downhill_step(cur))
    }

    /// 对照私有 `buildDistanceMap(int from, int to, boolean[] passable)`（L204-L246）：
    /// 从 `to` 反向泛洪，抵达 `from` 即停；返回是否可达。
    /// 注意 `n == from` 分支（L236）绕过 `passable` 检查——`from` 自身
    /// （角色所站格）在 SPD 中总被标记为不可通行。
    fn build_distance_map_from_to(&mut self, from: usize, to: usize, passable: &[bool]) -> bool {
        if from == to {
            return false;
        }

        self.distance.fill(UNREACHABLE);

        let mut path_found = false;
        let dir_lr = self.dir_lr;

        self.queue.clear();
        let mut head = 0;

        // 入队
        self.queue.push(to);
        self.distance[to] = 0;

        while head < self.queue.len() {
            // 出队
            let step = self.queue[head];
            head += 1;
            if step == from {
                path_found = true;
                break;
            }
            let next_distance = self.distance[step] + 1;

            let start = if step.is_multiple_of(self.width) {
                3
            } else {
                0
            };
            let end = if (step + 1).is_multiple_of(self.width) {
                3
            } else {
                0
            };
            for &off in &dir_lr[start..dir_lr.len() - end] {
                let n = step as isize + off;
                if n == from as isize
                    || (n >= 0
                        && (n as usize) < self.size
                        && passable[n as usize]
                        && self.distance[n as usize] > next_distance)
                {
                    // 入队
                    self.queue.push(n as usize);
                    self.distance[n as usize] = next_distance;
                }
            }
        }

        path_found
    }

    /// 对照公有 `buildDistanceMap(int to, boolean[] passable)`（L382-L412）：
    /// 从 `to` 全图泛洪，结果经 [`PathFinder::distance`] 读取
    /// （SPD 消费方如 `Mob.chooseEnemy`、`Level.spawnMob`）。
    pub fn build_distance_map(&mut self, to: usize, passable: &[bool]) {
        assert_eq!(passable.len(), self.size, "passable 长度必须等于地图尺寸");

        self.distance.fill(UNREACHABLE);

        let dir_lr = self.dir_lr;
        self.queue.clear();
        let mut head = 0;

        // 入队
        self.queue.push(to);
        self.distance[to] = 0;

        while head < self.queue.len() {
            // 出队
            let step = self.queue[head];
            head += 1;
            let next_distance = self.distance[step] + 1;

            let start = if step.is_multiple_of(self.width) {
                3
            } else {
                0
            };
            let end = if (step + 1).is_multiple_of(self.width) {
                3
            } else {
                0
            };
            for &off in &dir_lr[start..dir_lr.len() - end] {
                let n = step as isize + off;
                if n >= 0
                    && (n as usize) < self.size
                    && passable[n as usize]
                    && self.distance[n as usize] > next_distance
                {
                    // 入队
                    self.queue.push(n as usize);
                    self.distance[n as usize] = next_distance;
                }
            }
        }
    }

    /// 对照公有 `buildDistanceMap(int to, boolean[] passable, int limit)`（L248-L282）：
    /// 从 `to` 全图泛洪，但只填充距离 ≤ `limit` 的格子。
    pub fn build_distance_map_limited(&mut self, to: usize, passable: &[bool], limit: i32) {
        assert_eq!(passable.len(), self.size, "passable 长度必须等于地图尺寸");

        self.distance.fill(UNREACHABLE);

        let dir_lr = self.dir_lr;
        self.queue.clear();
        let mut head = 0;

        // 入队
        self.queue.push(to);
        self.distance[to] = 0;

        while head < self.queue.len() {
            // 出队
            let step = self.queue[head];
            head += 1;

            let next_distance = self.distance[step] + 1;
            if next_distance > limit {
                return;
            }

            let start = if step.is_multiple_of(self.width) {
                3
            } else {
                0
            };
            let end = if (step + 1).is_multiple_of(self.width) {
                3
            } else {
                0
            };
            for &off in &dir_lr[start..dir_lr.len() - end] {
                let n = step as isize + off;
                if n >= 0
                    && (n as usize) < self.size
                    && passable[n as usize]
                    && self.distance[n as usize] > next_distance
                {
                    // 入队
                    self.queue.push(n as usize);
                    self.distance[n as usize] = next_distance;
                }
            }
        }
    }

    /// 对照私有 `buildDistanceMap(int from, boolean[] to, boolean[] passable)`（L284-L330）：
    /// 多目标反向泛洪（目标集在 `self.goals`），抵达 `from` 即停。
    fn build_distance_map_to_goals(&mut self, from: usize, passable: &[bool]) -> bool {
        if self.goals[from] {
            return false;
        }

        self.distance.fill(UNREACHABLE);

        let mut path_found = false;
        let dir_lr = self.dir_lr;

        self.queue.clear();
        let mut head = 0;

        // 全部目标格入队（L298-L303）
        for (i, &is_goal) in self.goals.iter().enumerate() {
            if is_goal {
                self.queue.push(i);
                self.distance[i] = 0;
            }
        }

        while head < self.queue.len() {
            // 出队
            let step = self.queue[head];
            head += 1;
            if step == from {
                path_found = true;
                break;
            }
            let next_distance = self.distance[step] + 1;

            let start = if step.is_multiple_of(self.width) {
                3
            } else {
                0
            };
            let end = if (step + 1).is_multiple_of(self.width) {
                3
            } else {
                0
            };
            for &off in &dir_lr[start..dir_lr.len() - end] {
                let n = step as isize + off;
                if n == from as isize
                    || (n >= 0
                        && (n as usize) < self.size
                        && passable[n as usize]
                        && self.distance[n as usize] > next_distance)
                {
                    // 入队
                    self.queue.push(n as usize);
                    self.distance[n as usize] = next_distance;
                }
            }
        }

        path_found
    }

    /// 对照 `buildEscapeDistanceMap`（L332-L380）：以逃离点 `from` 为源正向泛洪；
    /// `lookAhead` 是希望在当前距离之上再拉开的格数。返回找到的最高距离
    /// （至多 `distance[cur] + look_ahead`）。
    fn build_escape_distance_map(
        &mut self,
        cur: usize,
        from: usize,
        look_ahead: i32,
        passable: &[bool],
    ) -> i32 {
        self.distance.fill(UNREACHABLE);

        let mut dest_dist = i32::MAX;
        let dir_lr = self.dir_lr;

        self.queue.clear();
        let mut head = 0;

        // 入队
        self.queue.push(from);
        self.distance[from] = 0;

        let mut dist = 0;

        while head < self.queue.len() {
            // 出队
            let step = self.queue[head];
            head += 1;
            dist = self.distance[step];

            if dist > dest_dist {
                return dest_dist;
            }

            if step == cur {
                dest_dist = dist + look_ahead;
            }

            let next_distance = dist + 1;

            let start = if step.is_multiple_of(self.width) {
                3
            } else {
                0
            };
            let end = if (step + 1).is_multiple_of(self.width) {
                3
            } else {
                0
            };
            for &off in &dir_lr[start..dir_lr.len() - end] {
                let n = step as isize + off;
                if n >= 0
                    && (n as usize) < self.size
                    && passable[n as usize]
                    && self.distance[n as usize] > next_distance
                {
                    // 入队
                    self.queue.push(n as usize);
                    self.distance[n as usize] = next_distance;
                }
            }
        }

        dist
    }

    /// `find`/`getStep`/`getStepBack` 共用的"单步下坡"扫描
    /// （L88-L102、L116-L127、L184-L199）：按 `dir` 顺序取距离严格更小的邻居，
    /// 平手取先序；无更小邻居时原地返回 `s`。
    /// 越界候选跳过（Java 会抛异常，见模块级差异说明 1）。
    fn downhill_step(&self, s: usize) -> usize {
        let mut min_d = self.distance[s];
        let mut best = s;

        for &off in &self.dir {
            let n = s as isize + off;
            if n < 0 || n as usize >= self.size {
                continue;
            }
            let this_d = self.distance[n as usize];
            if this_d < min_d {
                min_d = this_d;
                best = n as usize;
            }
        }

        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: i32 = UNREACHABLE;

    /// `#` 为墙、`.` 为可通行。
    fn parse_passable(rows: &[&str]) -> (usize, Vec<bool>) {
        let width = rows[0].len();
        let mut passable = Vec::with_capacity(width * rows.len());
        for row in rows {
            assert_eq!(row.len(), width);
            passable.extend(row.bytes().map(|b| b == b'.'));
        }
        (width, passable)
    }

    /// 邻接表内容对拍 `PathFinder.java` L66-L74（width = 7）。
    #[test]
    fn neighbour_tables_match_java() {
        let pf = PathFinder::new(7, 5);
        assert_eq!(pf.neighbours4, [-7, -1, 1, 7]);
        assert_eq!(pf.neighbours8, [-8, -7, -6, -1, 1, 6, 7, 8]);
        assert_eq!(pf.neighbours9, [-8, -7, -6, -1, 0, 1, 6, 7, 8]);
        assert_eq!(pf.circle4, [-7, 1, 7, -1]);
        assert_eq!(pf.circle8, [-8, -7, -6, 1, 8, 7, 6, -1]);
        assert_eq!(pf.dir, [-1, 1, -7, 7, -8, -6, 6, 8]);
        assert_eq!(pf.dir_lr, [-8, -1, 6, -7, 7, -6, 1, 8]);
    }

    /// 7×7 迷宫全量距离图，期望值来自逐字复刻的 Java `buildDistanceMap(int, boolean[])`
    /// （L382-L412）在同一地图上的实际输出。
    #[test]
    fn maze_distance_map_matches_java() {
        let (width, passable) = parse_passable(&[
            "#######", //
            "#.....#", //
            "#.###.#", //
            "#.#...#", //
            "#.#.#.#", //
            "#.....#", //
            "#######", //
        ]);
        let mut pf = PathFinder::new(width, 7);
        pf.build_distance_map(8, &passable); // to = (1,1)

        #[rustfmt::skip]
        let expected = [
            M, M, M, M, M, M, M,
            M, 0, 1, 2, 3, 4, M,
            M, 1, M, M, M, 4, M,
            M, 2, M, 6, 5, 5, M,
            M, 3, M, 5, M, 6, M,
            M, 4, 4, 5, 6, 7, M,
            M, M, M, M, M, M, M,
        ];
        assert_eq!(pf.distance(), expected);
    }

    /// 带 `limit` 的距离图（L248-L282）：距离 > limit 的格子保持不可达。
    /// 期望值来自 Java 输出。
    #[test]
    fn maze_limited_distance_map_matches_java() {
        let (width, passable) = parse_passable(&[
            "#######", //
            "#.....#", //
            "#.###.#", //
            "#.#...#", //
            "#.#.#.#", //
            "#.....#", //
            "#######", //
        ]);
        let mut pf = PathFinder::new(width, 7);
        pf.build_distance_map_limited(8, &passable, 3);

        #[rustfmt::skip]
        let expected = [
            M, M, M, M, M, M, M,
            M, 0, 1, 2, 3, M, M,
            M, 1, M, M, M, M, M,
            M, 2, M, M, M, M, M,
            M, 3, M, M, M, M, M,
            M, M, M, M, M, M, M,
            M, M, M, M, M, M, M,
        ];
        assert_eq!(pf.distance(), expected);
    }

    /// 最短路与单步对拍 Java `find`/`getStep`（L77-L130）在同一迷宫的输出：
    /// `find(31, 8) = [37, 29, 22, 15, 8]`、`getStep(31, 8) = 37`。
    /// 路径首步走对角 (3,4)→(2,5)，验证下坡扫描的 `dir` 平手顺序（L66）。
    #[test]
    fn maze_path_and_step_match_java() {
        let (width, passable) = parse_passable(&[
            "#######", //
            "#.....#", //
            "#.###.#", //
            "#.#...#", //
            "#.#.#.#", //
            "#.....#", //
            "#######", //
        ]);
        let mut pf = PathFinder::new(width, 7);

        let path = pf.get_path(31, 8, &passable).expect("可达");
        assert_eq!(path, Path::from([37, 29, 22, 15, 8]));

        assert_eq!(pf.get_step(31, 8, &passable), Some(37));
    }

    /// 对角步规则对拍：SPD 3.3.8 的 `PathFinder` 没有 `canStep`，BFS 对角扩展
    /// 只要求目标格 passable（L233-L242），两侧正交格全是墙也允许斜穿。
    /// 地图中 (1,1)→(2,2) 的两个正交格 (2,1)、(1,2) 均为墙，Java 输出
    /// `distance[(2,2)] = 1`、`getStep(18, 6) = 12`、`find(18, 6) = [12, 6]`。
    #[test]
    fn diagonal_corner_cut_allowed_like_java() {
        let (width, passable) = parse_passable(&[
            "#####", //
            "#.#.#", //
            "##..#", //
            "#...#", //
            "#####", //
        ]);
        let mut pf = PathFinder::new(width, 5);
        pf.build_distance_map(6, &passable); // to = (1,1)

        #[rustfmt::skip]
        let expected = [
            M, M, M, M, M,
            M, 0, M, 2, M,
            M, M, 1, 2, M,
            M, 2, 2, 2, M,
            M, M, M, M, M,
        ];
        assert_eq!(pf.distance(), expected);

        assert_eq!(pf.get_step(18, 6, &passable), Some(12));
        assert_eq!(pf.get_path(18, 6, &passable), Some(Path::from([12, 6])));
    }

    /// 地图左右边缘的 `dirLR` 首尾裁剪（L231-L232）防止线性索引回绕：
    /// 4×2 全通地图上 (0,1) 与 (3,0) 索引相邻（4 - 1 = 3）但不相通，
    /// Java 输出 `distance[4] = 3` 而非 1。
    #[test]
    fn map_edge_trim_prevents_wraparound() {
        let (width, passable) = parse_passable(&[
            "....", //
            "....", //
        ]);
        let mut pf = PathFinder::new(width, 2);
        pf.build_distance_map(3, &passable); // to = (3,0)

        #[rustfmt::skip]
        let expected = [
            3, 2, 1, 0,
            3, 2, 1, 1,
        ];
        assert_eq!(pf.distance(), expected);
    }

    /// 不可达与 `from == to` 时返回 `None`（L206-L208 与 L245 的 `pathFound`）。
    #[test]
    fn unreachable_and_same_cell_yield_none() {
        let (width, passable) = parse_passable(&[
            "#####", //
            "#.#.#", //
            "#####", //
        ]);
        let mut pf = PathFinder::new(width, 3);
        assert_eq!(pf.get_path(6, 8, &passable), None);
        assert_eq!(pf.get_step(6, 8, &passable), None);
        assert_eq!(pf.get_path(6, 6, &passable), None);
        assert_eq!(pf.get_step(6, 6, &passable), None);
    }

    /// `getStepBack` 逃跑步语义（L132-L202）对拍 Java 输出：
    /// 直走廊 `#.....#` 中从 (1,1) 逃离、身处 (3,1) 时后退一步到 (4,1)=11；
    /// 已被逼到死角 (5,1) 时目标集含自身（L286-L288）返回 -1（`None`）。
    #[test]
    fn get_step_back_corridor_and_dead_end() {
        let (width, passable) = parse_passable(&[
            "#######", //
            "#.....#", //
            "#######", //
        ]);
        let mut pf = PathFinder::new(width, 3);

        let mut p1 = passable.clone();
        assert_eq!(pf.get_step_back(10, 8, 8, &mut p1, true), Some(11));

        let mut p2 = passable.clone();
        assert_eq!(pf.get_step_back(12, 8, 8, &mut p2, true), None);
    }

    /// `canApproachFromPos` 两种语义对拍 Java 输出（对应 `Dungeon.flee` 的
    /// 恐惧分支，lookahead 8 / 4）：岔路图中威胁在 (1,1)=10、自身在 (3,1)=12，
    /// 长逃生臂须借道威胁邻格 (2,1)=11 进入——
    /// 可接近时走 11（Java 返回 11）；恐惧时 11 被原地改为不可通行（L162-L163）
    /// 只能退进短死路臂，Java 返回 13 且 `passable[11]` 变为 `false`。
    #[test]
    fn get_step_back_terror_blocks_approach_and_mutates_passable() {
        let (width, passable) = parse_passable(&[
            "#########", //
            "#......##", //
            "#.#######", //
            "#......##", //
            "#########", //
        ]);
        let mut pf = PathFinder::new(width, 5);

        let mut p1 = passable.clone();
        assert_eq!(pf.get_step_back(12, 10, 8, &mut p1, true), Some(11));
        assert_eq!(p1, passable, "可接近逃离点时不改写 passable");

        let mut p2 = passable.clone();
        assert_eq!(pf.get_step_back(12, 10, 4, &mut p2, false), Some(13));
        let mutated: Vec<usize> = passable
            .iter()
            .zip(&p2)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(mutated, [11], "恐惧语义只把靠近威胁的格 11 改为不可通行");
    }
}
