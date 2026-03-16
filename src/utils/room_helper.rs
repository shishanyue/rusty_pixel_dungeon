use bevy::prelude::*;

use crate::levels::room::RoomCore;

pub struct RoomHelper<'a> {
    rooms: Vec<RoomStorage<'a>>,
}

pub struct RoomStorage<'a> {
    entity: Entity,
    room: &'a mut RoomCore,
    dirt: bool,
}

impl<'a> RoomStorage<'a> {
    pub fn new(entity: Entity, room: &'a mut RoomCore, dirt: bool) -> Self {
        Self { entity, room, dirt }
    }
}

impl<'a> RoomHelper<'a> {
    pub fn new(rooms: &'a mut Query<'_, '_, (Entity, &mut RoomCore)>) -> Self {
        Self {
            rooms: rooms
                .iter_mut()
                .map(|(entity, room_core)| RoomStorage::new(entity, room_core.into_inner(), false))
                .collect(),
        }
    }
    pub fn rooms_mut(&mut self) -> Vec<(Entity, &mut RoomCore)> {
        self.rooms
            .iter_mut()
            .map(|room| (room.entity, &mut *room.room))
            .collect()
    }
    pub fn rooms(&mut self) -> Vec<(Entity, &RoomCore)> {
        self.rooms
            .iter()
            .map(|room| (room.entity, &*room.room))
            .collect()
    }
}
