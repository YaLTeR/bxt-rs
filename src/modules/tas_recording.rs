//! `TAS Recording`

use std::borrow::Cow;
use std::convert::TryInto;
use std::ffi::CStr;
use std::fs::File;
use std::mem;
use std::os::raw::c_char;
use std::path::PathBuf;

use hltas::HLTAS;

use super::Module;
use crate::ffi::buttons::Buttons;
use crate::ffi::usercmd::usercmd_s;
use crate::handler;
use crate::hooks::engine::{self, con_print};
use crate::hooks::server;
use crate::modules::commands::{self, Command};
use crate::utils::*;

pub struct TasRecording;
impl Module for TasRecording {
    fn name(&self) -> &'static str {
        "TAS Recording"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_TAS_RECORDING_START, &BXT_TAS_RECORDING_STOP];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker)
            && engine::CL_Move.is_set(marker)
            && engine::cls.is_set(marker)
            && engine::frametime_remainder.is_set(marker)
            && engine::host_frametime.is_set(marker)
            && engine::SV_Frame.is_set(marker)
            && engine::sv.is_set(marker)
    }
}

static BXT_TAS_RECORDING_START: Command = Command::new(
    b"bxt_tas_recording_start\0",
    handler!(
        "Usage: bxt_tas_recording_start <filename.hltas>\n \
          Starts recording gameplay into a HLTAS script.\n",
        tas_recording_start as fn(_, _)
    ),
);

static BXT_TAS_RECORDING_STOP: Command = Command::new(
    b"bxt_tas_recording_stop\0",
    handler!(
        "Usage: bxt_tas_recording_stop\n \
          Stops gameplay recording.\n",
        tas_recording_stop as fn(_)
    ),
);

enum State {
    Idle,
    Recording(Recorder),
}

#[derive(Default)]
struct Recorder {
    hltas: HLTAS<'static>,
    filename: PathBuf,
    pending_frame_times: Vec<f64>,
    pending_remainders: Vec<f64>,
    pending_bound_commands: Vec<String>,
    pending_console_commands: Vec<String>,
    keys: Keys,
    last_cmd_was_zero_ms: bool,
    was_loading: bool,
    last_shared_seed_before_load: u32,
}

static STATE: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);

#[derive(Debug, Default, Clone, Copy)]
struct Key {
    state: u8,
}

impl Key {
    fn down(&mut self) {
        self.state |= 1 + 2;
    }

    fn up(&mut self) {
        self.state = 4;
    }

    fn is_down(self) -> bool {
        (self.state & 1) != 0
    }

    fn update(&mut self, down: bool) {
        if down && !self.is_down() {
            self.down();
        } else if !down && self.is_down() {
            self.up();
        }
    }

    fn clear_impulses(&mut self) {
        self.state &= !(2 + 4);
    }

    fn multiplier(self) -> f64 {
        if (self.state & 2) != 0 {
            if (self.state & 4) != 0 {
                0.75
            } else {
                0.5
            }
        } else {
            1.
        }
    }
}

#[derive(Debug, Default)]
struct Keys {
    forward: Key,
    back: Key,
    left: Key,
    right: Key,
}

impl Keys {
    fn clear_impulses(&mut self) {
        self.forward.clear_impulses();
        self.back.clear_impulses();
        self.left.clear_impulses();
        self.right.clear_impulses();
    }
}

