use std::f32::consts::FRAC_PI_2;
use avian3d::prelude::Rotation;

use bevy::{input::mouse::MouseMotion, prelude::*, window::CursorGrabMode, window::Window};
use bevy::input::keyboard::KeyboardInput;
use crate::{menu::MenuState, LocalPlayer};

#[derive(Debug, Default, Deref, DerefMut)]
pub struct CameraLookEvent(pub Vec2);

const PITCH_BOUND: f32 = FRAC_PI_2 - 1E-3;
const MENU_YAW_BOUND: f32 = FRAC_PI_2 - 1E-3;
const SENSITIVITY: f32 = 0.001;

#[derive(Resource)]
pub struct CameraLookResource(pub Vec<CameraLookEvent>);

pub fn read_mouse_input(
    #[cfg(target_family = "wasm")] mut is_firefox: Local<Option<bool>>,
    menu: Res<State<MenuState>>,
    mut look_events: ResMut<CameraLookResource>,
    mut look_xy: Local<Vec2>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut prev_menu: Local<MenuState>,
    mut yaw_bound: Local<(f32, f32)>,
    windows: Query<&Window>,
) {
    if mouse_motion_events.is_empty() {
        return;
    }

    let Ok(window) = windows.get_single() else { return };

    if window.cursor.grab_mode == CursorGrabMode::None {
        return;
    }

    let mut delta = Vec2::ZERO;

    for motion in mouse_motion_events.read() {
        delta -= motion.delta;
    }

    delta *= SENSITIVITY;

    #[cfg(target_family = "wasm")]
    {
        // Adjust the sensitivity when running in Firefox.
        // I think because of incorrect values within mouse move events.
        if let Some(is_firefox) = *is_firefox {
            if is_firefox {
                delta *= 10.0;
            }
        } else {
            let window = web_sys::window().unwrap();
            let navigator = window.navigator().user_agent().unwrap();
            *is_firefox = Some(navigator.to_lowercase().contains("firefox"));
        }
    }

    *look_xy += delta;
    look_xy.y = look_xy.y.clamp(-PITCH_BOUND, PITCH_BOUND);

    let menu = *menu.get();

    if menu == MenuState::Open {
        if *prev_menu != menu {
            *yaw_bound = ((look_xy.x - MENU_YAW_BOUND), (look_xy.x + MENU_YAW_BOUND));
        } else {
            look_xy.x = look_xy.x.clamp(yaw_bound.0, yaw_bound.1);
        }
    }

    *prev_menu = menu;

    look_events.0.push(CameraLookEvent(*look_xy));
}

const CAM_LERP_FACTOR: f32 = 30.0;

pub fn apply_camera_look(
    menu: Res<State<MenuState>>,
    mut cameras: Query<&mut Transform, With<Camera>>,
    mut look_events: ResMut<CameraLookResource>,
    mut menu_yaw: Local<Option<Quat>>,
    mut players: Query<(&mut Transform, &mut Rotation, &Children), (With<LocalPlayer>, Without<Camera>)>,
    mut target_pitch: Local<Quat>,
    mut target_yaw: Local<Quat>,
    time: Res<Time>,
) {
    for look in look_events.0.iter() {
        *target_yaw = Quat::from_rotation_y(look.x);
        *target_pitch = Quat::from_rotation_x(look.y);
    }

    look_events.0.clear();

    let s = time.delta_seconds() * CAM_LERP_FACTOR;
    let open = *menu.get() == MenuState::Open;

    for (mut player_tr, mut rotation, children) in players.iter_mut() {
        if !open {
            rotation.0 = rotation.0.lerp(*target_yaw, s);
            //player_tr.rotation = player_tr.rotation;
            player_tr.rotation = player_tr.rotation.lerp(*target_yaw, s);
        }

        for child in children.iter() {
            if let Ok(mut camera_tr) = cameras.get_mut(*child) {
                let target = if open {
                    if let Some(menu_yaw) = *menu_yaw {
                        (*target_yaw * menu_yaw.inverse()) * *target_pitch
                    } else {
                        *menu_yaw = Some(*target_yaw);
                        *target_pitch
                    }
                } else {
                    if menu_yaw.is_some() {
                        *menu_yaw = None;
                    }

                    *target_pitch
                };

                camera_tr.rotation = camera_tr.rotation.lerp(target, s);
            }
        }
    }
}

/*pub fn grab_mouse(
    mut windows: Query<&mut Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    key: Res<ButtonInput<KeyCode>>,
) {
    for mut window in windows.iter_mut() {
        if mouse.just_pressed(MouseButton::Left) {
            window.cursor.visible = false;
            window.cursor.grab_mode = CursorGrabMode::Locked;
        }

        if key.just_pressed(KeyCode::Escape) {
            window.cursor.visible = true;
            window.cursor.grab_mode = CursorGrabMode::None;
        }
    }
}
*/

pub fn capture_cursor(
    mut q_window: Query<&mut Window>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
) {
    let Ok(mut window) = q_window.get_single_mut() else { return };
    if mouse_button_input.just_pressed(MouseButton::Left) {
        window.cursor.visible = false;
        window.cursor.grab_mode = CursorGrabMode::Locked;
        // Clear Bevy's grab mode cache by setting a different grab mode
        // because an unlocked cursor will not update the current `CursorGrabMode`.
        // See <https://github.com/bevyengine/bevy/issues/8949>
        window.cursor.grab_mode = CursorGrabMode::Confined;
    }
}

pub fn release_cursor(mut q_window: Query<&mut Window>, keyboard_input: Res<ButtonInput<KeyCode>>,) {
    let Ok(mut window) = q_window.get_single_mut() else { return };
    if keyboard_input.just_pressed(KeyCode::Escape) {
        window.cursor.visible = true;
        window.cursor.grab_mode = CursorGrabMode::None;
    }
}