use crate::networking::{ExternalPlayer, Message, PlayerUuid, SocketSendMessage};
use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::prelude::*;
use bevy_vrm::loader::Vrm;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use bevy::asset::AsyncReadExt;
use bevy::asset::io::ErasedAssetReader;
use bevy::tasks::futures_lite::AsyncRead;
use bevy_blob_loader::path::deserialize_path;
use bevy_health_bar3d::plugin::HealthBarPlugin;
use bevy_health_bar3d::prelude::Percentage;
use bevy_matchbox::MatchboxSocket;
use bevy_matchbox::prelude::MultipleChannels;
use futures::channel::mpsc::{channel, Receiver, SendError, Sender};
use futures::SinkExt;
use unavi_player::LocalPlayer;
use uuid::Uuid;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::js_sys::JSON;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Blob, File, Response};
use web_sys::js_sys::Uint8Array;

pub struct FileSharingPlugin;

impl Plugin for FileSharingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HealthBarPlugin::<LoadingBar>::default());
        app.add_event::<NewLocalAvatar>();
        app.add_event::<AvatarPartEnum>();
        app.add_systems(Update, read_dropped_files);
        app.add_systems(Update, set_local_avatar);
        app.add_systems(Update, handle_avatar_part);
        app.add_systems(Startup, setup);
        app.add_systems(Update, other_system);
        app.add_systems(Update, loading_bar_handler);
        app.add_systems(Startup, || {
            prevent_default_drop().unwrap();
        });
    }
}

fn prevent_default_drop() -> Result<(), JsValue> {
    let window = window().ok_or_else(|| JsValue::from_str("No window found"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("No document found"))?;
    let body = document
        .body()
        .ok_or_else(|| JsValue::from_str("No body found"))?;

    let callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
        event.prevent_default();
    }) as Box<dyn FnMut(web_sys::Event)>);

    body.add_event_listener_with_callback("drop", callback.as_ref().unchecked_ref())?;

    // Leak the closure to keep it alive (since it's used in a callback)
    callback.forget();

    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvatarPart {
    uuid: PlayerUuid,
    avatar_name: String,
    data: Vec<u8>,
}

#[derive(Event, Clone, Debug, Serialize, Deserialize)]
pub enum AvatarPartEnum {
    Len(PlayerUuid, usize),
    AvatarPart(AvatarPart),
    Done,
}


struct Thing(pub Vec<u8>, pub PlayerUuid);

impl Default for Thing {
    fn default() -> Self {
        Thing {
            0: vec![],
            1: PlayerUuid("".to_string()),
        }
    }
}

pub fn handle_avatar_part(
    asset_server: Res<AssetServer>,
    mut event_reader: EventReader<AvatarPartEnum>,
    mut commands: Commands,
    mut external_players: Query<(Entity, Option<&mut LoadingBar>, &Children, &PlayerUuid), With<ExternalPlayer>>,
    mut vrm: Query<&mut Handle<Vrm>>,
    mut embedded_asset_registry: ResMut<EmbeddedAssetRegistry>,
    mut local: Local<Thing>
) {
    for event in event_reader.read() {
        match event {
            AvatarPartEnum::AvatarPart(part) => {
                if local.0.len() == 0 {
                    let part = part.clone();
                    local.0 = part.data;
                    local.1 = part.uuid;
                } else {
                    let mut part = part.clone();
                    info!("getting part");
                    local.0.append(&mut part.data);
                }
                for (entity, mut loading_bar, children, player) in external_players.iter_mut() {
                    if *player != local.1 {
                        continue;
                    }
                    if let Some(loading_bar) = loading_bar.as_mut() {
                        loading_bar.current = local.0.len();
                    }
                }
            }
            AvatarPartEnum::Done => {
                info!("getting done");
                for (entity, mut loading_bar, children, player) in external_players.iter_mut() {
                    if *player != local.1 {
                        continue;
                    }
                    for child in children.iter() {
                        if let Ok(mut awa) = vrm.get_mut(*child) {
                            let uuid = Uuid::new_v4();
                            let f = format!("{}.vrm", uuid);
                            embedded_asset_registry.insert_asset(
                                f.parse().unwrap(),
                                f.as_ref(),
                                local.0.clone(),
                            );
                            local.0.clear();
                            *awa = asset_server.load(format!("embedded://{}", f));
                        }
                    }
                }
            }
            AvatarPartEnum::Len(player_uuid, len) => {
                for (entity, mut loading_bar, children, player) in external_players.iter_mut() {
                    if *player != *player_uuid {
                        continue;
                    }
                    commands.entity(entity).insert(LoadingBar {
                        len: *len,
                        current: 0,
                    });
                }
            }
        }
    }
}

#[derive(Component, Reflect)]
pub struct LoadingBar {
    pub(crate) len: usize,
    pub(crate) current: usize,
}

impl Percentage for LoadingBar {
    fn value(&self) -> f32 {
        self.current as f32 / self.len as f32
    }
}

#[derive(Event, Clone)]
pub struct NewLocalAvatar(String);

#[derive(Resource)]
pub struct TryingThings(Receiver<(Vec<u8>, PlayerUuid)>);

#[derive(Resource, Clone)]
pub struct OtherThing(Sender<(Vec<u8>, PlayerUuid)>);

