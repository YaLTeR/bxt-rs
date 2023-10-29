use std::array::from_fn;

use nom::bytes::complete::tag;
use nom::character::complete::{multispace0, newline};
use nom::combinator::{all_consuming, map, opt};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;

use super::common::{angle_diff, cam_double, cam_float, lerp, ViewInfo};

#[derive(Clone, Copy)]
pub struct ViewInfoCamIO {
    pub viewinfo: ViewInfo,
    pub time: f64,
    pub fov: f32,
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

        self.campaths
            .iter()
            .position(|e| time < e.time)
            .unwrap_or(self.campaths.len())
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

        let new_vieworg = curr_entry
            .viewinfo
            .vieworg
            .lerp(next_entry.viewinfo.vieworg, target as f32);

        let viewangles_diff: [f32; 3] = from_fn(|i| {
            angle_diff(
                // normalize is not what we want as we are in between +/-
                curr_entry.viewinfo.viewangles[i],
                next_entry.viewinfo.viewangles[i],
            )
        });
        let viewangles_diff = glam::Vec3::from(viewangles_diff);
        let new_viewangles = curr_entry.viewinfo.viewangles.lerp(
            curr_entry.viewinfo.viewangles + viewangles_diff,
            target as f32,
        );

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
            tag("advancedfx Cam"),
            multispace0,
            tag("version 2"), // fixed version
            multispace0,
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
            cam_double, cam_float, cam_float, cam_float, cam_float, cam_float, cam_float, cam_float,
        )),
        |(time, xpos, ypos, zpos, xrot, yrot, zrot, fov)| ViewInfoCamIO {
            time,
            fov,
            viewinfo: ViewInfo {
                vieworg: [xpos, ypos, zpos].into(),
                viewangles: [yrot, zrot, xrot].into(), // be mindful of the flip
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_ok() {
        let input = "\
advancedfx Cam
version 2
channels time xPosition yPosition zPositon xRotation yRotation zRotation fov
DATA
0 2443.9453125 2796.3203125 -1898.9375 0 -5.559475421905518 -116.619873046875 90
0.016666699200868607 2443.9453125 2796.3203125 -1898.9375 0 -5.559475421905518 -116.619873046875 90";

        let res = read_camio(input);
        if let Ok(mdt) = res {
            assert!(mdt.0.is_empty());
            assert_eq!(mdt.1.campaths.len(), 2);
            assert_eq!(mdt.1.campaths[0].fov, 90.);
            assert_eq!(mdt.1.campaths[1].time, 0.016666699200868607);
        }
    }

    #[test]
    fn parse_no_fov() {
        let input = "\
advancedfx Cam
version 2
channels time xPosition yPosition zPositon xRotation yRotation zRotation fov
DATA
0 2443.9453125 2796.3203125 -1898.9375 0 -5.559475421905518 -116.619873046875 90
0.016666699200868607 2443.9453125 2796.3203125 -1898.9375 0 -5.559475421905518 -116.619873046875";

        let res = read_camio(input);
        assert!(res.is_err())
    }

    #[test]
    fn parse_faulty_header() {
        let input = "\
advancedfx Camio
version 2
channels time xPosition yPosition zPositon xRotation yRotation zRotation fov
DATA
0 2443.9453125 2796.3203125 -1898.9375 0 -5.559475421905518 -116.619873046875 90
0.016666699200868607 2443.9453125 2796.3203125 -1898.9375 0 -5.559475421905518 -116.619873046875 90";

        let res = read_camio(input);
        assert!(res.is_err())
    }

    #[test]
    fn parse_wrong_version() {
        let input = "\
advancedfx Cam
version 24096
channels time xPosition yPosition zPositon xRotation yRotation zRotation fov
DATA
0 2443.9453125 2796.3203125 -1898.9375 0 -5.559475421905518 -116.619873046875 90
0.016666699200868607 2443.9453125 2796.3203125 -1898.9375 0 -5.559475421905518 -116.619873046875 90";

        let res = read_camio(input);
        assert!(res.is_err())
    }
}
