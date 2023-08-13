use bevy_ecs::{prelude::Component, reflect::ReflectComponent};
use bevy_reflect::Reflect;
use bevy_scene2::{ReflectSchematic, Schematic};

#[derive(Component, Reflect, Schematic, Default, Debug)]
#[reflect(Component, Schematic)]
pub struct Div {
    pub width: u32,
    pub height: u32,
}

#[derive(Component, Reflect, Schematic, Default, Debug)]
#[reflect(Component, Schematic)]
pub struct Label {
    pub val: String,
}
