//! 特殊房间与密室框架，对照
//! `core/.../levels/rooms/special/SpecialRoom.java` 与 `rooms/secret/SecretRoom.java`。
//!
//! 三期只移植样板：特殊房 `GardenRoom`（上锁门 + 花园地形），
//! 密室 `SecretGardenRoom`（隐藏门 + 高草 Patch）。池子/轮换/预算的**框架**
//! 按 Java 结构完整移植，池内容随后续移植扩充（清单见 docs/plans/24 实现笔记）。
//!
//! # run 级状态的重放
//!
//! Java 的 `runSpecials`/`runSecrets`/`regionSecretsThisRun` 是一局游戏的跨层
//! 状态（`Dungeon.init` L243-L250 在 `seed+1` 流上 initForRun，各层 initRooms
//! 消费并轮换）。本工程 `generate_level(seed, depth)` 是纯函数，等价方案：
//! 生成第 `depth` 层前，把 1..depth-1 各普通层的 initRooms 在**各自层流**上
//! 重放一遍，推进池子到与真实连续下潜一致的状态（见 `generator::run_pools_for_depth`）。

use bevy::math::IVec2;
use rand::{Rng, SeedableRng};

use crate::levels::{
    Level,
    painter::{fill_room, fill_room_inset},
    patch,
    random::{LevelRng, chances, float, shuffle},
    rooms::{DoorType, Room, set_door},
    terrain::Terrain,
};

/// 已移植的特殊房间池。Java 的 `EQUIP_SPECIALS`（9 种，SpecialRoom.java L83-L86）
/// 均未移植；`CONSUMABLE_SPECIALS`（10 种，L90-L94）移植了 `GardenRoom`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKind {
    /// `GardenRoom`：上锁的花园（LOCKED 门 + 高草/矮草）
    Garden,
}

/// 已移植的密室池。Java 的 `ALL_SECRETS`（12 种，SecretRoom.java L37-L41）
/// 移植了 `SecretGardenRoom`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// `SecretGardenRoom`：隐藏门 + 高草 Patch
    Garden,
}

/// 每域密室预算基数：整数部分保底 + 小数部分掷点加一
/// （`SecretRoom.baseRegionSecrets`，SecretRoom.java L47）。
const BASE_REGION_SECRETS: [f32; 5] = [2.0, 2.25, 2.5, 2.75, 3.0];

/// 一局游戏的房间池状态（Java 的 `SpecialRoom`/`SecretRoom` 静态字段）。
#[derive(Debug, Clone)]
pub struct RunPools {
    /// `SpecialRoom.runSpecials`：整局特殊房队列，用过的轮换到队尾
    run_specials: Vec<SpecialKind>,
    /// `SpecialRoom.floorSpecials`：本层克隆池，用过即移除
    floor_specials: Vec<SpecialKind>,
    /// `SecretRoom.runSecrets`：整局密室队列，用过的轮换到队尾（池不缩水）
    run_secrets: Vec<SecretKind>,
    /// `SecretRoom.regionSecretsThisRun`：各域剩余密室预算
    region_secrets: [i32; 5],
}

