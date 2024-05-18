use std::fs::File;
use std::io::Read;

use hldemo::FrameData;

use super::*;

pub fn demo_ghost_parse(filename: &str) -> eyre::Result<GhostInfo> {
    let pathbuf = PathBuf::from(filename);

    let mut bytes = Vec::new();
    let mut f = File::open(pathbuf)?;
    f.read_to_end(&mut bytes)?;

    let demo = hldemo::Demo::parse(&bytes);

    // Huh cannot propagate error from the `parse`. Interesting.
    if demo.is_err() {
        return Err(eyre::eyre!("Cannot read demo."));
    }

    let demo = demo.unwrap();

    let ghost_frames = demo.directory.entries[1]
        .frames
        .iter()
        .filter_map(|frame| match &frame.data {
            FrameData::ClientData(client) => Some(GhostFrame {
                origin: client.origin.into(),
                viewangles: client.viewangles.into(),
                frametime: Some(frame.time as f64), // time here is accummulative, will fix after
                sequence: None,
                frame: None,
                animtime: None,
                buttons: None,
            }),
            _ => None,
        })
        .scan(0., |acc, mut frame| {
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
        ghost_anim_frame: 0.,
    })
}

// pub fn demo_ghost_parse(
//     name: &str,
//     offset: f32,
//     parse_anim: bool,
// ) -> GhostInfo {
//     // New ghost
//     let mut ghost = GhostInfo::new();
//     ghost.set_name(name.to_owned());
//     ghost.reset_ghost_anim_frame();

//     let mut delta_decoders = get_initial_delta();
//     let mut custom_messages = HashMap::<u8, SvcNewUserMsg>::new();

//     // Help with checking out which demo is unparse-able.
//     // println!("Last parsed demo {}", ghost.get_name());

//     // Because player origin/viewangles and animation are on different frame, we have to sync it.
//     // Order goes: players info > animation > player info > ...
//     let mut sequence: Option<Vec<u8>> = None;
//     let mut anim_frame: Option<Vec<u8>> = None;
//     let mut animtime: Option<Vec<u8>> = None;

//     for (_, entry) in demo.directory.entries.iter().enumerate() {
//         for frame in &entry.frames {
//             match &frame.data {
//                 FrameData::NetMsg((_, data)) => {
//                     if !parse_anim {
//                         continue;
//                     }

//                     let (_, messages) =
//                         parse_netmsg(data.msg, &mut delta_decoders, &mut
// custom_messages).unwrap();

//                     for message in messages {
//                         match message {
//                             Message::EngineMessage(what) => match what {
//                                 EngineMessage::SvcDeltaPacketEntities(what) => {
//                                     for entity in &what.entity_states {
//                                         if entity.entity_index == 1 && entity.delta.is_some() {
//                                             sequence = entity
//                                                 .delta
//                                                 .as_ref()
//                                                 .unwrap()
//                                                 .get("gaitsequence\0")
//                                                 .cloned();
//                                             anim_frame = entity
//                                                 .delta
//                                                 .as_ref()
//                                                 .unwrap()
//                                                 .get("frame\0")
//                                                 .cloned();
//                                             animtime = entity
//                                                 .delta
//                                                 .as_ref()
//                                                 .unwrap()
//                                                 .get("animtime\0")
//                                                 .cloned();
//                                         }
//                                     }
//                                     // These numbers are not very close to what we want.
//                                     // They are vieworigin, not player origin.
//                                     // origin.push(data.info.ref_params.vieworg);
//                                     // viewangles.push(data.info.ref_params.viewangles);
//                                 }
//                                 _ => (),
//                             },
//                             _ => (),
//                         }
//                     }
//                 }
//                 FrameData::ClientData(what) => {
//                     // Append frame on this frame because the demo orders like it.
//                     ghost.append_frame(
//                         what.origin,
//                         what.viewangles,
//                         sequence.to_owned(),
//                         anim_frame.to_owned(),
//                         animtime.to_owned(),
//                         None,
//                     );

//                     // Reset for next find.
//                     sequence = None;
//                     anim_frame = None;
//                     animtime = None;
//                 }
//                 _ => (),
//             }
//         }
//     }

//     ghost
// }
