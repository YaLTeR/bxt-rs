use std::array::from_fn;
use std::path::PathBuf;

// use crate::demo_doer::get_ghost::romanian_jumpers::romanian_jumpers_ghost_parse;
// use crate::{
//     demo_doer::get_ghost::{
//         demo::demo_ghost_parse, simen::simen_ghost_parse,
// surf_gateway::surf_gateway_ghost_parse,     },
//     open_demo,
// };
use color_eyre::eyre;
use glam::Vec3;

use self::demo::demo_ghost_parse;
// use rayon::prelude::*;
use self::romanian_jumpers::romanian_jumpers_ghost_parse;
use self::simen::simen_ghost_parse;
use self::surf_gateway::surf_gateway_ghost_parse;

mod demo;
mod romanian_jumpers;
mod simen;
mod surf_gateway;

pub fn get_ghost(filename: &str) -> eyre::Result<GhostInfo> {
    let pathbuf = PathBuf::from(filename);

    if pathbuf.to_str().unwrap().ends_with(".dem") {
        demo_ghost_parse(filename)
    } else if pathbuf.to_str().unwrap().ends_with(".simen.txt") {
        // Either this, or use enum in main file.
        simen_ghost_parse(filename)
    } else if pathbuf.to_str().unwrap().ends_with(".sg.json") {
        // Surf Gateway
        surf_gateway_ghost_parse(filename)
    } else if pathbuf.to_str().unwrap().ends_with(".rj.json") {
        // Romanian-Jumpers
        romanian_jumpers_ghost_parse(filename)
    } else {
        Err(eyre::eyre!("Unknown ghost file extension."))
    }
}

// Intentionally ignore errors for greater goods.
// pub fn get_ghosts(others: &Vec<(String, f64)>) -> Vec<GhostInfo> {
//     others
//         .par_iter()
//         .filter_map(|(filename, offset)| get_ghost(filename).ok())
//         .collect()
// }

#[derive(Debug, Clone)]
pub struct GhostFrame {
    pub origin: Vec3,
    pub viewangles: Vec3,
    pub frametime: Option<f64>,
    pub buttons: Option<u32>,
    pub anim: Option<GhostFrameAnim>,
}

#[derive(Debug, Clone)]
pub struct GhostFrameAnim {
    pub sequence: Option<i32>,
    pub frame: Option<f32>,
    pub animtime: Option<f32>,
    pub gaitsequence: Option<i32>,
    // 0 is the same as no blending so there is no need to do optional type.
    pub blending: [u8; 2],
}

#[derive(Debug)]
pub struct GhostInfo {
    pub ghost_name: String,
    pub frames: Vec<GhostFrame>,
}

impl GhostInfo {
    /// Returns an interpolated [`GhostFrame`] based on current time.
    ///
    /// Takes an optional argument to force frametime.
    pub fn get_frame(&self, time: f64, frametime: Option<f64>) -> Option<GhostFrame> {
        let frame0 = self.frames.first()?;

        // No frame time, not sure how to accumulate correctly
        if frame0.frametime.is_none() && frametime.is_none() {
            return None;
        }

        let mut from_time = 0f64;
        let mut to_time = 0f64;
        let mut to_index = 0usize;

        for (index, frame) in self.frames.iter().enumerate() {
            let add_time = if let Some(frametime) = frametime {
                frametime
            } else {
                frame.frametime.unwrap()
            };

            // only exit when greater means we are having the "to" frame
            if to_time > time {
                break;
            }

            from_time = to_time;
            to_time += add_time;
            to_index = index;
        }

        if to_index == 0 {
            return Some(frame0.clone());
        }

        // If exceeding the number of available frames then we have nothing.
        // This is to make sure that we know when it ends.
        if to_index == self.frames.len() - 1 && time >= to_time {
            return None;
        }

        let to_frame = self.frames.get(to_index)?;

        let from_frame = self.frames.get(to_index - 1).unwrap();

        let target = (time - from_time) / (to_time - from_time);
        // clamp because vec lerp extrapolates as well.
        let target = target.clamp(0., 1.);

        let new_origin = from_frame.origin.lerp(to_frame.origin, target as f32);

        let viewangles_diff: [f32; 3] = from_fn(|i| {
            angle_diff(
                // normalize is not what we want as we are in between +/-
                from_frame.viewangles[i],
                to_frame.viewangles[i],
            )
        });
        let viewangles_diff = Vec3::from(viewangles_diff);
        let new_viewangles = from_frame
            .viewangles
            // attention, lerp to `from + diff`
            .lerp(from_frame.viewangles + viewangles_diff, target as f32);

        // Maybe do some interpolation for sequence in the future? Though only demo would have it.
        Some(GhostFrame {
            origin: new_origin,
            viewangles: new_viewangles,
            frametime: from_frame.frametime,
            buttons: from_frame.buttons.clone(),
            anim: from_frame.anim.clone(),
        })
    }

    /// Returns the frame index from a given time.
    pub fn get_frame_index(&self, time: f64, frametime: Option<f64>) -> usize {
        let mut to_time = 0f64;
        let mut to_index = 0usize;

        for (index, frame) in self.frames.iter().enumerate() {
            let add_time = if let Some(frametime) = frametime {
                frametime
            } else {
                frame.frametime.unwrap()
            };

            // only exit when greater means we are having the "to" frame
            if to_time > time {
                break;
            }

            to_time += add_time;
            to_index = index;
        }

        if to_index == 0 {
            return 0;
        }

        return to_index;
    }
}

/// Difference between curr and next
pub fn angle_diff(curr: f32, next: f32) -> f32 {
    let curr = curr.to_radians();
    let next = next.to_radians();

    (-(curr - next).sin()).asin().to_degrees()
}