impl RunPools {
    /// `Dungeon.init`（L243-L250）：`seed+1` 起 run 流，依次
    /// `SpecialRoom.initForRun`（L111-L129）、`SecretRoom.initForRun`（L50-L64）。
    /// 之前的 Scroll/Potion/Ring 标签洗牌属物品域，未移植、不消耗随机数。
    pub fn init_for_run(world_seed: u64) -> Self {
        let mut rng = LevelRng::seed_from_u64(world_seed.wrapping_add(1));

        // SpecialRoom.initForRun：equip/consumable 两队列各自洗牌后交错合并，
        // 队首恒为 consumable（L120-L126）。已移植集 equip=[]、consumable=[Garden]。
        let mut equips: Vec<SpecialKind> = vec![];
        let mut cons: Vec<SpecialKind> = vec![SpecialKind::Garden];
        shuffle(&mut rng, &mut equips);
        shuffle(&mut rng, &mut cons);
        let mut run_specials = vec![cons.remove(0)];
        while !equips.is_empty() || !cons.is_empty() {
            if !equips.is_empty() {
                run_specials.push(equips.remove(0));
            }
            if !cons.is_empty() {
                run_specials.push(cons.remove(0));
            }
        }

        // SecretRoom.initForRun L52-L59：各域预算 = 整数 + 小数位掷点
        let mut region_secrets = [0i32; 5];
        for (i, base) in BASE_REGION_SECRETS.iter().enumerate() {
            region_secrets[i] = *base as i32;
            // 域 0 小数位为 0：掷点必假但照样消耗（对齐 Java 循环结构）
            if float(&mut rng) < base.fract() {
                region_secrets[i] += 1;
            }
        }
        // L61-L62：密室队列洗牌
        let mut run_secrets = vec![SecretKind::Garden];
        shuffle(&mut rng, &mut run_secrets);

        Self {
            run_specials,
            floor_specials: Vec::new(),
            run_secrets,
            region_secrets,
        }
    }

    /// `SpecialRoom.initForFloor`（L131-L139）：本层池 = run 池克隆。
    /// LaboratoryRoom（每章 3/4 层）未移植。
    pub fn init_for_floor(&mut self) {
        self.floor_specials = self.run_specials.clone();
    }

    /// `SpecialRoom.createRoom`（L158-L190）：{6,3,1} 权重取队首附近，
    /// 用过的从本层池移除并轮换到 run 池队尾（`useType` L141-L152）。
    /// PitRoom/Laboratory 前置分支未移植；本层池耗尽返回 `None`
    /// （Java 池有 19 种耗不尽；本工程移植面窄，由调用方截断数量——见笔记）。
    pub fn create_special(&mut self, rng: &mut impl Rng) -> Option<SpecialKind> {
        if self.floor_specials.is_empty() {
            return None;
        }
        // L176-L178：60%/30%/10% 取前三位，越界向下收
        let mut index = chances(rng, &[6.0, 3.0, 1.0]).expect("权重和恒正");
        while index >= self.floor_specials.len() {
            index -= 1;
        }
        let kind = self.floor_specials[index];
        self.use_type(kind);
        Some(kind)
    }

    /// `SpecialRoom.useType`（L141-L152）。CRYSTAL_KEY/POTION_SPAWN 分组连坐
    /// （L143-L148）对已移植集为空集，结构略。
    fn use_type(&mut self, kind: SpecialKind) {
        if let Some(i) = self.floor_specials.iter().position(|&k| k == kind) {
            self.floor_specials.remove(i);
        }
        if let Some(i) = self.run_specials.iter().position(|&k| k == kind) {
            self.run_specials.remove(i);
            self.run_specials.push(kind);
        }
    }

    /// `SecretRoom.createRoom`（L90-L101）：{6,3,1} 取队首附近并轮换到队尾。
    /// 密室池不缩水，同层多密室会复用同类。
    pub fn create_secret(&mut self, rng: &mut impl Rng) -> SecretKind {
        let mut index = chances(rng, &[6.0, 3.0, 1.0]).expect("权重和恒正");
        while index >= self.run_secrets.len() {
            index -= 1;
        }
        let kind = self.run_secrets.remove(index);
        self.run_secrets.push(kind);
        kind
    }

    /// `SecretRoom.secretsForFloor`（L66-L88）：本层密室数 = 域剩余预算按
    /// 剩余层数均摊 + 小数位掷点（消耗调用方的层流），并从预算扣除。
    /// Java `floorsLeft == 0` 分支不可达（`depth%5 ∈ [0,4]` → 剩余 ∈ [1,5]），未移植。
    pub fn secrets_for_floor(&mut self, rng: &mut impl Rng, depth: i32) -> i32 {
        if depth == 1 {
            return 0;
        }
        let region = (depth / 5) as usize;
        let floor = depth % 5;
        let floors_left = 5 - floor;

        let raw = self.region_secrets[region] as f32 / floors_left as f32;
        let secrets = if float(rng) < raw.fract() {
            raw.ceil() as i32
        } else {
            raw.floor() as i32
        };
        self.region_secrets[region] -= secrets;
        secrets
    }

