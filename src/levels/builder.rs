use crate::levels::room::{RoomCore, Room};
use bevy::prelude::*;


pub trait Builder {
    fn build(&mut self, rooms: &mut Query<(Entity, &mut RoomCore)>) -> Option<Vec<Entity>>;

    fn find_neighbors(&self, rooms: &mut Query<(Entity, &mut RoomCore)>) {
        let mut rooms = rooms.iter_mut().collect::<Vec<_>>();
        let len = rooms.len();
        for i in 0..len.saturating_sub(1) {
            let (left, right) = rooms.split_at_mut(i + 1);

            let (cur_room_entity, cur_room) = &mut left[i];

            for (other_room_entity, other_room) in right.iter_mut() {
                cur_room.add_neighbor(*cur_room_entity, (*other_room_entity, other_room));
            }
        }
    }

    fn angle_between_rooms(from: &RoomCore, to: &RoomCore) -> f32 {
        from.rect.center().angle_to(to.rect.center())
    }

    fn place_room(collision: &mut Query<(Entity, &mut RoomCore)>);

    fn find_free_space(start: Vec2, collision: &mut Query<(Entity, &mut RoomCore)>, max_size: i32) {
    }
}
