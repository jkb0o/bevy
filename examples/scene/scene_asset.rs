//! This example illustrates loading scenes from files.
use bevy::prelude::*;
use bevy::{scene2::Scene, ui::scene2::Div};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin::default().watch_for_changes()))
        .add_systems(Startup, spawn_scene)
        .add_systems(Update, print)
        .run();
}

fn spawn_scene(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(assets.load::<Scene>("scenes/test.bsn"));
}

fn print(query: Query<(Option<&Name>, &Div, &Children), Changed<Div>>) {
    for (name, div, children) in &query {
        println!("Changed {name:?} {div:?} {children:?}");
    }
}
