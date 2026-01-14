use crate::modules::hud::{screen_info, Draw};
use crate::modules::timer::{Timer, BXT_HUD_TIMER, BXT_HUD_TIMER_ANCHOR, TIME};
use crate::modules::Module;
use crate::utils::MainThreadMarker;

pub fn draw_timer(marker: MainThreadMarker, draw: &Draw) {
    if !Timer.is_enabled(marker) {
        return;
    }

    if !BXT_HUD_TIMER.as_bool(marker) {
        return;
    }

    let time = TIME.borrow(marker).get_time();

    let hour = (time / 3600.).floor() as u32;
    let minute = (time / 60.).floor() as u32 % 60;
    let second = time.floor() as u32 % 60;
    let decimals = (time.fract() * 1e3).floor() as u32;

    // hardcoded color
    let color = glam::IVec3::new(255, 180, 30);

    let anchor = BXT_HUD_TIMER_ANCHOR.to_string(marker);
    let anchor: Vec<f32> = anchor
        .split_ascii_whitespace()
        .filter_map(|x| x.parse::<f32>().ok())
        .collect();

    if anchor.len() != 2 {
        return;
    }

    let screen_info = screen_info(marker);
    let Some(mut sprite_line) = draw.sprite_digit_line(
        (
            (screen_info.iWidth as f32 * anchor[0]) as i32,
            (screen_info.iHeight as f32 * anchor[1]) as i32,
        ),
        color,
    ) else {
        return;
    };

    if hour != 0 {
        sprite_line.number_pad_zero(hour, 1).colon();
    }

    if minute != 0 || hour != 0 {
        sprite_line.number_pad_zero(minute, 2).colon();
    }

    // always draw seconds and decimals
    sprite_line
        .number_pad_zero(second, 2)
        .decimal_separator()
        .number_pad_zero(decimals, 3);
}