    /// 测试用：域剩余预算。
    #[cfg(test)]
    pub(crate) fn region_budget(&self, region: usize) -> i32 {
        self.region_secrets[region]
    }
}

/// `SewerLevel.specialRooms`（L94-L99）：forceMax（LARGE 氛围）恒 2 不掷点；
/// 否则 1 + chances({1,4}) ∈ [1,2]，均值 1.8。
pub(crate) fn sewer_special_rooms_count(rng: &mut impl Rng, force_max: bool) -> usize {
    if force_max {
        2
    } else {
        1 + chances(rng, &[1.0, 4.0]).unwrap_or(0)
    }
}

/// 把特殊/密室房的"入口门"（首个连接，`SpecialRoom.entrance()` L53-L62）
/// 升级为指定门型并同步两侧。
fn upgrade_entrance_door(rooms: &mut [Room], ri: usize, kind: DoorType) {
    let (n, door) = rooms[ri]
        .connected
        .first()
        .map(|&(n, d)| (n, d.expect("place_doors 已就位")))
        .expect("特殊/密室房必有恰一个连接");
    let mut door = door;
    door.set(kind);
    set_door(rooms, ri, n, door);
}

/// 特殊房间 paint 分发。
pub(crate) fn paint_special(
    _rng: &mut impl Rng,
    level: &mut Level,
    rooms: &mut [Room],
    ri: usize,
    kind: SpecialKind,
) {
    match kind {
        SpecialKind::Garden => paint_garden(level, rooms, ri),
    }
}

/// 密室 paint 分发。
pub(crate) fn paint_secret(
    rng: &mut impl Rng,
    level: &mut Level,
    rooms: &mut [Room],
    ri: usize,
    kind: SecretKind,
) {
    match kind {
        SecretKind::Garden => paint_secret_garden(rng, level, rooms, ri),
    }
}

/// `GardenRoom.paint`（GardenRoom.java L36-L65）：高草圈 + 矮草芯，入口上锁。
/// 降级（见笔记）：
/// - `level.addItemToSpawn(new IronKey(depth))`（L43）——钥匙投放属物品域，TODO；
/// - 灌木掷点与种植（L45-L53）、Foliage 光照 blob（L55-L64）——植物/blob 域，
///   未移植、不消耗随机数。
fn paint_garden(level: &mut Level, rooms: &mut [Room], ri: usize) {
    fill_room(level, &rooms[ri], Terrain::Wall);
    fill_room_inset(level, &rooms[ri], 1, Terrain::HighGrass);
    fill_room_inset(level, &rooms[ri], 2, Terrain::Grass);

    // L42：entrance().set(Door.Type.LOCKED) —— 铁钥匙投放 TODO（物品域）
    upgrade_entrance_door(rooms, ri, DoorType::Locked);
}

