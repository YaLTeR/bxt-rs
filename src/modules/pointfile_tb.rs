//! `Real-time pointfile`

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{BufWriter, Seek, Write};
use std::thread;

use super::Module;
use crate::handler;
use crate::hooks::engine::{self, con_print, player_edict};
use crate::modules::commands::{self, Command};
use crate::modules::cvars::{self, CVar};
use crate::utils::*;

pub struct PointfileTB;
impl Module for PointfileTB {
    fn name(&self) -> &'static str {
        "Real-time pointfile"
    }

    fn description(&self) -> &'static str {
        "Generating a Quake/TrenchBroom pointfile from player's position in real time"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &BXT_POINTFILE_START,
            &BXT_POINTFILE_STOP,
            &BXT_POINTFILE_CLEAR,
        ];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_POINTFILE_MAX_LINE, &BXT_POINTFILE_FREQUENCY];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker)
            && cvars::CVars.is_enabled(marker)
            && engine::host_frametime.is_set(marker)
            && engine::svs.is_set(marker) // player_edict()
            && engine::sv.is_set(marker) // is_paused
            && engine::cls.is_set(marker)
    }
}

type Vec3 = [f32; 3];
struct PointFileData {
    data: VecDeque<Vec3>,
    max_size: usize,
    frequency: f32,
}

impl PointFileData {
    fn new(start_point: Vec3) -> Self {
        let mut data = VecDeque::new();

        // needs at least 2 points in a point file so TB can load it
        data.push_back(start_point);
        data.push_back(start_point);

        Self {
            data,
            max_size: 3000,
            frequency: 10.,
        }
    }

    fn process_new_point(&mut self, point: Vec3) {
        let curr = glam::Vec3::from_array(point);
        let last = self
            .data
            .iter()
            .last()
            .cloned()
            .map(glam::Vec3::from_array)
            .unwrap_or(glam::Vec3::ZERO);
        let d = curr.distance(last);

        // same position, don't write anything
        if d == 0. {
            return;
        }

        // if the player is moving at 2000ups at 100fps, it is 20 unit per frame.
        // so, if velocity is greater than XXX coded number, it is assumed to be a teleport
        // maybe there is a better way but lower frequency/fps doesn't give enough sample
        let is_teleport = d * self.frequency > 1500.;
        const MAXIMUM_CLOSE_DISTANCE: f32 = 20.;

        if is_teleport {
            let closest_point = self
                .data
                .iter()
                // use reverse because  we only have to repeat since the last teleport
                // if not, basically checkpoint branches are repeated
                .rposition(|x| {
                    let v = glam::Vec3::from_slice(x.as_slice());

                    v.distance(curr) <= MAXIMUM_CLOSE_DISTANCE
                });

            if let Some(closest_point) = closest_point {
                self.repeat_from_reverse(closest_point);
            }
        }

        self.insert_point(point);
        self.truncate();
    }

    fn insert_point(&mut self, point: Vec3) {
        self.data.push_back(point);
    }

    fn repeat_from_reverse(&mut self, pos: usize) {
        let slice_to_extend: Vec<[f32; 3]> = self.data.iter().skip(pos).rev().cloned().collect();

        self.data.extend(slice_to_extend);
    }

    fn truncate(&mut self) {
        while self.data.len() > self.max_size {
            self.data.pop_front();
        }
    }

    fn clear(&mut self) {
        let last_point = self.data.pop_back();

        self.data.clear();

        // then add 2 points......
        if let Some(last_point) = last_point {
            self.insert_point(last_point);
            self.insert_point(last_point);
        } else {
            self.insert_point([0f32; 3]);
            self.insert_point([0f32; 3]);
        }
    }
}

enum PointFileWriteThreadMessage {
    ProcessNewPoint {
        point: Vec3,
        frequency: f32,
        max_size: usize,
    },
    Clear,
    GoDie,
}

enum State {
    Idle,
    Recording {
        writer: std::sync::mpsc::Sender<PointFileWriteThreadMessage>,
    },
}

static STATE: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);
static TIME: MainThreadCell<f64> = MainThreadCell::new(0.);
static TIME_2: MainThreadCell<f64> = MainThreadCell::new(0.);

static BXT_POINTFILE_MAX_LINE: CVar = CVar::new(
    b"bxt_pointfile_max_line\0",
    b"3000\0",
    "Number of max lines allowed in a pointfile. Older lines will be removed if the limit is exceeded.",
);

static BXT_POINTFILE_FREQUENCY: CVar = CVar::new(
    b"bxt_pointfile_frequency\0",
    b"10\0",
    "Number of pointfile entries added per second.",
);

static BXT_POINTFILE_START: Command = Command::new(
    b"bxt_pointfile_start\0",
    handler!(
        "bxt_pointfile_start

Starts recording a pointfile",
        pointfile_start as fn(_),
        pointfile_start_with_name as fn(_, _)
    ),
);

static BXT_POINTFILE_STOP: Command = Command::new(
    b"bxt_pointfile_stop\0",
    handler!(
        "bxt_pointfile_stop

Stop recording a pointfile",
        pointfile_stop as fn(_)
    ),
);

static BXT_POINTFILE_CLEAR: Command = Command::new(
    b"bxt_pointfile_clear\0",
    handler!(
        "bxt_pointfile_clear

Clears the content inside the currently recorded pointfile",
        pointfile_clear as fn(_)
    ),
);

