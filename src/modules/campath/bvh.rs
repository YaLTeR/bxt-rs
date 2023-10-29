use std::array::from_fn;

use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{multispace0, newline, space0, u32};
use nom::combinator::{all_consuming, map, opt};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;

use super::common::{cam_double, cam_float, ViewInfo};
use crate::modules::campath::common::angle_diff;

#[derive(Clone, Copy)]
pub struct BvhHeader {
    pub frames: usize,
    pub frametime: f64,
}

#[derive(Clone)]
pub struct Bvh {
    pub header: BvhHeader,
    pub campaths: Vec<ViewInfo>,
}

impl Bvh {
    fn find_next_entry(&self, time: f64) -> usize {
        if time <= 0. {
            return 1;
        }

        let entry = (time / self.header.frametime).floor() as usize;

        if entry >= self.header.frames {
            return self.header.frames;
        }

        entry
    }

    pub fn get_view(&self, time: f64) -> Option<ViewInfo> {
        let next_campath_index = self.find_next_entry(time);

        if next_campath_index == self.campaths.len() {
            return None;
        }

        let next_campath_index = {
            if next_campath_index == 0 {
                1
            } else {
                next_campath_index
            }
        };

        let curr_entry = self.campaths[next_campath_index - 1];
        let next_entry = self.campaths[next_campath_index];
        let target =
            (time - next_campath_index as f64 * self.header.frametime) / (self.header.frametime);

        let new_vieworg = curr_entry.vieworg.lerp(next_entry.vieworg, target as f32);

        let viewangles_diff: [f32; 3] = from_fn(|i| {
            angle_diff(
                // normalize is not what we want as we are in between +/-
                curr_entry.viewangles[i],
                next_entry.viewangles[i],
            )
        });
        let viewangles_diff = glam::Vec3::from(viewangles_diff);
        let new_viewangles = curr_entry
            .viewangles
            .lerp(curr_entry.viewangles + viewangles_diff, target as f32);

        Some(ViewInfo {
            vieworg: new_vieworg,
            viewangles: new_viewangles,
        })
    }
}

fn channel(i: &str) -> IResult<&str, &str> {
    tag("CHANNELS 6 Xposition Yposition Zposition Zrotation Xrotation Yrotation")(i)
}

fn frames(i: &str) -> IResult<&str, u32> {
    preceded(tuple((tag("Frames:"), space0)), u32)(i)
}

fn frametime(i: &str) -> IResult<&str, f64> {
    preceded(tuple((tag("Frame Time:"), space0)), cam_double)(i)
}

fn header(i: &str) -> IResult<&str, BvhHeader> {
    map(
        preceded(
            tuple((
                tag("HIERARCHY"),
                newline,
                take_until("{"),           // skip root name
                take_until("CHANNELS 6 "), // skip offset
                channel,                   // verify channel
                take_until("MOTION"),      // eh
                take_until("Frames:"),
            )),
            tuple((frames, preceded(newline, frametime))),
        ),
        |(frames, frametime)| BvhHeader {
            frames: frames as usize,
            frametime,
        },
    )(i)
}

fn cam(i: &str) -> IResult<&str, ViewInfo> {
    map(
        tuple((
            cam_float, cam_float, cam_float, cam_float, cam_float, cam_float,
        )), // parameters appear in the order of CHANNELS 6 specified in channel()
        |(ypos, zpos, xpos, zrot, xrot, yrot)| ViewInfo {
            vieworg: [-xpos, -ypos, zpos].into(), // HLAE does this
            viewangles: [-xrot, yrot, -zrot].into(),
        },
    )(i)
}

pub fn read_bvh(i: &str) -> IResult<&str, Bvh> {
    map(
        tuple((
            header,
            preceded(
                newline,
                all_consuming(delimited(
                    opt(multispace0),
                    separated_list0(newline, cam),
                    opt(multispace0),
                )),
            ),
        )),
        |(header, campaths)| Bvh { header, campaths },
    )(i)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_ok() {
        let input = "\
HIERARCHY
ROOT MdtCam
{
	OFFSET 0.00 0.00 0.00
	CHANNELS 6 Xposition Yposition Zposition Zrotation Xrotation Yrotation
	End Site
	{
		OFFSET 0.00 0.00 -1.00
	}
}
MOTION
Frames: 1152
Frame Time: 0.041667
-2796.320343 -1898.937416 -2443.945312 -0.000002 5.559472 -116.619866
-3161.070251 3701.062393 1584.867191 0.000000 -15.507801 7.794800";

        let res = read_bvh(input);
        if let Ok(mdt) = res {
            assert!(mdt.0.is_empty());
            assert_eq!(mdt.1.header.frames, 1152);
            assert_eq!(mdt.1.header.frametime, 0.041667);
            assert_eq!(mdt.1.campaths.len(), 2);
        }
    }
}