/// `SecretGardenRoom.paint`（SecretGardenRoom.java L35-L71）：
/// 矮草打底 + Patch 高草，入口为隐藏门。
/// 降级：星花/再生藤种植（L51-L59）与 Foliage blob（L61-L70）属植物/blob 域，
/// 未移植、不消耗随机数（Patch 掷点保留）。
fn paint_secret_garden(rng: &mut impl Rng, level: &mut Level, rooms: &mut [Room], ri: usize) {
    let rect = rooms[ri].rect;
    let (w, h) = (rect.width() + 1, rect.height() + 1);
    fill_room(level, &rooms[ri], Terrain::Wall);
    fill_room_inset(level, &rooms[ri], 1, Terrain::Grass);

    // L40-L47：Patch(0.5, 0) 高草，房内坐标系 (w-2)×(h-2)
    let grass = patch::generate(rng, (w - 2) as usize, (h - 2) as usize, 0.5, 0, true);
    for y in (rect.top + 1)..rect.bottom {
        for x in (rect.left + 1)..rect.right {
            let idx = ((x - rect.left - 1) + (y - rect.top - 1) * (w - 2)) as usize;
            if grass[idx] {
                level.set_terrain(IVec2::new(x, y), Terrain::HighGrass);
            }
        }
    }

    // L49：entrance().set(Door.Type.HIDDEN)
    upgrade_entrance_door(rooms, ri, DoorType::Hidden);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::random::LevelRng;
    use rand::SeedableRng;

    /// initForRun 的池内容与确定性：同种子同池序。
    #[test]
    fn init_for_run_is_deterministic() {
        let a = RunPools::init_for_run(99);
        let b = RunPools::init_for_run(99);
        assert_eq!(a.run_specials, b.run_specials);
        assert_eq!(a.run_secrets, b.run_secrets);
        assert_eq!(a.region_secrets, b.region_secrets);
        // 域 0（下水道）预算恒 2：基数 2.0 的小数位掷点必假（SecretRoom.java L47）
        for seed in 0..64u64 {
            assert_eq!(RunPools::init_for_run(seed).region_secrets[0], 2);
        }
    }

    /// createSpecial：{6,3,1} 收缩到池首；层池耗尽返回 None；run 池轮换到队尾。
    #[test]
    fn create_special_drains_floor_pool_and_rotates() {
        let mut pools = RunPools::init_for_run(7);
        let mut rng = LevelRng::seed_from_u64(1);

        pools.init_for_floor();
        assert_eq!(pools.create_special(&mut rng), Some(SpecialKind::Garden));
        // 本层池已耗尽（移植面 1 种）
        assert_eq!(pools.create_special(&mut rng), None);
        // run 池轮换后仍含 Garden，下一层可再抽
        pools.init_for_floor();
        assert_eq!(pools.create_special(&mut rng), Some(SpecialKind::Garden));
    }

    /// createSecret：池不缩水，可连续抽取。
    #[test]
    fn create_secret_never_exhausts() {
        let mut pools = RunPools::init_for_run(7);
        let mut rng = LevelRng::seed_from_u64(2);
        for _ in 0..8 {
            assert_eq!(pools.create_secret(&mut rng), SecretKind::Garden);
        }
    }

    /// secretsForFloor：depth 1 恒 0 且不消耗预算；域内各层取数之和恰为预算，
    /// 域末层（depth%5==4 → floorsLeft 1）拿走全部剩余。
    #[test]
    fn secrets_for_floor_spends_exact_region_budget() {
        for seed in 0..64u64 {
            let mut pools = RunPools::init_for_run(seed);
            let mut rng = LevelRng::seed_from_u64(seed ^ 0xABCD);
            assert_eq!(pools.secrets_for_floor(&mut rng, 1), 0);
            assert_eq!(pools.region_budget(0), 2, "depth 1 不消耗预算");

            let mut total = 0;
            for depth in 2..=4 {
                let n = pools.secrets_for_floor(&mut rng, depth);
                assert!((0..=2).contains(&n), "seed {seed} depth {depth}：{n}");
                total += n;
            }
            assert_eq!(total, 2, "seed {seed}：域 0 预算应被精确花完");
            assert_eq!(pools.region_budget(0), 0);
        }
    }

    /// specialRooms 数量表：forceMax 恒 2；否则 1-2、均值约 1.8（{1,4} 权重）。
    #[test]
    fn sewer_special_count_distribution() {
        let mut rng = LevelRng::seed_from_u64(3);
        assert_eq!(sewer_special_rooms_count(&mut rng, true), 2);

        let mut twos = 0;
        const N: usize = 5000;
        for _ in 0..N {
            let n = sewer_special_rooms_count(&mut rng, false);
            assert!((1..=2).contains(&n));
            if n == 2 {
                twos += 1;
            }
        }
        // 期望 4/5 = 4000，±300 已远超 6σ
        assert!((3700..=4300).contains(&twos), "P(2) 应约 0.8，得 {twos}/{N}");
    }
}
