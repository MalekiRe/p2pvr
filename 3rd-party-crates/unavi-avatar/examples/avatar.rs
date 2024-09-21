use avian3d::{debug_render::PhysicsDebugPlugin, PhysicsPlugins};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_vrm::VrmBundle;
use unavi_avatar::{
    animation::AvatarAnimationClips, default_character_animations, AvatarBundle, AvatarPlugin,
    DEFAULT_VRM,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: "../unavi-app/assets".to_string(),
                ..default()
            }),
            EguiPlugin,
            PanOrbitCameraPlugin,
            PhysicsDebugPlugin::default(),
            PhysicsPlugins::default(),
            WorldInspectorPlugin::default(),
            AvatarPlugin,
        ))
        .init_resource::<Settings>()
        .add_systems(Startup, (setup_avatars, setup_scene))
        .add_systems(Update, (draw_gizmo, draw_ui, load_avatar, move_avatar))
        .run();
}

fn draw_gizmo(mut gizmos: Gizmos) {
    gizmos.axes(Transform::default(), 1.0);
}

#[derive(Resource)]
struct Settings {
    move_x: bool,
    move_z: bool,
    rotate: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            move_x: false,
            move_z: true,
            rotate: false,
        }
    }
}

fn draw_ui(mut contexts: EguiContexts, mut settings: ResMut<Settings>) {
    bevy_egui::egui::Window::new("Settings").show(contexts.ctx_mut(), |ui| {
        ui.checkbox(&mut settings.move_x, "Move X");
        ui.checkbox(&mut settings.move_z, "Move Z");
        ui.checkbox(&mut settings.rotate, "Rotate");
    });
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

    commands.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(Cuboid::new(10.0, 0.1, 10.0))),
        material: materials.add(StandardMaterial::default()),
        transform: Transform::from_xyz(0.0, -0.05, 0.0),
        ..default()
    });

    let mut transform = Transform::from_xyz(0.0, 3.0, -10.0);
    transform.look_at(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 1.0, 0.0));

    commands.spawn((
        Camera3dBundle {
            transform,
            ..default()
        },
        PanOrbitCamera::default(),
    ));
}

fn setup_avatars(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        AvatarBundle::new(default_character_animations(&asset_server)),
        SpatialBundle::default(),
        MoveDir::default(),
        Name::new("Avatar"),
    ));
}

#[derive(Component, Default)]
struct MoveDir {
    x: f32,
    z: f32,
}

/// Simulate an avatar that takes some time to load, showing a fallback in it's place.
fn load_avatar(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut done: Local<bool>,
    query: Query<(Entity, &Transform), With<AvatarAnimationClips>>,
    time: Res<Time>,
) {
    if *done || time.elapsed_seconds() < 1.5 {
        return;
    }

    *done = true;

    for (entity, transform) in query.iter() {
        commands.entity(entity).insert(VrmBundle {
            scene_bundle: SceneBundle {
                transform: *transform,
                ..default()
            },
            vrm: asset_server.load(DEFAULT_VRM),
            ..default()
        });
    }
}

fn move_avatar(
    mut transforms: Query<(&mut MoveDir, &mut Transform)>,
    settings: Res<Settings>,
    time: Res<Time>,
) {
    let delta = time.delta_seconds();

    for (mut dir, mut transform) in transforms.iter_mut() {
        if settings.move_x {
            if dir.x == 0.0 {
                dir.x = 1.0;
            }
        } else {
            dir.x = 0.0;
        }

        if settings.move_z {
            if dir.z == 0.0 {
                dir.z = 1.0;
            }
        } else {
            dir.z = 0.0;
        }

        if settings.rotate {
            transform.rotation *= Quat::from_rotation_y(delta);
        }

        transform.translation.x += dir.x * delta;
        transform.translation.z += dir.z * delta;

        if (dir.x.is_sign_positive() && transform.translation.x > 1.0)
            || (dir.x.is_sign_negative() && transform.translation.x < -1.0)
        {
            dir.x = -dir.x;
        }

        if (dir.z.is_sign_positive() && transform.translation.z > 1.0)
            || (dir.z.is_sign_negative() && transform.translation.z < -1.0)
        {
            dir.z = -dir.z;
        }
    }
}
