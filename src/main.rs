mod custom_audio;
mod file_sharing;
mod networking;
mod voice_chat;

use crate::custom_audio::audio_output::AudioOutputPlugin;
use crate::custom_audio::microphone::MicrophonePlugin;
use crate::custom_audio::spatial_audio::{SpatialAudioListener, SpatialAudioPlugin};
use crate::file_sharing::FileSharingPlugin;
use crate::networking::message::SpawnCube;
use crate::networking::{
    Authority, Message, NetworkingPlugin, PlayerUuid, PropUuid, SocketSendMessage,
};
use crate::voice_chat::VoiceChatPlugin;
use avian3d::prelude::*;
use avian3d::prelude::{Collider, RigidBody};
use avian3d::PhysicsPlugins;
use avian_interpolation3d::{
    AvianInterpolationPlugin, InterpolateTransformFields, InterpolationMode,
};
use avian_pickup::actor::AvianPickupActorState;
use avian_pickup::prelude::{AvianPickupAction, AvianPickupActor, AvianPickupInput};
use avian_pickup::AvianPickupPlugin;
use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::time::run_fixed_main_schedule;
use bevy_embedded_assets::EmbeddedAssetPlugin;
use bevy_matchbox::prelude::*;
use bevy_tnua_physics_integration_layer::data_for_backends::TnuaProximitySensor;
use serde::{Deserialize, Serialize};
use unavi_avatar::PLAYER_HEIGHT;
use unavi_player::layers::LAYER_PROPS;
use unavi_player::{LocalPlayer, PlayerCamera, PlayerPlugin};
use uuid::Uuid;

fn main() {
    App::new()
        .add_plugins((
            EmbeddedAssetPlugin::default(),
            bevy_web_file_drop::WebFileDropPlugin,
            DefaultPlugins.set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..AssetPlugin::default()
            }),
            PhysicsPlugins::default(),
            PlayerPlugin,
            AvianPickupPlugin::default(),
            // Add interpolation
            AvianInterpolationPlugin::default(),
        ))
        .add_plugins((
            NetworkingPlugin,
            AudioOutputPlugin,
            MicrophonePlugin,
            VoiceChatPlugin,
            SpatialAudioPlugin,
            FileSharingPlugin,
        ))
        .add_systems(Startup, setup_scene)
        .add_systems(Update, player_add_pickup)
        .add_systems(Update, add_uuid)
        .add_systems(Update, handle_spawn_cube)
        .add_systems(
            FixedPreUpdate,
            (handle_input).before(run_fixed_main_schedule),
        )
        .add_systems(Update, update_prop_authority)
        .add_systems(Startup, start_socket)
        .run();
}

#[derive(Component)]
pub struct LocalProp;

fn update_prop_authority(
    actors: Query<(Entity, &AvianPickupActorState)>,
    mut prop: Query<&mut Authority, Without<LocalProp>>,
    changed_prop: Query<(Entity, &Authority), (Changed<Authority>, With<LocalProp>)>,
    uuid: Query<&PlayerUuid, With<LocalPlayer>>,
    mut commands: Commands,
    mut avian_pickup_input_writer: EventWriter<AvianPickupInput>,
) {
    let Ok(uuid) = uuid.get_single() else {
        return;
    };
    for (actor_e, actor) in actors.iter() {
        match actor {
            AvianPickupActorState::Idle => {}
            AvianPickupActorState::Pulling(e) | AvianPickupActorState::Holding(e) => {
                if let Ok((prop, authority)) = changed_prop.get(*e) {
                    if authority.player != uuid.clone() {
                        println!("no longer in charge of prop");
                        avian_pickup_input_writer.send(AvianPickupInput {
                            action: AvianPickupAction::Drop,
                            actor: actor_e,
                        });
                        commands.entity(actor_e).insert(AvianPickupActorState::Idle);
                        commands.entity(prop).remove::<LocalProp>();
                        return;
                    }
                }
                if let Ok(mut prop) = prop.get_mut(*e) {
                    if prop.player != uuid.clone() {
                        prop.counter += 1;
                        prop.player = uuid.clone();
                        commands.entity(*e).insert(LocalProp);
                    }
                }
            }
        }
    }
}

