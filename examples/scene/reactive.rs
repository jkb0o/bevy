use bevy::{
    prelude::*,
    scene2::{bsn, spawn_bsn, Bsn, Prop, ReflectSchematic, Schematic},
    ui::scene2::{Div, Label},
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin::default().watch_for_changes()))
        .register_type::<Example>()
        .insert_resource(Score(7))
        .add_systems(Startup, setup.pipe(spawn_bsn))
        .add_systems(Update, print)
        .run();
}

fn setup() -> Bsn {
    bsn! {
        Div [
            Example { score_multiplier: 2 }
        ]
    }
}

#[derive(Resource)]
pub struct Score(u32);

#[derive(Component, Clone, Schematic, Reflect, Default, Debug)]
#[schematic(example)]
#[reflect(Schematic)]
pub struct Example {
    score_multiplier: Prop<u32>,
}

/// NOTE: THIS IS NOT ACTUALLY REACTIVE YET. THIS IS JUST PROVING THAT WE CAN GENERATE SCENES
/// FROM PROPS PASSED INTO A SYSTEM, WHICH IS A FOUNDATION FOR A REACTIVE APPROACH.
/// In theory, we would check to see if the props change (using ECS change detection), then
/// generate the new BSN, then decide how to apply that BSN on top of the existing scene.
fn example(In(props): In<Example>, score: Res<Score>) -> Bsn {
    let score = score.0 * props.score_multiplier.get();
    bsn! {
        Div [
            Label { val: {format!("Score: {score}")} }
        ]
    }
}

fn print(labels: Query<&Label, Changed<Label>>) {
    for label in &labels {
        println!("{}", label.val);
    }
}
