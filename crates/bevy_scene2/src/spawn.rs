use crate::{EntityContext, Scene, SchematicContext, SchematicError};
use bevy_asset::{Asset, AssetEvent, AssetId, AssetServer, Assets, Handle};
use bevy_ecs::{
    event::ManualEventReader,
    prelude::{Entity, Events},
    query::{Added, QueryState},
    reflect::AppTypeRegistry,
    system::{In, Local, Res, ResMut, Resource},
    world::{FromWorld, Mut, World},
};
use bevy_log::error;
use bevy_reflect::{TypePath, TypeRegistry};
use bevy_utils::{HashMap, HashSet};

pub struct SpawnScene {
    pub entity: Entity,
    pub handle: Handle<Scene>,
}

#[derive(Asset, TypePath)]
pub struct Bsn {
    func: Box<
        dyn Fn(&mut SchematicContext, &[Handle<Scene>]) -> Result<(), SchematicError> + Send + Sync,
    >,
    scenes_loaded: bool,
    scene_paths: Vec<String>,
    #[dependency]
    scenes: Vec<Handle<Scene>>,
}

impl Bsn {
    pub fn new(
        func: Box<
            dyn Fn(&mut SchematicContext, &[Handle<Scene>]) -> Result<(), SchematicError>
                + Send
                + Sync,
        >,
        scene_paths: Vec<String>,
    ) -> Self {
        Self {
            scenes: Vec::with_capacity(scene_paths.len()),
            scenes_loaded: false,
            func,
            scene_paths,
        }
    }

    #[inline]
    pub fn scene_paths(&self) -> &[String] {
        &self.scene_paths
    }

    pub fn apply(&self, context: &mut SchematicContext) -> Result<(), SchematicError> {
        (self.func)(context, &self.scenes)
    }
    /// Loads scenes from `scene_paths`. This _must_ be called before registering [`Bsn`] with the
    /// asset system. Calling a second time will do nothing.
    pub fn load_scenes(&mut self, assets: &AssetServer) {
        if self.scenes_loaded {
            return;
        }
        for scene in &self.scene_paths {
            self.scenes.push(assets.load(scene));
        }
        self.scenes_loaded = true;
    }
}

#[derive(Resource)]
pub struct SceneSpawner {
    waiting: HashMap<AssetId<Scene>, Vec<SpawnScene>>,
    // PERF: disable this tracking when not in dev mode to reduce work
    spawned: HashMap<AssetId<Scene>, HashSet<Entity>>,
    watching_for_changes: bool,
    queued: Vec<SpawnScene>,
    // TODO: Make this a component
    queued_commands: HashSet<Handle<Bsn>>,
}

impl FromWorld for SceneSpawner {
    fn from_world(world: &mut World) -> Self {
        Self {
            watching_for_changes: world.resource::<AssetServer>().is_watching_for_changes(),
            queued_commands: Default::default(),
            waiting: Default::default(),
            spawned: Default::default(),
            queued: Default::default(),
        }
    }
}

impl SceneSpawner {
    pub fn queue(&mut self, spawn_scene: SpawnScene) {
        self.queued.push(spawn_scene);
    }
    pub fn queue_command(&mut self, command: Handle<Bsn>) {
        self.queued_commands.insert(command);
    }
    pub fn spawn(
        &mut self,
        world: &mut World,
        assets: &AssetServer,
        type_registry: &TypeRegistry,
        scenes: &Assets<Scene>,
        scene: &Scene,
        scene_id: AssetId<Scene>,
        root: Entity,
    ) -> Result<(), SchematicError> {
        let mut context = SchematicContext {
            assets,
            scenes,
            type_registry,
            entity: EntityContext::new(world, root),
        };
        scene.apply(&mut context)?;
        if self.watching_for_changes {
            let entities = self.spawned.entry(scene_id).or_default();
            entities.insert(root);
        }
        Ok(())
    }

