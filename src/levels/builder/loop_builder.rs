use bevy::{math, prelude::*};
use rand::seq::IndexedRandom;

use crate::levels::{
    builder::{Builder, regular_builder::RegularBuilder},
    room::RoomCore,
};

pub struct LoopBuilder {
    regular_builder: RegularBuilder,

    // LoopBuilder 特有的字段
    curve_exponent: i32,
    curve_intensity: f32,
    curve_offset: f32,
    loop_center: Option<Vec2>,
}

impl Default for LoopBuilder {
    fn default() -> Self {
        Self {
            regular_builder: RegularBuilder::default(),
            curve_exponent: 0,
            curve_intensity: 1.0,
            curve_offset: 0.0,
            loop_center: None,
        }
    }
}
impl LoopBuilder {
    pub fn regular_builder_mut(&mut self) -> &mut RegularBuilder {
        &mut self.regular_builder
    }

    pub fn regular_builder(&self) -> &RegularBuilder {
        &self.regular_builder
    }

    pub fn main_path_rooms_mut(&mut self) -> &mut Vec<Entity> {
        &mut self.regular_builder.main_path_rooms
    }

    pub fn path_tunnel_chances(&self) -> &Vec<f32> {
        &self.regular_builder.path_tunnel_chances
    }

    pub fn main_path_rooms(&self) -> &Vec<Entity> {
        &self.regular_builder.main_path_rooms
    }

    fn calculate_angle(&self, percent_along: f32) -> f32 {
        let percent_along = percent_along + self.curve_offset;
        360.0
            * (self.curve_intensity * self.curve_equation(percent_along as f64) as f32
                + (1.0 - self.curve_intensity) * percent_along
                - self.curve_offset)
    }

    fn curve_equation(&self, x: f64) -> f64 {
        let exp = 2. * self.curve_exponent as f64;
        4_f64.powf(exp) * ((x % 0.5) - 0.25).powf(exp + 1.0) + 0.25 + 0.5 * (2.0 * x).floor()
    }

    fn random_branch_angle(&self, (_, room_core): (Entity, &mut RoomCore)) -> f32 {
        if let Some(center) = self.loop_center {
            // 基于环路中心计算角度
            let room_center = room_core.rect.center();
            let to_center = room_center.angle_to(center);

            // 生成多个角度，选择最接近中心方向的
            let mut curr_angle = rand::random::<f32>() * 360.0;
            for _ in 0..4 {
                let new_angle = rand::random::<f32>() * 360.0;
                if (to_center - new_angle).abs() < (to_center - curr_angle).abs() {
                    curr_angle = new_angle;
                }
            }
            curr_angle
        } else {
            self.regular_builder.random_branch_angle()
        }
    }
}

impl Builder for LoopBuilder {
    fn build(&mut self, rooms: &mut Query<(Entity, &mut RoomCore)>) -> Option<Vec<Entity>> {
        self.regular_builder.setup_rooms(&mut rooms);

        if let Some(enterance) = self.regular_builder.entrance {
            //rooms.get(enterance).unwrap().1.s
        } else {
            return None;
        }
        //self.regular_builder.entrance.as_mut()?.set_size();
        //self.regular_builder.entrance.as_mut()?.set_pos(0, 0);

        let start_angle = rand::random::<f32>() * 360.0;

        self.main_path_rooms_mut()
            .push(self.regular_builder.entrance.clone()?);
        if let Some(ref exit) = self.regular_builder.exit {
            self.main_path_rooms_mut()
                .insert(self.main_path_rooms().len() / 2, exit.clone());
        }

        let mut loop_rooms = Vec::new();
        let mut path_tunnels = self.path_tunnel_chances().clone();

        let mut rng = rand::rng();

        for room in &self.regular_builder.main_path_rooms {
            loop_rooms.push(room.clone());

            let tunnels = path_tunnels.choose(&mut rng).unwrap();
            if *tunnels == -1. {
                path_tunnels = self.path_tunnel_chances().clone();
                tunnels = path_tunnels.choose(&mut rng).unwrap();
            }

            for _ in 0..(*tunnels as usize) {
                //loop_rooms.push();
            }
        }

        let mut prev = self.regular_builder.entrance.clone()?;
        for (i, room) in loop_rooms.iter().enumerate().skip(1) {
            if self.place_room(&rooms, &prev, room, Builder::angle_between_rooms(rooms.get(prev).unwrap().1, to)).is_none() {
                return None;
            }
            prev = room.clone();
        }

        //self.loop_center = Some(self.calculate_loop_center(&loop_rooms));

        Some(rooms.iter().map(|(entity, _)| entity).collect())
    }
}
