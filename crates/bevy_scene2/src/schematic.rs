pub use bevy_scene_macros::Schematic;

use crate::Scene;
use bevy_asset::{Asset, AssetId, AssetPath, AssetServer, Assets, Handle};
use bevy_bsn::{BsnValue, FromBsn, FromBsnError};
use bevy_ecs::{
    prelude::{Bundle, Entity},
    world::{EntityMut, World},
};
use bevy_hierarchy::BuildWorldChildren;
use bevy_math::{Quat, Vec2, Vec3};
use bevy_reflect::{FromType, Reflect, TypeRegistry};
use smallvec::SmallVec;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SchematicError {
    #[error("Scene {asset_id:?}(Path: {asset_path:?}) does not exist")]
    MissingScene {
        asset_path: Option<AssetPath<'static>>,
        asset_id: AssetId<Scene>,
    },
    #[error("Entity {0:?} does not exist.")]
    MissingEntity(Entity),
    #[error("Resource {type_name} does not exist")]
    MissingResource { type_name: &'static str },
}

pub trait Schematic: Sized {
    type Props: Props;
    fn from_props(
        props: Self::Props,
        context: &mut SchematicContext,
    ) -> Result<Self, SchematicError>;
}

#[derive(Clone, Reflect, Debug)]
// Perf: if we can get away with Option<T> here we can probably reduce generated code in some cases
pub enum Prop<T: Props> {
    Unset,
    Value(T),
}

impl<T: Props> Prop<T> {
    pub fn get(self) -> T {
        match self {
            Prop::Unset => T::default(),
            Prop::Value(value) => value,
        }
    }

    pub fn apply(&mut self, other: &Prop<T>) {
        match self {
            Prop::Unset => *self = other.clone(),
            Prop::Value(this) => match other {
                Prop::Unset => {}
                Prop::Value(other) => this.apply_props(other),
            },
        }
    }
}

impl<T: Props> Default for Prop<T> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<T: Props> From<T> for Prop<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}
pub trait Props: Clone + FromBsn + Default {
    fn apply_props(&mut self, other: &Self);
}

pub struct SchematicContext<'a> {
    pub type_registry: &'a TypeRegistry,
    pub scenes: &'a Assets<Scene>,
    pub assets: &'a AssetServer,
    pub entity: EntityContext<'a>,
}

pub struct EntityContext<'a> {
    pub world: &'a mut World,
    pub(crate) id: Entity,
    pub(crate) children: SmallVec<[Entity; 8]>,
}

impl<'a> EntityContext<'a> {
    pub fn new(world: &'a mut World, id: Entity) -> Self {
        Self {
            id,
            world,
            children: SmallVec::new(),
        }
    }

    pub fn spawn(world: &'a mut World) -> Self {
        let id = world.spawn_empty().id();
        Self::new(world, id)
    }

    pub fn get(&mut self) -> Result<EntityMut, SchematicError> {
        self.world
            .get_entity_mut(self.id)
            .ok_or_else(|| SchematicError::MissingEntity(self.id))
    }
}

impl<'a> SchematicContext<'a> {
    pub fn spawn_child(
        &mut self,
        func: impl FnOnce(&mut SchematicContext) -> Result<(), SchematicError>,
    ) -> Result<(), SchematicError> {
        let mut child_context = SchematicContext {
            assets: self.assets,
            scenes: self.scenes,
            type_registry: self.type_registry,
            entity: EntityContext::spawn(self.entity.world),
        };
        self.entity.children.push(child_context.entity.id);
        func(&mut child_context)
    }
}

impl<'a> Drop for EntityContext<'a> {
    fn drop(&mut self) {
        self.world.entity_mut(self.id).push_children(&self.children);
    }
}

#[derive(Clone)]
pub struct ReflectSchematic {
    pub props_from_bsn: fn(bsn: BsnValue) -> Result<Box<dyn Reflect>, FromBsnError>,
    pub from_props: fn(
        props: &dyn Reflect,
        world: &mut SchematicContext,
    ) -> Result<Box<dyn Reflect>, SchematicError>,
    pub insert_from_props:
        fn(props: &dyn Reflect, world: &mut SchematicContext) -> Result<(), SchematicError>,
    pub apply_props: fn(a: &mut dyn Reflect, b: &dyn Reflect),
}

impl<S: Schematic> FromType<S> for ReflectSchematic
where
    S: Reflect + Bundle,
    S::Props: Reflect,
{
    fn from_type() -> Self {
        Self {
            props_from_bsn: |bsn| Ok(Box::new(<S::Props as FromBsn>::from_bsn(bsn)?)),
            from_props: |props, context| {
                let props = props.downcast_ref::<S::Props>().unwrap();
                Ok(Box::new(S::from_props(props.clone(), context)?))
            },
            insert_from_props: |props, context| {
                let props = props.downcast_ref::<S::Props>().unwrap();
                let bundle = S::from_props(props.clone(), context)?;
                context.entity.get()?.insert(bundle);
                Ok(())
            },
            apply_props: |a, b| {
                let a_props = a.downcast_mut::<S::Props>().unwrap();
                let b_props = b.downcast_ref::<S::Props>().unwrap();
                <S::Props as Props>::apply_props(a_props, b_props);
            },
        }
    }
}

macro_rules! impl_props_copy {
    ($ty:ty) => {
        impl Props for $ty {
            fn apply_props(&mut self, parent: &Self) {
                *self = *parent;
            }
        }
    };
}

macro_rules! impl_props_clone {
    ($ty:ty) => {
        impl Props for $ty {
            fn apply_props(&mut self, parent: &Self) {
                *self = parent.clone();
            }
        }
    };
}

impl_props_copy!(u8);
impl_props_copy!(u16);
impl_props_copy!(u32);
impl_props_copy!(u64);
impl_props_copy!(u128);
impl_props_copy!(Vec2);
impl_props_copy!(Vec3);
impl_props_copy!(Quat);
impl_props_clone!(String);

impl Props for () {
    fn apply_props(&mut self, _other: &Self) {}
}

impl Props for AssetPath<'static> {
    fn apply_props(&mut self, other: &Self) {
        *self = other.clone()
    }
}

impl<T: Asset> Schematic for Handle<T> {
    type Props = AssetPath<'static>;

    fn from_props(
        asset_path: Self::Props,
        context: &mut SchematicContext,
    ) -> Result<Self, SchematicError> {
        Ok(context.assets.load(asset_path))
    }
}
