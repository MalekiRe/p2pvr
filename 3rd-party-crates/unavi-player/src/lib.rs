use bevy::app::RunFixedMainLoop;
use bevy::prelude::*;
use bevy::time::run_fixed_main_schedule;
use bevy_tnua::prelude::*;
use bevy_tnua_avian3d::TnuaAvian3dPlugin;
use unavi_avatar::AvatarPlugin;

mod body;
mod controls;
mod input;
mod look;
mod menu;

pub use body::{LocalPlayer, PlayerCamera};

pub mod layers {
    use avian3d::prelude::LayerMask;

    pub const LAYER_LOCAL_PLAYER: LayerMask = LayerMask(1 << 0);
    pub const LAYER_OTHER_PLAYER: LayerMask = LayerMask(1 << 1);
    pub const LAYER_WORLD: LayerMask = LayerMask(1 << 2);
    pub const LAYER_PROPS: LayerMask = LayerMask(1 << 3);
}


pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            AvatarPlugin,
            TnuaAvian3dPlugin::default(),
            TnuaControllerPlugin::default(),
        ))
            .insert_resource(look::CameraLookResource(vec![]))
        .init_state::<menu::MenuState>()
        .insert_resource(input::InputMap::default())
        .add_systems(Startup, body::spawn_player)
        .add_systems(OnEnter(menu::MenuState::Open), menu::open_menu)
        .add_systems(OnExit(menu::MenuState::Open), menu::close_menu)
        //.add_systems(RunFixedMainLoop, look::apply_camera_look.before(run_fixed_main_schedule))
        .add_systems(
            Update,
            (
                body::calc_eye_offset,
                body::set_avatar_head,
                body::setup_first_person,
                input::read_keyboard_input,
                look::grab_mouse,
                (
                    (look::read_mouse_input, look::apply_camera_look).chain(),
                    //look::read_mouse_input,
                    (
                        (
                            controls::void_teleport,
                            controls::move_player.before(input::read_keyboard_input),
                        )
                            .chain(),
                        body::rotate_avatar_head,
                    ),
                )
                    .chain(),
            ),
        );
    }
}
