// rust_dungeon_gen/src/room_macro.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Room)]
pub fn derive_room(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    let expanded = impl_room_macro(&input);

    TokenStream::from(expanded)
}

fn impl_room_macro(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // 生成自动添加的字段
    quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// 获取相邻房间列表
            pub fn neighbours(&self) -> &[Box<dyn Room>] {
                &self.neighbours
            }

            /// 获取可变的相邻房间列表
            pub fn neighbours_mut(&mut self) -> &mut Vec<Box<dyn Room>> {
                &mut self.neighbours
            }

            /// 获取已连接的房间
            pub fn connected(&self) -> &HashMap<usize, Door> {
                &self.connected
            }

            /// 获取可变的已连接房间
            pub fn connected_mut(&mut self) -> &mut HashMap<usize, Door> {
                &mut self.connected
            }

            /// 添加相邻房间
            pub fn add_neighbour(&mut self, other: Box<dyn Room>) -> bool {
                if self.neighbours.iter().any(|r| r.rect() == other.rect()) {
                    return true;
                }

                let intersection = self.rect().intersect(other.rect());
                if (intersection.width() == 0 && intersection.height() >= 2) ||
                   (intersection.height() == 0 && intersection.width() >= 2) {
                    self.neighbours.push(other);
                    true
                } else {
                    false
                }
            }

            /// 连接两个房间
            pub fn connect(&mut self, other: &mut dyn Room) -> bool {
                if (self.neighbours.iter().any(|r| r.rect() == other.rect()) ||
                    self.add_neighbour(Box::new(other.clone()))) &&
                   !self.connected.contains_key(&other.id()) &&
                   self.can_connect_room(other) {
                    self.connected.insert(other.id(), Door::new(self.find_connection_point(other)));
                    true
                } else {
                    false
                }
            }

            /// 清除所有连接
            pub fn clear_connections(&mut self) {
                self.neighbours.clear();
                self.connected.clear();
            }

            /// 查找连接点
            fn find_connection_point(&self, other: &dyn Room) -> Point {
                let intersection = self.rect().intersect(other.rect());
                let points = intersection.get_points();
                points.into_iter()
                    .find(|p| self.can_connect_point(p) && other.can_connect_point(p))
                    .unwrap_or_else(|| intersection.center())
            }
        }
    }
}
