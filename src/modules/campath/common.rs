use nom::bytes::complete::tag;
use nom::character::complete::space0;
use nom::combinator::{map, opt, recognize};
use nom::number::complete::float;
use nom::sequence::preceded;
use nom::IResult;

#[derive(Clone, Copy)]
pub struct ViewInfo {
    pub vieworg: glam::Vec3,
    pub viewangles: glam::Vec3,
}

fn signed_float(i: &str) -> IResult<&str, f32> {
    map(recognize(preceded(opt(tag("-")), float)), |what: &str| {
        what.parse().unwrap() // eh, not sure how this would crash
    })(i)
}

fn signed_double(i: &str) -> IResult<&str, f64> {
    map(recognize(preceded(opt(tag("-")), float)), |what: &str| {
        what.parse().unwrap()
    })(i)
}

pub fn cam_float(i: &str) -> IResult<&str, f32> {
    preceded(space0, signed_float)(i)
}

pub fn cam_double(i: &str) -> IResult<&str, f64> {
    preceded(space0, signed_double)(i)
}

pub fn lerp(v0: f32, v1: f32, t: f64) -> f32 {
    ((1. - t) * v0 as f64 + t * v1 as f64) as f32
}

/// Difference between curr and next
pub fn angle_diff(curr: f32, next: f32) -> f32 {
    let curr = curr.to_radians();
    let next = next.to_radians();

    (-(curr - next).sin()).asin().to_degrees()
}
