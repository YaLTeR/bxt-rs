use std::fs::File;
use std::io::Read;

use dem::hldemo::{Demo, FrameData};
use dem::types::{EngineMessage, NetMessage};
use dem::{init_parse, parse_netmsg};

use super::*;

pub fn demo_ghost_parse(filename: &str) -> eyre::Result<GhostInfo> {
    let pathbuf = PathBuf::from(filename);

    let mut bytes = Vec::new();
    let mut f = File::open(pathbuf)?;
    f.read_to_end(&mut bytes)?;

    let demo = Demo::parse(&bytes);

    // Huh cannot propagate error from the `parse`. Interesting.
    if demo.is_err() {
        return Err(eyre::eyre!("Cannot read demo."));
    }

    let demo = demo.unwrap();

    // Because player origin/viewangles and animation are on different frame, we have to sync it.
    // Order goes: players info > animation > player info > ...
    // TODO parses everything within netmsg
    let mut sequence: Option<i32> = None;
    let mut anim_frame: Option<f32> = None;
    let mut animtime: Option<f32> = None;
    let mut gaitsequence: Option<i32> = None;
    // No need to do optional type for this.
    // Just make sure that blending is persistent across frames.
    let mut blending = [0u8; 2];

    let mut origin = [0f32; 3];
    let mut viewangles = [0f32; 3];

    let aux = init_parse!(demo);

    let ghost_frames = demo.directory.entries[1]
        .frames
        .iter()
        .filter_map(|frame| match &frame.data {
            // FrameData::ClientData(client) => {
            //     Some(GhostFrame {
            //         origin: client.origin.into(),
            //         viewangles: client.viewangles.into(),
            //         frametime: Some(frame.time as f64), /* time here is accummulative, will fix
            //                                              * after */
            //         sequence: None,
            //         frame: None,
            //         animtime: None,
            //         buttons: None,
            //     })
            // }
            FrameData::ClientData(client) => {
                origin = client.origin;
                viewangles = client.viewangles;

                // ClientData happens before NetMsg so we can reset some values here.
                sequence = None;
                anim_frame = None;
                animtime = None;

                None
            }
            FrameData::NetMsg((_, data)) => {
                let parse = parse_netmsg(data.msg, &aux);

                if parse.is_err() {
                    return None;
                }

                let (_, messages) = parse.unwrap();

                // Every time there is svc_clientdata, there is svc_deltapacketentities
                // Even if there isn't, this is more safe to make sure that we have the client data.
                let client_data = messages.iter().find_map(|message| {
                    if let NetMessage::EngineMessage(engine_message) = message {
                        if let EngineMessage::SvcClientData(ref client_data) = **engine_message {
                            Some(client_data)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                // If no client_dat then we that means there won't be packet entity. Typically.
                client_data?;

                // Cannot use client_data here because it only reports delta.
                // Even though it is something that can be worked with. Ehh.
                // let client_data = client_data.unwrap();

                // let (origin, viewangles) = if let Some(client_data) = client_data {
                //     (client_data.client_data.get(""))
                // } else {
                //     (None, None)
                // };

                let delta_packet_entities = messages.iter().find_map(|message| {
                    if let NetMessage::EngineMessage(engine_message) = message {
                        if let EngineMessage::SvcDeltaPacketEntities(ref delta_packet_entities) =
                            **engine_message
                        {
                            Some(delta_packet_entities)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                if let Some(delta_packet_entities) = delta_packet_entities {
                    if delta_packet_entities.entity_states.first().is_some()
                        && delta_packet_entities.entity_states[0].delta.is_some()
                    {
                        let delta = &delta_packet_entities.entity_states[0]
                            .delta
                            .as_ref()
                            .unwrap();

                        if let Some(sequence_bytes) = delta.get("sequence\0") {
                            let sequence_bytes: [u8; 4] = from_fn(|i| sequence_bytes[i]);
                            sequence = Some(i32::from_le_bytes(sequence_bytes));
                        }

                        if let Some(anim_frame_bytes) = delta.get("frame\0") {
                            let anim_frame_bytes: [u8; 4] = from_fn(|i: usize| anim_frame_bytes[i]);
                            anim_frame = Some(f32::from_le_bytes(anim_frame_bytes));
                        }

                        if let Some(animtime_bytes) = delta.get("animtime\0") {
                            let animtime_bytes: [u8; 4] = from_fn(|i| animtime_bytes[i]);
                            animtime = Some(f32::from_le_bytes(animtime_bytes));
                        }

                        if let Some(gaitsequence_bytes) = delta.get("gaitsequence\0") {
                            let gaitsequence_bytes: [u8; 4] = from_fn(|i| gaitsequence_bytes[i]);
                            gaitsequence = Some(i32::from_le_bytes(gaitsequence_bytes));
                        }

                        if let Some(blending0) = delta.get("blending[0]\0") {
                            // blending is just [u8; 1]
                            blending[0] = blending0[0];
                        }

                        if let Some(blending1) = delta.get("blending[1]\0") {
                            // blending is just [u8; 1]
                            blending[1] = blending1[0];
                        }
                    }
                }

                Some(GhostFrame {
                    origin: Vec3::from_array(origin),
                    viewangles: Vec3::from_array(viewangles),
                    frametime: Some(frame.time as f64), /* time here is accummulative, will fix
                                                         * after */
                    buttons: None,
                    anim: Some(GhostFrameAnim {
                        sequence,
                        frame: anim_frame,
                        animtime,
                        gaitsequence,
                        blending,
                    }),
                })
            }
            _ => None,
        })
        .scan(0., |acc, mut frame: GhostFrame| {
            // Cummulative time is 1 2 3 4, so do subtraction to get the correct frametime
            let cum_time = frame.frametime.unwrap();

            frame.frametime = Some(cum_time - *acc);
            *acc = cum_time;

            Some(frame)
        })
        .collect::<Vec<GhostFrame>>();

    Ok(GhostInfo {
        ghost_name: filename.to_owned(),
        frames: ghost_frames,
    })
}
