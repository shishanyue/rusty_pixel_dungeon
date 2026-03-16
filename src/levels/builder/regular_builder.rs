use crate::{
    levels::room::{RoomCore, RoomImpl},
    utils::room_helper::RoomHelper,
};
use bevy::prelude::*;
use rand::seq::{IndexedRandom, SliceRandom};

pub struct RegularBuilder {
    pub path_variance: f32,
    pub path_length: f32,
    pub path_tunnel_chances: Vec<f32>,
    pub branch_tunnel_chances: Vec<f32>,
    pub extra_connection_chance: f32,

    pub entrance: Option<Entity>,
    pub exit: Option<Entity>,
    pub shop: Option<Entity>,
    pub main_path_rooms: Vec<Entity>,
    pub multi_connections: Vec<Entity>,
    pub single_connections: Vec<Entity>,
}

impl Default for RegularBuilder {
    fn default() -> Self {
        Self {
            path_variance: 45.0,
            path_length: 0.25,
            path_tunnel_chances: vec![2.0, 2.0, 1.0],
            branch_tunnel_chances: vec![1.0, 1.0, 0.0],
            extra_connection_chance: 0.30,
            entrance: None,
            exit: None,
            shop: None,
            main_path_rooms: Vec::new(),
            multi_connections: Vec::new(),
            single_connections: Vec::new(),
        }
    }
}

impl RegularBuilder {
    pub fn setup_rooms(&mut self, room_helper: &mut RoomHelper) {
        self.entrance = None;
        self.exit = None;
        self.shop = None;
        self.main_path_rooms.clear();
        self.multi_connections.clear();
        self.single_connections.clear();

        let mut rooms = room_helper.rooms_mut();

        for (room_entity, room_core) in rooms.iter_mut() {
            let room_entity = *room_entity;

            room_core.set_empty();

            if room_core.is_entrance {
                self.entrance = Some(room_entity);
            } else if room_core.is_exit {
                self.exit = Some(room_entity);
            } else if room_core.max_connections > 1 {
                self.multi_connections.push(room_entity);
            } else if room_core.max_connections == 1 {
                self.single_connections.push(room_entity);
            }
        }
        let mut rng = rand::rng();
        self.weight_rooms(&mut rooms);
        self.multi_connections.shuffle(&mut rng);

        let mut rooms_on_main_path = (self.multi_connections.len() as f32 * self.path_length
            + self.path_tunnel_chances.choose(&mut rng).unwrap())
            as usize;

        while rooms_on_main_path > 0 && !self.multi_connections.is_empty() {
            let r = self.multi_connections.pop().unwrap();
            rooms_on_main_path -= rooms
                .iter()
                .find(|(entity, _)| *entity == r)
                .unwrap()
                .1
                .size_factor() as usize;
            self.main_path_rooms.push(r);
        }
    }

    pub fn weight_rooms(&mut self, rooms: &mut Vec<(Entity, &mut RoomCore)>) {
        for (room_entity, room_core) in rooms {
            let room_entity = *room_entity;

            if self.multi_connections.contains(&room_entity) {
                let weight = room_core.connection_weight;
                for _ in 0..weight {
                    self.multi_connections.push(room_entity);
                }
            }
        }
    }

    pub fn create_branches(&mut self, rooms: Vec<Entity>, branchable: Vec<Entity>) -> bool {
        true
    }

    pub fn random_branch_angle(&self) -> f32 {
        rand::random::<f32>() * 360.0
    }
}
