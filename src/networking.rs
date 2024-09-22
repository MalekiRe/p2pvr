use crate::networking::message::{DeleteProp, PlayerPosition, SpawnCube, UpdateProp};
use crate::networking::systems::{message_handling, remove_dead_players, sync_local_player_to_network, sync_local_props_to_network};
use crate::SPAWN;
use avian3d::collision::{Collider, CollisionLayers};
use avian3d::prelude::{LockedAxes, RigidBody};
use bevy::app::App;
use bevy::asset::AssetServer;
use bevy::prelude::{
    default, BuildChildren, Commands, Component, GlobalTransform, IntoSystemConfigs, Plugin,
    SceneBundle, SpatialBundle, Transform, Update,
};
use bevy_matchbox::matchbox_socket::{Packet, SingleChannel};
use bevy_matchbox::prelude::PeerId;
use bevy_matchbox::MatchboxSocket;
use bevy_vrm::VrmBundle;
use serde::{Deserialize, Serialize};
use std::str::Utf8Error;
use unavi_avatar::{
    default_character_animations, AvatarBundle, AverageVelocity, FallbackAvatar, DEFAULT_VRM,
    PLAYER_HEIGHT, PLAYER_WIDTH,
};
use unavi_player::layers::LAYER_OTHER_PLAYER;

#[derive(Component, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PlayerUuid(pub String);

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub struct PropUuid(pub String);

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Authority {
    pub(crate) player: PlayerUuid,
    pub(crate) counter: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Message {
    SpawnCube(SpawnCube),
    UpdateProp(UpdateProp),
    DeleteProp(DeleteProp),
    PlayerPosition(PlayerPosition),
}

#[derive(Component)]
pub struct ExternalPlayer {
    uuid: PlayerUuid,
    peer_id: PeerId,
}

pub trait SocketSendMessage {
    fn send_msg(&mut self, peer: PeerId, message: &Message);
    fn receive_msg(&mut self) -> Vec<(PeerId, Message)>;
    fn send_msg_all(&mut self, message: &Message);
}

impl SocketSendMessage for MatchboxSocket<SingleChannel> {
    fn send_msg(&mut self, peer: PeerId, message: &Message) {
        let msg = serde_json::to_string(message).unwrap();

        self.send(msg.as_bytes().into(), peer);
    }
    fn receive_msg(&mut self) -> Vec<(PeerId, Message)> {
        self.receive()
            .into_iter()
            .map(|(id, packet)| {
                let str = std::str::from_utf8(&packet).unwrap();
                (id, serde_json::from_str::<Message>(str).unwrap())
            })
            .collect()
    }
    fn send_msg_all(&mut self, message: &Message) {
        let peers = self.connected_peers().collect::<Vec<_>>();
        for peer in peers {
            self.send_msg(peer, message);
        }
    }
}

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlayerPosition>()
            .add_event::<SpawnCube>()
            .add_event::<UpdateProp>()
            .add_event::<DeleteProp>();

        app.add_systems(Update, message_handling::route_messages)
            .add_systems(
                Update,
                (
                    message_handling::player_position,
                    message_handling::update_prop,
                )
                    .after(message_handling::route_messages),
            );

        app.add_systems(Update, (sync_local_props_to_network, sync_local_player_to_network, remove_dead_players));
    }
}

pub mod message {
    use crate::networking::{Authority, PlayerUuid, PropUuid};
    use avian3d::prelude::{LinearVelocity, Position, Rotation};
    use bevy::prelude::Event;
    use bevy_matchbox::prelude::PeerId;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Serialize, Deserialize, Debug, Event)]
    pub struct SpawnCube {
        pub authority: Authority,
        pub prop_uuid: PropUuid,
        pub position: Position,
    }

    #[derive(Clone, Serialize, Deserialize, Debug, Event)]
    pub struct UpdateProp {
        pub authority: Authority,
        pub prop_uuid: PropUuid,
        pub position: Position,
        pub rotation: Rotation,
        pub linear_velocity: LinearVelocity,
    }
    #[derive(Clone, Serialize, Deserialize, Debug, Event)]
    pub struct DeleteProp {
        pub authority: Authority,
        pub prop_uuid: PropUuid,
    }

    #[derive(Clone, Serialize, Deserialize, Debug, Event)]
    pub struct PlayerPosition {
        pub player_uuid: PlayerUuid,
        pub peer_id: PeerId,
        pub position: Position,
        pub rotation: Rotation,
        pub linear_velocity: LinearVelocity,
    }
}

pub mod systems {
    use crate::networking::message::{PlayerPosition, UpdateProp};
    use crate::networking::{
        Authority, ExternalPlayer, Message, PlayerUuid, PropUuid, SocketSendMessage,
    };
    use avian3d::prelude::{LinearVelocity, Position, Rotation};
    use bevy::prelude::*;
    use bevy_matchbox::matchbox_socket::SingleChannel;
    use bevy_matchbox::MatchboxSocket;
    use unavi_player::LocalPlayer;

    pub fn sync_local_player_to_network(
        mut socket: ResMut<MatchboxSocket<SingleChannel>>,
        local_player: Query<
            (&Position, &Rotation, &LinearVelocity, &PlayerUuid),
            (
                With<LocalPlayer>,
                Or<(
                    Changed<Position>,
                    Changed<Rotation>,
                    Changed<LinearVelocity>,
                )>,
            ),
        >,
    ) {
        let Some(socket_id) = socket.id() else {
            return;
        };

        let (position, rotation, linear_velocity, uuid) = match local_player.get_single() {
            Ok(val) => val,
            Err(err) => {
                println!("there is not exactly one local player: {}", err);
                return;
            }
        };

        socket.update_peers();

        let message = Message::PlayerPosition(PlayerPosition {
            player_uuid: uuid.clone(),
            peer_id: socket_id,
            position: position.clone(),
            rotation: rotation.clone(),
            linear_velocity: linear_velocity.clone(),
        });
        socket.send_msg_all(&message);
    }

