use bevy::{
    prelude::*,
    scene2::{bsn, spawn_bsn, Bsn, Prop, ReflectSchematic, Schematic},
    ui::scene2::{Div, Label},
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin::default().watch_for_changes()))
        .register_type::<Colored>()
        .register_type::<Range>()
        .add_systems(Startup, setup.pipe(spawn_bsn))
        .add_systems(Update, print)
        .run();
}

fn setup() -> Bsn {
    bsn! {
        Div [(
            Colored {
                // the next line won't compile
                // color: { Rgba { r: 1, g: 0, b: 0, a: 1 } }
                color: { Rgba::WHITE }
            }
            Range { min: 0 value: 30 max: 10 }
        )]
    }
}

// can derive Prop without Schematic and Component?
#[derive(Clone, Schematic, Component, Default, Reflect, Debug)]
#[schematic(needed_for_compiling)]
#[reflect(Schematic, Component)]
pub struct Rgba {
    pub r: Prop<u32>,
    pub g: Prop<u32>,
    pub b: Prop<u32>,
    pub a: Prop<u32>,
}

impl Rgba {
    pub const WHITE: Rgba=Rgba { r: Prop::Value(1), g: Prop::Value(1), b: Prop::Value(1), a: Prop::Value(1) };
}

#[derive(Component, Clone, Schematic, Reflect, Default, Debug)]
#[schematic(colored)]
#[reflect(Schematic)]
pub struct Colored {
    color: Prop<Rgba>,
}

fn colored(In(props): In<Colored>) -> Bsn {
    let color = props.color.get();
    info!("colored");
    bsn! {
        Div [
            Label { val: {format!("Color: {color:?}")} }
        ]
    }
}

#[derive(Component, Clone, Schematic, Reflect, Default, Debug)]
#[schematic(range)]
#[reflect(Schematic)]
pub struct Range {
    min: Prop<u32>,
    value: Prop<u32>,
    max: Prop<u32>,
}

fn range(In(props): In<Range>) -> Bsn {
    let min = props.min.get();
    let value = props.value.get();
    let max = props.max.get();
    info!("range");
    bsn! {
        Div [
            Label { val: {format!("Range: {min} <= {value} <= {max}")} }
        ]
    }
}

fn print(labels: Query<&Label, Changed<Label>>) {
    for label in &labels {
        println!("{}", label.val);
    }
}

fn needed_for_compiling(In(_): In<Rgba>) -> Bsn {
    bsn! {
        Label
    }
}