use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::*;

// Order of appearance for serde.
#[derive(Serialize, Deserialize, Debug)]
struct SgGhostInfo {
    name: String,
    authid: String,
    time: f32,
    startvel: [f32; 3],
    frames: Vec<SgGhostFrame>,
}

#[derive(Serialize, Deserialize, Debug)]
struct SgGhostFrame {
    origin: [f32; 3],
    viewangles: [f32; 3],
    moves: [f32; 3],
    buttons: u32,
    impulses: u32,
    frametime: u32, // This one is something else.
}

pub fn surf_gateway_ghost_parse(filename: &str) -> eyre::Result<GhostInfo> {
    let pathbuf = PathBuf::from(filename.to_owned());
    let file = std::fs::read_to_string(pathbuf)?;

    let surf_gateway_ghost: SgGhostInfo = serde_json::from_str(&file)?;

    // Convert surf_gateway_ghost to our normal ghost.
    Ok(GhostInfo {
        ghost_name: filename.to_owned(),
        frames: surf_gateway_ghost
            .frames
            .iter()
            .map(|ghost| GhostFrame {
                frametime: None,
                origin: Vec3::from_array(ghost.origin),
                viewangles: Vec3::from_array(ghost.viewangles),
                sequence: None,
                frame: None,
                animtime: None,
                buttons: ghost.buttons.into(),
            })
            .collect(),
        ghost_anim_frame: 0.,
    })
}
