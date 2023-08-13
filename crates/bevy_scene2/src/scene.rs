use crate::{schematic::ReflectSchematic, Props, Schematic, SchematicContext, SchematicError};
use bevy_asset::{Asset, AssetLoader, AsyncReadExt, Handle, LoadContext, LoadDirectError};
use bevy_bsn::{BsnEntity, BsnEntityConfig, BsnScene, FromBsnError};
use bevy_core::Name;
use bevy_ecs::{
    reflect::AppTypeRegistry,
    world::{FromWorld, World},
};
use bevy_reflect::{Reflect, TypeInfo, TypePath, TypeRegistry, TypeRegistryArc};
use bevy_utils::{BoxedFuture, Entry, HashMap};
use std::any::TypeId;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SceneFromBsnError {
    #[error("The type for type path {type_path} is not registered.")]
    UnregisteredTypePath { type_path: String },
    #[error(
        "The type for type path {type_path} does not have a Schematic implementation registered."
    )]
    MissingSchematicRegistrationForValue { type_path: String },
    #[error("'{type_path}' should be a struct with named fields.")]
    ExpectedNamedFieldStruct { type_path: String },
    #[error("'{type_path}' does not have field '{field}'.")]
    UnknownFieldInStruct { type_path: String, field: String },
    #[error("'{type_path}' is a tuple struct that requires a value at index '{index}'.")]
    ExpectedTupleStructIndex { type_path: String, index: usize },
    #[error("'{type_path}' should be a tuple struct.")]
    ExpectedTupleStruct { type_path: String },
    #[error(transparent)]
    FromBsnError(#[from] FromBsnError),
    #[error("Failed to load scene file: {0}")]
    LoadSceneError(#[from] LoadDirectError),
}

#[derive(Asset, TypePath, Debug)]
pub struct Scene {
    pub root: SceneEntity,
}

impl Scene {
    pub async fn from_bsn<'a, 'b>(
        context: &'a mut LoadContext<'b>,
        registry: &'a TypeRegistry,
        bsn: BsnScene<'a>,
    ) -> Result<Self, SceneFromBsnError> {
        Ok(Scene {
            root: SceneEntity::from_bsn(context, registry, bsn.root).await?,
        })
    }

    pub fn apply(&self, context: &mut SchematicContext) -> Result<(), SchematicError> {
        self.root.apply(context)
    }
}

#[derive(Debug)]
pub struct SceneEntity {
    pub name: Option<String>,
    pub schematics: HashMap<TypeId, SceneSchematic>,
    pub children: Vec<SceneEntity>,
}

impl SceneEntity {
    pub fn from_bsn<'a>(
        context: &'a mut LoadContext,
        registry: &'a TypeRegistry,
        bsn: BsnEntity<'a>,
    ) -> BoxedFuture<'a, Result<Self, SceneFromBsnError>> {
        Box::pin(async move {
            let mut children = Vec::with_capacity(bsn.children.len());
            let mut schematics = HashMap::new();

            let mut name = bsn.name.map(|n| n.to_string());

            fn apply_or_insert_schematic(
                registry: &TypeRegistry,
                resolved: &mut HashMap<TypeId, SceneSchematic>,
                schematic: SceneSchematic,
            ) {
                match resolved.entry(schematic.type_info.type_id()) {
                    Entry::Occupied(mut entry) => {
                        // We already have an entry for the schematic, therefore the registry and schematic entry must exist.
                        let registration = registry.get(*entry.key()).unwrap();
                        let reflect_schematic = registration.data::<ReflectSchematic>().unwrap();
                        (reflect_schematic.apply_props)(
                            &mut *entry.get_mut().props,
                            &*schematic.props,
                        );
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(schematic);
                    }
                }
            }

            for bsn_config in bsn.configs {
                let unresolved =
                    UnresolvedEntityConfig::from_bsn(context, registry, bsn_config).await?;
                // TODO: is there some way to share this code with "runtime" resolution?
                match unresolved {
                    UnresolvedEntityConfig::Schematic(schematic) => {
                        apply_or_insert_schematic(registry, &mut schematics, schematic);
                    }
                    UnresolvedEntityConfig::Scene(mut scene) => {
                        if name.is_none() {
                            name = scene.root.name.clone();
                        }
                        for (_, schematic) in scene.root.schematics.drain() {
                            apply_or_insert_schematic(registry, &mut schematics, schematic);
                        }

                        children.append(&mut scene.root.children);
                    }
                }
            }

            for bsn_child in bsn.children {
                children.push(SceneEntity::from_bsn(context, registry, bsn_child).await?);
            }
            Ok(SceneEntity {
                name,
                schematics,
                children,
            })
        })
    }

    pub fn apply_to_props<S: Schematic + 'static, P: Props + Reflect>(
        &self,
        props: &mut P,
    ) -> bool {
        if let Some(scene_schematic) = self.schematics.get(&TypeId::of::<S>()) {
            let scene_props = scene_schematic.props.downcast_ref::<P>().unwrap();
            props.apply_props(&scene_props);
            true
        } else {
            false
        }
    }

    fn apply(&self, context: &mut SchematicContext) -> Result<(), SchematicError> {
        if let Some(name) = &self.name {
            context.entity.get()?.insert(Name::new(name.clone()));
        }
        // TODO: spawn children
        for child_scene_entity in &self.children {
            context.spawn_child(|mut context| child_scene_entity.apply(&mut context))?;
        }
        // TODO: handle error
        for schematic in self.schematics.values() {
            // TODO: handle error
            let registration = context
                .type_registry
                .get(schematic.type_info.type_id())
                .unwrap();
            let reflect_schematic = registration.data::<ReflectSchematic>().unwrap();
            (reflect_schematic.insert_from_props)(&*schematic.props, context)?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct SceneSchematic {
    pub type_info: &'static TypeInfo,
    pub props: Box<dyn Reflect>,
}

#[derive(Debug)]
pub enum EntityConfig {
    Schematics(HashMap<TypeId, SceneSchematic>),
    SceneHandle(Handle<Scene>),
}

#[derive(Debug)]
pub enum UnresolvedEntityConfig {
    Schematic(SceneSchematic),
    Scene(Scene),
}

impl UnresolvedEntityConfig {
    pub async fn from_bsn<'a, 'b>(
        context: &'a mut LoadContext<'b>,
        registry: &'a TypeRegistry,
        entity_config: BsnEntityConfig<'a>,
    ) -> Result<Self, SceneFromBsnError> {
        match entity_config {
            BsnEntityConfig::Schematic {
                type_path,
                schematic_type,
            } => {
                let registration = registry.get_with_short_name(type_path).ok_or(
                    SceneFromBsnError::UnregisteredTypePath {
                        type_path: type_path.to_string(),
                    },
                )?;
                let reflect_schematic =
                    registration.data::<ReflectSchematic>().ok_or_else(|| {
                        SceneFromBsnError::MissingSchematicRegistrationForValue {
                            type_path: type_path.to_string(),
                        }
                    })?;
                let props = (reflect_schematic.props_from_bsn)(schematic_type.into())?;
                Ok(UnresolvedEntityConfig::Schematic(SceneSchematic {
                    props,
                    type_info: registration.type_info(),
                }))
            }
            BsnEntityConfig::Scene { path } => {
                let result = context.load_direct(path).await?;
                let scene = result.take::<Scene>().unwrap();
                Ok(UnresolvedEntityConfig::Scene(scene))
            }
        }
    }
}

pub struct SceneLoader {
    type_registry: TypeRegistryArc,
}

impl FromWorld for SceneLoader {
    fn from_world(world: &mut World) -> Self {
        let type_registry = world.resource::<AppTypeRegistry>();
        SceneLoader {
            type_registry: type_registry.0.clone(),
        }
    }
}

impl AssetLoader for SceneLoader {
    type Asset = Scene;

    type Settings = ();

    fn load<'a>(
        &'a self,
        reader: &'a mut bevy_asset::io::Reader,
        _settings: &'a Self::Settings,
        load_context: &'a mut bevy_asset::LoadContext,
    ) -> bevy_utils::BoxedFuture<'a, Result<Self::Asset, anyhow::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let str = String::from_utf8(bytes)?;
            let bsn = BsnScene::parse_str(&str)?;
            println!("{:#?}", bsn);
            let scene = Scene::from_bsn(load_context, &self.type_registry.read(), bsn).await?;
            println!("{:#?}", scene);
            Ok(scene)
        })
    }

    fn extensions(&self) -> &[&str] {
        &["bsn"]
    }
}
