use crate::custom_audio::microphone::MicrophoneAudio;
use crate::custom_audio::spatial_audio::SpatialAudioSink;
use crate::networking::message::DeleteProp;
use crate::networking::{
    Authority, ExternalPlayer, Message, PlayerUuid, PropUuid, SocketSendMessage,
};
use bevy::app::App;
use bevy::prelude::{
    Event, EventReader, EventWriter, Local, NonSendMut, Query, ResMut, Resource, Update, With,
};
use bevy_matchbox::prelude::MultipleChannels;
use bevy_matchbox::MatchboxSocket;
use opus::{Application, Channels, Decoder, Encoder};
use serde::{Deserialize, Serialize};
use unavi_player::LocalPlayer;

pub struct VoiceChatPlugin;

impl bevy::prelude::Plugin for VoiceChatPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(target_os = "android")]
        {
            app.insert_non_send_resource(crate::voice_chat::MicrophoneEncoder(
                Encoder::new(48_000, Channels::Stereo, Application::Voip)
                    .expect("unable to create microphone audio compressing encoder"),
            ));
        }
        #[cfg(not(target_os = "android"))]
        {
            app.insert_non_send_resource(MicrophoneEncoder(
                Encoder::new(48_000, Channels::Mono, Application::Voip)
                    .expect("unable to create microphone audio compressing encoder"),
            ));
        }
        app.insert_non_send_resource(MicrophoneDecoder {
            channels_1_decoder: Decoder::new(48_000, Channels::Mono)
                .expect("unable to create microphone audio compressing decoder"),
            channels_2_decoder: Decoder::new(48_000, Channels::Stereo)
                .expect("unable to create microphone audio compressing decoder"),
        });
        app.add_systems(Update, send_voice_msg);
        app.add_systems(Update, rec_voice_msg);
        app.add_systems(Update, bad_jitter_buffer);
        app.add_event::<VoiceMsg>();
    }
}

#[derive(Resource)]
pub struct MicrophoneEncoder(pub Encoder);
#[derive(Resource)]
pub struct MicrophoneDecoder {
    pub channels_1_decoder: Decoder,
    pub channels_2_decoder: Decoder,
}
unsafe impl Sync for MicrophoneEncoder {}
unsafe impl Sync for MicrophoneDecoder {}

#[derive(Event, Serialize, Deserialize, Clone, Debug)]
pub struct VoiceMsg {
    data: Vec<u8>,
    uuid: PlayerUuid,
    pub channels: u16,
}

fn send_voice_msg(
    mut encoder: NonSendMut<MicrophoneEncoder>,
    microphone: ResMut<MicrophoneAudio>,
    mut voice_chat_socket: ResMut<MatchboxSocket<MultipleChannels>>,
    mut local_size: Local<Vec<f32>>,
    local_player: Query<&PlayerUuid, With<LocalPlayer>>,
) {
    #[allow(unused_mut)]
    let mut channels = 1;
    #[cfg(target_os = "android")]
    {
        channels = 2;
    }
    while let Ok(mut audio) = microphone.0.lock().unwrap().try_recv() {
        local_size.append(&mut audio);
    }
    if local_size.len() < 2880 * channels {
        return;
    }

    let player_uuid = match local_player.get_single() {
        Ok(val) => val,
        Err(err) => {
            println!("there is not exactly one local player: {}", err);
            return;
        }
    };

    while local_size.len() > 2880 * channels {
        voice_chat_socket.send_msg_all_unreliable(&Message::VoiceChat(VoiceMsg {
            data: encoder
                .0
                .encode_vec_float(
                    local_size.drain(0..(2880 * channels)).as_ref(),
                    2880 * channels,
                )
                .expect("couldnt' encode audio"),
            uuid: player_uuid.clone(),
            channels: channels as u16,
        }));
    }
}

fn rec_voice_msg(
    mut event_reader: EventReader<VoiceMsg>,
    mut microphone_decoder: NonSendMut<MicrophoneDecoder>,
    mut players: Query<(&PlayerUuid, &mut SpatialAudioSink), With<ExternalPlayer>>,
) {
    for event in event_reader.read() {
        let uuid = &event.uuid;
        let audio = &event.data;
        let channels = event.channels;

        for (id, audio_sink) in players.iter_mut() {
            println!("awa");
            if *id != *uuid {
                continue;
            }
            println!("adding audio");
            let mut output1 = [0.0; 2880];
            let mut output2 = [0.0; 2880 * 2];

            match channels {
                1 => {
                    microphone_decoder
                        .channels_1_decoder
                        .decode_float(audio, &mut output1, false)
                        .expect("unable to decode audio");
                }
                2 => {
                    microphone_decoder
                        .channels_2_decoder
                        .decode_float(audio, &mut output2, false)
                        .expect("unable to decode audio");
                }
                _ => panic!("wrong number of audio channels for decoding"),
            };
            match channels {
                1 => {
                    audio_sink
                        .sink
                        .append(rodio::buffer::SamplesBuffer::new(channels, 48_000, output1));
                }
                2 => {
                    audio_sink
                        .sink
                        .append(rodio::buffer::SamplesBuffer::new(channels, 48_000, output2));
                }
                _ => panic!("impossible"),
            }
            audio_sink.sink.play();
            audio_sink.sink.set_volume(1.0);
        }
    }
}

fn bad_jitter_buffer(mut spatial_audio_sinks: Query<&mut SpatialAudioSink, With<ExternalPlayer>>) {
    for spatial_audio_sink in spatial_audio_sinks.iter_mut() {
        if spatial_audio_sink.sink.len() <= 1 {
            spatial_audio_sink.sink.pause();
            continue;
        } else {
            spatial_audio_sink.sink.play();
        }
        if spatial_audio_sink.sink.len() <= 3 {
            spatial_audio_sink.sink.set_speed(0.95);
        } else if spatial_audio_sink.sink.len() >= 10 {
            spatial_audio_sink
                .sink
                .set_speed(spatial_audio_sink.sink.len() as f32 / 10.0);
        } else {
            spatial_audio_sink.sink.set_speed(1.0);
        }
    }
}
