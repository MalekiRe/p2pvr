use avian3d::prelude::*;
use avian3d::prelude::{Collider, RigidBody};
use avian3d::PhysicsPlugins;
use avian_interpolation3d::{
    AvianInterpolationPlugin, InterpolateTransformFields, InterpolationMode,
};
use avian_pickup::prelude::{AvianPickupAction, AvianPickupActor, AvianPickupInput};
use avian_pickup::AvianPickupPlugin;
use bevy::app::RunFixedMainLoop;
use bevy::ecs::query::QuerySingleError;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseMotion;
use bevy::render::camera::camera_system;
use bevy::render::view::RenderLayers;
use bevy::time::run_fixed_main_schedule;
use bevy::{prelude::*, time::common_conditions::on_timer, utils::Duration};
use bevy_basic_portals::{AsPortalDestination, CreatePortal, CreatePortalBundle, PortalsPlugin};
use bevy_matchbox::prelude::*;
use bevy_tnua_physics_integration_layer::data_for_backends::TnuaProximitySensor;
use bevy_vrm::first_person::{FirstPersonFlag, RENDER_LAYERS};
use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_PI_2;
use bevy_vrm::VrmBundle;
use unavi_avatar::{AvatarBundle, AverageVelocity, default_character_animations, DEFAULT_VRM, FallbackAvatar};
use unavi_player::layers::{LAYER_LOCAL_PLAYER, LAYER_OTHER_PLAYER, LAYER_PROPS};
use unavi_avatar::{PLAYER_HEIGHT, PLAYER_WIDTH};
use unavi_player::{LocalPlayer, PlayerCamera, PlayerPlugin};
use uuid::{uuid, Uuid};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PhysicsPlugins::default(),
            PlayerPlugin,
            AvianPickupPlugin::default(),
            // Add interpolation
            AvianInterpolationPlugin::default(),
            PortalsPlugin::default(),
        ))
        .add_systems(Startup, setup_scene)
        .add_systems(Update, player_add_pickup)
        .add_systems(Update, (remove_thing, add_uuid))
        .add_systems(FixedPreUpdate, (handle_input).before(run_fixed_main_schedule))
        .add_systems(Startup, start_socket)
        .add_systems(Update, receive_messages)
        .add_systems(
            Update,
            send_message,
        )
        .run();
}

fn handle_input(
    mut avian_pickup_input_writer: EventWriter<AvianPickupInput>,
    key_input: Res<ButtonInput<MouseButton>>,
    actors: Query<Entity, With<AvianPickupActor>>,
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
}

fn rotate_camera(
    mut mouse_motion: EventReader<MouseMotion>,
    mut cameras: Query<&mut Transform, With<Camera>>,
    mut players: Query<(&mut Transform, &Children), (With<LocalPlayer>, Without<Camera>)>,
) {
    for (mut player_tr, children) in players.iter_mut() {}
    for mut transform in &mut cameras {
        let mouse_sensitivity = Vec2::new(0.003, 0.002);

        for motion in mouse_motion.read() {
            let delta_yaw = -motion.delta.x * mouse_sensitivity.x;
            let delta_pitch = -motion.delta.y * mouse_sensitivity.y;

            const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;
            let (yaw, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
            let yaw = yaw + delta_yaw;
            let pitch = (pitch + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
        }
    }
}

const GROUND_SIZE: f32 = 15.0;
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
            }
        ));
    }
}

fn remove_thing(
    player: Query<Entity, (With<LocalPlayer>, Without<InterpolateTransformFields>)>,
    mut commands: Commands,
) {
    for awa in player.iter() {
        /*commands.entity(awa)
        .insert(InterpolateTransformFields {
            translation: InterpolationMode::Last,
            rotation: InterpolationMode::Last,
        });*/
    }
}

