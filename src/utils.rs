pub mod room_helper;

pub trait PropertyPath {
    fn get_property_path(&self) -> &'static str;
}

pub trait StaticPropertyPath {
    fn get_property_path() -> &'static str;
}

pub trait LevelPropertyPath {
    fn get_property_path() -> &'static str {
        "messages/levels/levels.properties"
    }
    fn get_property_key(&self) -> String;
}