    pub fn spawn_scenes(
        world: &mut World,
        mut scene_event_reader: Local<ManualEventReader<AssetEvent<Scene>>>,
        mut command_event_reader: Local<ManualEventReader<AssetEvent<Bsn>>>,
        added: &mut QueryState<(Entity, &Handle<Scene>), Added<Handle<Scene>>>,
    ) {
        // NOTE: This nesting is an affront to everything beautiful
        world.resource_scope(|world, mut spawner: Mut<SceneSpawner>| {
            world.resource_scope(|world, scenes: Mut<Assets<Scene>>| {
                world.resource_scope(|world, scene_events: Mut<Events<AssetEvent<Scene>>>| {
                    world.resource_scope(|world, mut spawn_commands: Mut<Assets<Bsn>>| {
                        world.resource_scope(
                            |world, mut spawn_command_events: Mut<Events<AssetEvent<Bsn>>>| {
                                world.resource_scope(
                                    |world, type_registry: Mut<AppTypeRegistry>| {
                                        world.resource_scope(|world, assets: Mut<AssetServer>| {
                                            spawner.spawn_scenes_internal(
                                                world,
                                                &assets,
                                                &scenes,
                                                &scene_events,
                                                &mut scene_event_reader,
                                                &type_registry.0.read(),
                                                added,
                                            );
                                            spawner.spawn_commands_internal(
                                                world,
                                                &assets,
                                                &scenes,
                                                &mut spawn_commands,
                                                &mut spawn_command_events,
                                                &mut command_event_reader,
                                                &type_registry.0.read(),
                                            );
                                        });
                                    },
                                );
                            },
                        );
                    });
                });
            });
        });
    }

    fn spawn_scenes_internal(
        &mut self,
        world: &mut World,
        assets: &AssetServer,
        scenes: &Assets<Scene>,
        events: &Events<AssetEvent<Scene>>,
        event_reader: &mut ManualEventReader<AssetEvent<Scene>>,
        registry: &TypeRegistry,
        added: &mut QueryState<(Entity, &Handle<Scene>), Added<Handle<Scene>>>,
    ) {
        // PERF: can we avoid this vec allocation?
        for (entity, handle) in added
            .iter(world)
            .map(|(e, h)| (e, h.clone()))
            .collect::<Vec<_>>()
        {
            if let Some(scene) = scenes.get(handle.id()) {
                if let Err(err) =
                    self.spawn(world, assets, registry, scenes, scene, handle.id(), entity)
                {
                    error!("{err}");
                }
            } else {
                let spawns = self.waiting.entry(handle.id()).or_default();
                spawns.push(SpawnScene { entity, handle });
            }
        }

        for event in event_reader.iter(&events) {
            match event {
                AssetEvent::LoadedWithDependencies { id } => {
                    let Some(scene) = scenes.get(*id) else { continue };
                    let Some(waiting) = self.waiting.remove(id) else {
                        continue
                    };
                    for spawn_scene in waiting {
                        if let Err(err) = self.spawn(
                            world,
                            assets,
                            registry,
                            scenes,
                            scene,
                            *id,
                            spawn_scene.entity,
                        ) {
                            error!("{err}");
                        }
                    }
                }
                AssetEvent::Modified { id } => {
                    if self.watching_for_changes {
                        let Some(scene) = scenes.get(*id) else { continue };
                        // TODO: removing here is to work around ownership issues ... not ideal?
                        let Some(mut entities) = self.spawned.remove(id) else { continue };
                        entities.retain(|e| {
                            let result =
                                self.spawn(world, assets, registry, scenes, scene, *id, *e);
                            match result {
                                Ok(_) => true,
                                Err(SchematicError::MissingEntity(_)) => false,
                                Err(err) => {
                                    error!("{err}");
                                    true
                                }
                            }
                        });
                        self.spawned
                            .entry(*id)
                            .or_default()
                            .extend(entities.drain());
                    }
                }
                _ => {}
            }
        }
    }

    fn spawn_commands_internal(
        &mut self,
        world: &mut World,
        assets: &AssetServer,
        scenes: &Assets<Scene>,
        spawn_commands: &mut Assets<Bsn>,
        events: &Events<AssetEvent<Bsn>>,
        event_reader: &mut ManualEventReader<AssetEvent<Bsn>>,
        type_registry: &TypeRegistry,
    ) {
        for event in event_reader.iter(&events) {
            if let AssetEvent::LoadedWithDependencies { id } = event {
                let Some(bsn_command) = spawn_commands.remove(*id) else { continue };
                let mut context = SchematicContext {
                    assets,
                    scenes,
                    type_registry,
                    entity: EntityContext::spawn(world),
                };
                if let Err(err) = bsn_command.apply(&mut context) {
                    error!("{err}");
                }
            }
        }
    }
}

/// System adapter that will spawn an entity from a function that produces a SpawnCommand
pub fn spawn_bsn(
    In(mut bsn): In<Bsn>,
    mut spawner: ResMut<SceneSpawner>,
    assets: Res<AssetServer>,
) {
    bsn.load_scenes(&assets);
    spawner.queue_command(assets.add(bsn));
}
