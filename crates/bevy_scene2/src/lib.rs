mod scene;
mod schematic;
mod spawn;

pub use crate::scene::*;
pub use crate::schematic::*;
pub use crate::spawn::*;

pub use bevy_scene_macros::bsn;

use bevy_app::{App, Plugin, Update};
use bevy_asset::AssetApp;

#[derive(Default)]
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneSpawner>()
            .init_asset::<Scene>()
            .init_asset::<Bsn>()
            .init_asset_loader::<SceneLoader>()
            .add_systems(Update, SceneSpawner::spawn_scenes);
    }
}
