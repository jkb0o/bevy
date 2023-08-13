use bevy::{
    prelude::*,
    scene2::{bsn, spawn_bsn, Bsn, ReflectSchematic, Schematic},
    ui::scene2::{Div, Label},
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin::default().watch_for_changes()))
        .register_type::<Img>()
        .insert_resource(Score(7))
        .add_systems(Startup, setup.pipe(spawn_bsn))
        .add_systems(Update, print)
        .run();
}

fn setup(score: Res<Score>) -> Bsn {
    let score = score.0;
    let ten = 10;
    bsn! {
        Div { width: 10 height: 100 } [
            Img { handle: "branding/icon.png" }
            Div {
                width: 100
                height: 200
            }
            (Div { width: ten } Label { val: {format!("Score {score}")}})
            (Div { width: 200 } @"scenes/nested.bsn")
        ]
    }
}

#[derive(Resource)]
pub struct Score(u32);

#[derive(Component, Reflect, Schematic, Default, Debug)]
#[reflect(Component, Schematic)]
pub struct Img {
    #[schematic]
    handle: Handle<Image>,
}

fn print(query: Query<(Option<&Name>, &Div, &Children), Changed<Div>>) {
    for (name, div, children) in &query {
        println!("Changed {name:?} {div:?} {children:?}");
    }
}