fn pointfile_start(marker: MainThreadMarker) {
    pointfile_start_with_name(marker, "pointfile.pts".to_string());
}

fn pointfile_start_with_name(marker: MainThreadMarker, file_name: String) {
    if !PointfileTB.is_enabled(marker) {
        return;
    }

    if !file_name.ends_with(".pts") {
        con_print(marker, "Error: File name must end with \".pts\".\n");
        return;
    }

    if !matches!(*STATE.borrow_mut(marker), State::Idle) {
        con_print(marker, "Error: Currently recording a pointfile");
        return;
    }

    con_print(
        marker,
        &format!("Recording player motion into {}.\n", &file_name),
    );

    reset(marker);

    // nice to have player start point if possible
    let start_point = get_player_pos(marker).unwrap_or([0f32; 3]);

    // use a different thread to write file so we don't lag on the main thread
    // second thread will do lots of IO work, poor second thread
    let (tx, rx) = std::sync::mpsc::channel();

    let Ok(file) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&file_name)
    else {
        con_print(marker, &format!("Cannot create file {}.\n", &file_name));
        return;
    };

    thread::spawn(move || {
        file_writer_thread(file, start_point, rx);
    });

    *STATE.borrow_mut(marker) = State::Recording { writer: tx };
}

fn pointfile_stop(marker: MainThreadMarker) {
    reset(marker);
}

fn pointfile_clear(marker: MainThreadMarker) {
    let State::Recording { ref writer } = *STATE.borrow_mut(marker) else {
        return;
    };

    let _ = writer.send(PointFileWriteThreadMessage::Clear);
}

fn reset(marker: MainThreadMarker) {
    TIME.set(marker, 0.);
    TIME_2.set(marker, 0.);

    if let State::Recording { ref mut writer } = *STATE.borrow_mut(marker) {
        let _ = writer.send(PointFileWriteThreadMessage::GoDie);
    }

    *STATE.borrow_mut(marker) = State::Idle;
}

pub fn update_time(marker: MainThreadMarker) {
    if !PointfileTB.is_enabled(marker) {
        return;
    }

    let is_paused: bool = unsafe { *engine::sv.get(marker).offset(4).cast() };

    // dont update the point file if we are in pause
    if is_paused {
        return;
    }

    TIME.set(
        marker,
        TIME.get(marker) + unsafe { *engine::host_frametime.get(marker) },
    );
}

fn file_writer_thread(
    file: std::fs::File,
    start_point: Vec3,
    rx: std::sync::mpsc::Receiver<PointFileWriteThreadMessage>,
) {
    let mut file = BufWriter::new(file);
    let mut entries = PointFileData::new(start_point);

    let mut last_time = std::time::Instant::now();
    let mut should_write = false; // enable
    const WRITE_FREQUENCY: f32 = 1.; // write every x second(s)

    loop {
        match rx.try_recv() {
            Ok(message) => match message {
                PointFileWriteThreadMessage::ProcessNewPoint {
                    point,
                    frequency,
                    max_size,
                } => {
                    entries.frequency = frequency;
                    entries.max_size = max_size;
                    entries.process_new_point(point);
                    should_write = true;
                }
                PointFileWriteThreadMessage::Clear => {
                    entries.clear();
                    should_write = true;
                }
                PointFileWriteThreadMessage::GoDie => {
                    let _ = file.flush();
                    break;
                }
            },
            Err(err) => match err {
                std::sync::mpsc::TryRecvError::Empty => (),
                std::sync::mpsc::TryRecvError::Disconnected => {
                    // have to handle this explicitly
                    break;
                }
            },
        }

        let now_time = std::time::Instant::now();

        if should_write
            && now_time.duration_since(last_time)
                >= std::time::Duration::from_secs_f32(WRITE_FREQUENCY)
        {
            // truncate on every write
            let _ = file.get_mut().set_len(0);
            let _ = file.seek(std::io::SeekFrom::Start(0));

            for entry in &entries.data {
                let _ = writeln!(file, "{} {} {}", entry[0], entry[1], entry[2]);
            }

            let _ = file.flush();

            should_write = false;
            last_time = now_time;
        }
    }
}

pub fn capture_point(marker: MainThreadMarker) {
    let State::Recording { ref mut writer } = *STATE.borrow_mut(marker) else {
        return;
    };

    // dont record if in main menu
    if (unsafe { &*engine::cls.get(marker) }).state != 5 {
        return;
    }

    // not the time to record
    let time = TIME.get(marker);
    let time_2 = TIME_2.get(marker);

    if time < time_2 {
        return;
    }

    let max_size = BXT_POINTFILE_MAX_LINE.as_u64(marker) as usize;
    let frequency = BXT_POINTFILE_FREQUENCY.as_f32(marker);

    // it is the time to record, increment the timer
    TIME_2.set(marker, time_2 + (1. / frequency) as f64);

    let Some(player_pos) = get_player_pos(marker) else {
        return;
    };

    let _ = writer.send(PointFileWriteThreadMessage::ProcessNewPoint {
        point: player_pos,
        frequency,
        max_size,
    });
}

fn get_player_pos(marker: MainThreadMarker) -> Option<Vec3> {
    let player = unsafe { player_edict(marker) }?;
    let origin = unsafe { player.as_ref().v.origin };

    Some(origin)
}
