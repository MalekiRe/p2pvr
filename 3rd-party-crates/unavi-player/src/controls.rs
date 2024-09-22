use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::prelude::*;

use crate::{body::SPAWN, LocalPlayer};
use unavi_avatar::PLAYER_HEIGHT;

#[derive(Default)]
pub struct InputState {
    pub menu: bool,
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
}

const FLOAT_HEIGHT: f32 = (PLAYER_HEIGHT / 2.0) + 0.1;

pub fn move_player(
    mut last_time: Local<f32>,
    mut players: Query<(&Transform, &mut LocalPlayer, &mut TnuaController)>,
    time: Res<Time>,
) {
    for (transform, mut player, mut controller) in players.iter_mut() {
        let dir_forward = transform.rotation.mul_vec3(Vec3::NEG_Z);
        let dir_left = transform.rotation.mul_vec3(Vec3::NEG_X);

        let mut move_direction = Vec3::ZERO;

        if player.input.forward {
            move_direction += dir_forward;
        }
        if player.input.backward {
            move_direction -= dir_forward;
        }
        if player.input.left {
            move_direction += dir_left;
        }
        if player.input.right {
            move_direction -= dir_left;
        }

        let desired_velocity = move_direction.normalize_or_zero() * player.speed;

        if player.input.jump {
            controller.action(TnuaBuiltinJump {
                height: player.jump_height,
                ..default()
            });
        }

        controller.basis(TnuaBuiltinWalk {
            coyote_time: 0.2,
            desired_velocity,
            float_height: FLOAT_HEIGHT,
            ..default()
        });

        player.input = InputState::default();
    }

    *last_time = time.elapsed_seconds();
}

const VOID_LEVEL: f32 = -50.0;

pub fn void_teleport(
    mut players: Query<
        (&Transform, &mut Position, &mut LinearVelocity, &mut AngularVelocity),
        With<LocalPlayer>,
    >,
) {
    for (transform, mut position, mut linvel, mut angvel) in players.iter_mut() {
        if transform.translation.y < VOID_LEVEL {
            info!("Player fell into void! Teleporting player to spawn...");
            //transform.translation = SPAWN;
            position.x = SPAWN.x;
            position.y = SPAWN.y;
            position.z = SPAWN.z;
            angvel.x = 0.0;
            angvel.y = 0.0;
            angvel.z = 0.0;
            linvel.x = 0.0;
            linvel.y = 0.0;
            linvel.z = 0.0;

            // TODO: Reset camera rotation
        }
    }
}