fn tas_recording_start(marker: MainThreadMarker, filename: PathBuf) {
    if !TasRecording.is_enabled(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    if matches!(*state, State::Idle) {
        *state = State::Recording(Recorder {
            filename,
            ..Default::default()
        });
        con_print(marker, "Recording started\n");
    } else {
        con_print(marker, "Already recording\n");
    }
}

fn tas_recording_stop(marker: MainThreadMarker) {
    if !TasRecording.is_enabled(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    if let State::Recording(Recorder {
        hltas, filename, ..
    }) = mem::replace(&mut *state, State::Idle)
    {
        let file = match File::create(filename) {
            Ok(file) => file,
            Err(err) => {
                con_print(marker, &format!("Error opening the output file: {}\n", err));
                return;
            }
        };

        if let Err(err) = hltas.to_writer(file) {
            con_print(
                marker,
                &format!("Error writing to the output file: {}\n", err),
            );
        }

        con_print(marker, "Recording stopped\n");
    } else {
        con_print(marker, "No recording in progress\n");
    }
}

pub unsafe fn on_cl_move(marker: MainThreadMarker) {
    if !server::CmdStart.is_set(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    let recorder = match &mut *state {
        State::Recording(recorder) => recorder,
        State::Idle => return,
    };

    let client_state = (*engine::cls.get(marker)).state;
    if client_state != 4 && client_state != 5 {
        return;
    }

    recorder
        .pending_remainders
        .push(*engine::frametime_remainder.get(marker));
}

pub unsafe fn on_sv_frame_start(marker: MainThreadMarker) {
    if !server::CmdStart.is_set(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    let recorder = match &mut *state {
        State::Recording(recorder) => recorder,
        State::Idle => return,
    };

    let client_state = (*engine::cls.get(marker)).state;
    if client_state != 4 && client_state != 5 {
        return;
    }

    recorder
        .pending_frame_times
        .push(*engine::host_frametime.get(marker));

    recorder
        .pending_console_commands
        .push(recorder.pending_bound_commands.join(";"));
    recorder.pending_bound_commands.clear();
}

pub unsafe fn on_cmd_start(marker: MainThreadMarker, cmd: usercmd_s, random_seed: u32) {
    let mut state = STATE.borrow_mut(marker);
    let recorder = match &mut *state {
        State::Recording(recorder) => recorder,
        State::Idle => return,
    };

    if recorder.hltas.properties.seeds.is_none() {
        recorder.hltas.properties.seeds = Some(hltas::types::Seeds {
            shared: random_seed,
            non_shared: 1337,
        });
    }

    if let Some(hltas::types::Line::FrameBulk(last_frame_bulk)) = recorder.hltas.lines.last_mut() {
        if last_frame_bulk.frame_time == "" && cmd.msec != 0 && !recorder.last_cmd_was_zero_ms {
            // This command is a part of a command-split sequence that we already made a frame bulk
            // for.
            return;
        }
    }

    let is_paused = *engine::sv.get(marker).offset(4).cast();
    if is_paused {
        // TODO: pauses which aren't loads.
        recorder.was_loading = true;
        return;
    }

    if recorder.was_loading {
        // Loads can vary in length, thus record the seed change.
        recorder.hltas.lines.push(hltas::types::Line::SharedSeed(
            random_seed - recorder.last_shared_seed_before_load,
        ));
    }

    recorder.last_cmd_was_zero_ms = cmd.msec == 0;
    recorder.was_loading = false;
    recorder.last_shared_seed_before_load = random_seed;

    let mut frame_bulk = hltas::types::FrameBulk {
        auto_actions: Default::default(),
        movement_keys: Default::default(),
        action_keys: Default::default(),
        frame_time: Default::default(), // Will be set in on_sv_frame_end().
        pitch: Default::default(),
        frame_count: 1.try_into().unwrap(),
        console_command: Default::default(),
    };

    let buttons = Buttons::from_bits_truncate(cmd.buttons);

    recorder
        .keys
        .forward
        .update(buttons.contains(Buttons::IN_FORWARD));
    recorder
        .keys
        .back
        .update(buttons.contains(Buttons::IN_BACK));
    recorder
        .keys
        .left
        .update(buttons.contains(Buttons::IN_MOVELEFT));
    recorder
        .keys
        .right
        .update(buttons.contains(Buttons::IN_MOVERIGHT));

    if buttons.contains(Buttons::IN_FORWARD) {
        frame_bulk.movement_keys.forward = true;
    }
    if buttons.contains(Buttons::IN_BACK) {
        frame_bulk.movement_keys.back = true;
    }
    if buttons.contains(Buttons::IN_MOVELEFT) {
        frame_bulk.movement_keys.left = true;
    }
    if buttons.contains(Buttons::IN_MOVERIGHT) {
        frame_bulk.movement_keys.right = true;
    }
    if buttons.contains(Buttons::IN_JUMP) {
        frame_bulk.action_keys.jump = true;
    }
    if buttons.contains(Buttons::IN_DUCK) {
        frame_bulk.action_keys.duck = true;
    }
    if buttons.contains(Buttons::IN_USE) {
        frame_bulk.action_keys.use_ = true;
    }
    if buttons.contains(Buttons::IN_ATTACK) {
        frame_bulk.action_keys.attack_1 = true;
    }
    if buttons.contains(Buttons::IN_ATTACK2) {
        frame_bulk.action_keys.attack_2 = true;
    }
    if buttons.contains(Buttons::IN_RELOAD) {
        frame_bulk.action_keys.reload = true;
    }
    frame_bulk.auto_actions.movement = Some(hltas::types::AutoMovement::SetYaw(cmd.viewangles[1]));
    frame_bulk.pitch = Some(cmd.viewangles[0]);

    let mut commands = Vec::new();

    // Handle different combinations of *move and buttons. HLTAS cannot quite do any unusual actions
    // (e.g. left and right down at once with non-zero sidemove), so filter those out for now.
    if cmd.forwardmove != 0. || (frame_bulk.movement_keys.forward || frame_bulk.movement_keys.back)
    {
        if frame_bulk.movement_keys.forward && frame_bulk.movement_keys.back {
            if cmd.forwardmove > 0. {
                frame_bulk.movement_keys.back = false;
                recorder.keys.back.update(false);
                commands.push(format!(
                    "cl_forwardspeed {}",
                    cmd.forwardmove as f64 / recorder.keys.forward.multiplier()
                ));
            } else {
                frame_bulk.movement_keys.forward = false;
                recorder.keys.forward.update(false);
                commands.push(format!(
                    "cl_backspeed {}",
                    -cmd.forwardmove as f64 / recorder.keys.back.multiplier()
                ));
            }
        } else if frame_bulk.movement_keys.back {
            commands.push(format!(
                "cl_backspeed {}",
                -cmd.forwardmove as f64 / recorder.keys.back.multiplier()
            ));
        } else {
            frame_bulk.movement_keys.forward = true;
            recorder.keys.forward.update(true);
            commands.push(format!(
                "cl_forwardspeed {}",
                cmd.forwardmove as f64 / recorder.keys.forward.multiplier()
            ));
        }
    }

    if cmd.sidemove != 0. || (frame_bulk.movement_keys.right || frame_bulk.movement_keys.left) {
        if frame_bulk.movement_keys.right && frame_bulk.movement_keys.left {
            if cmd.sidemove > 0. {
                frame_bulk.movement_keys.left = false;
                recorder.keys.left.update(false);
                commands.push(format!(
                    "cl_sidespeed {}",
                    cmd.sidemove as f64 / recorder.keys.right.multiplier()
                ));
            } else {
                frame_bulk.movement_keys.right = false;
                recorder.keys.right.update(false);
                commands.push(format!(
                    "cl_sidespeed {}",
                    -cmd.sidemove as f64 / recorder.keys.left.multiplier()
                ));
            }
        } else if frame_bulk.movement_keys.left {
            commands.push(format!(
                "cl_sidespeed {}",
                -cmd.sidemove as f64 / recorder.keys.left.multiplier()
            ));
        } else {
            frame_bulk.movement_keys.right = true;
            recorder.keys.right.update(true);
            commands.push(format!(
                "cl_sidespeed {}",
                cmd.sidemove as f64 / recorder.keys.right.multiplier()
            ));
        }
    }

    // TODO: upmove.
    // TODO: non-shared RNG.
    // TODO: confirming selection in invnext, invprev.

    frame_bulk.console_command = Some(Cow::Owned(commands.join(";")));

    recorder
        .hltas
        .lines
        .push(hltas::types::Line::FrameBulk(frame_bulk));

    recorder.keys.clear_impulses();
}

pub unsafe fn on_sv_frame_end(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    let recorder = match &mut *state {
        State::Recording(recorder) => recorder,
        State::Idle => return,
    };

    // With 0 ms frames, we might have built up a few "unused" frame times and a few frame bulks
    // with empty frame times to fill. Fill the frame times starting from the end and discard the
    // rest.
    let mut had_cmd = false;
    for frame_bulk in recorder
        .hltas
        .lines
        .iter_mut()
        .rev()
        .filter_map(|line| {
            if let hltas::types::Line::FrameBulk(frame_bulk) = line {
                Some(frame_bulk)
            } else {
                None
            }
        })
        .take_while(|frame_bulk| frame_bulk.frame_time == "")
    {
        had_cmd = true;

        frame_bulk.frame_time = Cow::Owned(
            recorder
                .pending_frame_times
                .pop()
                .expect("unexpected more commands than physics frames")
                .to_string(),
        );

        let console_command = frame_bulk.console_command.as_mut().unwrap().to_mut();
        if !console_command.is_empty() {
            console_command.push(';');
        }
        console_command.push_str(&format!(
            "_bxt_set_frametime_remainder {}",
            recorder
                .pending_remainders
                .pop()
                .expect("unexpected more commands than frame time remainders"),
        ));

        let player_command = recorder
            .pending_console_commands
            .pop()
            .expect("unexpected more commands than console commands");
        if !player_command.is_empty() {
            console_command.push(';');
            console_command.push_str(&player_command);
        }
    }

    if had_cmd {
        recorder.pending_frame_times.clear();
        recorder.pending_console_commands.clear();
        recorder.pending_remainders.clear();
    }
}

static INSIDE_KEY_EVENT: MainThreadCell<bool> = MainThreadCell::new(false);

pub fn on_key_event_start(marker: MainThreadMarker) {
    INSIDE_KEY_EVENT.set(marker, true);
}

pub fn on_key_event_end(marker: MainThreadMarker) {
    INSIDE_KEY_EVENT.set(marker, false);
}

pub unsafe fn on_cbuf_addtext(marker: MainThreadMarker, text: *const c_char) {
    if !INSIDE_KEY_EVENT.get(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    let recorder = match &mut *state {
        State::Recording(recorder) => recorder,
        State::Idle => return,
    };

    let text = match CStr::from_ptr(text).to_str() {
        Ok(text) => text,
        Err(_) => return,
    };

    let text = text.trim_end_matches(&['\n', ';'][..]);
    if text.is_empty() {
        return;
    }

    // Ignore commands that we handle with frame bulk inputs.
    if matches!(text.as_bytes()[0], b'+' | b'-') {
        for prefix in [
            "forward ",
            "back ",
            "moveright ",
            "moveleft ",
            "moveup ",
            "movedown ",
            "jump ",
            "duck ",
            "use ",
            "attack ",
            "attack2 ",
            "reload ",
            "left ",
            "right ",
            "lookup ",
            "lookdown ",
        ] {
            if text[1..].starts_with(prefix) {
                return;
            }
        }
    }

    recorder.pending_bound_commands.push(text.to_string());
}
