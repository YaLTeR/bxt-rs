use nom::bytes::complete::tag;
use nom::character::complete::space0;
use nom::combinator::{map, opt, recognize};
use nom::number::complete::float;
use nom::sequence::preceded;
use nom::IResult;

#[derive(Clone, Copy)]
pub struct ViewInfo {
    pub vieworg: [f64; 3],
    pub viewangles: [f64; 3],
}

fn signed_float(i: &str) -> IResult<&str, f64> {
    map(recognize(preceded(opt(tag("-")), float)), |what: &str| {
        what.parse().unwrap() // eh, not sure how this would crash
    })(i)
}

pub fn cam_float(i: &str) -> IResult<&str, f64> {
    preceded(space0, signed_float)(i)
}

pub fn lerp(v0: f64, v1: f64, t: f64) -> f64 {
    (1. - t) * v0 + t * v1
}

pub fn angle_diff(a1: f64, a2: f64) -> f64 {
    let a1 = a1.to_radians();
    let a2 = a2.to_radians();

    (a2.sin() * a1.cos() - a2.cos() * a1.sin())
        .asin()
        .to_degrees()
}
