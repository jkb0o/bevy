#![allow(clippy::type_complexity)]

#[cfg(feature = "bevy_animation")]
use bevy_animation::AnimationClip;
use bevy_utils::HashMap;

mod loader;
mod vertex_attributes;
pub use loader::*;

use bevy_app::prelude::*;
use bevy_asset::{Asset, AssetApp, Handle};
use bevy_ecs::{prelude::Component, reflect::ReflectComponent, world::FromWorld};
use bevy_pbr::StandardMaterial;
use bevy_reflect::Reflect;
use bevy_render::mesh::{Mesh, MeshVertexAttribute};
use bevy_scene::Scene;

/// Adds support for glTF file loading to the app.
#[derive(Default)]
pub struct GltfPlugin {
    custom_vertex_attributes: HashMap<String, MeshVertexAttribute>,
}

impl GltfPlugin {
    pub fn add_custom_vertex_attribute(
        mut self,
        name: &str,
        attribute: MeshVertexAttribute,
    ) -> Self {
        self.custom_vertex_attributes
            .insert(name.to_string(), attribute);
        self
    }
}

impl Plugin for GltfPlugin {
    fn build(&self, app: &mut App) {
        let mut loader = GltfLoader::from_world(&mut app.world);
        loader.custom_vertex_attributes = self.custom_vertex_attributes.clone();
        app.register_asset_loader(loader)
            .register_type::<GltfExtras>()
            .init_asset::<Gltf>()
            .init_asset::<GltfNode>()
            .init_asset::<GltfPrimitive>()
            .init_asset::<GltfMesh>();
    }
}

/// Representation of a loaded glTF file.
#[derive(Asset, Debug)]
pub struct Gltf {
    pub scenes: Vec<Handle<Scene>>,
    pub named_scenes: HashMap<String, Handle<Scene>>,
    pub meshes: Vec<Handle<GltfMesh>>,
    pub named_meshes: HashMap<String, Handle<GltfMesh>>,
    pub materials: Vec<Handle<StandardMaterial>>,
    pub named_materials: HashMap<String, Handle<StandardMaterial>>,
    pub nodes: Vec<Handle<GltfNode>>,
    pub named_nodes: HashMap<String, Handle<GltfNode>>,
    pub default_scene: Option<Handle<Scene>>,
    #[cfg(feature = "bevy_animation")]
    pub animations: Vec<Handle<AnimationClip>>,
    #[cfg(feature = "bevy_animation")]
    pub named_animations: HashMap<String, Handle<AnimationClip>>,
}

/// A glTF node with all of its child nodes, its [`GltfMesh`],
/// [`Transform`](bevy_transform::prelude::Transform) and an optional [`GltfExtras`].
#[derive(Asset, Debug, Clone)]
pub struct GltfNode {
    pub children: Vec<GltfNode>,
    pub mesh: Option<Handle<GltfMesh>>,
    pub transform: bevy_transform::prelude::Transform,
    pub extras: Option<GltfExtras>,
}

/// A glTF mesh, which may consist of multiple [`GltfPrimitives`](GltfPrimitive)
/// and an optional [`GltfExtras`].
#[derive(Asset, Debug, Clone)]
pub struct GltfMesh {
    pub primitives: Vec<GltfPrimitive>,
    pub extras: Option<GltfExtras>,
}

/// Part of a [`GltfMesh`] that consists of a [`Mesh`], an optional [`StandardMaterial`] and [`GltfExtras`].
#[derive(Asset, Debug, Clone)]
pub struct GltfPrimitive {
    pub mesh: Handle<Mesh>,
    pub material: Option<Handle<StandardMaterial>>,
    pub extras: Option<GltfExtras>,
    pub material_extras: Option<GltfExtras>,
}

#[derive(Clone, Debug, Reflect, Default, Component)]
#[reflect(Component)]
pub struct GltfExtras {
    pub value: String,
}