fn handle_input(
    mut avian_pickup_input_writer: EventWriter<AvianPickupInput>,
    key_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    actors: Query<Entity, With<AvianPickupActor>>,
    mut spawn_cube: EventWriter<SpawnCube>,
    local_player: Query<&PlayerUuid, With<LocalPlayer>>,
    mut socket: ResMut<MatchboxSocket<MultipleChannels>>,
) {
    for actor in &actors {
        if key_input.just_pressed(MouseButton::Left) {
            avian_pickup_input_writer.send(AvianPickupInput {
                action: AvianPickupAction::Throw,
                actor,
            });
        }
        if key_input.just_pressed(MouseButton::Right) {
            avian_pickup_input_writer.send(AvianPickupInput {
                action: AvianPickupAction::Drop,
                actor,
            });
        }
        if key_input.pressed(MouseButton::Right) {
            avian_pickup_input_writer.send(AvianPickupInput {
                action: AvianPickupAction::Pull,
                actor,
            });
        }
    }

    let Ok(local_player) = local_player.get_single() else {
        return;
    };

    if keyboard_input.just_pressed(KeyCode::KeyC) {
        let cube = SpawnCube {
            authority: Authority {
                player: local_player.clone(),
                counter: 0,
            },
            prop_uuid: PropUuid(Uuid::new_v4().to_string()),
            position: Position::new(Vec3::new(0.0, 2.0, 0.0)),
        };
        socket.send_msg_all_reliable(&Message::SpawnCube(cube.clone()));
        spawn_cube.send(cube.clone());
    }
}

const GROUND_SIZE: f32 = 30.0;
const GROUND_THICK: f32 = 0.2;
const MIRROR_H: f32 = 3.0;

fn player_add_pickup(
    mut player: Query<Entity, (With<PlayerCamera>, Without<AvianPickupActor>)>,
    mut commands: Commands,
) {
    for awa in player.iter() {
        commands.entity(awa).insert((
            AvianPickupActor {
                //actor_filter: SpatialQueryFilter::from_mask(LAYER_LOCAL_PLAYER),
                prop_filter: SpatialQueryFilter::from_mask(LAYER_PROPS),
                ..default()
            },
            InterpolateTransformFields {
                translation: InterpolationMode::Linear,
                rotation: InterpolationMode::Linear,
            },
        ));
    }
}

fn add_uuid(
    mut commands: Commands,
    local_player: Query<Entity, (With<LocalPlayer>, Without<PlayerUuid>)>,
) {
    for e in local_player.iter() {
        commands
            .entity(e)
            .insert(PlayerUuid(Uuid::new_v4().to_string()))
            .insert(SpatialAudioListener);
    }
}

fn other_thing(
    mut awa: Query<
        Entity,
        With<bevy_tnua_physics_integration_layer::data_for_backends::TnuaRigidBodyTracker>,
    >,
    mut owo: Query<Entity, With<TnuaProximitySensor>>,
    mut ewe: Query<Entity, With<TnuaProximitySensor>>,
    mut commands: Commands,
) {
    for awa in awa.iter() {
        commands.entity(awa).insert(
            (InterpolateTransformFields {
                translation: InterpolationMode::Last,
                rotation: InterpolationMode::Last,
            }),
        );
    }
    for awa in owo.iter() {
        commands.entity(awa).insert(
            (InterpolateTransformFields {
                translation: InterpolationMode::Last,
                rotation: InterpolationMode::Last,
            }),
        );
    }
    for awa in ewe.iter() {
        commands.entity(awa).insert(
            (InterpolateTransformFields {
                translation: InterpolationMode::Last,
                rotation: InterpolationMode::Last,
            }),
        );
    }
}