fn setup(mut commands: Commands) {
    let (tx, rx) = channel(100);

    commands.insert_resource(TryingThings(rx));
    commands.insert_resource(OtherThing(tx));
}


pub struct Thing2(Vec<Vec<u8>>, PlayerUuid);

impl Default for Thing2 {
    fn default() -> Self {
        Self {
            0: vec![],
            1: PlayerUuid("".to_string()),
        }
    }
}


fn other_system(
    mut socket: ResMut<MatchboxSocket<MultipleChannels>>,
    mut trying_things: ResMut<TryingThings>,
    mut avatar_parts: Local<Option<Vec<AvatarPart>>>,
) {
    if avatar_parts.is_none() {
        if let Ok(Some(awa)) = trying_things.0.try_next() {
            socket.send_msg_all_reliable(&Message::AvatarPart(AvatarPartEnum::Len(awa.1.clone(), awa.0.len())));
            avatar_parts.replace(vec![]);
            for i in awa.0.chunks(10_000) {
                let part = AvatarPart {
                    uuid: awa.1.clone(),
                    avatar_name: "suzah.vrm".to_string(),
                    data: i.to_vec(),
                };
                avatar_parts.as_mut().unwrap().push(part);
            }
        }
    }

    for _ in 0..10 {
        let number_left = avatar_parts.as_ref().map(|a| a.len());
        if let Some(number_left) = number_left {
            if number_left == 0 {
                info!("sending done");
                socket.send_msg_all_reliable(&Message::AvatarPart(AvatarPartEnum::Done));
                avatar_parts.take();
            }
        }

        if let Some(avatar_parts) = avatar_parts.deref_mut().as_mut() {
            let avatar_part = avatar_parts.remove(0);
            let message = Message::AvatarPart(AvatarPartEnum::AvatarPart(avatar_part));

            let number_left = number_left.unwrap();

            info!("sending part");
            info!("number left is: {}", number_left);

            match socket.try_send_msg_all_reliable(&message) {
                Ok(_) => {}
                Err(err) => {
                    if err.is_full() {
                        let avatar_part = match message {
                            Message::AvatarPart(avatar_part) => {
                                match avatar_part {
                                    AvatarPartEnum::AvatarPart(avatar_part) => avatar_part,
                                    _ => unreachable!()
                                }
                            }
                            _ => unreachable!()
                        };
                        info!("channel full, retrying later");
                        avatar_parts.insert(0, avatar_part);
                        return;
                    }
                }
            }
        }
    }
}

fn loading_bar_handler(mut commands: Commands, query: Query<(Entity, &LoadingBar)>) {
    for (e, l) in query.iter() {
        if l.value() == 1.0 {
            commands.get_entity(e).unwrap().remove::<LoadingBar>();
        }
    }
}

fn set_local_avatar(
    asset_server: Res<AssetServer>,
    mut events: EventReader<NewLocalAvatar>,
    local_player: Query<(&Children, &PlayerUuid), With<LocalPlayer>>,
    mut vrm: Query<&mut Handle<Vrm>>,
    mut other_thing: ResMut<OtherThing>,
) {
    let Ok((children, uuid)) = local_player.get_single() else {
        return;
    };
    for child in children.iter() {
        if let Ok(mut vrm) = vrm.get_mut(*child) {
            for event in events.read() {
                let event = event.clone();
                let ev = event.clone();


                let mut other_thing = other_thing.clone();
                let uuid = uuid.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let window = web_sys::window().unwrap();

                    let resp_value = JsFuture::from(window.fetch_with_str(deserialize_path(event.0.as_ref()).to_str().unwrap()))
                        .await
                        .map_err(js_value_to_err("fetch path")).unwrap();

                    let resp = resp_value
                        .dyn_into::<Response>()
                        .map_err(js_value_to_err("convert fetch to Response")).unwrap();

                    let bytes = match resp.status() {
                        200 => {
                            let data = JsFuture::from(resp.array_buffer().unwrap()).await.unwrap();
                            let bytes = Uint8Array::new(&data).to_vec();
                            bytes
                        }
                        _ => panic!("AWA"),
                    };

                    other_thing.0.send((bytes, uuid.clone())).await.unwrap();
                });

                *vrm = asset_server.load(ev.0.clone());
                return;
            }
        }
    }
}

fn read_dropped_files(
    mut events: EventReader<FileDragAndDrop>,
    mut event_writer: EventWriter<NewLocalAvatar>,
) {
    for event in events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            #[cfg(target_family = "wasm")]
            let path = String::from(path_buf.to_str().unwrap());
            #[cfg(not(target_family = "wasm"))]
            let path = bevy::asset::AssetPath::from_path(path_buf.as_path());

            info!("DroppedFile: {}", path);

            event_writer.send(NewLocalAvatar(path.to_string()));
        }
    }
}

fn js_value_to_err<'a>(context: &'a str) -> impl FnOnce(JsValue) -> std::io::Error + 'a {
    move |value| {
        let message = match JSON::stringify(&value) {
            Ok(js_str) => format!("Failed to {context}: {js_str}"),
            Err(_) => {
                format!("Failed to {context} and also failed to stringify the JSValue of the error")
            }
        };

        std::io::Error::new(std::io::ErrorKind::Other, message)
    }
}
