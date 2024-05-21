use nom::bytes::complete::{tag, take, take_till};
use nom::character::complete::{multispace0, newline, space0, u32};
use nom::combinator::{all_consuming, map, opt, recognize};
use nom::multi::separated_list0;
use nom::number::complete::float as _float;
use nom::sequence::{delimited, preceded, tuple};
use nom::IResult;

use super::*;

#[allow(dead_code)]
struct SimenGhostFrame {
    frame: GhostFrame,
    velocity: [f32; 3],
    buttons: u32,
    moves: [f32; 2],
}

pub fn simen_ghost_parse(filename: &str) -> eyre::Result<GhostInfo> {
    let pathbuf = PathBuf::from(filename.to_owned());
    let file = std::fs::read_to_string(pathbuf)?;

    let res = match map(
        preceded(
            simen_wrbot_header,
            all_consuming(delimited(
                opt(multispace0),
                separated_list0(
                    newline,
                    // Conversion from simen ghost to our generic ghost. Maybe we can add more
                    // things down the line.
                    map(simen_wrbot_line, |simen_ghost| simen_ghost.frame),
                ),
                opt(multispace0),
            )),
        ),
        |frames| GhostInfo {
            ghost_name: filename.to_owned(),
            frames,
        },
    )(&file)
    {
        Ok((_, ghost)) => Ok(ghost),
        // Do this to avoid propagating nom's &str into error.
        Err(_) => Err(eyre::eyre!("Cannot parse file.")),
    };

    res
}

fn simen_wrbot_header(i: &str) -> IResult<&str, u8> {
    map(
        tuple((
            skip_line, // Time
            skip_line, // Name
            skip_line, // SteamID
            skip_line, // Date
            skip_line, // Location
            skip_line, // ??
        )),
        |_| 0u8,
    )(i)
}

fn simen_wrbot_line(i: &str) -> IResult<&str, SimenGhostFrame> {
    map(
        tuple((
            float, float, float, float, float, float, float, float, u32, float, float,
        )),
        |(pitch, yaw, posx, posy, posz, velx, vely, velz, buttons, move1, move2)| SimenGhostFrame {
            frame: GhostFrame {
                frametime: None,
                origin: Vec3::from_array([posx, posy, posz]),
                viewangles: Vec3::from_array([pitch, yaw, 0.]),
                buttons: buttons.into(),
                anim: None,
            },
            velocity: [velx, vely, velz],
            buttons,
            moves: [move1, move2],
        },
    )(i)
}

fn skip_line(i: &str) -> IResult<&str, u8> {
    map(tuple((take_till(|c| c == '\n'), take(1usize))), |_| 0u8)(i)
}

fn signed_float(i: &str) -> IResult<&str, f32> {
    map(recognize(preceded(opt(tag("-")), _float)), |what: &str| {
        what.parse().unwrap()
    })(i)
}

pub fn float(i: &str) -> IResult<&str, f32> {
    preceded(space0, signed_float)(i)
}