fn handle_spawn_cube(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut event_reader: EventReader<SpawnCube>,
) {
    for cube in event_reader.read() {
        let cube_material = materials.add(Color::linear_rgb(0.0, 1.0, 0.0));

        let box_shape = Cuboid::from_size(Vec3::splat(0.5));
        let box_mesh = meshes.add(box_shape);
        commands.spawn((
            Name::new("Light Box"),
            PbrBundle {
                mesh: box_mesh.clone(),
                material: cube_material.clone(),
                transform: Transform::from_xyz(cube.position.x, cube.position.y, cube.position.z),
                ..default()
            },
            cube.authority.clone(),
            cube.prop_uuid.clone(),
            // All `RigidBody::Dynamic` entities are able to be picked up.
            RigidBody::Dynamic,
            Collider::from(box_shape),
            CollisionLayers::new(LAYER_PROPS, LayerMask::ALL),
        ));
    }
}

fn setup_scene(
    mut ambient: ResMut<AmbientLight>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let floor_texture = asset_server.load("grass.ktx2");
    let floor_normal_texture = asset_server.load("grass_normal.ktx2");

    ambient.brightness = 100.0;
    ambient.color = Color::linear_rgb(0.95, 0.95, 1.0);

    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_xyz(4.5, 10.0, -7.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    let cube_material = materials.add(Color::linear_rgb(1.0, 0.0, 0.0));

    let box_shape = Cuboid::from_size(Vec3::splat(0.5));
    let box_mesh = meshes.add(box_shape);
    commands.spawn((
        Name::new("Light Box"),
        PbrBundle {
            mesh: box_mesh.clone(),
            material: cube_material.clone(),
            transform: Transform::from_xyz(0.0, 2.0, 3.5),
            ..default()
        },
        // All `RigidBody::Dynamic` entities are able to be picked up.
        RigidBody::Dynamic,
        Collider::from(box_shape),
        CollisionLayers::new(LAYER_PROPS, LayerMask::ALL),
    ));
    const FLOOR_TILING: i32 = 20;

    for x in -FLOOR_TILING..FLOOR_TILING {
        for y in -FLOOR_TILING..FLOOR_TILING {
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Mesh::from(Cuboid::new(
                        GROUND_SIZE / FLOOR_TILING as f32,
                        GROUND_THICK,
                        GROUND_SIZE / FLOOR_TILING as f32,
                    ))),
                    material: materials.add(StandardMaterial {
                        base_color_texture: Some(floor_texture.clone()),
                        normal_map_texture: Some(floor_normal_texture.clone()),
                        emissive_exposure_weight: 0.0,
                        perceptual_roughness: 1.0,
                        reflectance: 0.2,
                        specular_transmission: 0.0,
                        diffuse_transmission: 0.0,
                        thickness: 0.0,
                        ior: 1.0,
                        clearcoat: 0.0,
                        anisotropy_strength: 0.0,
                        lightmap_exposure: 1.0,
                        //parallax_depth_scale: 0.3,
                        ..default()
                    }),
                    transform: Transform::from_xyz(
                        (GROUND_SIZE / FLOOR_TILING as f32) * x as f32,
                        -1.0 - GROUND_THICK / 2.0,
                        (GROUND_SIZE / FLOOR_TILING as f32) * y as f32,
                    ),
                    ..default()
                },
                RigidBody::Static,
                Collider::cuboid(
                    GROUND_SIZE / FLOOR_TILING as f32,
                    GROUND_THICK,
                    GROUND_SIZE / FLOOR_TILING as f32,
                ),
            ));
        }
    }

    let mut transform = Transform::from_xyz(0.0, 3.0, -10.0);
    transform.look_at(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 1.0, 0.0));
}

fn start_socket(mut commands: Commands) {
    let matchbox = MatchboxSocket::from(
        WebRtcSocketBuilder::new("wss://mb.v-sekai.cloud/hello5")
            .add_reliable_channel()
            .add_unreliable_channel()
            .build(),
    );
    commands.insert_resource(matchbox);
}

pub const SPAWN: Vec3 = Vec3::new(0.0, PLAYER_HEIGHT * 2.0, 0.0);
