use std::cmp::min;
use std::num::NonZeroU32;

use hltas::types::{FrameBulk, Line};
use hltas::HLTAS;
use serde::{Deserialize, Serialize};

use super::utils::{line_first_frame_idx, line_first_frame_idx_and_frame_count};
use crate::modules::tas_studio::editor::utils::{
    bulk_and_first_frame_idx_mut, line_idx_and_repeat_at_frame, FrameBulkExt,
    MaxAccelOffsetValuesMut,
};

// This enum is stored in a SQLite DB as bincode bytes. All changes MUST BE BACKWARDS COMPATIBLE to
// be able to load old projects.
// Make sure that newer operations are added at the end of the enum.
/// A basic operation on a HLTAS.
///
/// All operations can be applied and undone. They therefore store enough information to be able to
/// do that. For example, [`SetFrameCount`] stores the original frame count together with the new
/// one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Operation {
    SetFrameCount {
        bulk_idx: usize,
        from: u32,
        to: u32,
    },
    SetYaw {
        bulk_idx: usize,
        from: f32,
        to: f32,
    },
    Delete {
        line_idx: usize,
        line: String,
    },
    Split {
        frame_idx: usize,
    },
    Replace {
        line_idx: usize,
        from: String,
        to: String,
    },
    ToggleKey {
        bulk_idx: usize,
        key: Key,
        to: bool,
    },
    Insert {
        line_idx: usize,
        line: String,
    },
    SetLeftRightCount {
        bulk_idx: usize,
        from: u32,
        to: u32,
    },
    Rewrite {
        from: String,
        to: String,
    },
    ReplaceMultiple {
        first_line_idx: usize,
        from: String,
        to: String,
    },
    SetAdjacentFrameCount {
        bulk_idx: usize,
        from: u32,
        to: u32,
    },
    SetAdjacentYaw {
        first_bulk_idx: usize,
        bulk_count: usize,
        from: f32,
        to: f32,
    },
    SetAdjacentLeftRightCount {
        first_bulk_idx: usize,
        bulk_count: usize,
        from: u32,
        to: u32,
    },
    SetYawspeed {
        bulk_idx: usize,
        from: f32,
        to: f32,
    },
    SetAdjacentYawspeed {
        first_bulk_idx: usize,
        bulk_count: usize,
        from: f32,
        to: f32,
    },
    SetFrameTime {
        bulk_idx: usize,
        from: String,
        to: String,
    },
    SetCommands {
        bulk_idx: usize,
        from: Option<String>,
        to: Option<String>,
    },
    SetMaxAccelOffsetStart {
        bulk_idx: usize,
        from: f32,
        to: f32,
    },
    SetMaxAccelOffsetTarget {
        bulk_idx: usize,
        from: f32,
        to: f32,
    },
    SetMaxAccelOffsetAccel {
        bulk_idx: usize,
        from: f32,
        to: f32,
    },
    SetMaxAccelOffsetStartAndTarget {
        bulk_idx: usize,
        from: (f32, f32),
        to: (f32, f32),
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Key {
    Forward,
    Left,
    Right,
    Back,
    Up,
    Down,

    Jump,
    Duck,
    Use,
    Attack1,
    Attack2,
    Reload,
}

// The semantics of apply() or undo() MUST NOT CHANGE, because that will break persistent undo/redo
// for old projects.
impl Operation {
    /// Applies operation to HLTAS and returns index of first affected frame.
    ///
    /// Returns `None` if all frames remain valid.
    pub fn apply(&self, hltas: &mut HLTAS) -> Option<usize> {
        match *self {
            Operation::SetFrameCount { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                assert_eq!(bulk.frame_count.get(), from, "wrong current frame count");

                if from != to {
                    bulk.frame_count = NonZeroU32::new(to).expect("invalid new frame count");
                    return Some(first_frame_idx + min(from, to) as usize);
                }
            }
            Operation::SetYaw { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let yaw = bulk.yaw_mut().expect("frame bulk should have yaw");
                assert_eq!(*yaw, from, "wrong current yaw");

                if *yaw != to {
                    *yaw = to;
                    return Some(first_frame_idx);
                }
            }
            Operation::Delete { line_idx, .. } => {
                let first_frame_idx = line_first_frame_idx(hltas)
                    .nth(line_idx)
                    .expect("invalid line index");

                hltas.lines.remove(line_idx);
                return Some(first_frame_idx);
            }
            Operation::Split { frame_idx } => {
                let (line_idx, repeat) = line_idx_and_repeat_at_frame(&hltas.lines, frame_idx)
                    .expect("invalid frame index");

                assert!(repeat > 0, "repeat should be above 0");

                let bulk = hltas.lines[line_idx].frame_bulk_mut().unwrap();
                let mut new_bulk = bulk.clone();
                new_bulk.frame_count = NonZeroU32::new(bulk.frame_count.get() - repeat)
                    .expect("frame bulk should have more than 1 repeat");
                bulk.frame_count = NonZeroU32::new(repeat).unwrap();

                hltas.lines.insert(line_idx + 1, Line::FrameBulk(new_bulk));

                // Splitting does not invalidate any frames.
            }
            Operation::Replace {
                line_idx, ref to, ..
            } => {
                let to = hltas::read::line(to).expect("line should be parse-able").1;

                let first_frame_idx = line_first_frame_idx(hltas)
                    .nth(line_idx)
                    .expect("invalid line index");

                hltas.lines[line_idx] = to;
                return Some(first_frame_idx);
            }
            Operation::ToggleKey { bulk_idx, key, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let value = key.value_mut(bulk);
                assert_ne!(*value, to);
                *value = to;
                return Some(first_frame_idx);
            }
            Operation::Insert { line_idx, ref line } => {
                let line = hltas::read::line(line)
                    .expect("line should be parse-able")
                    .1;

                hltas.lines.insert(line_idx, line);

                let first_frame_idx = line_first_frame_idx(hltas)
                    .nth(line_idx)
                    .expect("invalid line index");

                return Some(first_frame_idx);
            }
            Operation::SetLeftRightCount { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let count = bulk
                    .left_right_count_mut()
                    .expect("frame bulk should have left-right count");
                assert_eq!(count.get(), from, "wrong current left-right count");

                if from != to {
                    *count = NonZeroU32::new(to).expect("invalid new left-right count");
                    return Some(first_frame_idx);
                }
            }
            Operation::Rewrite { ref to, .. } => {
                let to = HLTAS::from_str(to).expect("script should be parse-able");
                *hltas = to;
                return Some(1);
            }
            Operation::ReplaceMultiple {
                first_line_idx,
                ref from,
                ref to,
            } => {
                let from = hltas::read::all_consuming_lines(from)
                    .expect("lines should be parse-able")
                    .1;
                let to = hltas::read::all_consuming_lines(to)
                    .expect("lines should be parse-able")
                    .1;

                let first_frame_idx = line_first_frame_idx_and_frame_count(hltas)
                    .nth(first_line_idx)
                    .expect("invalid line index");

                hltas
                    .lines
                    .splice(first_line_idx..first_line_idx + from.len(), to);
                return Some(first_frame_idx);
            }
            Operation::SetAdjacentFrameCount { bulk_idx, from, to } => {
                let mut bulks = bulk_and_first_frame_idx_mut(hltas).skip(bulk_idx);
                let (bulk, first_frame_idx) = bulks.next().expect("invalid bulk index");
                let (next_bulk, _) = bulks.next().expect("invalid bulk index");
                drop(bulks);

                assert_eq!(bulk.frame_count.get(), from, "wrong current frame count");

                if from != to {
                    bulk.frame_count = NonZeroU32::new(to).expect("invalid new frame count");

                    let delta = from as i64 - to as i64;
                    next_bulk.frame_count =
                        NonZeroU32::new((next_bulk.frame_count.get() as i64 + delta) as u32)
                            .expect("invalid new frame count");

                    return Some(first_frame_idx + min(from, to) as usize);
                }
            }
            Operation::SetAdjacentYaw {
                first_bulk_idx,
                bulk_count,
                from,
                to,
            } => {
                let mut bulks = bulk_and_first_frame_idx_mut(hltas).skip(first_bulk_idx);
                let (bulk, first_frame_idx) = bulks.next().expect("invalid bulk index");

                let yaw = bulk.yaw_mut().expect("frame bulk should have yaw");
                assert_eq!(*yaw, from, "wrong current yaw");

                if *yaw != to {
                    for _ in 1..bulk_count {
                        let bulk = bulks.next().expect("invalid bulk index").0;
                        let next_yaw = bulk.yaw_mut().expect("frame bulk should have yaw");
                        assert_eq!(*next_yaw, from, "wrong current yaw");
                        *next_yaw = to;
                    }

                    *yaw = to;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetAdjacentLeftRightCount {
                first_bulk_idx,
                bulk_count,
                from,
                to,
            } => {
                let mut bulks = bulk_and_first_frame_idx_mut(hltas).skip(first_bulk_idx);
                let (bulk, first_frame_idx) = bulks.next().expect("invalid bulk index");

                let count = bulk
                    .left_right_count_mut()
                    .expect("frame bulk should have left-right count");
                assert_eq!(count.get(), from, "wrong current left-right count");

                if from != to {
                    let to = NonZeroU32::new(to).expect("invalid new left-right count");

                    for _ in 1..bulk_count {
                        let bulk = bulks.next().expect("invalid bulk index").0;
                        let next_count = bulk
                            .left_right_count_mut()
                            .expect("frame bulk should have left-right count");
                        assert_eq!(next_count.get(), from, "wrong current left-right count");
                        *next_count = to;
                    }

                    *count = to;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetYawspeed { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let yawspeed = bulk
                    .yawspeed_mut()
                    .expect("frame bulk should have yawspeed");
                assert_eq!(from, *yawspeed, "wrong current yawspeed value");

                if from != to {
                    *yawspeed = to;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetAdjacentYawspeed {
                first_bulk_idx,
                bulk_count,
                from,
                to,
            } => {
                let mut bulks = bulk_and_first_frame_idx_mut(hltas).skip(first_bulk_idx);
                let (bulk, first_frame_idx) = bulks.next().expect("invalid bulk index");

                let yawspeed = bulk
                    .yawspeed_mut()
                    .expect("frame bulk should have yawspeed");
                assert_eq!(from, *yawspeed, "wrong current yawspeed value");

                if from != to {
                    for _ in 1..bulk_count {
                        let bulk = bulks.next().expect("invalid bulk index").0;
                        let next_yawspeed = bulk
                            .yawspeed_mut()
                            .expect("frame bulk should have yawspeed");
                        assert_eq!(from, *next_yawspeed, "wrong current yawspeed value");

                        *next_yawspeed = to;
                    }
                }

                *yawspeed = to;
                return Some(first_frame_idx);
            }
            Operation::SetMaxAccelOffsetStart { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let start = bulk
                    .max_accel_yaw_offset_mut()
                    .expect("frame bulk should have starting yaw offset")
                    .start;
                assert_eq!(from, *start, "wrong current starting yaw offset");

                if from != to {
                    *start = to;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetMaxAccelOffsetTarget { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let target = bulk
                    .max_accel_yaw_offset_mut()
                    .expect("frame bulk should have target yaw offset")
                    .target;
                assert_eq!(from, *target, "wrong current target yaw offset");

                if from != to {
                    *target = to;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetMaxAccelOffsetAccel { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let accel = bulk
                    .max_accel_yaw_offset_mut()
                    .expect("frame bulk should have yaw acceleration")
                    .accel;
                assert_eq!(from, *accel, "wrong current yaw acceleration");

                if from != to {
                    *accel = to;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetMaxAccelOffsetStartAndTarget { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let MaxAccelOffsetValuesMut { start, target, .. } = bulk
                    .max_accel_yaw_offset_mut()
                    .expect("frame bulk should have starting and target yaw offset");

                assert_eq!(
                    from,
                    (*start, *target),
                    "wrong current starting and target yaw offset"
                );

                if from != to {
                    (*start, *target) = to;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetFrameTime {
                bulk_idx,
                ref from,
                ref to,
            } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");
                assert_eq!(&bulk.frame_time, from, "wrong current frame time");

                if from != to {
                    bulk.frame_time = to.to_string();
                    return Some(first_frame_idx);
                }
            }
            Operation::SetCommands {
                bulk_idx,
                ref from,
                ref to,
            } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");
                assert_eq!(&bulk.console_command, from, "wrong current commands");

                if from != to {
                    bulk.console_command = to.clone();
                    return Some(first_frame_idx);
                }
            }
        }

        None
    }

    /// Undoes operation on HLTAS and returns index of first affected frame.
    ///
    /// Returns `None` if all frames remain valid.
    pub fn undo(&self, hltas: &mut HLTAS) -> Option<usize> {
        match *self {
            Operation::SetFrameCount { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                assert_eq!(bulk.frame_count.get(), to, "wrong current frame count");

                if from != to {
                    bulk.frame_count = NonZeroU32::new(from).expect("invalid original frame count");
                    return Some(first_frame_idx + min(from, to) as usize);
                }
            }
            Operation::SetYaw { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let yaw = bulk.yaw_mut().expect("frame bulk should have yaw");
                assert_eq!(*yaw, to, "wrong current yaw");

                if *yaw != from {
                    *yaw = from;
                    return Some(first_frame_idx);
                }
            }
            Operation::Delete { line_idx, ref line } => {
                let line = hltas::read::line(line)
                    .expect("line should be parse-able")
                    .1;

                hltas.lines.insert(line_idx, line);

                let first_frame_idx = line_first_frame_idx(hltas)
                    .nth(line_idx)
                    .expect("invalid line index");

                return Some(first_frame_idx);
            }
            Operation::Split { frame_idx } => {
                let (line_idx, repeat) = line_idx_and_repeat_at_frame(&hltas.lines, frame_idx)
                    .expect("invalid frame index");

                assert_eq!(repeat, 0, "current repeat should be 0");
                assert!(line_idx > 0, "line index should be above 0");

                let prev_bulk = match hltas.lines.remove(line_idx - 1) {
                    Line::FrameBulk(prev_bulk) => prev_bulk,
                    _ => panic!("previous line should be frame bulk"),
                };
                let bulk = hltas.lines[line_idx - 1].frame_bulk_mut().unwrap();
                bulk.frame_count = bulk
                    .frame_count
                    .checked_add(prev_bulk.frame_count.get())
                    .expect("combined frame count should fit");

                // Merging equal frame bulks (undoing a split) does not invalidate any frames.
            }
            Operation::Replace {
                line_idx, ref from, ..
            } => {
                let from = hltas::read::line(from)
                    .expect("line should be parse-able")
                    .1;

                let first_frame_idx = line_first_frame_idx(hltas)
                    .nth(line_idx)
                    .expect("invalid line index");

                hltas.lines[line_idx] = from;
                return Some(first_frame_idx);
            }
            Operation::ToggleKey { bulk_idx, key, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let value = key.value_mut(bulk);
                assert_eq!(*value, to);
                *value = !to;
                return Some(first_frame_idx);
            }
            Operation::Insert { line_idx, .. } => {
                let first_frame_idx = line_first_frame_idx(hltas)
                    .nth(line_idx)
                    .expect("invalid line index");

                hltas.lines.remove(line_idx);
                return Some(first_frame_idx);
            }
            Operation::SetLeftRightCount { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let count = bulk
                    .left_right_count_mut()
                    .expect("frame bulk should have left-right count");
                assert_eq!(count.get(), to, "wrong current left-right count");

                if from != to {
                    *count = NonZeroU32::new(from).expect("invalid original left-right count");
                    return Some(first_frame_idx);
                }
            }
            Operation::Rewrite { ref from, .. } => {
                let from = HLTAS::from_str(from).expect("script should be parse-able");
                *hltas = from;
                return Some(1);
            }
            Operation::ReplaceMultiple {
                first_line_idx,
                ref from,
                ref to,
            } => {
                let from = hltas::read::all_consuming_lines(from)
                    .expect("lines should be parse-able")
                    .1;
                let to = hltas::read::all_consuming_lines(to)
                    .expect("lines should be parse-able")
                    .1;

                let first_frame_idx = line_first_frame_idx_and_frame_count(hltas)
                    .nth(first_line_idx)
                    .expect("invalid line index");

                hltas
                    .lines
                    .splice(first_line_idx..first_line_idx + to.len(), from);
                return Some(first_frame_idx);
            }
            Operation::SetAdjacentFrameCount { bulk_idx, from, to } => {
                let mut bulks = bulk_and_first_frame_idx_mut(hltas).skip(bulk_idx);
                let (bulk, first_frame_idx) = bulks.next().expect("invalid bulk index");
                let (next_bulk, _) = bulks.next().expect("invalid bulk index");
                drop(bulks);

                assert_eq!(bulk.frame_count.get(), to, "wrong current frame count");

                if from != to {
                    bulk.frame_count = NonZeroU32::new(from).expect("invalid original frame count");

                    let delta = from as i64 - to as i64;
                    next_bulk.frame_count =
                        NonZeroU32::new((next_bulk.frame_count.get() as i64 - delta) as u32)
                            .expect("invalid new frame count");

                    return Some(first_frame_idx + min(from, to) as usize);
                }
            }
            Operation::SetAdjacentYaw {
                first_bulk_idx,
                bulk_count,
                from,
                to,
            } => {
                let mut bulks = bulk_and_first_frame_idx_mut(hltas).skip(first_bulk_idx);
                let (bulk, first_frame_idx) = bulks.next().expect("invalid bulk index");

                let yaw = bulk.yaw_mut().expect("frame bulk should have yaw");
                assert_eq!(*yaw, to, "wrong current yaw");

                if *yaw != from {
                    for _ in 1..bulk_count {
                        let bulk = bulks.next().expect("invalid bulk index").0;
                        let next_yaw = bulk.yaw_mut().expect("frame bulk should have yaw");
                        assert_eq!(*next_yaw, to, "wrong current yaw");
                        *next_yaw = from;
                    }

                    *yaw = from;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetAdjacentLeftRightCount {
                first_bulk_idx,
                bulk_count,
                from,
                to,
            } => {
                let mut bulks = bulk_and_first_frame_idx_mut(hltas).skip(first_bulk_idx);
                let (bulk, first_frame_idx) = bulks.next().expect("invalid bulk index");

                let count = bulk
                    .left_right_count_mut()
                    .expect("frame bulk should have left-right count");
                assert_eq!(count.get(), to, "wrong current left-right count");

                if from != to {
                    let from = NonZeroU32::new(from).expect("invalid original left-right count");

                    for _ in 1..bulk_count {
                        let bulk = bulks.next().expect("invalid bulk index").0;
                        let next_count = bulk
                            .left_right_count_mut()
                            .expect("frame bulk should have left-right count");
                        assert_eq!(next_count.get(), to, "wrong current left-right count");
                        *next_count = from;
                    }

                    *count = from;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetYawspeed { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let yawspeed = bulk
                    .yawspeed_mut()
                    .expect("frame bulk should have yawspeed");

                assert_eq!(to, *yawspeed, "wrong current yawspeed value");

                if from != to {
                    *yawspeed = from;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetAdjacentYawspeed {
                first_bulk_idx,
                bulk_count,
                from,
                to,
            } => {
                let mut bulks = bulk_and_first_frame_idx_mut(hltas).skip(first_bulk_idx);
                let (bulk, first_frame_idx) = bulks.next().expect("invalid bulk index");

                let yawspeed = bulk
                    .yawspeed_mut()
                    .expect("frame bulk should have yawspeed");
                assert_eq!(to, *yawspeed, "wrong current yawspeed value");

                if from != to {
                    for _ in 1..bulk_count {
                        let bulk = bulks.next().expect("invalid bulk index").0;
                        let next_yawspeed = bulk
                            .yawspeed_mut()
                            .expect("frame bulk should have yawspeed");
                        assert_eq!(to, *next_yawspeed, "wrong current yawspeed value");

                        *next_yawspeed = from;
                    }
                }

                *yawspeed = from;
                return Some(first_frame_idx);
            }
            Operation::SetMaxAccelOffsetStart { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let start = bulk
                    .max_accel_yaw_offset_mut()
                    .expect("frame bulk should have starting yaw offset")
                    .start;
                assert_eq!(to, *start, "wrong current starting yaw offset");

                if from != to {
                    *start = from;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetMaxAccelOffsetTarget { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let target = bulk
                    .max_accel_yaw_offset_mut()
                    .expect("frame bulk should have target yaw offset")
                    .target;
                assert_eq!(to, *target, "wrong current target yaw offset");

                if from != to {
                    *target = from;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetMaxAccelOffsetAccel { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let accel = bulk
                    .max_accel_yaw_offset_mut()
                    .expect("frame bulk should have yaw acceleration")
                    .accel;
                assert_eq!(to, *accel, "wrong current yaw acceleration");

                if from != to {
                    *accel = from;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetMaxAccelOffsetStartAndTarget { bulk_idx, from, to } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");

                let MaxAccelOffsetValuesMut { start, target, .. } = bulk
                    .max_accel_yaw_offset_mut()
                    .expect("frame bulk should have starting and target yaw offset");

                assert_eq!(
                    to,
                    (*start, *target),
                    "wrong current starting and target yaw offset"
                );

                if from != to {
                    (*start, *target) = from;
                    return Some(first_frame_idx);
                }
            }
            Operation::SetFrameTime {
                bulk_idx,
                ref from,
                ref to,
            } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");
                assert_eq!(&bulk.frame_time, to, "wrong current frame time");

                if from != to {
                    bulk.frame_time = from.to_string();
                    return Some(first_frame_idx);
                }
            }
            Operation::SetCommands {
                bulk_idx,
                ref from,
                ref to,
            } => {
                let (bulk, first_frame_idx) = bulk_and_first_frame_idx_mut(hltas)
                    .nth(bulk_idx)
                    .expect("invalid bulk index");
                assert_eq!(&bulk.console_command, to, "wrong current commands");

                if from != to {
                    bulk.console_command = from.clone();
                    return Some(first_frame_idx);
                }
            }
        }

        None
    }
}

impl Key {
    pub fn value_mut(self, bulk: &mut FrameBulk) -> &mut bool {
        match self {
            Key::Forward => &mut bulk.movement_keys.forward,
            Key::Left => &mut bulk.movement_keys.left,
            Key::Right => &mut bulk.movement_keys.right,
            Key::Back => &mut bulk.movement_keys.back,
            Key::Up => &mut bulk.movement_keys.up,
            Key::Down => &mut bulk.movement_keys.down,

            Key::Jump => &mut bulk.action_keys.jump,
            Key::Duck => &mut bulk.action_keys.duck,
            Key::Use => &mut bulk.action_keys.use_,
            Key::Attack1 => &mut bulk.action_keys.attack_1,
            Key::Attack2 => &mut bulk.action_keys.attack_2,
            Key::Reload => &mut bulk.action_keys.reload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn check_op(input: &str, op: Operation, output: &str) {
        let header = "version 1\nframes\n";
        let input = HLTAS::from_str(&(header.to_string() + input)).unwrap();
        let output = HLTAS::from_str(&(header.to_string() + output)).unwrap();

        let mut modified = input.clone();
        assert_ne!(
            op.apply(&mut modified),
            Some(0),
            "initial frame should never be invalidated"
        );
        assert_eq!(modified, output, "apply produced wrong result");

        assert_ne!(
            op.undo(&mut modified),
            Some(0),
            "initial frame should never be invalidated"
        );
        assert_eq!(modified, input, "undo produced wrong result");
    }

    #[test]
    fn op_set_yaw() {
        check_op(
            "----------|------|------|0.004|10|-|6",
            Operation::SetYaw {
                bulk_idx: 0,
                from: 10.,
                to: 15.,
            },
            "----------|------|------|0.004|15|-|6",
        );
    }

    #[test]
    fn op_set_frame_count() {
        check_op(
            "----------|------|------|0.004|10|-|6",
            Operation::SetFrameCount {
                bulk_idx: 0,
                from: 6,
                to: 10,
            },
            "----------|------|------|0.004|10|-|10",
        );
    }

    #[test]
    fn op_set_left_right_count() {
        check_op(
            "s06-------|------|------|0.004|10|-|6",
            Operation::SetLeftRightCount {
                bulk_idx: 0,
                from: 10,
                to: 20,
            },
            "s06-------|------|------|0.004|20|-|6",
        );
        check_op(
            "s07-------|------|------|0.004|10|-|6",
            Operation::SetLeftRightCount {
                bulk_idx: 0,
                from: 10,
                to: 20,
            },
            "s07-------|------|------|0.004|20|-|6",
        );
    }

    #[test]
    fn op_split() {
        check_op(
            "----------|------|------|0.004|10|-|6",
            Operation::Split { frame_idx: 4 },
            "----------|------|------|0.004|10|-|4\n\
            ----------|------|------|0.004|10|-|2",
        );
    }

    #[test]
    fn op_delete() {
        check_op(
            "----------|------|------|0.004|10|-|4\n\
            ----------|------|------|0.004|10|-|2",
            Operation::Delete {
                line_idx: 0,
                line: "----------|------|------|0.004|10|-|4".to_string(),
            },
            "----------|------|------|0.004|10|-|2",
        );
    }

    #[test]
    fn op_insert() {
        check_op(
            "----------|------|------|0.004|10|-|2",
            Operation::Insert {
                line_idx: 0,
                line: "----------|------|------|0.004|10|-|4".to_string(),
            },
            "----------|------|------|0.004|10|-|4\n\
            ----------|------|------|0.004|10|-|2",
        );
    }

    #[test]
    fn op_replace() {
        check_op(
            "----------|------|------|0.004|10|-|4",
            Operation::Replace {
                line_idx: 0,
                from: "----------|------|------|0.004|10|-|4".to_string(),
                to: "s03lj-----|------|------|0.001|15|10|2".to_string(),
            },
            "s03lj-----|------|------|0.001|15|10|2",
        );
    }

    #[test]
    fn op_toggle_key() {
        fn check_key(result: &str, key: Key) {
            check_op(
                "----------|------|------|0.004|10|-|4",
                Operation::ToggleKey {
                    bulk_idx: 0,
                    key,
                    to: true,
                },
                &("----------|".to_string() + result + "|0.004|10|-|4"),
            );
        }

        check_key("f-----|------", Key::Forward);
        check_key("-l----|------", Key::Left);
        check_key("--r---|------", Key::Right);
        check_key("---b--|------", Key::Back);
        check_key("----u-|------", Key::Up);
        check_key("-----d|------", Key::Down);
        check_key("------|j-----", Key::Jump);
        check_key("------|-d----", Key::Duck);
        check_key("------|--u---", Key::Use);
        check_key("------|---1--", Key::Attack1);
        check_key("------|----2-", Key::Attack2);
        check_key("------|-----r", Key::Reload);
    }

    #[test]
    fn op_rewrite() {
        let input = "version 1
frames

// Hello
s03lj-----|------|------|0.001|15|10|2
        ";
        let output = "version 1
hlstrafe_version 4
demo my_tas
frames
// World

s00--d----|------|------|0.002|-|10|2
s00--d----|------|------|0.002|-|-|5
        ";
        let op = Operation::Rewrite {
            from: input.to_string(),
            to: output.to_string(),
        };

        let input = HLTAS::from_str(input).unwrap();
        let output = HLTAS::from_str(output).unwrap();

        let mut modified = input.clone();
        assert_ne!(
            op.apply(&mut modified),
            Some(0),
            "initial frame should never be invalidated"
        );
        assert_eq!(modified, output, "apply produced wrong result");

        assert_ne!(
            op.undo(&mut modified),
            Some(0),
            "initial frame should never be invalidated"
        );
        assert_eq!(modified, input, "undo produced wrong result");
    }

    #[test]
    fn op_replace_multiple() {
        let input = "\
----------|------|------|0.004|10|-|4
----------|------|------|0.004|10|-|5
----------|------|------|0.004|10|-|6
----------|------|------|0.004|10|-|7";

        check_op(
            input,
            Operation::ReplaceMultiple {
                first_line_idx: 1,
                from: "\
----------|------|------|0.004|10|-|5
----------|------|------|0.004|10|-|6"
                    .to_string(),
                to: "s03lj-----|------|------|0.001|15|10|2".to_string(),
            },
            "\
----------|------|------|0.004|10|-|4
s03lj-----|------|------|0.001|15|10|2
----------|------|------|0.004|10|-|7",
        );

        check_op(
            input,
            Operation::ReplaceMultiple {
                first_line_idx: 1,
                from: "\
----------|------|------|0.004|10|-|5
----------|------|------|0.004|10|-|6"
                    .to_string(),
                to: "\
s03lj-----|------|------|0.001|15|10|2
s03lj-----|------|------|0.001|15|10|3"
                    .to_string(),
            },
            "\
----------|------|------|0.004|10|-|4
s03lj-----|------|------|0.001|15|10|2
s03lj-----|------|------|0.001|15|10|3
----------|------|------|0.004|10|-|7",
        );

        check_op(
            input,
            Operation::ReplaceMultiple {
                first_line_idx: 1,
                from: "\
----------|------|------|0.004|10|-|5
----------|------|------|0.004|10|-|6"
                    .to_string(),
                to: "\
s03lj-----|------|------|0.001|15|10|2
s03lj-----|------|------|0.001|15|10|3
s03lj-----|------|------|0.001|15|10|4"
                    .to_string(),
            },
            "\
----------|------|------|0.004|10|-|4
s03lj-----|------|------|0.001|15|10|2
s03lj-----|------|------|0.001|15|10|3
s03lj-----|------|------|0.001|15|10|4
----------|------|------|0.004|10|-|7",
        );

        check_op(
            input,
            Operation::ReplaceMultiple {
                first_line_idx: 1,
                from: "".to_string(),
                to: "s03lj-----|------|------|0.001|15|10|2".to_string(),
            },
            "\
----------|------|------|0.004|10|-|4
s03lj-----|------|------|0.001|15|10|2
----------|------|------|0.004|10|-|5
----------|------|------|0.004|10|-|6
----------|------|------|0.004|10|-|7",
        );

        check_op(
            input,
            Operation::ReplaceMultiple {
                first_line_idx: 1,
                from: "----------|------|------|0.004|10|-|5".to_string(),
                to: "".to_string(),
            },
            "\
----------|------|------|0.004|10|-|4
----------|------|------|0.004|10|-|6
----------|------|------|0.004|10|-|7",
        );

        check_op(
            input,
            Operation::ReplaceMultiple {
                first_line_idx: 4,
                from: "".to_string(),
                to: "s03lj-----|------|------|0.001|15|10|2".to_string(),
            },
            "\
----------|------|------|0.004|10|-|4
----------|------|------|0.004|10|-|5
----------|------|------|0.004|10|-|6
----------|------|------|0.004|10|-|7
s03lj-----|------|------|0.001|15|10|2",
        );
    }

    #[test]
    fn op_set_adjacent_frame_count() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
----------|------|------|0.004|20|-|10",
            Operation::SetAdjacentFrameCount {
                bulk_idx: 0,
                from: 6,
                to: 11,
            },
            "\
----------|------|------|0.004|10|-|11
----------|------|------|0.004|20|-|5",
        );
    }

    #[test]
    fn op_set_adjacent_yaw() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
----------|------|------|0.004|20|-|10
----------|------|------|0.004|20|-|12
----------|------|------|0.004|20|-|5
----------|------|------|0.004|15|-|6",
            Operation::SetAdjacentYaw {
                first_bulk_idx: 1,
                bulk_count: 3,
                from: 20.,
                to: 15.,
            },
            "\
----------|------|------|0.004|10|-|6
----------|------|------|0.004|15|-|10
----------|------|------|0.004|15|-|12
----------|------|------|0.004|15|-|5
----------|------|------|0.004|15|-|6",
        );
    }

    #[test]
    fn op_set_adjacent_left_right_count() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s06-------|------|------|0.004|20|-|10
s06-------|------|------|0.004|20|-|12
s06-------|------|------|0.004|20|-|5
----------|------|------|0.004|15|-|6",
            Operation::SetAdjacentLeftRightCount {
                first_bulk_idx: 1,
                bulk_count: 3,
                from: 20,
                to: 15,
            },
            "\
----------|------|------|0.004|10|-|6
s06-------|------|------|0.004|15|-|10
s06-------|------|------|0.004|15|-|12
s06-------|------|------|0.004|15|-|5
----------|------|------|0.004|15|-|6",
        );
    }

    #[test]
    fn op_set_yawspeed() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s40-------|------|------|0.004|0|-|10
s41-------|------|------|0.004|70|-|10",
            Operation::SetYawspeed {
                bulk_idx: 1,
                from: 0.,
                to: 69.,
            },
            "\
----------|------|------|0.004|10|-|6
s40-------|------|------|0.004|69|-|10
s41-------|------|------|0.004|70|-|10",
        );

        check_op(
            "\
----------|------|------|0.004|10|-|6
s40-------|------|------|0.004|71|-|10
s41-------|------|------|0.004|70|-|10",
            Operation::SetYawspeed {
                bulk_idx: 1,
                from: 71.,
                to: 0.,
            },
            "\
----------|------|------|0.004|10|-|6
s40-------|------|------|0.004|0|-|10
s41-------|------|------|0.004|70|-|10",
        );

        check_op(
            "\
----------|------|------|0.004|10|-|6
s40-------|------|------|0.004|0|-|10
s41-------|------|------|0.004|70|-|10",
            Operation::SetYawspeed {
                bulk_idx: 2,
                from: 70.,
                to: 69.,
            },
            "\
----------|------|------|0.004|10|-|6
s40-------|------|------|0.004|0|-|10
s41-------|------|------|0.004|69|-|10",
        );
    }

    #[test]
    fn op_set_adjacent_yawspeed() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s40-------|------|------|0.004|0|-|10
s41-------|------|------|0.004|70|-|10",
            Operation::SetAdjacentYawspeed {
                first_bulk_idx: 1,
                bulk_count: 1,
                from: 0.,
                to: 69.,
            },
            "\
----------|------|------|0.004|10|-|6
s40-------|------|------|0.004|69|-|10
s41-------|------|------|0.004|70|-|10",
        );

        check_op(
            "\
----------|------|------|0.004|10|-|6
s41-------|------|------|0.004|70|-|10
s41-------|------|------|0.004|70|-|10
----------|------|------|0.004|70|-|6",
            Operation::SetAdjacentYawspeed {
                first_bulk_idx: 1,
                bulk_count: 2,
                from: 70.,
                to: 69.,
            },
            "\
----------|------|------|0.004|10|-|6
s41-------|------|------|0.004|69|-|10
s41-------|------|------|0.004|69|-|10
----------|------|------|0.004|70|-|6",
        );
    }

    #[test]
    fn op_set_frame_time() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s41-------|------|------|0.004|70|-|10
s41-------|------|------|0.004|70|-|10
----------|------|------|0.004|70|-|6",
            Operation::SetFrameTime {
                bulk_idx: 1,
                from: String::from("0.004"),
                to: String::from("0.0069"),
            },
            "\
----------|------|------|0.004|10|-|6
s41-------|------|------|0.0069|70|-|10
s41-------|------|------|0.004|70|-|10
----------|------|------|0.004|70|-|6",
        );
    }

    #[test]
    fn op_set_commands() {
        check_op(
            "\
----------|------|------|0.004|70|-|6",
            Operation::SetCommands {
                bulk_idx: 0,
                from: None,
                to: Some("quit".to_string()),
            },
            "\
----------|------|------|0.004|70|-|6|quit",
        );

        check_op(
            "\
----------|------|------|0.004|70|-|6|",
            Operation::SetCommands {
                bulk_idx: 0,
                from: Some("".to_string()),
                to: None,
            },
            "\
----------|------|------|0.004|70|-|6",
        );
    }

    #[test]
    fn op_set_accelerated_yawspeed_start() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s50-------|------|------|0.004|- 0 0 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
            Operation::SetMaxAccelOffsetStart {
                bulk_idx: 1,
                from: 0.,
                to: 69.,
            },
            "\
----------|------|------|0.004|10|-|6
s50-------|------|------|0.004|- 69 0 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
        );
    }

    #[test]
    fn op_set_accelerated_yawspeed_start_and_target() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s50-------|------|------|0.004|- 0 0 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
            Operation::SetMaxAccelOffsetStartAndTarget {
                bulk_idx: 1,
                from: (0., 0.),
                to: (69., -89.34),
            },
            "\
----------|------|------|0.004|10|-|6
s50-------|------|------|0.004|- 69 -89.34 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
        );
    }

    #[test]
    fn op_set_accelerated_yawspeed_yaw() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s53-------|------|------|0.004|0 0 0 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
            Operation::SetYaw {
                bulk_idx: 1,
                from: 0.,
                to: 169.3,
            },
            "\
----------|------|------|0.004|10|-|6
s53-------|------|------|0.004|169.3 0 0 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
        );
    }

    #[test]
    fn op_set_accelerated_yawspeed_count() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s57-------|------|------|0.004|13 0 0 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
            Operation::SetLeftRightCount {
                bulk_idx: 1,
                from: 13,
                to: 36,
            },
            "\
----------|------|------|0.004|10|-|6
s57-------|------|------|0.004|36 0 0 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
        );
    }

    #[test]
    fn op_set_accelerated_yawspeed_acceleration() {
        check_op(
            "\
----------|------|------|0.004|10|-|6
s53-------|------|------|0.004|13 0 0 0|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
            Operation::SetMaxAccelOffsetAccel {
                bulk_idx: 1,
                from: 0.,
                to: -14.3,
            },
            "\
----------|------|------|0.004|10|-|6
s53-------|------|------|0.004|13 0 0 -14.3|-|10
s51-------|------|------|0.004|- 0 70 10|-|10",
        );
    }
}