    pub fn sync_local_props_to_network(
        mut socket: ResMut<MatchboxSocket<SingleChannel>>,
        local_props: Query<
            (&Position, &Rotation, &LinearVelocity, &PropUuid, &Authority),
            (
                With<Authority>,
                Or<(
                    Changed<Position>,
                    Changed<Rotation>,
                    Changed<LinearVelocity>,
                )>,
            ),
        >,
        local_player: Query<&PlayerUuid, With<LocalPlayer>>,
    ) {
        let Some(socket_id) = socket.id() else {
            return;
        };
        let player_uuid = match local_player.get_single() {
            Ok(val) => val,
            Err(err) => {
                println!("there is not exactly one local player: {}", err);
                return;
            }
        };

        socket.update_peers();

        for (position, rotation, linear_velocity, uuid, authority) in local_props.iter() {
            if authority.player != *player_uuid {
                continue;
            }
            let message = Message::UpdateProp(UpdateProp {
                authority: authority.clone(),
                prop_uuid: uuid.clone(),
                position: position.clone(),
                rotation: rotation.clone(),
                linear_velocity: linear_velocity.clone(),
            });
            socket.send_msg_all(&message);
        }
    }

    pub fn remove_dead_players(
        mut commands: Commands,
        mut socket: ResMut<MatchboxSocket<SingleChannel>>,
        external_players: Query<(Entity, &ExternalPlayer)>,
    ) {
        // TODO this is stupid and simple and will start to get slow if you have like
        // millions of peers who have connected and disconnected, but it's fine for now
        for peer_id in socket.disconnected_peers() {
            for (entity, external_player) in external_players.iter() {
                if external_player.peer_id == *peer_id {
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
    }

    pub mod message_handling {
        use crate::networking::message::*;
        use crate::networking::{
            spawn_external_player, Authority, ExternalPlayer, Message, PlayerUuid, PropUuid,
            SocketSendMessage,
        };
        use avian3d::prelude::{LinearVelocity, Position, Rotation};
        use bevy::prelude::*;
        use bevy_matchbox::prelude::SingleChannel;
        use bevy_matchbox::MatchboxSocket;

        pub fn route_messages(
            mut socket: ResMut<MatchboxSocket<SingleChannel>>,
            mut player_position: EventWriter<PlayerPosition>,
            mut spawn_cube: EventWriter<SpawnCube>,
            mut update_prop: EventWriter<UpdateProp>,
            mut delete_prop: EventWriter<DeleteProp>,
        ) {
            for (_id, message) in socket.receive_msg() {
                match message {
                    Message::SpawnCube(sc) => {
                        spawn_cube.send(sc);
                    }
                    Message::UpdateProp(up) => {
                        update_prop.send(up);
                    }
                    Message::DeleteProp(dp) => {
                        delete_prop.send(dp);
                    }
                    Message::PlayerPosition(pp) => {
                        player_position.send(pp);
                    }
                };
            }
        }

        pub fn update_prop(
            mut event_reader: EventReader<UpdateProp>,
            mut external_props: Query<(
                &mut Position,
                &mut Rotation,
                &mut LinearVelocity,
                &PropUuid,
                &Authority,
            )>,
        ) {
            for update_prop in event_reader.read() {
                for (mut position, mut rotation, mut linear_velocity, prop_uuid, authority) in
                    external_props.iter_mut()
                {
                    if update_prop.prop_uuid != *prop_uuid {
                        continue;
                    }
                    println!("updating prop");
                    *position = update_prop.position;
                    *rotation = update_prop.rotation;
                    *linear_velocity = update_prop.linear_velocity;
                }
            }
        }

        pub fn player_position(
            mut commands: Commands,
            mut event_reader: EventReader<PlayerPosition>,
            mut external_players: Query<
                (
                    &mut Position,
                    &mut Rotation,
                    &mut LinearVelocity,
                    &PlayerUuid,
                ),
                With<ExternalPlayer>,
            >,
            external_player_query: Query<&ExternalPlayer>,
            asset_server: Res<AssetServer>,
        ) {
            for player_position in event_reader.read() {
                for (mut position, mut rotation, mut linear_velocity, player_uuid) in
                    external_players.iter_mut()
                {
                    if player_position.player_uuid != *player_uuid {
                        continue;
                    }
                    *position = player_position.position;
                    *rotation = player_position.rotation;
                    *linear_velocity = player_position.linear_velocity;
                }
                let mut exists = false;
                for external_player in external_player_query.iter() {
                    if external_player.uuid == player_position.player_uuid {
                        exists = true;
                    }
                }
                if !exists {
                    spawn_external_player(
                        &asset_server,
                        &mut commands,
                        player_position.player_uuid.clone(),
                        player_position.peer_id,
                    );
                    event_reader.clear();
                    // return early to prevent bugs, simple solution sorta stupid and not optimal.
                    return;
                }
            }
        }
    }
}

pub fn spawn_external_player(
    asset_server: &AssetServer,
    commands: &mut Commands,
    uuid: PlayerUuid,
    peer_id: PeerId,
) {
    println!("spawning external player: {}, {}", peer_id, uuid.0);

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
            uuid.clone(),
            ExternalPlayer { uuid, peer_id },
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
