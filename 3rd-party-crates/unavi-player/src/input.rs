use avian3d::prelude::*;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::{menu::MenuState, PlayerCamera};

use super::LocalPlayer;

#[derive(Resource)]
pub struct InputMap {
    pub key_menu: KeyCode,
    pub key_forward: KeyCode,
    pub key_backward: KeyCode,
    pub key_left: KeyCode,
    pub key_right: KeyCode,
    pub key_jump: KeyCode,
}

impl Default for InputMap {
    fn default() -> Self {
        Self {
            key_menu: KeyCode::Tab,
            key_forward: KeyCode::KeyW,
            key_backward: KeyCode::KeyS,
            key_left: KeyCode::KeyA,
            key_right: KeyCode::KeyD,
            key_jump: KeyCode::Space,
        }
    }
}

pub fn read_keyboard_input(
    input_map: Res<InputMap>,
    keys: Res<ButtonInput<KeyCode>>,
    menu: Res<State<MenuState>>,
    mut next_menu: ResMut<NextState<MenuState>>,
    mut players: Query<&mut LocalPlayer>,
) {
    for mut player in players.iter_mut() {
        player.input.forward = keys.pressed(input_map.key_forward);
        player.input.backward = keys.pressed(input_map.key_backward);
        player.input.left = keys.pressed(input_map.key_left);
        player.input.right = keys.pressed(input_map.key_right);
        player.input.jump = keys.pressed(input_map.key_jump);

        let did_move = player.input.forward
            || player.input.backward
            || player.input.left
            || player.input.right
            || player.input.jump;

        if did_move {
            next_menu.set(MenuState::Closed);
        } else if keys.just_pressed(input_map.key_menu) {
            match menu.get() {
                MenuState::Closed => next_menu.set(MenuState::Open),
                MenuState::Open => next_menu.set(MenuState::Closed),
            }
        }
    }
}