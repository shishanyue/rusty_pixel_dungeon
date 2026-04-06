
use bevy::prelude::*;
pub mod terrain;

/// 地牢感觉/氛围类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feeling {
    None,
    Chasm,
    Water,
    Grass,
    Dark,
    Large,
    Traps,
    Secrets,
}

/// 地牢等级资源
#[derive(Resource, Debug)]
pub struct Level {
    /// 地图数据
    pub map: Vec<Terrain>,

    /// 地图尺寸
    pub width: usize,
    pub height: usize,

    /// 当前深度
    pub depth: i32,

    /// 地牢感觉
    pub feeling: Feeling,

    /// 入口位置
    pub entrance: IVec2,

    /// 出口位置
    pub exit: IVec2,

    /// 房间实体列表
    pub rooms: Vec<Entity>,

    /// 陷阱位置
    pub traps: Vec<IVec2>,

    /// 地形标志缓存
    pub passable: Vec<bool>,
    pub los_blocking: Vec<bool>,
    pub solid: Vec<bool>,
}

impl Level {
    /// 创建新的地牢等级
    pub fn new(width: usize, height: usize, depth: i32) -> Self {
        let size = width * height;
        Self {
            map: vec![Terrain::WALL; size],
            width,
            height,
            depth,
            feeling: Feeling::None,
            entrance: IVec2::ZERO,
            exit: IVec2::ZERO,
            rooms: Vec::new(),
            traps: Vec::new(),
            passable: vec![false; size],
            los_blocking: vec![true; size],
            solid: vec![true; size],
        }
    }

    /// 获取指定坐标的地形
    pub fn terrain(&self, pos: IVec2) -> Terrain {
        if self.is_valid(pos) {
            self.map[pos_to_index(pos, self.width)]
        } else {
            Terrain::WALL
        }
    }

    /// 设置指定坐标的地形
    pub fn set_terrain(&mut self, pos: IVec2, terrain: Terrain) {
        if self.is_valid(pos) {
            let index = pos_to_index(pos, self.width);
            self.map[index] = terrain;
            self.update_flags(index, terrain);
        }
    }

    /// 填充矩形区域的地形
    pub fn fill(&mut self, rect: IRect, terrain: Terrain) {
        for y in rect.min.y..=rect.max.y {
            for x in rect.min.x..=rect.max.x {
                self.set_terrain(IVec2::new(x, y), terrain);
            }
        }
    }

    /// 检查坐标是否有效
    pub fn is_valid(&self, pos: IVec2) -> bool {
        pos.x >= 0 && pos.x < self.width as i32 
            && pos.y >= 0 && pos.y < self.height as i32
    }

    /// 更新地形标志
    fn update_flags(&mut self, index: usize, terrain: Terrain) {
        let flags = terrain.flags();
        self.passable[index] = flags.contains(TerrainFlags::PASSABLE);
        self.los_blocking[index] = flags.contains(TerrainFlags::LOS_BLOCKING);
        self.solid[index] = flags.contains(TerrainFlags::SOLID);
    }

    /// 重置地牢
    pub fn reset(&mut self, width: usize, height: usize, depth: i32) {
        let size = width * height;
        self.map = vec![Terrain::WALL; size];
        self.width = width;
        self.height = height;
        self.depth = depth;
        self.feeling = Feeling::None;
        self.entrance = IVec2::ZERO;
        self.exit = IVec2::ZERO;
        self.rooms.clear();
        self.traps.clear();
        self.passable = vec![false; size];
        self.los_blocking = vec![true; size];
        self.solid = vec![true; size];
    }
}

/// 将坐标转换为索引
pub fn pos_to_index(pos: IVec2, width: usize) -> usize {
    (pos.y as usize * width) + pos.x as usize
}

/// 将索引转换为坐标
pub fn index_to_pos(index: usize, width: usize) -> IVec2 {
    IVec2::new(
        (index % width) as i32,
        (index / width) as i32,
    )
}

/// 矩形区域
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub min: IVec2,
    pub max: IVec2,
}

impl IRect {
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            min: IVec2::new(x1.min(x2), y1.min(y2)),
            max: IVec2::new(x1.max(x2), y1.max(y2)),
        }
    }

    pub fn from_center(center: IVec2, width: i32, height: i32) -> Self {
        let half_w = width / 2;
        let half_h = height / 2;
        Self {
            min: IVec2::new(center.x - half_w, center.y - half_h),
            max: IVec2::new(center.x + half_w, center.y + half_h),
        }
    }

    pub fn width(&self) -> i32 {
        self.max.x - self.min.x + 1
    }

    pub fn height(&self) -> i32 {
        self.max.y - self.min.y + 1
    }

    pub fn center(&self) -> IVec2 {
        IVec2::new(
            (self.min.x + self.max.x) / 2,
            (self.min.y + self.max.y) / 2,
        )
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
    }

    pub fn intersection(&self, other: &Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }

        Some(Self {
            min: IVec2::new(
                self.min.x.max(other.min.x),
                self.min.y.max(other.min.y),
            ),
            max: IVec2::new(
                self.max.x.min(other.max.x),
                self.max.y.min(other.max.y),
            ),
        })
    }
}

/// 地牢生成插件
pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Level>();
    }
}
