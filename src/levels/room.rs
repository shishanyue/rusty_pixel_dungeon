use bevy::prelude::*;

pub mod empty_room;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub enum ConnectionLogic {
    #[default]
    All,
    Left,
    Top,
    Right,
    Bottom,
}

#[derive(Debug, Reflect, Clone)]
pub struct SizeCategory {
    pub min_dim: i32,
    pub max_dim: i32,
    pub room_value: i32,
}

impl SizeCategory {
    pub const NORMAL: Self = Self {
        min_dim: 4,
        max_dim: 10,
        room_value: 1,
    };

    pub const LARGE: Self = Self {
        min_dim: 10,
        max_dim: 14,
        room_value: 2,
    };

    pub const GIANT: Self = Self {
        min_dim: 14,
        max_dim: 18,
        room_value: 3,
    };

    pub const ZERO: Self = Self {
        min_dim: 0,
        max_dim: 0,
        room_value: 0,
    };

    pub fn new(min_dim: i32, max_dim: i32, room_value: i32) -> Self {
        Self {
            min_dim,
            max_dim,
            room_value,
        }
    }
}

impl Default for SizeCategory {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Component, Default, Debug, Clone, Reflect)]
pub struct RoomCore {
    pub rect: Rect,
    pub size_category: SizeCategory,
    pub neighbours: Vec<Entity>,
    pub connection_logic: ConnectionLogic,
    pub distance: i32,
    pub price: i32,
    pub min_connections: i32,
    pub max_connections: i32,
    pub is_entrance: bool,
    pub is_exit: bool,
    pub connection_weight: i32,
}

impl RoomCore {
    pub const STANDARD: Self = Self {
        rect: Rect::EMPTY,
        size_category: SizeCategory::NORMAL,
        neighbours: Vec::new(),
        connection_logic: ConnectionLogic::All,
        distance: 0,
        price: 0,
        min_connections: 1,
        max_connections: 4,
        is_entrance: false,
        is_exit: false,
        connection_weight: 1,
    };
}

impl Room for RoomCore {
    fn size_category(&self) -> &SizeCategory {
        &self.size_category
    }

    fn set_size_category(&mut self, size_category: SizeCategory) -> bool {
        self.size_category = size_category;
        true
    }
    fn mob_spawn_weight(&self) -> i32 {
        if self.is_entrance {
            return 1;
        }
        self.size_category.room_value
    }
    fn connection_weight(&self) -> i32 {
        self.size_category.room_value.pow(2)
    }

    fn set_empty(&mut self) {
        self.rect = Rect::EMPTY;
    }

    fn add_neighbor(
        &mut self,
        self_entity: Entity,
        (other_entity, other): (Entity, &mut RoomCore),
    ) -> bool {
        if self.neighbours.contains(&other_entity) {
            return true;
        }

        let rect = Rect::intersect(&self.rect, other.rect);

        if (rect.width() == 0. && rect.height() >= 2.)
            || (rect.height() == 0. && rect.width() >= 2.)
        {
            self.neighbours.push(other_entity);
            other.neighbours.push(self_entity);
            true
        } else {
            false
        }
    }
}

pub trait Room {
    fn size_category(&self) -> &SizeCategory;

    fn min_dim(&self) -> i32 {
        self.size_category().min_dim
    }
    fn max_dim(&self) -> i32 {
        self.size_category().max_dim
    }
    fn max_height(&self) -> i32 {
        self.size_category().room_value
    }
    fn min_height(&self) -> i32 {
        self.size_category().room_value
    }

    fn size_factor(&self) -> i32 {
        self.size_category().room_value as i32
    }

    fn set_size_category(&mut self, size_category: SizeCategory) -> bool;
    fn mob_spawn_weight(&self) -> i32;
    fn connection_weight(&self) -> i32;
    fn set_empty(&mut self);
    fn add_neighbor(&mut self, self_entity: Entity, other: (Entity, &mut RoomCore)) -> bool;
    //fn can_merge();
}

pub struct RoomPlugin;

impl Plugin for RoomPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, |mut aa: Query<(Entity, &mut RoomCore)>| {});
    }
}
