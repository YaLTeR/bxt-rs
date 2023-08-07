use std::array::from_fn;

use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{multispace0, newline};
use nom::combinator::{all_consuming, map, opt};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;

use super::common::{angle_diff, cam_float, lerp, ViewInfo};

#[derive(Clone, Copy)]
pub struct ViewInfoCamIO {
    pub viewinfo: ViewInfo,
    pub time: f64,
    pub fov: f64,
}

#[derive(Clone)]
pub struct CamIO {
    pub campaths: Vec<ViewInfoCamIO>,
}

impl CamIO {
    fn find_next_entry(&self, time: f64) -> usize {
        if time <= 0. || self.campaths.is_empty() {
            return 1;
        }

        let end = self.campaths.last().unwrap();

        if time >= end.time {
            return self.campaths.len();
        }

        // TODO: it can be more efficient
        self.campaths
            .iter()
            .enumerate()
            .find(|(_, e)| time < e.time)
            .unwrap() // the condition above guarantees we always have result
            .0
    }

    pub fn get_view(&self, time: f64) -> Option<ViewInfoCamIO> {
        let next_campath_index = self.find_next_entry(time);

        if next_campath_index >= self.campaths.len() {
            return None;
        }

        let next_campath_index = {
            if next_campath_index == 0 {
                1 // it is possible that offset is too high and we're at first frame
            } else {
                next_campath_index
            }
        };

        let curr_entry = self.campaths[next_campath_index - 1];
        let next_entry = self.campaths[next_campath_index];
        let target = (time - curr_entry.time) / (next_entry.time - curr_entry.time);

        let new_vieworg: [f64; 3] = from_fn(|i| {
            lerp(
                curr_entry.viewinfo.vieworg[i],
                next_entry.viewinfo.vieworg[i],
                target,
            )
        });

        let new_viewangles: [f64; 3] = from_fn(|i| {
            lerp(
                curr_entry.viewinfo.viewangles[i],
                curr_entry.viewinfo.viewangles[i]
                    + angle_diff(
                        // normalize is not what we want as we are in between +/-
                        curr_entry.viewinfo.viewangles[i],
                        next_entry.viewinfo.viewangles[i],
                    ),
                target,
            )
        });

        let new_fov = lerp(curr_entry.fov, next_entry.fov, target);

        Some(ViewInfoCamIO {
            viewinfo: ViewInfo {
                vieworg: new_vieworg,
                viewangles: new_viewangles,
            },
            time: curr_entry.time,
            fov: new_fov,
        })
    }
}

fn channel(i: &str) -> IResult<&str, &str> {
    tag("channels time xPosition yPosition zPositon xRotation yRotation zRotation fov")(i)
}

fn header(i: &str) -> IResult<&str, &str> {
    preceded(
        tuple((
            tag("advancedfx Cam"),  // verify
            take_until("channels"), // skip version
            channel,
            multispace0,
            tag("DATA"),
        )),
        multispace0,
    )(i)
}

fn cam(i: &str) -> IResult<&str, ViewInfoCamIO> {
    map(
        tuple((
            cam_float, cam_float, cam_float, cam_float, cam_float, cam_float, cam_float, cam_float,
        )),
        |(time, xpos, ypos, zpos, xrot, yrot, zrot, fov)| ViewInfoCamIO {
            time,
            fov,
            viewinfo: ViewInfo {
                vieworg: [xpos, ypos, zpos],
                viewangles: [yrot, zrot, xrot], // be mindful of the flip
            },
        },
    )(i)
}

pub fn read_camio(i: &str) -> IResult<&str, CamIO> {
    map(
        preceded(
            header,
            all_consuming(delimited(
                opt(multispace0),
                separated_list0(newline, cam),
                opt(multispace0),
            )),
        ),
        |campaths| CamIO { campaths },
    )(i)
}