fn add_uuid(
    mut commands: Commands,
    local_player: Query<Entity, (With<LocalPlayer>, Without<PlayerUuid>)>
) {
    for e in local_player.iter() {
        commands.entity(e).insert(PlayerUuid(Uuid::new_v4().to_string()));
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

fn setup_scene(
    mut ambient: ResMut<AmbientLight>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    ambient.brightness = 100.0;
    ambient.color = Color::linear_rgb(0.95, 0.95, 1.0);

    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_xyz(4.5, 10.0, -7.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    let cube_material = materials.add(Color::linear_rgb(0.0, 1.0, 0.0));

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
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::new(
                GROUND_SIZE,
                GROUND_THICK,
                GROUND_SIZE,
            ))),
            material: materials.add(StandardMaterial::default()),
            transform: Transform::from_xyz(0.0, -1.0 - GROUND_THICK / 2.0, 0.0),
            ..default()
        },
        RigidBody::Static,
        Collider::cuboid(GROUND_SIZE, GROUND_THICK, GROUND_SIZE),
    ));

    commands.spawn(CreatePortalBundle {
        mesh: meshes.add(Mesh::from(Rectangle::new(GROUND_SIZE, MIRROR_H))),
        create_portal: CreatePortal {
            destination: AsPortalDestination::CreateMirror,
            render_layer: RenderLayers::layer(0)
                .union(&RENDER_LAYERS[&FirstPersonFlag::ThirdPersonOnly]),
            ..default()
        },
        portal_transform: Transform::from_xyz(0.0, -1.0 + MIRROR_H / 2.0, -GROUND_SIZE / 2.0),
        ..default()
    });

    let mut transform = Transform::from_xyz(0.0, 3.0, -10.0);
    transform.look_at(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 1.0, 0.0));
}

fn start_socket(mut commands: Commands) {
    let socket = MatchboxSocket::new_reliable("ws://localhost:3536/hello");
    commands.insert_resource(socket);
}

#[derive(Component)]
pub struct PlayerUuid(pub String);

fn send_message(
    mut socket: ResMut<MatchboxSocket<SingleChannel>>,
    local_player: Query<(&Position, &Rotation, &LinearVelocity, &PlayerUuid), With<LocalPlayer>>,
) {
    let (position, rotation, linear_velocity, uuid) = match local_player.get_single() {
        Ok(val) => val,
        Err(err) => {
            println!("returning: {}", err);
            return;
        }
    };

    let peers: Vec<_> = socket.connected_peers().collect();

    for peer in peers {
        let msg = Message::Position(uuid.0.clone(), position.clone(), rotation.clone(), linear_velocity.clone());
        let msg = serde_json::to_string(&msg).unwrap();

        //info!("Sending message: {msg:?} to {peer}");
        socket.send(msg.as_bytes().into(), peer);
    }
}

fn receive_messages(mut commands: Commands, mut socket: ResMut<MatchboxSocket<SingleChannel>>, mut query: Query<(&mut Position, &mut Rotation, &mut LinearVelocity, &PlayerUuid)>, asset_server: Res<AssetServer>) {
    for (peer, state) in socket.update_peers() {
        //info!("{peer}: {state:?}");
    }

    for (_id, message) in socket.receive() {
        match std::str::from_utf8(&message) {
            Ok(message) => {
                let message = serde_json::from_str::<Message>(message).unwrap();
                match message {
                    Message::Position(uuid, position, rotation, linear_velocity) => {
                        let mut contains = false;
                        for (mut p, mut r, mut l, u) in query.iter_mut() {
                            if u.0 != uuid {
                                continue;
                            }
                            contains = true;
                            *p = position;
                            *r = rotation;
                            *l = linear_velocity;
                        }
                        if !contains {
                            spawn_other_player(&asset_server, &mut commands, PlayerUuid(uuid));
                            return;
                        }
                    }
                }
            },
            Err(e) => error!("Failed to convert message to string: {e}"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum Message {
    Position(String, Position, Rotation, LinearVelocity),
}
pub const SPAWN: Vec3 = Vec3::new(0.0, PLAYER_HEIGHT * 2.0, 0.0);

pub fn spawn_other_player(asset_server: &AssetServer, commands: &mut Commands, uuid: PlayerUuid) {
    let animations = default_character_animations(&asset_server);

    let body = commands
        .spawn((
            Collider::capsule(PLAYER_WIDTH / 2.0, PLAYER_HEIGHT - PLAYER_WIDTH),
            CollisionLayers {
                memberships: LAYER_OTHER_PLAYER,
                ..default()
            },
            RigidBody::Dynamic,
            LockedAxes::ROTATION_LOCKED,
            SpatialBundle {
                global_transform: GlobalTransform::from_translation(SPAWN),
                ..default()
            },
            uuid
        ))
        .id();

    let avatar = commands
        .spawn((
            AvatarBundle {
                animations,
                fallback: FallbackAvatar,
                velocity: AverageVelocity {
                    target: Some(body),
                    ..default()
                },
            },
            VrmBundle {
                scene_bundle: SceneBundle {
                    transform: Transform::from_xyz(0.0, -PLAYER_HEIGHT / 2.0, 0.0),
                    ..default()
                },
                vrm: asset_server.load(DEFAULT_VRM),
                ..default()
            },
        ))
        .id();



    commands.entity(body).push_children(&[avatar]);
}