use std::cmp::{max, min};
use std::fmt::Write;
use std::iter::{self, zip};
use std::num::NonZeroU32;
use std::ops::ControlFlow;
use std::path::Path;
use std::time::Instant;

use bxt_ipc_types::Frame;
use bxt_strafe::{Hull, Trace};
use color_eyre::eyre::{self, ensure};
use glam::{IVec2, Vec2, Vec3};
use hltas::types::{
    AutoMovement, Change, ChangeTarget, Line, StrafeDir, StrafeSettings, StrafeType,
    VectorialStrafingConstraints,
};
use hltas::HLTAS;
use itertools::Itertools;
use thiserror::Error;

use self::db::{Action, ActionKind, Branch, Db};
use self::operation::{Key, Operation};
use self::toggle_auto_action::ToggleAutoActionTarget;
use self::utils::{
    bulk_and_first_frame_idx, bulk_and_first_frame_idx_mut, bulk_idx_and_is_last,
    bulk_idx_and_repeat_at_frame, join_lines, line_first_frame_idx, line_idx_and_repeat_at_frame,
    FrameBulkExt, MaxAccelOffsetValuesMut,
};
use super::remote::{AccurateFrame, PlayRequest};
use crate::hooks::sdl::MouseState;
use crate::modules::tas_optimizer::simulator::Simulator;
use crate::modules::tas_studio::editor::utils::MaxAccelOffsetValues;
use crate::modules::triangle_drawing::triangle_api::{Primitive, RenderMode};
use crate::modules::triangle_drawing::TriangleApi;

mod db;
pub mod operation;
pub mod toggle_auto_action;
pub mod utils;

pub struct Editor {
    /// Database storing information on disk.
    db: Db,

    // BranchData::branch, undo_log and redo_log are essentially an in-memory cache for the on-disk
    // data. Operations on them should be committed to db right away.
    /// Branches of the project.
    branches: Vec<BranchData>,
    /// Index of the branch we're currently editing.
    branch_idx: usize,

    /// Log of actions for undo.
    undo_log: Vec<Action>,
    /// Log of actions for redo.
    redo_log: Vec<Action>,

    /// Current project generation.
    ///
    /// Generation increases with every change to any of the branches' scripts and ensures that we
    /// don't store accurate frames that came from an outdated script.
    generation: u16,
    /// Index of the hovered frame.
    ///
    /// Might be `None` for example if the player is looking away from the entire path (so there's
    /// no visible frame which could be under the cursor).
    ///
    /// During adjustment, it will be the last frame of currently selected framebulk.
    hovered_frame_idx: Option<usize>,

    /// Mouse state from the last time `tick()` was called.
    prev_mouse_state: MouseState,
    /// Keyboard state from the last time `tick()` was called.
    prev_keyboard_state: KeyboardState,

    /// Whether to enable automatic global smoothing.
    auto_smoothing: bool,
    /// Whether to show the player bbox for the frame under cursor.
    show_player_bbox: bool,
    /// Index of the first frame that should be fully shown and able to be interacted with.
    ///
    /// Frames before this cannot be interacted with and can be hidden from display.
    first_shown_frame_idx: usize,

    /// Whether the editor is in the camera editor mode.
    in_camera_editor: bool,

    /// Frame index calculated from bxt_tas_studio_norefresh_until_stop_frame.
    norefresh_until_stop_frame_frame_idx: usize,

    // ==============================================
    // Movement-editor-specific state.
    /// Index of the hovered frame bulk.
    hovered_bulk_idx: Option<usize>,
    /// Index of the selected frame bulk.
    ///
    /// When drag-editing a frame bulk, it remains the selected one.
    selected_bulk_idx: Option<usize>,

    // Adjustments MUST BE applied or cancelled, never simply dropped. Dropping without applying or
    // cancelling will result in database corruption!
    /// Frame bulk frame count adjustment.
    frame_count_adjustment: Option<MouseAdjustment<u32>>,
    /// Frame bulk yaw adjustment.
    ///
    /// This can be a set yaw, or a strafing target yaw.
    yaw_adjustment: Option<MouseAdjustment<f32>>,
    /// Frame bulk left-right strafing frame count adjustment.
    left_right_count_adjustment: Option<MouseAdjustment<u32>>,
    /// Adjacent frame bulk frame count adjustment.
    ///
    /// Preserves the total frame count of the two adjacent frame bulks.
    ///
    /// This adjustment requires a following frame bulk to exist (since it keeps the total frame
    /// count the same).
    adjacent_frame_count_adjustment: Option<MouseAdjustment<u32>>,
    /// Adjacent frame bulk yaw adjustment.
    ///
    /// Adjusts the yaw in the same way for all adjacent frame bulks with equal yaw.
    adjacent_yaw_adjustment: Option<AdjacentYawAdjustment>,
    /// Adjacent frame bulk left-right strafing frame count adjustment.
    ///
    /// Adjusts the left-right count in the same way for all adjacent frame bulks with equal
    /// left-right count.
    adjacent_left_right_count_adjustment: Option<AdjacentLeftRightCountAdjustment>,
    /// Frame bulk side strafe yawspeed adjustment.
    side_strafe_yawspeed_adjustment: Option<MouseAdjustment<f32>>,
    /// Adjacent frame bulk side strafe yawspeed adjustment.
    ///
    /// Adjusts the yawspeed in the same way for all adjacent frame bulks with equal yawspeed.
    adjacent_side_strafe_yawspeed_adjustment: Option<AdjacentYawspeedAdjustment>,
    /// Frame bulk max acceleration yaw offset adjustment.
    max_accel_yaw_offset_adjustment: Option<MaxAccelYawOffsetAdjustment>,

    // ==============================================
    // Camera-editor-specific state.
    // TODO: we want to be able to drag the end points of the change lines separately from the
    // start points. This cannot be tracked with just line index.
    // TODO: camera editor actions should also use selection and not hover.
    /// Index of the hovered frame bulk.
    hovered_line_idx: Option<usize>,

    /// Adjustment to insert a camera line.
    insert_camera_line_adjustment: Option<InsertCameraLineAdjustment>,

    /// Smoothing window size in seconds.
    smooth_window_s: f32,
    /// Smoothing small window size in seconds.
    smooth_small_window_s: f32,
    /// Smoothing small window impact multiplier.
    smooth_small_window_multiplier: f32,
}

#[derive(Debug, Clone)]
pub struct BranchData {
    /// Edited branch.
    branch: Branch,

    /// Accurate and predicted frames.
    ///
    /// Data in every frame is sampled right before the call to `HLStrafe::MainFunc`. So the very
    /// first frame contains data before any TAS input, the second frame contains data after one
    /// frame of TAS input, and so on.
    pub frames: Vec<Frame>,
    /// Index of the first frame in `frames` that is predicted (inaccurate) rather than played.
    first_predicted_frame: usize,
    /// Extra camera editor data for every frame.
    ///
    /// Has the same number of elements as `frames`. Valid/computed only when in the camera editor
    /// mode.
    extra_cam: Vec<ExtraCameraEditorFrameData>,
    /// Data for auto-smoothing.
    auto_smoothing: AutoSmoothing,
}

impl BranchData {
    fn new(branch: Branch) -> Self {
        Self {
            branch,
            frames: vec![],
            first_predicted_frame: 0,
            extra_cam: vec![],
            auto_smoothing: AutoSmoothing {
                script: None,
                frames: vec![],
            },
        }
    }
}

/// Extra camera editor data for every frame.
#[derive(Debug, Default, Clone)]
struct ExtraCameraEditorFrameData {
    /// Whether this frame is in a smoothing idempotent region.
    in_smoothing_idempotent_region: bool,
    /// Frame indices of the smoothing input region this frame is part of.
    smoothing_input_region: Option<(usize, usize)>,
    /// Index into `script.lines` of a change frame that ends on this frame.
    change_line_that_ends_here: Vec<usize>,
    /// Index of a frame where the change line which starts on this frame ends.
    change_ends_at: Vec<usize>,
    /// Index into `script.lines` of a camera frame that starts or ends on this frame.
    camera_line_that_starts_or_ends_here: Vec<usize>,
    /// Index into `script.lines` of a camera frame that starts on this frame.
    camera_line_that_starts_here: Vec<usize>,
}

/// Data for handling adjustment done by pressing and dragging the mouse.
#[derive(Debug, Clone, Copy)]
struct MouseAdjustment<T> {
    /// Original value before adjustment.
    original_value: T,
    /// Mouse coordinates when mouse was pressed.
    pressed_at: Vec2,
    /// Path direction when mouse was pressed.
    ///
    /// We want dragging in the same direction to do the same thing over the whole duration of
    /// holding the mouse down. However, as we move the mouse and adjust the frame bulk, the
    /// direction will change, as the path moves around. Therefore we need to store the direction
    /// at the time the mouse was pressed and use that for the computation.
    ///
    /// The direction is normalized.
    reference_direction: Vec2,
    /// Whether the adjustment made a change at least once.
    ///
    /// Clicking on a frame bulk to select it triggers a quick adjustment that does not change the
    /// frame bulk. We don't want to store that into the undo log. However, we do want to store
    /// adjustments that did result in a change, but then were dragged back to the original value.
    /// Hence, store this status here.
    changed_once: bool,
}

impl<T> MouseAdjustment<T> {
    fn new(original_value: T, pressed_at: Vec2, reference_direction: Vec2) -> Self {
        Self {
            original_value,
            pressed_at,
            // Try normalizing, or fall back to the X axis.
            reference_direction: reference_direction.try_normalize().unwrap_or(Vec2::X),
            changed_once: false,
        }
    }

    /// Returns the adjustment delta for the current mouse position.
    fn delta(&self, mouse_pos: Vec2) -> f32 {
        (mouse_pos - self.pressed_at).dot(self.reference_direction)
    }
}

/// Data for handling the adjacent yaw adjustment.
///
/// We need to store which frame bulks are affected so that as we drag the mouse we don't "pick up"
/// any extra frame bulks when the yaw suddenly starts to match them.
#[derive(Debug, Clone, Copy)]
struct AdjacentYawAdjustment {
    /// The mouse adjustment itself.
    mouse_adjustment: MouseAdjustment<f32>,
    /// Index of the first of the affected frame bulks.
    first_bulk_idx: usize,
    /// Number of the affected frame bulks.
    bulk_count: usize,
}

/// Data for handling the adjacent left-right count adjustment.
///
/// We need to store which frame bulks are affected so that as we drag the mouse we don't "pick up"
/// any extra frame bulks when the left-right count suddenly starts to match them.
#[derive(Debug, Clone, Copy)]
struct AdjacentLeftRightCountAdjustment {
    /// The mouse adjustment itself.
    mouse_adjustment: MouseAdjustment<u32>,
    /// Index of the first of the affected frame bulks.
    first_bulk_idx: usize,
    /// Number of the affected frame bulks.
    bulk_count: usize,
}

/// Data for handling the adjacent side strafe yawspeed adjustment.
///
/// We need to store which frame bulks are affected so that as we drag the mouse we don't "pick up"
/// any extra frame bulks when the yawspeed suddenly starts to match them.
#[derive(Debug, Clone, Copy)]
struct AdjacentYawspeedAdjustment {
    /// The mouse adjustment itself.
    mouse_adjustment: MouseAdjustment<f32>,
    /// Index of the first of the affected frame bulks.
    first_bulk_idx: usize,
    /// Number of the affected frame bulks.
    bulk_count: usize,
}

/// Data for handling the side strafe accelerated yawspeed adjustment.
#[derive(Debug, Clone, Copy)]
pub struct MaxAccelYawOffsetAdjustment {
    /// The mouse adjustment itself.
    ///
    /// (start, target, accel, yaw field value)
    ///
    /// Yaw field value here is the first value in the yaw field.
    ///
    /// It would be current yaw for yaw strafe or left-right count for left-right strafing.
    mouse_adjustment: MouseAdjustment<MaxAccelYawOffsetMouseAdjustment>,
    /// Indicates which change mode is in use.
    ///
    /// Left click while holding down right mouse button to switch change mode.
    pub mode: MaxAccelYawOffsetMode,
    /// Offset the mouse delta position when mode is switched.
    mouse_offset: IVec2,
    /// Skip Alt cycle if the mode is not available.
    cycle_again: bool,
}

/// Modes of operation for maximum acceleration yaw offset adjustment.
///
/// Cycling by holding right mouse button and left click.
#[derive(Debug, Clone, Copy)]
pub enum MaxAccelYawOffsetMode {
    StartAndTarget = 0,
    Target,
    Acceleration,
    Start,
    /// Alt is for adjusting the yaw field value of the strafe direction.
    Alt,
}

#[derive(Debug, Clone, Copy)]
pub struct MaxAccelYawOffsetMouseAdjustment {
    pub start: f32,
    pub target: f32,
    pub accel: f32,
    pub alt: MaxAccelYawOffsetYawField,
}

#[derive(Debug, Clone, Copy)]
pub enum MaxAccelYawOffsetYawField {
    None,
    Yaw(f32),
    LeftRight(NonZeroU32),
}

impl MaxAccelYawOffsetMode {
    fn cycle(&self) -> Self {
        match self {
            MaxAccelYawOffsetMode::StartAndTarget => MaxAccelYawOffsetMode::Target,
            MaxAccelYawOffsetMode::Target => MaxAccelYawOffsetMode::Acceleration,
            MaxAccelYawOffsetMode::Acceleration => MaxAccelYawOffsetMode::Start,
            MaxAccelYawOffsetMode::Start => MaxAccelYawOffsetMode::Alt,
            MaxAccelYawOffsetMode::Alt => MaxAccelYawOffsetMode::StartAndTarget,
        }
    }
}

/// Data for handling the insert-camera-line adjustment.
#[derive(Debug, Clone, Copy)]
struct InsertCameraLineAdjustment {
    /// The mouse adjustment itself.
    mouse_adjustment: MouseAdjustment<i32>,
    starting_frame_idx: usize,
    camera_line_idx: usize,
    initial_yaw: f32,
    did_split: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct KeyboardState {
    /// Whether the "faster" key is pressed.
    pub adjust_faster: bool,
    /// Whether the "slower" key is pressed.
    pub adjust_slower: bool,
    /// Whether the "insert camera line" key is pressed.
    pub insert_camera_line: bool,
}

impl KeyboardState {
    fn adjustment_speed(self) -> f32 {
        let mut speed = 1.;
        if self.adjust_slower {
            speed /= 20.;
        }
        if self.adjust_faster {
            speed *= 20.;
        }
        speed
    }
}

struct DrawLine {
    start: Vec3,
    end: Vec3,
    color: Vec3,
}

/// Data for auto-smoothing.
#[derive(Debug, Clone)]
struct AutoSmoothing {
    /// Smoothed script when it is available.
    ///
    /// The smoothed script can only be created after receiving all accurate frames for the
    /// original script. Before that it is `None`.
    script: Option<HLTAS>,
    /// Smoothed accurate frames.
    ///
    /// The result of playing back the smoothed `script`.
    frames: Vec<Frame>,
}

/// Error of a manually-triggered operation.
#[derive(Debug, Error)]
pub enum ManualOpError {
    #[error("cannot do this during an active adjustment")]
    CannotDoDuringAdjustment,
    #[error("you need to be in the camera editor to do this")]
    CannotDoInMovementEditor,
    #[error("you need to be in the movement editor to do this")]
    CannotDoInCameraEditor,
    #[error("you need to point the cursor at a frame to do this")]
    NoHoveredFrame,
    #[error("you need to select a frame bulk to do this")]
    NoSelectedBulk,
    #[error("this branch does not exist")]
    BranchDoesNotExist,
    /// Other non-fatal user error (i.e. unable to trigger the operation at this time).
    #[error("{0}")]
    UserError(String),
    /// Error of the operation which was triggered.
    #[error("{0}")]
    InternalError(#[from] eyre::Report),
}

type ManualOpResult<T> = Result<T, ManualOpError>;

impl ManualOpError {
    /// Returns `true` if the manual op error is [`InternalError`].
    ///
    /// [`InternalError`]: ManualOpError::InternalError
    #[must_use]
    pub fn is_internal(&self) -> bool {
        matches!(self, Self::InternalError(..))
    }
}

impl Editor {
    pub fn open_db(mut db: Db) -> eyre::Result<Self> {
        let branches = db.branches()?;
        ensure!(!branches.is_empty(), "there must be at least one branch");

        let global_settings = db.global_settings()?;
        let branch_idx = branches
            .iter()
            .enumerate()
            .find(|(_, branch)| branch.branch_id == global_settings.current_branch_id)
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        let branches = branches.into_iter().map(BranchData::new).collect();
        let (undo_log, redo_log) = db.undo_redo()?;

        Ok(Self {
            db,
            branches,
            branch_idx,
            generation: 0,
            undo_log,
            redo_log,
            hovered_bulk_idx: None,
            selected_bulk_idx: None,
            hovered_frame_idx: None,
            prev_mouse_state: MouseState::default(),
            prev_keyboard_state: KeyboardState::default(),
            frame_count_adjustment: None,
            yaw_adjustment: None,
            left_right_count_adjustment: None,
            adjacent_frame_count_adjustment: None,
            adjacent_yaw_adjustment: None,
            adjacent_left_right_count_adjustment: None,
            side_strafe_yawspeed_adjustment: None,
            adjacent_side_strafe_yawspeed_adjustment: None,
            max_accel_yaw_offset_adjustment: None,
            in_camera_editor: false,
            auto_smoothing: false,
            show_player_bbox: false,
            first_shown_frame_idx: 0,
            hovered_line_idx: None,
            insert_camera_line_adjustment: None,
            smooth_window_s: 0.15,
            smooth_small_window_s: 0.03,
            smooth_small_window_multiplier: 3.,
            norefresh_until_stop_frame_frame_idx: 0,
        })
    }

    pub fn open(path: &Path) -> eyre::Result<Self> {
        let db = Db::open(path)?;
        Self::open_db(db)
    }

    pub fn create(path: &Path, script: &HLTAS) -> eyre::Result<Self> {
        let db = Db::create(path, script)?;
        Self::open_db(db)
    }

    #[cfg(test)]
    pub fn create_in_memory(script: &HLTAS) -> eyre::Result<Self> {
        let db = Db::create_in_memory(script)?;
        Self::open_db(db)
    }

    pub fn branch(&self) -> &BranchData {
        &self.branches[self.branch_idx]
    }

    fn branch_mut(&mut self) -> &mut BranchData {
        &mut self.branches[self.branch_idx]
    }

    pub fn generation(&self) -> u16 {
        self.generation
    }

    pub fn branch_idx(&self) -> usize {
        self.branch_idx
    }

    pub fn script(&self) -> &HLTAS {
        &self.branch().branch.script
    }

    pub fn smoothed_script(&self) -> Option<&HLTAS> {
        self.branch().auto_smoothing.script.as_ref()
    }

    pub fn stop_frame(&self) -> u32 {
        self.branch().branch.stop_frame
    }

    pub fn selected_bulk_idx(&self) -> Option<usize> {
        self.selected_bulk_idx
    }

    pub fn hovered_frame_idx(&self) -> Option<usize> {
        self.hovered_frame_idx
    }

    pub fn hovered_frame(&self) -> Option<&Frame> {
        self.hovered_frame_idx.map(|idx| &self.branch().frames[idx])
    }

    pub fn undo_log_len(&self) -> usize {
        self.undo_log.len()
    }

    pub fn side_strafe_accelerated_yawspeed_adjustment(
        &self,
    ) -> Option<MaxAccelYawOffsetAdjustment> {
        self.max_accel_yaw_offset_adjustment
    }

    pub fn set_in_camera_editor(&mut self, value: bool) {
        if self.in_camera_editor == value {
            return;
        }

        self.cancel_ongoing_adjustments();
        self.in_camera_editor = value;

        self.recompute_extra_camera_frame_data_if_needed();
    }

    pub fn set_smooth_window_s(&mut self, value: f32) {
        if self.smooth_window_s == value {
            return;
        }

        self.smooth_window_s = value;
        self.branch_mut().extra_cam.clear();
        self.recompute_extra_camera_frame_data_if_needed();
    }

    pub fn set_smooth_small_window_s(&mut self, value: f32) {
        if self.smooth_small_window_s == value {
            return;
        }

        self.smooth_small_window_s = value;
        self.branch_mut().extra_cam.clear();
        self.recompute_extra_camera_frame_data_if_needed();
    }

    pub fn set_smooth_small_window_multiplier(&mut self, value: f32) {
        self.smooth_small_window_multiplier = value;
    }

    pub fn set_auto_smoothing(&mut self, value: bool) {
        self.auto_smoothing = value;
    }

    pub fn set_show_player_bbox(&mut self, value: bool) {
        self.show_player_bbox = value;
    }

    pub fn set_norefresh_until_stop_frame(&mut self, value: usize) {
        self.norefresh_until_stop_frame_frame_idx = value;
    }

    /// Invalidates frames starting from given.
    ///
    /// Erases cached frame data and adjusts the first predicted frame index if needed.
    fn invalidate(&mut self, frame_idx: usize) {
        let branch = &mut self.branch_mut();
        branch.frames.truncate(frame_idx);
        branch.first_predicted_frame = min(branch.first_predicted_frame, frame_idx);

        // TODO: probably possible to do a finer-grained invalidation.
        branch.auto_smoothing.script = None;
        branch.auto_smoothing.frames.clear();

        branch.extra_cam.clear();
        self.recompute_extra_camera_frame_data_if_needed();

        self.generation = self.generation.wrapping_add(1);
    }

    pub fn recompute_extra_camera_frame_data_if_needed(&mut self) {
        if !self.in_camera_editor {
            return;
        }

        for branch_idx in 0..self.branches.len() {
            let branch = &self.branches[branch_idx];
            if branch.extra_cam.len() != branch.frames.len() {
                self.recompute_extra_camera_frame_data(branch_idx);
            }
        }
    }

    fn recompute_extra_camera_frame_data(&mut self, branch_idx: usize) {
        let _span = info_span!("recompute_extra_frame_data", branch_idx).entered();

        let branch = &mut self.branches[branch_idx];
        let frames = &branch.frames;

        // Some of the data is dependent on future frames and cannot be trivially invalidated. To
        // be safe, recompute it from scratch.
        branch.extra_cam.clear();

        // Fill in all vector elements.
        for _ in 0..frames.len() {
            branch.extra_cam.push(ExtraCameraEditorFrameData::default());
        }

        if frames.len() <= 1 {
            // No frames past the initial frame.
            return;
        }

        // Fill in the rest of the fields.

        // These functions find the first and the last frame of the smoothing input region, given
        // the boundary frames of the non-idempotent region.
        let window_size = self.smooth_window_s.max(self.smooth_small_window_s);
        let walk_back = move |mut idx: usize| {
            let mut rem_win_size = window_size / 2. - frames[idx].parameters.frame_time / 2.;

            while idx > 0 && rem_win_size >= 0. {
                idx -= 1;
                rem_win_size -= frames[idx].parameters.frame_time;
            }

            idx
        };

        let walk_forward = move |mut idx: usize| {
            let mut rem_win_size = window_size / 2. - frames[idx].parameters.frame_time / 2.;

            while idx + 1 < frames.len() && rem_win_size >= 0. {
                idx += 1;
                rem_win_size -= frames[idx].parameters.frame_time;
            }

            idx
        };

        // TODO: This should check the differences between successive yaws, not the yaws themselves.
        //
        // Our smoothing infinitely pads the input frames from both sides.
        let mut idempotent_duration = f32::INFINITY;
        let mut idempotent_started_at = 0;
        let mut input_started_at = 0;
        for (prev_idx, (prev, frame)) in frames.iter().tuple_windows().enumerate() {
            let idx = prev_idx + 1;

            let prev_yaw = prev.state.prev_frame_input.yaw;
            let yaw = frame.state.prev_frame_input.yaw;

            if yaw != prev_yaw {
                // TODO: check for off-by-ones.
                if idempotent_duration >= window_size {
                    // The region that just ended was idempotent. Mark it as such.
                    for extra_cam in &mut branch.extra_cam[idempotent_started_at..idx] {
                        extra_cam.in_smoothing_idempotent_region = true;
                    }

                    if idempotent_started_at > 0 {
                        let input_ended_at = walk_forward(idempotent_started_at - 1);
                        let region = Some((input_started_at, input_ended_at));
                        for extra_cam in &mut branch.extra_cam[input_started_at..=input_ended_at] {
                            extra_cam.smoothing_input_region = region;
                        }
                    }

                    input_started_at = walk_back(idx);
                }

                idempotent_duration = 0.;

                // Rememeber the first frame of the potentially idempotent region.
                idempotent_started_at = idx;

                // We're starting a new window of frames, and this frame is already included. So,
                // let the += below run.
            }

            idempotent_duration += frame.parameters.frame_time;
        }

        // Since our smoothing infinitely pads input frames from both sides, the last region is
        // idempotent regardless of how many frames it contains.
        for extra_cam in &mut branch.extra_cam[idempotent_started_at..] {
            extra_cam.in_smoothing_idempotent_region = true;
        }

        if idempotent_started_at > 0 {
            let input_ended_at = walk_forward(idempotent_started_at - 1);
            let region = Some((input_started_at, input_ended_at));
            for extra_cam in &mut branch.extra_cam[input_started_at..=input_ended_at] {
                extra_cam.smoothing_input_region = region;
            }
        }

        let script = &branch.branch.script;
        for (line_idx, frame_idx) in line_first_frame_idx(script).enumerate() {
            if frame_idx >= branch.extra_cam.len() {
                // TODO: If we have target_yaw velocity_lock past the very last frame bulk, it will
                // have a frame_idx == number of frames. This is because the first frame it affects
                // is the frame right after all frames in the script, as it should be. However, this
                // causes us to not render it, even though we could if we instead stored it on the
                // previuos frame.
                break;
            }

            let line = &script.lines[line_idx];
            if let Line::Change(Change { mut over, .. }) = line {
                // Find the end of the change.
                let mut end_frame_idx = frame_idx;
                while end_frame_idx < branch.frames.len() {
                    over -= branch.frames[end_frame_idx].parameters.frame_time;
                    if over <= 0. {
                        branch.extra_cam[end_frame_idx]
                            .change_line_that_ends_here
                            .push(line_idx);
                        branch.extra_cam[frame_idx]
                            .change_ends_at
                            .push(end_frame_idx);
                        branch.extra_cam[end_frame_idx]
                            .camera_line_that_starts_or_ends_here
                            .push(line_idx);
                        break;
                    }

                    end_frame_idx += 1;
                }
                // If we ran out of frames, don't mark an end.
            }

            if matches!(
                line,
                Line::Change(_)
                    | Line::TargetYawOverride { .. }
                    | Line::RenderYawOverride { .. }
                    | Line::VectorialStrafingConstraints(_)
            ) {
                branch.extra_cam[frame_idx]
                    .camera_line_that_starts_here
                    .push(line_idx);
                branch.extra_cam[frame_idx]
                    .camera_line_that_starts_or_ends_here
                    .push(line_idx);
            }
        }
    }

    fn is_any_adjustment_active(&self) -> bool {
        self.frame_count_adjustment.is_some()
            || self.yaw_adjustment.is_some()
            || self.left_right_count_adjustment.is_some()
            || self.adjacent_frame_count_adjustment.is_some()
            || self.adjacent_yaw_adjustment.is_some()
            || self.adjacent_left_right_count_adjustment.is_some()
            || self.side_strafe_yawspeed_adjustment.is_some()
            || self.adjacent_side_strafe_yawspeed_adjustment.is_some()
            || self.max_accel_yaw_offset_adjustment.is_some()
            || self.insert_camera_line_adjustment.is_some()
    }

    /// Updates the editor state.
    pub fn tick<T: Trace>(
        &mut self,
        tracer: &T,
        world_to_screen: impl Fn(Vec3) -> Option<Vec2>,
        mouse: MouseState,
        keyboard: KeyboardState,
        deadline: Instant,
    ) -> ManualOpResult<()> {
        let _span = info_span!("Editor::tick").entered();

        // Update ongoing adjustments.
        self.tick_frame_count_adjustment(mouse, keyboard)?;
        self.tick_yaw_adjustment(mouse, keyboard)?;
        self.tick_left_right_count_adjustment(mouse, keyboard)?;
        self.tick_adjacent_frame_count_adjustment(mouse, keyboard)?;
        self.tick_adjacent_yaw_adjustment(mouse, keyboard)?;
        self.tick_adjacent_left_right_count_adjustment(mouse, keyboard)?;
        self.tick_side_strafe_yawspeed_adjustment(mouse, keyboard)?;
        self.tick_adjacent_side_strafe_yawspeed_adjustment(mouse, keyboard)?;
        self.tick_max_accel_yaw_offset_adjustment(mouse, keyboard, self.prev_mouse_state)?;

        self.tick_insert_camera_line_adjustment(mouse, keyboard)?;

        // Predict any frames that need prediction.
        //
        // Do this after adjustment and before computing input to have the most up-to-date data.
        //
        // TODO: add a timeout on running prediction after receiving an accurate frame. So that when
        // we're receiving accurate frames, we don't run prediction every frame, which will be
        // invalidated next frame due to receiving the next accurate frame.
        {
            let _span = info_span!("predict").entered();

            let branch = self.branch_mut();
            let simulator = Simulator::new(tracer, &branch.frames, &branch.branch.script.lines);
            for frame in simulator {
                // Always simulate at least one frame.
                branch.frames.push(frame);

                // Break if the deadline has passed.
                if Instant::now() >= deadline {
                    break;
                }
            }
        }

        // Recompute extra data in case the prediction above added frames.
        self.recompute_extra_camera_frame_data_if_needed();

        let mouse_pos = mouse.pos.as_vec2();

        let any_mouse_was_down_before = self.prev_mouse_state.buttons.is_left_down()
            || self.prev_mouse_state.buttons.is_right_down()
            || self.prev_mouse_state.buttons.is_middle_down()
            || self.prev_mouse_state.buttons.is_mouse4_down();

        let any_mouse_is_down = mouse.buttons.is_left_down()
            || mouse.buttons.is_right_down()
            || mouse.buttons.is_middle_down()
            || mouse.buttons.is_mouse4_down();

        let mouse_became_down = !any_mouse_was_down_before && any_mouse_is_down;

        // Only update the hovered and active bulk index if we are not holding, or just pressed a
        // mouse button.
        if !any_mouse_is_down || mouse_became_down {
            // If we're in the camera editor, update its hover and make the movement editor's hover
            // `None`. If we're in the movement editor, do the opposite.
            if self.in_camera_editor {
                self.hovered_bulk_idx = None;

                self.hovered_line_idx = iter::zip(
                    self.branches[self.branch_idx].frames.iter(),
                    // We take the next frame's extra_cam here because we're mostly concerned with
                    // camera line *starts*, and those are rendered *before* the frame, rather than
                    // after the frame. So visually we see it one frame index earlier.
                    self.branches[self.branch_idx].extra_cam.iter().skip(1),
                )
                // Add frame indices.
                .enumerate()
                // Skip past hidden frames.
                .filter_map(|(frame_idx, rest)| {
                    if frame_idx < self.first_shown_frame_idx {
                        None
                    } else {
                        Some(rest)
                    }
                })
                // Take the last of the change lines.
                .filter_map(|(frame, next_extra_cam)| {
                    next_extra_cam
                        .camera_line_that_starts_or_ends_here
                        .last()
                        .map(|line_idx| (frame, *line_idx))
                })
                // Convert to screen and take only successfully converted coordinates.
                .filter_map(|(frame, line_idx)| {
                    world_to_screen(frame.state.player.pos).map(|screen| (screen, line_idx))
                })
                // Compute distance to cursor.
                .map(|(screen, line_idx)| (screen.distance_squared(mouse_pos), line_idx))
                // Take only ones close enough to the cursor.
                // .filter(|(dist_sq, _)| *dist_sq < 100. * 100.)
                // Find closest to cursor.
                .min_by(|(dist_a, _), (dist_b, _)| dist_a.total_cmp(dist_b))
                // Extract line index.
                .map(|(_, line_idx)| line_idx);
            } else {
                self.hovered_bulk_idx = iter::zip(
                    self.branches[self.branch_idx].frames.iter().skip(1),
                    bulk_idx_and_is_last(&self.branches[self.branch_idx].branch.script.lines),
                )
                // Add frame indices.
                .enumerate()
                // Skip past hidden frames.
                .filter_map(|(frame_idx, rest)| {
                    if frame_idx < self.first_shown_frame_idx {
                        None
                    } else {
                        Some(rest)
                    }
                })
                // Take only last frame in each bulk.
                .filter_map(|(frame, (bulk_idx, _, is_last_in_bulk))| {
                    is_last_in_bulk.then_some((frame, bulk_idx))
                })
                // Convert to screen and take only successfully converted coordinates.
                .filter_map(|(frame, bulk_idx)| {
                    world_to_screen(frame.state.player.pos).map(|screen| (screen, bulk_idx))
                })
                // Compute distance to cursor.
                .map(|(screen, bulk_idx)| (screen.distance_squared(mouse_pos), bulk_idx))
                // Take only ones close enough to the cursor.
                // .filter(|(dist_sq, _)| *dist_sq < 100. * 100.)
                // Find closest to cursor.
                .min_by(|(dist_a, _), (dist_b, _)| dist_a.total_cmp(dist_b))
                // Extract bulk index.
                .map(|(_, bulk_idx)| bulk_idx);

                self.hovered_line_idx = None;
            }

            // Only update the selected bulk and start the adjustments if the mouse has just been
            // pressed.
            if mouse_became_down && !self.in_camera_editor {
                // Since all mouse buttons that start adjustments have been down last frame, there
                // cannot be any active adjustments now. All movement editor adjustments are
                // triggered by the mouse only.
                assert!(!self.is_any_adjustment_active());

                // Make the hovered bulk the selected bulk (or clear the selected bulk if not
                // hovering anything).
                self.selected_bulk_idx = self.hovered_bulk_idx;

                // Now that we have up-to-date active bulk index, start any adjustments if needed.
                if let Some(active_bulk_idx) = self.selected_bulk_idx {
                    let branch = self.branch();

                    // TODO add bulk to the iterator above to avoid re-walking.
                    //
                    // Prepare the iterator lazily in advance so it can be used in every branch.
                    //
                    // Returns the frame bulk and the index of the last frame simulated by this
                    // frame bulk. It seems to be 1 more than needed, but that is because the very
                    // first frame is always the initial frame, which is not simulated by any frame
                    // bulk, so we're essentially adding 1 to compensate.
                    let mut bulk_and_last_frame_idx = branch
                        .branch
                        .script
                        .frame_bulks()
                        .scan(0, |frame_idx, bulk| {
                            *frame_idx += bulk.frame_count.get() as usize;
                            Some((bulk, *frame_idx))
                        })
                        .skip(active_bulk_idx);

                    if mouse.buttons.is_left_down() {
                        let (bulk, last_frame_idx) = bulk_and_last_frame_idx.next().unwrap();

                        let frame = &branch.frames[last_frame_idx];
                        let prev = &branch.frames[last_frame_idx - 1];

                        let frame_screen = world_to_screen(frame.state.player.pos);
                        let prev_screen = world_to_screen(prev.state.player.pos);

                        let dir = match (frame_screen, prev_screen) {
                            (Some(frame), Some(prev)) => frame - prev,
                            // Presumably, previous frame is invisible, so just fall back.
                            _ => Vec2::X,
                        };

                        self.frame_count_adjustment =
                            Some(MouseAdjustment::new(bulk.frame_count.get(), mouse_pos, dir));
                    } else if mouse.buttons.is_right_down() {
                        let (bulk, last_frame_idx) = bulk_and_last_frame_idx.next().unwrap();

                        let frame = &branch.frames[last_frame_idx];
                        let prev = &branch.frames[last_frame_idx - 1];

                        let pos = frame.state.player.pos;
                        let prev_pos = prev.state.player.pos;
                        let perp = perpendicular(prev_pos, pos) * 5.;

                        let a_screen = world_to_screen(pos + perp);
                        let b_screen = world_to_screen(pos - perp);

                        let adjustment_dir = match (a_screen, b_screen) {
                            (Some(a), Some(b)) => a - b,
                            // Presumably, one of the points is invisible, so just fall back.
                            _ => Vec2::X,
                        };

                        if let Some(MaxAccelOffsetValues {
                            start,
                            target,
                            accel,
                            ..
                        }) = bulk.max_accel_yaw_offset()
                        {
                            // Mode to know which to switch to.
                            // For left or right, it would be TargetAndEnd so we can turn better.
                            // For left-right and yaw, it would be Alt where we can change the
                            // first yaw field value.
                            let (mode, adjustment_dir, alt) = if let Some(AutoMovement::Strafe(
                                StrafeSettings { dir, .. },
                            )) = bulk.auto_actions.movement
                            {
                                // Match StrafiDir so we can get adjustment mode and
                                // adjustment_dir.
                                // Then match StrafeType to get values for alt.
                                match dir {
                                    StrafeDir::Left | StrafeDir::Right | StrafeDir::Best => {
                                        let adjustment_dir = if matches!(dir, StrafeDir::Right) {
                                            -adjustment_dir
                                        } else {
                                            adjustment_dir
                                        };

                                        (
                                            MaxAccelYawOffsetMode::StartAndTarget,
                                            adjustment_dir,
                                            MaxAccelYawOffsetYawField::None,
                                        )
                                    }
                                    StrafeDir::Yaw(yaw) | StrafeDir::Line { yaw } => (
                                        MaxAccelYawOffsetMode::Alt,
                                        adjustment_dir,
                                        MaxAccelYawOffsetYawField::Yaw(yaw),
                                    ),
                                    StrafeDir::LeftRight(count) | StrafeDir::RightLeft(count) => {
                                        let adjustment_dir =
                                            if matches!(dir, StrafeDir::RightLeft(_)) {
                                                -adjustment_dir
                                            } else {
                                                adjustment_dir
                                            };

                                        (
                                            MaxAccelYawOffsetMode::Alt,
                                            adjustment_dir,
                                            MaxAccelYawOffsetYawField::LeftRight(count),
                                        )
                                    }
                                    _ => return Err(ManualOpError::UserError(
                                        "Editor does not support current strafe dir for max accel yaw offset."
                                            .to_owned(),
                                    )),
                                }
                            } else {
                                unreachable!()
                            };

                            self.max_accel_yaw_offset_adjustment =
                                Some(MaxAccelYawOffsetAdjustment {
                                    mouse_adjustment: MouseAdjustment::new(
                                        MaxAccelYawOffsetMouseAdjustment {
                                            start: *start,
                                            target: *target,
                                            accel: *accel,
                                            alt,
                                        },
                                        mouse_pos,
                                        adjustment_dir,
                                    ),
                                    mode,
                                    mouse_offset: IVec2::ZERO,
                                    cycle_again: false,
                                });
                        } else if let Some(yaw) = bulk.yaw() {
                            self.yaw_adjustment =
                                Some(MouseAdjustment::new(*yaw, mouse_pos, adjustment_dir));
                        } else if let Some(count) = bulk.left_right_count() {
                            // Make the adjustment face the expected way.
                            let dir = match bulk.auto_actions.movement {
                                Some(AutoMovement::Strafe(StrafeSettings {
                                    dir: StrafeDir::RightLeft(_),
                                    ..
                                })) => -adjustment_dir,
                                _ => adjustment_dir,
                            };

                            self.left_right_count_adjustment =
                                Some(MouseAdjustment::new(count.get(), mouse_pos, dir));
                        } else if let Some(yawspeed) = bulk.yawspeed() {
                            // Make the adjustment face the expected way.
                            let dir = match bulk.auto_actions.movement {
                                Some(AutoMovement::Strafe(StrafeSettings {
                                    dir: StrafeDir::Right,
                                    ..
                                })) => -adjustment_dir,
                                _ => adjustment_dir,
                            };

                            self.side_strafe_yawspeed_adjustment =
                                Some(MouseAdjustment::new(*yawspeed, mouse_pos, dir))
                        }
                    } else if mouse.buttons.is_middle_down() {
                        let (bulk, last_frame_idx) = bulk_and_last_frame_idx.next().unwrap();

                        if bulk_and_last_frame_idx.next().is_some() {
                            let frame = &branch.frames[last_frame_idx];
                            let prev = &branch.frames[last_frame_idx - 1];

                            let frame_screen = world_to_screen(frame.state.player.pos);
                            let prev_screen = world_to_screen(prev.state.player.pos);

                            let dir = match (frame_screen, prev_screen) {
                                (Some(frame), Some(prev)) => frame - prev,
                                // Presumably, previous frame is invisible, so just fall back.
                                _ => Vec2::X,
                            };

                            self.adjacent_frame_count_adjustment =
                                Some(MouseAdjustment::new(bulk.frame_count.get(), mouse_pos, dir));
                        }
                    } else if mouse.buttons.is_mouse4_down() {
                        let (bulk, last_frame_idx) = bulk_and_last_frame_idx.next().unwrap();

                        let frame = &branch.frames[last_frame_idx];
                        let prev = &branch.frames[last_frame_idx - 1];

                        let pos = frame.state.player.pos;
                        let prev_pos = prev.state.player.pos;
                        let perp = perpendicular(prev_pos, pos) * 5.;

                        let a_screen = world_to_screen(pos + perp);
                        let b_screen = world_to_screen(pos - perp);

                        let dir = match (a_screen, b_screen) {
                            (Some(a), Some(b)) => a - b,
                            // Presumably, one of the points is invisible, so just fall back.
                            _ => Vec2::X,
                        };

                        let active_line_idx = branch
                            .branch
                            .script
                            .lines
                            .iter()
                            .enumerate()
                            .filter(|(_, line)| matches!(line, Line::FrameBulk(_)))
                            .nth(active_bulk_idx)
                            .unwrap()
                            .0;

                        let bulks = branch
                            .branch
                            .script
                            .lines
                            .iter()
                            .take(active_line_idx + 1)
                            .rev()
                            .filter_map(|prev_line| prev_line.frame_bulk())
                            .enumerate();

                        if let Some(yaw) = bulk.yaw() {
                            let affected_bulk_count_back = bulks
                                .take_while(|(_, prev_bulk)| prev_bulk.yaw() == Some(yaw))
                                .last()
                                .unwrap()
                                .0;
                            let first_bulk_idx = active_bulk_idx - affected_bulk_count_back;

                            let bulk_count = bulk_and_last_frame_idx
                                .take_while(|(next_bulk, _)| next_bulk.yaw() == Some(yaw))
                                .count()
                                + (active_bulk_idx - first_bulk_idx + 1);

                            self.adjacent_yaw_adjustment = Some(AdjacentYawAdjustment {
                                mouse_adjustment: MouseAdjustment::new(*yaw, mouse_pos, dir),
                                first_bulk_idx,
                                bulk_count,
                            });
                        } else if let Some(count) = bulk.left_right_count() {
                            let affected_bulk_count_back = bulks
                                .take_while(|(_, prev_bulk)| {
                                    prev_bulk.left_right_count() == Some(count)
                                })
                                .last()
                                .unwrap()
                                .0;
                            let first_bulk_idx = active_bulk_idx - affected_bulk_count_back;

                            let bulk_count = bulk_and_last_frame_idx
                                .take_while(|(next_bulk, _)| {
                                    next_bulk.left_right_count() == Some(count)
                                })
                                .count()
                                + (active_bulk_idx - first_bulk_idx + 1);

                            // Make the adjustment face the expected way.
                            let dir = match bulk.auto_actions.movement {
                                Some(AutoMovement::Strafe(StrafeSettings {
                                    dir: StrafeDir::RightLeft(_),
                                    ..
                                })) => -dir,
                                _ => dir,
                            };

                            self.adjacent_left_right_count_adjustment =
                                Some(AdjacentLeftRightCountAdjustment {
                                    mouse_adjustment: MouseAdjustment::new(
                                        count.get(),
                                        mouse_pos,
                                        dir,
                                    ),
                                    first_bulk_idx,
                                    bulk_count,
                                });
                        } else if let Some(yawspeed) = bulk.yawspeed() {
                            let affect_bulk_count_back = bulks
                                .take_while(|(_, prev_bulk)| prev_bulk.yawspeed() == Some(yawspeed))
                                .last()
                                .unwrap()
                                .0;
                            let first_bulk_idx = active_bulk_idx - affect_bulk_count_back;

                            let bulk_count = bulk_and_last_frame_idx
                                .take_while(|(next_bulk, _)| next_bulk.yawspeed() == Some(yawspeed))
                                .count()
                                + (active_bulk_idx - first_bulk_idx + 1);

                            // Make adjustment face the expected way.
                            let dir = match bulk.auto_actions.movement {
                                Some(AutoMovement::Strafe(StrafeSettings {
                                    dir: StrafeDir::Right,
                                    ..
                                })) => -dir,
                                _ => dir,
                            };

                            self.adjacent_side_strafe_yawspeed_adjustment =
                                Some(AdjacentYawspeedAdjustment {
                                    mouse_adjustment: MouseAdjustment::new(
                                        *yawspeed, mouse_pos, dir,
                                    ),
                                    first_bulk_idx,
                                    bulk_count,
                                });
                        }
                    }
                }
            }
        }

        // Update the hovered frame index.
        self.hovered_frame_idx =
            if self.is_any_adjustment_active() && self.selected_bulk_idx.is_some() {
                let (bulk, frame_idx) = bulk_and_first_frame_idx(self.script())
                    .nth(self.selected_bulk_idx.unwrap())
                    .unwrap();

                // Returned value from bulk_and_first_frame_idx might be outdated due to some
                // prediction going during adjustment. Ergo index out of bound.
                // Min is needed to make sure it never happens.
                Some(
                    (frame_idx + (bulk.frame_count.get() as usize) - 1)
                        .min(self.branch().frames.len() - 1),
                )
            } else {
                self.branch()
                    .frames
                    .iter()
                    .enumerate()
                    // Skip past hidden frames.
                    .filter(|(frame_idx, _)| *frame_idx >= self.first_shown_frame_idx)
                    // Convert to screen and take only successfully converted coordinates.
                    .filter_map(|(frame_idx, frame)| {
                        world_to_screen(frame.state.player.pos).map(|screen| (frame_idx, screen))
                    })
                    // Find closest to cursor.
                    .min_by(|(_, screen_a), (_, screen_b)| {
                        let dist_a = screen_a.distance_squared(mouse_pos);
                        let dist_b = screen_b.distance_squared(mouse_pos);
                        dist_a.total_cmp(&dist_b)
                    })
                    // Extract frame index.
                    .map(|(frame_idx, _)| frame_idx)
            };

        // Start the camera editor insert-camera-line adjustment here as, countrary to movement
        // editor adjustments, it starts from the hovered frame, updated just above.
        if self.in_camera_editor {
            if let Some(hovered_frame_idx) = self.hovered_frame_idx {
                if keyboard.insert_camera_line && !self.prev_keyboard_state.insert_camera_line {
                    // Keyboard was released last frame so the adjustment cannot be active.
                    assert!(self.insert_camera_line_adjustment.is_none());

                    // There are no other adjustments in the camera editor at the moment; and anyhow
                    // when more are added, this condition should still be upheld.
                    assert!(!self.is_any_adjustment_active());

                    let branch = self.branch_mut();

                    let frame = &branch.frames[hovered_frame_idx];
                    let prev = &branch.frames[hovered_frame_idx - 1];

                    let frame_screen = world_to_screen(frame.state.player.pos);
                    let prev_screen = world_to_screen(prev.state.player.pos);

                    let dir = match (frame_screen, prev_screen) {
                        (Some(frame), Some(prev)) => frame - prev,
                        // Presumably, previous frame is invisible, so just fall back.
                        _ => Vec2::X,
                    };

                    let lines = &mut branch.branch.script.lines;

                    let line = Line::VectorialStrafingConstraints(
                        VectorialStrafingConstraints::VelocityYawLocking { tolerance: 0. },
                    );

                    let (line_idx, repeat) =
                        line_idx_and_repeat_at_frame(lines, hovered_frame_idx).unwrap();

                    let camera_line_idx = if repeat == 0 {
                        lines.insert(line_idx, line);
                        line_idx
                    } else {
                        let Line::FrameBulk(bulk) = &mut lines[line_idx] else {
                            unreachable!()
                        };

                        let mut new_bulk = bulk.clone();
                        bulk.frame_count = NonZeroU32::new(repeat).unwrap();
                        new_bulk.frame_count =
                            NonZeroU32::new(new_bulk.frame_count.get() - repeat).unwrap();

                        lines.insert(line_idx + 1, line);
                        lines.insert(line_idx + 2, Line::FrameBulk(new_bulk));
                        line_idx + 1
                    };

                    let initial_yaw = branch.frames[hovered_frame_idx]
                        .state
                        .prev_frame_input
                        .yaw
                        .to_degrees();

                    // This unfortunately means we won't have any predicted frames past this
                    // until the next tick, but oh well.
                    branch.extra_cam.clear();
                    self.invalidate(hovered_frame_idx + 1);
                    self.recompute_extra_camera_frame_data_if_needed();

                    // Reset the selected bulk index as dragging the insert camera line adjustment
                    // around will split and rejoin frame bulks and generally mess with the index.
                    self.selected_bulk_idx = None;

                    self.insert_camera_line_adjustment = Some(InsertCameraLineAdjustment {
                        mouse_adjustment: MouseAdjustment::new(0, mouse_pos, dir),
                        starting_frame_idx: hovered_frame_idx,
                        camera_line_idx,
                        initial_yaw,
                        did_split: repeat != 0,
                    });
                }
            }
        }

        // Finally, update the previous mouse and keyboard state.
        self.prev_mouse_state = mouse;
        self.prev_keyboard_state = keyboard;

        Ok(())
    }

    fn tick_frame_count_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(adjustment) = &mut self.frame_count_adjustment else {
            return Ok(());
        };

        let bulk_idx = self.selected_bulk_idx.unwrap();
        let (bulk, first_frame_idx) =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .nth(bulk_idx)
                .unwrap();

        if !mouse.buttons.is_left_down() {
            if !adjustment.changed_once {
                self.frame_count_adjustment = None;
                return Ok(());
            }

            let op = Operation::SetFrameCount {
                bulk_idx,
                from: adjustment.original_value,
                to: bulk.frame_count.get(),
            };
            self.frame_count_adjustment = None;
            return self.store_operation(op);
        }

        let speed = keyboard.adjustment_speed();
        let delta = (adjustment.delta(mouse.pos.as_vec2()) * 0.1 * speed).round() as i32;
        let new_frame_count = adjustment
            .original_value
            .saturating_add_signed(delta)
            .max(1);

        let frame_count = bulk.frame_count.get();
        if frame_count != new_frame_count {
            adjustment.changed_once = true;
            bulk.frame_count = NonZeroU32::new(new_frame_count).unwrap();
            self.invalidate(first_frame_idx + min(frame_count, new_frame_count) as usize);
        }

        Ok(())
    }

    fn tick_yaw_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(adjustment) = &mut self.yaw_adjustment else {
            return Ok(());
        };

        let bulk_idx = self.selected_bulk_idx.unwrap();
        let (bulk, first_frame_idx) =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .nth(bulk_idx)
                .unwrap();

        let yaw = bulk.yaw_mut().unwrap();

        if !mouse.buttons.is_right_down() {
            if !adjustment.changed_once {
                self.yaw_adjustment = None;
                return Ok(());
            }

            let op = Operation::SetYaw {
                bulk_idx,
                from: adjustment.original_value,
                to: *yaw,
            };
            self.yaw_adjustment = None;
            return self.store_operation(op);
        }

        let speed = keyboard.adjustment_speed();
        let delta = adjustment.delta(mouse.pos.as_vec2()) * 0.1 * speed;
        let new_yaw = adjustment.original_value + delta;

        if *yaw != new_yaw {
            adjustment.changed_once = true;
            *yaw = new_yaw;
            self.invalidate(first_frame_idx);
        }

        Ok(())
    }

    fn tick_left_right_count_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(adjustment) = &mut self.left_right_count_adjustment else {
            return Ok(());
        };

        let bulk_idx = self.selected_bulk_idx.unwrap();
        let (bulk, first_frame_idx) =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .nth(bulk_idx)
                .unwrap();

        let left_right_count = bulk.left_right_count_mut().unwrap();

        if !mouse.buttons.is_right_down() {
            if !adjustment.changed_once {
                self.left_right_count_adjustment = None;
                return Ok(());
            }

            let op = Operation::SetLeftRightCount {
                bulk_idx,
                from: adjustment.original_value,
                to: left_right_count.get(),
            };
            self.left_right_count_adjustment = None;
            return self.store_operation(op);
        }

        let speed = keyboard.adjustment_speed();
        let delta = (adjustment.delta(mouse.pos.as_vec2()) * 0.1 * speed).round() as i32;
        let new_left_right_count = adjustment
            .original_value
            .saturating_add_signed(delta)
            .max(1);

        if left_right_count.get() != new_left_right_count {
            adjustment.changed_once = true;
            *left_right_count = NonZeroU32::new(new_left_right_count).unwrap();
            self.invalidate(first_frame_idx);
        }

        Ok(())
    }

    fn tick_adjacent_frame_count_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(adjustment) = &mut self.adjacent_frame_count_adjustment else {
            return Ok(());
        };

        let bulk_idx = self.selected_bulk_idx.unwrap();
        let mut bulks =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .skip(bulk_idx);
        let (bulk, first_frame_idx) = bulks.next().unwrap();

        if !mouse.buttons.is_middle_down() {
            drop(bulks);

            if !adjustment.changed_once {
                self.adjacent_frame_count_adjustment = None;
                return Ok(());
            }

            let op = Operation::SetAdjacentFrameCount {
                bulk_idx,
                from: adjustment.original_value,
                to: bulk.frame_count.get(),
            };
            self.adjacent_frame_count_adjustment = None;
            return self.store_operation(op);
        }

        let next_bulk = bulks.next().unwrap().0;
        drop(bulks);

        let speed = keyboard.adjustment_speed();
        let delta = (adjustment.delta(mouse.pos.as_vec2()) * 0.1 * speed).round() as i32;
        let new_frame_count = adjustment
            .original_value
            .saturating_add_signed(delta)
            .max(1);

        let frame_count = bulk.frame_count.get();

        let max_delta_from_current = next_bulk.frame_count.get() - 1;
        let delta_from_current =
            (new_frame_count as i64 - frame_count as i64).min(max_delta_from_current as i64);
        let new_frame_count = u32::try_from(frame_count as i64 + delta_from_current).unwrap();

        if frame_count != new_frame_count {
            adjustment.changed_once = true;
            bulk.frame_count = NonZeroU32::new(new_frame_count).unwrap();

            let new_next_frame_count = next_bulk.frame_count.get() as i64 - delta_from_current;
            next_bulk.frame_count =
                NonZeroU32::new(u32::try_from(new_next_frame_count).unwrap()).unwrap();

            self.invalidate(first_frame_idx + min(frame_count, new_frame_count) as usize);
        }

        Ok(())
    }

    fn tick_adjacent_yaw_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(AdjacentYawAdjustment {
            mouse_adjustment,
            first_bulk_idx,
            bulk_count,
        }) = &mut self.adjacent_yaw_adjustment
        else {
            return Ok(());
        };

        let mut bulks =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .skip(*first_bulk_idx);
        let (bulk, first_frame_idx) = bulks.next().unwrap();

        let yaw = bulk.yaw_mut().unwrap();

        if !mouse.buttons.is_mouse4_down() {
            drop(bulks);

            if !mouse_adjustment.changed_once {
                self.adjacent_yaw_adjustment = None;
                return Ok(());
            }

            let op = Operation::SetAdjacentYaw {
                first_bulk_idx: *first_bulk_idx,
                bulk_count: *bulk_count,
                from: mouse_adjustment.original_value,
                to: *yaw,
            };
            self.adjacent_yaw_adjustment = None;
            return self.store_operation(op);
        }

        let speed = keyboard.adjustment_speed();
        let delta = mouse_adjustment.delta(mouse.pos.as_vec2()) * 0.1 * speed;
        let new_yaw = mouse_adjustment.original_value + delta;

        if *yaw != new_yaw {
            mouse_adjustment.changed_once = true;

            for _ in 1..*bulk_count {
                let bulk = bulks.next().unwrap().0;
                let next_yaw = bulk.yaw_mut().unwrap();
                *next_yaw = new_yaw;
            }
            drop(bulks);

            *yaw = new_yaw;
            self.invalidate(first_frame_idx);
        }

        Ok(())
    }

    fn tick_adjacent_left_right_count_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(AdjacentLeftRightCountAdjustment {
            mouse_adjustment,
            first_bulk_idx,
            bulk_count,
        }) = &mut self.adjacent_left_right_count_adjustment
        else {
            return Ok(());
        };

        let mut bulks =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .skip(*first_bulk_idx);
        let (bulk, first_frame_idx) = bulks.next().unwrap();

        let left_right_count = bulk.left_right_count_mut().unwrap();

        if !mouse.buttons.is_mouse4_down() {
            drop(bulks);

            if !mouse_adjustment.changed_once {
                self.adjacent_left_right_count_adjustment = None;
                return Ok(());
            }

            let op = Operation::SetAdjacentLeftRightCount {
                first_bulk_idx: *first_bulk_idx,
                bulk_count: *bulk_count,
                from: mouse_adjustment.original_value,
                to: left_right_count.get(),
            };
            self.adjacent_left_right_count_adjustment = None;
            return self.store_operation(op);
        }

        let speed = keyboard.adjustment_speed();
        let delta = (mouse_adjustment.delta(mouse.pos.as_vec2()) * 0.1 * speed).round() as i32;
        let new_left_right_count = mouse_adjustment
            .original_value
            .saturating_add_signed(delta)
            .max(1);

        if left_right_count.get() != new_left_right_count {
            mouse_adjustment.changed_once = true;

            let new_left_right_count = NonZeroU32::new(new_left_right_count).unwrap();

            for _ in 1..*bulk_count {
                let bulk = bulks.next().unwrap().0;
                let next_left_right_count = bulk.left_right_count_mut().unwrap();
                *next_left_right_count = new_left_right_count;
            }
            drop(bulks);

            *left_right_count = new_left_right_count;
            self.invalidate(first_frame_idx);
        }

        Ok(())
    }

    fn tick_side_strafe_yawspeed_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(adjustment) = &mut self.side_strafe_yawspeed_adjustment else {
            return Ok(());
        };

        let bulk_idx = self.selected_bulk_idx.unwrap();
        let (bulk, first_frame_idx) =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .nth(bulk_idx)
                .unwrap();

        let yawspeed = bulk.yawspeed_mut().unwrap();

        if !mouse.buttons.is_right_down() {
            if !adjustment.changed_once {
                self.side_strafe_yawspeed_adjustment = None;
                return Ok(());
            }

            let op = Operation::SetYawspeed {
                bulk_idx,
                from: adjustment.original_value,
                to: *yawspeed,
            };
            self.side_strafe_yawspeed_adjustment = None;
            return self.store_operation(op);
        }

        let speed = keyboard.adjustment_speed();
        let delta = adjustment.delta(mouse.pos.as_vec2()) * 1. * speed;
        let new_yawspeed = (adjustment.original_value + delta).max(0.);

        if *yawspeed != new_yawspeed {
            adjustment.changed_once = true;
            *yawspeed = new_yawspeed;
            self.invalidate(first_frame_idx);
        }

        Ok(())
    }

    fn tick_adjacent_side_strafe_yawspeed_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(AdjacentYawspeedAdjustment {
            mouse_adjustment,
            first_bulk_idx,
            bulk_count,
        }) = &mut self.adjacent_side_strafe_yawspeed_adjustment
        else {
            return Ok(());
        };

        let mut bulks =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .skip(*first_bulk_idx);
        let (bulk, first_frame_idx) = bulks.next().unwrap();

        let yawspeed = bulk.yawspeed_mut().unwrap();

        if !mouse.buttons.is_mouse4_down() {
            drop(bulks);

            if !mouse_adjustment.changed_once {
                self.adjacent_side_strafe_yawspeed_adjustment = None;
                return Ok(());
            }

            let op = Operation::SetAdjacentYawspeed {
                first_bulk_idx: *first_bulk_idx,
                bulk_count: *bulk_count,
                from: mouse_adjustment.original_value,
                to: *yawspeed,
            };

            self.adjacent_side_strafe_yawspeed_adjustment = None;
            return self.store_operation(op);
        }

        let speed = keyboard.adjustment_speed();
        let delta = (mouse_adjustment.delta(mouse.pos.as_vec2())) * 1. * speed;
        let new_yawspeed = (mouse_adjustment.original_value + delta).max(0.);

        if *yawspeed != new_yawspeed {
            mouse_adjustment.changed_once = true;

            for _ in 1..*bulk_count {
                let bulk = bulks.next().unwrap().0;
                let next_yawspeed = bulk.yawspeed_mut().unwrap();
                *next_yawspeed = new_yawspeed;
            }
            drop(bulks);

            *yawspeed = new_yawspeed;
            self.invalidate(first_frame_idx);
        }

        Ok(())
    }

    fn tick_max_accel_yaw_offset_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
        prev_mouse: MouseState,
    ) -> eyre::Result<()> {
        let Some(adjustment) = &mut self.max_accel_yaw_offset_adjustment else {
            return Ok(());
        };

        let bulk_idx = self.selected_bulk_idx.unwrap();
        let (bulk, first_frame_idx) =
            bulk_and_first_frame_idx_mut(&mut self.branches[self.branch_idx].branch.script)
                .nth(bulk_idx)
                .unwrap();

        let MaxAccelOffsetValuesMut {
            start,
            target,
            accel,
            mut yaw,
            mut count,
        } = bulk.max_accel_yaw_offset_mut().unwrap();

        if !mouse.buttons.is_right_down() {
            if !adjustment.mouse_adjustment.changed_once {
                self.max_accel_yaw_offset_adjustment = None;
                return Ok(());
            }

            let op = match adjustment.mode {
                MaxAccelYawOffsetMode::StartAndTarget => {
                    Operation::SetMaxAccelOffsetStartAndTarget {
                        bulk_idx,
                        from: (
                            adjustment.mouse_adjustment.original_value.start,
                            adjustment.mouse_adjustment.original_value.target,
                        ),
                        to: (*start, *target),
                    }
                }
                MaxAccelYawOffsetMode::Start => Operation::SetMaxAccelOffsetStart {
                    bulk_idx,
                    from: adjustment.mouse_adjustment.original_value.start,
                    to: *start,
                },
                MaxAccelYawOffsetMode::Target => Operation::SetMaxAccelOffsetTarget {
                    bulk_idx,
                    from: adjustment.mouse_adjustment.original_value.target,
                    to: *target,
                },
                MaxAccelYawOffsetMode::Acceleration => Operation::SetMaxAccelOffsetAccel {
                    bulk_idx,
                    from: adjustment.mouse_adjustment.original_value.accel,
                    to: *accel,
                },
                MaxAccelYawOffsetMode::Alt => {
                    match adjustment.mouse_adjustment.original_value.alt {
                        // Can't do anything.
                        MaxAccelYawOffsetYawField::None => {
                            self.max_accel_yaw_offset_adjustment = None;
                            return Ok(());
                        }
                        MaxAccelYawOffsetYawField::Yaw(from) => Operation::SetYaw {
                            bulk_idx,
                            from,
                            to: *yaw.unwrap(),
                        },
                        MaxAccelYawOffsetYawField::LeftRight(from) => {
                            Operation::SetLeftRightCount {
                                bulk_idx,
                                from: from.get(),
                                to: count.unwrap().get(),
                            }
                        }
                    }
                }
            };

            self.max_accel_yaw_offset_adjustment = None;
            return self.store_operation(op);
        }

        let mut should_invalidate = false;

        // Mouse left click to switch mode.
        // This must happen before the delta calculation.
        if (mouse.buttons.is_left_down() && !prev_mouse.buttons.is_left_down())
            || adjustment.cycle_again
        {
            // Cycling
            adjustment.mode = adjustment.mode.cycle();

            // After switching mode, we have to reset all of the changes back to the original.
            *start = adjustment.mouse_adjustment.original_value.start;
            *target = adjustment.mouse_adjustment.original_value.target;
            *accel = adjustment.mouse_adjustment.original_value.accel;

            if let MaxAccelYawOffsetYawField::Yaw(original) =
                adjustment.mouse_adjustment.original_value.alt
            {
                let binding = yaw.as_deref_mut();
                *binding.unwrap() = original;
            }

            if let MaxAccelYawOffsetYawField::LeftRight(original) =
                adjustment.mouse_adjustment.original_value.alt
            {
                let binding = count.as_deref_mut();
                *binding.unwrap() = original;
            }

            adjustment.mouse_adjustment.changed_once = false;

            // Change mouse position so that all of the delta for adjustment is reset.
            adjustment.mouse_offset = mouse.pos
                - IVec2::new(
                    adjustment.mouse_adjustment.pressed_at.x as i32,
                    adjustment.mouse_adjustment.pressed_at.y as i32,
                );

            should_invalidate = true;
            adjustment.cycle_again = false;
        }

        let speed = keyboard.adjustment_speed();
        let delta = adjustment
            .mouse_adjustment
            // Mouse offset is used here to reset "pressed_at".
            .delta((mouse.pos - adjustment.mouse_offset).as_vec2())
            / 50. // Yes
            * speed;

        match adjustment.mode {
            MaxAccelYawOffsetMode::StartAndTarget => {
                let new_start = adjustment.mouse_adjustment.original_value.start + delta;
                let new_target = adjustment.mouse_adjustment.original_value.target + delta;

                if *start != new_start {
                    adjustment.mouse_adjustment.changed_once = true;
                    *start = new_start;
                    should_invalidate = true;
                }

                if *target != new_target {
                    adjustment.mouse_adjustment.changed_once = true;
                    *target = new_target;
                    should_invalidate = true;
                }
            }
            MaxAccelYawOffsetMode::Target => {
                let new_target = adjustment.mouse_adjustment.original_value.target + delta;

                if *target != new_target {
                    adjustment.mouse_adjustment.changed_once = true;
                    *target = new_target;
                    should_invalidate = true;
                }
            }
            MaxAccelYawOffsetMode::Acceleration => {
                // Accel is very delicate. Need to tone it down.
                let delta = delta / 20.;
                let new_accel = adjustment.mouse_adjustment.original_value.accel + delta;

                if *accel != new_accel {
                    adjustment.mouse_adjustment.changed_once = true;
                    *accel = new_accel;
                    should_invalidate = true;
                }
            }
            MaxAccelYawOffsetMode::Start => {
                let new_start = adjustment.mouse_adjustment.original_value.start + delta;

                if *start != new_start {
                    adjustment.mouse_adjustment.changed_once = true;
                    *start = new_start;
                    should_invalidate = true;
                }
            }
            MaxAccelYawOffsetMode::Alt => {
                // Undo delta decrease so we can adjust yaw and left right count.
                // * 0.1 to match the left-right and yaw adjustment.
                let delta = delta * 50. * 0.1;

                match adjustment.mouse_adjustment.original_value.alt {
                    MaxAccelYawOffsetYawField::None => {
                        adjustment.cycle_again = true;
                    }
                    MaxAccelYawOffsetYawField::Yaw(from) => {
                        let new_yaw = from + delta;

                        // We make sure the original_value.alt is correct.
                        if *yaw.as_deref().unwrap() != new_yaw {
                            adjustment.mouse_adjustment.changed_once = true;
                            *yaw.unwrap() = new_yaw;
                            should_invalidate = true;
                        }
                    }
                    MaxAccelYawOffsetYawField::LeftRight(from) => {
                        let new_count = from
                            .get()
                            .saturating_add_signed((delta).round() as i32)
                            .max(1);

                        if count.as_ref().unwrap().get() != new_count {
                            adjustment.mouse_adjustment.changed_once = true;
                            *count.unwrap() = NonZeroU32::new(new_count).unwrap();
                            should_invalidate = true;
                        }
                    }
                };
            }
        }

        if should_invalidate {
            self.invalidate(first_frame_idx);
        }

        Ok(())
    }

    fn tick_insert_camera_line_adjustment(
        &mut self,
        mouse: MouseState,
        keyboard: KeyboardState,
    ) -> eyre::Result<()> {
        let Some(InsertCameraLineAdjustment {
            mouse_adjustment,
            starting_frame_idx,
            camera_line_idx,
            initial_yaw,
            did_split,
        }) = &mut self.insert_camera_line_adjustment
        else {
            return Ok(());
        };

        let branch = &mut self.branches[self.branch_idx];

        if !keyboard.insert_camera_line {
            let line = &branch.branch.script.lines[*camera_line_idx];

            let op = if *did_split {
                let mut prev_line = branch.branch.script.lines[*camera_line_idx - 1].clone();
                let next_line = &branch.branch.script.lines[*camera_line_idx + 1];

                let mut buffer = Vec::new();
                hltas::write::gen_lines(&mut buffer, [&prev_line, line, next_line])
                    .expect("writing to an in-memory buffer should never fail");
                let to = String::from_utf8(buffer)
                    .expect("Line serialization should never produce invalid UTF-8");

                join_lines(&mut prev_line, next_line);
                let mut buffer = Vec::new();
                hltas::write::gen_lines(&mut buffer, [&prev_line])
                    .expect("writing to an in-memory buffer should never fail");
                let from = String::from_utf8(buffer)
                    .expect("Line serialization should never produce invalid UTF-8");

                Operation::ReplaceMultiple {
                    first_line_idx: *camera_line_idx - 1,
                    from,
                    to,
                }
            } else {
                let mut buffer = Vec::new();
                hltas::write::gen_lines(&mut buffer, [line])
                    .expect("writing to an in-memory buffer should never fail");
                let line = String::from_utf8(buffer)
                    .expect("Line serialization should never produce invalid UTF-8");

                Operation::Insert {
                    line_idx: *camera_line_idx,
                    line,
                }
            };

            self.insert_camera_line_adjustment = None;
            return self.store_operation(op);
        }

        let speed = keyboard.adjustment_speed();
        let delta = (mouse_adjustment.delta(mouse.pos.as_vec2()) * 0.1 * speed).round() as i32;
        // The original value is always 0 in this case so just use delta as is.
        //
        // Only allow dragging back from the original point as this is what makes the most sense.
        let delta = delta.min(0);

        let mut invalidate = false;

        // Switch the line type if dragging in or out of the initial frame.
        let line = &mut branch.branch.script.lines[*camera_line_idx];
        match line {
            Line::VectorialStrafingConstraints(_) => {
                if delta == 0 {
                    // We remain on the initial frame, nothing to change.
                    return Ok(());
                }

                // We dragged out of the initial frame, switch the line to a change.
                *line = Line::Change(Change {
                    target: ChangeTarget::VectorialStrafingYaw,
                    final_value: *initial_yaw,
                    over: 0.,
                });
                invalidate = true;
            }
            Line::Change(Change { .. }) => {
                if delta == 0 {
                    // We dragged back into the initial frame, switch the line back to target_yaw
                    // velocity_lock.
                    *line = Line::VectorialStrafingConstraints(
                        VectorialStrafingConstraints::VelocityYawLocking { tolerance: 0. },
                    );
                    invalidate = true;
                }
            }
            _ => unreachable!(),
        }

        let new_frame_idx = starting_frame_idx.saturating_add_signed(delta as isize);
        let curr_frame_idx = line_first_frame_idx(&branch.branch.script)
            .nth(*camera_line_idx)
            .unwrap()
            - 1;

        if new_frame_idx == curr_frame_idx {
            // No change.
            assert!(!invalidate);
            return Ok(());
        }

        let mut line = branch.branch.script.lines.remove(*camera_line_idx);
        let lines = &mut branch.branch.script.lines;

        // Try to re-join frame bulks we might have split before.
        if *did_split {
            let next_line = lines[*camera_line_idx].clone();
            let prev_line = &mut lines[*camera_line_idx - 1];

            join_lines(prev_line, &next_line);
            lines.remove(*camera_line_idx);
        }

        let (line_idx, repeat) = line_idx_and_repeat_at_frame(&*lines, new_frame_idx).unwrap();

        if let Line::Change(Change { over, .. }) = &mut line {
            *over = 0.;
            for (_, bulk, _) in bulk_idx_and_is_last(&*lines)
                .take(*starting_frame_idx)
                .skip(new_frame_idx)
            {
                *over += bulk.frame_time.parse::<f32>().unwrap_or(0.);
            }
            *over -= 1e-6;
        };

        if repeat == 0 {
            lines.insert(line_idx, line);
            *camera_line_idx = line_idx;
            *did_split = false;
        } else {
            let Line::FrameBulk(bulk) = &mut lines[line_idx] else {
                unreachable!()
            };

            let mut new_bulk = bulk.clone();
            bulk.frame_count = NonZeroU32::new(repeat).unwrap();
            new_bulk.frame_count = NonZeroU32::new(new_bulk.frame_count.get() - repeat).unwrap();

            lines.insert(line_idx + 1, line);
            lines.insert(line_idx + 2, Line::FrameBulk(new_bulk));
            *camera_line_idx = line_idx + 1;
            *did_split = true;
        }

        self.invalidate(new_frame_idx);

        Ok(())
    }

    pub fn cancel_ongoing_adjustments(&mut self) {
        if let Some(adjustment) = self.frame_count_adjustment.take() {
            let original_value = adjustment.original_value;

            let bulk_idx = self.selected_bulk_idx.unwrap();
            let (bulk, first_frame_idx) =
                bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script)
                    .nth(bulk_idx)
                    .unwrap();

            let frame_count = bulk.frame_count.get();
            if frame_count != original_value {
                bulk.frame_count = NonZeroU32::new(original_value).unwrap();
                self.invalidate(first_frame_idx + min(frame_count, original_value) as usize);
            }
        }

        if let Some(adjustment) = self.yaw_adjustment.take() {
            let original_value = adjustment.original_value;

            let bulk_idx = self.selected_bulk_idx.unwrap();
            let (bulk, first_frame_idx) =
                bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script)
                    .nth(bulk_idx)
                    .unwrap();

            let yaw = bulk.yaw_mut().unwrap();
            if *yaw != original_value {
                *yaw = original_value;
                self.invalidate(first_frame_idx);
            }
        }

        if let Some(adjustment) = self.left_right_count_adjustment.take() {
            let original_value = adjustment.original_value;

            let bulk_idx = self.selected_bulk_idx.unwrap();
            let (bulk, first_frame_idx) =
                bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script)
                    .nth(bulk_idx)
                    .unwrap();

            let left_right_count = bulk.left_right_count_mut().unwrap();
            if left_right_count.get() != original_value {
                *left_right_count = NonZeroU32::new(original_value).unwrap();
                self.invalidate(first_frame_idx);
            }
        }

        if let Some(adjustment) = self.adjacent_frame_count_adjustment.take() {
            let original_value = adjustment.original_value;

            let bulk_idx = self.selected_bulk_idx.unwrap();
            let mut bulks =
                bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script).skip(bulk_idx);
            let (bulk, first_frame_idx) = bulks.next().unwrap();
            let next_bulk = bulks.next().unwrap().0;
            drop(bulks);

            let frame_count = bulk.frame_count.get();
            if frame_count != original_value {
                bulk.frame_count = NonZeroU32::new(original_value).unwrap();

                let delta = original_value as i64 - frame_count as i64;
                next_bulk.frame_count =
                    NonZeroU32::new((next_bulk.frame_count.get() as i64 - delta) as u32).unwrap();

                self.invalidate(first_frame_idx + min(frame_count, original_value) as usize);
            }
        }

        if let Some(AdjacentYawAdjustment {
            mouse_adjustment,
            first_bulk_idx,
            bulk_count,
        }) = self.adjacent_yaw_adjustment.take()
        {
            let original_value = mouse_adjustment.original_value;

            let mut bulks = bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script)
                .skip(first_bulk_idx);
            let (bulk, first_frame_idx) = bulks.next().unwrap();

            let yaw = bulk.yaw_mut().unwrap();
            if *yaw != original_value {
                for _ in 1..bulk_count {
                    let bulk = bulks.next().unwrap().0;
                    let next_yaw = bulk.yaw_mut().unwrap();
                    *next_yaw = original_value;
                }

                *yaw = original_value;

                drop(bulks);
                self.invalidate(first_frame_idx);
            }
        }

        if let Some(AdjacentLeftRightCountAdjustment {
            mouse_adjustment,
            first_bulk_idx,
            bulk_count,
        }) = self.adjacent_left_right_count_adjustment.take()
        {
            let original_value = mouse_adjustment.original_value;

            let mut bulks = bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script)
                .skip(first_bulk_idx);
            let (bulk, first_frame_idx) = bulks.next().unwrap();

            let left_right_count = bulk.left_right_count_mut().unwrap();
            if left_right_count.get() != original_value {
                let original_value = NonZeroU32::new(original_value).unwrap();

                for _ in 1..bulk_count {
                    let bulk = bulks.next().unwrap().0;
                    let next_left_right_count = bulk.left_right_count_mut().unwrap();
                    *next_left_right_count = original_value;
                }

                *left_right_count = original_value;

                drop(bulks);
                self.invalidate(first_frame_idx);
            }
        }

        if let Some(adjustment) = self.side_strafe_yawspeed_adjustment.take() {
            let original_value = adjustment.original_value;

            let bulk_idx = self.selected_bulk_idx.unwrap();
            let (bulk, first_frame_idx) =
                bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script)
                    .nth(bulk_idx)
                    .unwrap();

            let yawspeed = bulk.yawspeed_mut().unwrap();
            if *yawspeed != original_value {
                *yawspeed = original_value;
                self.invalidate(first_frame_idx);
            }
        }

        if let Some(AdjacentYawspeedAdjustment {
            mouse_adjustment,
            first_bulk_idx,
            bulk_count,
        }) = self.adjacent_side_strafe_yawspeed_adjustment.take()
        {
            let original_value = mouse_adjustment.original_value;

            let mut bulks = bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script)
                .skip(first_bulk_idx);
            let (bulk, first_frame_idx) = bulks.next().unwrap();

            let yawspeed = bulk.yawspeed_mut().unwrap();
            if *yawspeed != original_value {
                for _ in 1..bulk_count {
                    let bulk = bulks.next().unwrap().0;
                    let next_yawspeed = bulk.yawspeed_mut().unwrap();
                    *next_yawspeed = original_value;
                }

                *yawspeed = original_value;

                drop(bulks);
                self.invalidate(first_frame_idx);
            }
        }

        if let Some(adjustment) = self.max_accel_yaw_offset_adjustment.take() {
            let original_value = adjustment.mouse_adjustment.original_value;

            let bulk_idx = self.selected_bulk_idx.unwrap();
            let (bulk, first_frame_idx) =
                bulk_and_first_frame_idx_mut(&mut self.branch_mut().branch.script)
                    .nth(bulk_idx)
                    .unwrap();

            let MaxAccelOffsetValuesMut {
                start,
                target,
                accel,
                yaw,
                count,
            } = bulk.max_accel_yaw_offset_mut().unwrap();
            let mut should_invalidate = false;

            if (*start, *target, *accel)
                != (
                    original_value.start,
                    original_value.target,
                    original_value.accel,
                )
            {
                *start = original_value.start;
                *target = original_value.target;
                *accel = original_value.accel;

                should_invalidate = true;
            }

            if let MaxAccelYawOffsetYawField::Yaw(original) =
                adjustment.mouse_adjustment.original_value.alt
            {
                if *yaw.as_deref().unwrap() != original {
                    *yaw.unwrap() = original;
                }

                should_invalidate = true;
            }

            if let MaxAccelYawOffsetYawField::LeftRight(original) =
                adjustment.mouse_adjustment.original_value.alt
            {
                if *count.as_deref().unwrap() != original {
                    *count.unwrap() = original;
                }

                should_invalidate = true;
            }

            if should_invalidate {
                self.invalidate(first_frame_idx);
            }
        }

        if let Some(InsertCameraLineAdjustment {
            camera_line_idx,
            did_split,
            ..
        }) = self.insert_camera_line_adjustment.take()
        {
            let branch = &mut self.branches[self.branch_idx];
            let curr_frame_idx = line_first_frame_idx(&branch.branch.script)
                .nth(camera_line_idx)
                .unwrap()
                - 1;

            branch.branch.script.lines.remove(camera_line_idx);

            if did_split {
                let lines = &mut branch.branch.script.lines;
                let next_line = lines[camera_line_idx].clone();
                let prev_line = &mut lines[camera_line_idx - 1];

                join_lines(prev_line, &next_line);
                lines.remove(camera_line_idx);
            }

            self.invalidate(curr_frame_idx);
        }
    }

    /// Stores already-applied operation.
    fn store_operation(&mut self, op: Operation) -> eyre::Result<()> {
        let action = Action {
            branch_id: self.branch().branch.branch_id,
            kind: ActionKind::ApplyOperation(op),
        };
        self.undo_log.push(action.clone());
        self.redo_log.clear();
        self.db
            .update_with_action(&self.branches[self.branch_idx].branch, &action.kind)?;
        Ok(())
    }

    /// Applies operation to editor.
    fn apply_operation(&mut self, op: Operation) -> eyre::Result<()> {
        let selected_line_idx = self.selected_bulk_idx.map(|idx| {
            self.branch()
                .branch
                .script
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| matches!(line, Line::FrameBulk(_)))
                .nth(idx)
                .unwrap()
                .0
        });

        if let Some(frame_idx) = op.apply(&mut self.branch_mut().branch.script) {
            self.invalidate(frame_idx);
        }

        // Adjust the selection if needed.
        let script = &self.branch().branch.script;
        if let Some(selected_line_idx) = selected_line_idx {
            match op {
                Operation::Delete { line_idx, .. } => {
                    // Selected bulk index isn't None because selected_line_idx is computed from it.
                    let selected_bulk_idx = self.selected_bulk_idx.unwrap();

                    #[allow(clippy::comparison_chain)]
                    if line_idx == selected_line_idx {
                        // The selected bulk was deleted. In this case, the selected bulk index
                        // should remain unchanged (the bulk right after the deleted one should
                        // be selected), or, if it was the last bulk, the previous one should
                        // be selected.
                        if script.frame_bulks().nth(selected_bulk_idx).is_none() {
                            // This was the last bulk in the script.
                            if selected_bulk_idx == 0 {
                                // There are no bulks left.
                                self.selected_bulk_idx = None;
                            } else {
                                // Select the previous bulk.
                                self.selected_bulk_idx = Some(selected_bulk_idx - 1);
                            }
                        }
                        // Otherwise, leave the index as is (selecting the next bulk).
                    } else if line_idx < selected_line_idx {
                        // Something before the selected bulk was deleted. If that was a frame bulk,
                        // move the selected bulk index one back to preserve the selection.
                        if script.lines[line_idx].frame_bulk().is_some() {
                            self.selected_bulk_idx = Some(selected_bulk_idx - 1);
                        }
                    }
                    // Otherwise, something was deleted ahead of the selected frame bulk, and we
                    // don't need to change anything.
                }
                Operation::Replace { line_idx, .. } => {
                    // TODO: handle this smarter
                    if line_idx <= selected_line_idx {
                        // TODO: if less and deleted bulk, move active bulk idx back
                        if script.lines[line_idx].frame_bulk().is_none() {
                            // Frame bulk was replaced by non-frame-bulk.
                            self.selected_bulk_idx = None;
                        }
                    }
                }
                Operation::ReplaceMultiple { first_line_idx, .. } => {
                    // TODO: handle this smarter
                    if first_line_idx <= selected_line_idx {
                        self.selected_bulk_idx = None;
                    }
                }
                Operation::Rewrite { .. } => {
                    self.selected_bulk_idx = None;
                }
                _ => (),
            }
        }

        // TODO: Since splitting does not invalidate frames, and we currently don't have an .hltas
        // invalidation mechnic, manually check for and recompute the camera data here.
        if let Operation::Split { .. } = op {
            self.branch_mut().extra_cam.clear();
            self.recompute_extra_camera_frame_data_if_needed();
        }

        self.store_operation(op)
    }

    /// Undoes the last action if any.
    pub fn undo(&mut self) -> ManualOpResult<()> {
        // Don't undo during active adjustments because:
        // 1. adustments store the orginal value, which will potentially change after an undo,
        // 2. what if undo removes the frame bulk being adjusted?
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        let Some(action) = self.undo_log.pop() else {
            return Err(ManualOpError::UserError(
                "there are no actions to undo".to_owned(),
            ));
        };
        let branch_idx = self
            .branches
            .iter()
            .enumerate()
            .find(|(_, branch)| branch.branch.branch_id == action.branch_id)
            .unwrap()
            .0;

        match &action.kind {
            ActionKind::ApplyOperation(op) => {
                if action.branch_id != self.branch().branch.branch_id {
                    self.branch_focus(branch_idx)?;
                }

                // TODO: smarter handling
                self.selected_bulk_idx = None;

                if let Some(frame_idx) = op.undo(&mut self.branch_mut().branch.script) {
                    self.invalidate(frame_idx);
                }

                self.branch_mut().extra_cam.clear();
                self.recompute_extra_camera_frame_data_if_needed();
            }
            ActionKind::Hide => {
                self.branches[branch_idx].branch.is_hidden = false;
            }
            ActionKind::Show => {
                self.branches[branch_idx].branch.is_hidden = true;
            }
        }

        self.redo_log.push(action.clone());

        self.db
            .update_after_undo(&self.branches[branch_idx].branch, &action.kind)?;
        Ok(())
    }

    /// Redoes the last action if any.
    pub fn redo(&mut self) -> ManualOpResult<()> {
        // Don't redo during active adjustments because:
        // 1. adustments store the orginal value, which will potentially change after an undo,
        // 2. what if undo removes the frame bulk being adjusted?
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        let Some(action) = self.redo_log.pop() else {
            return Err(ManualOpError::UserError(
                "there are no actions to redo".to_owned(),
            ));
        };
        let branch_idx = self
            .branches
            .iter()
            .enumerate()
            .find(|(_, branch)| branch.branch.branch_id == action.branch_id)
            .unwrap()
            .0;

        match &action.kind {
            ActionKind::ApplyOperation(op) => {
                if action.branch_id != self.branch().branch.branch_id {
                    self.branch_focus(branch_idx)?;
                }

                // TODO: smarter handling
                self.selected_bulk_idx = None;

                if let Some(frame_idx) = op.apply(&mut self.branch_mut().branch.script) {
                    self.invalidate(frame_idx);
                }

                self.branch_mut().extra_cam.clear();
                self.recompute_extra_camera_frame_data_if_needed();
            }
            ActionKind::Hide => {
                self.branches[branch_idx].branch.is_hidden = true;
            }
            ActionKind::Show => {
                self.branches[branch_idx].branch.is_hidden = false;
            }
        }

        self.undo_log.push(action.clone());

        self.db
            .update_after_redo(&self.branches[branch_idx].branch, &action.kind)?;
        Ok(())
    }

    /// Selects the given frame bulk.
    pub fn select_bulk(&mut self, bulk_idx: usize) -> ManualOpResult<()> {
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let bulk_count = self.script().frame_bulks().count();

        if bulk_idx >= bulk_count {
            return Err(ManualOpError::UserError(
                "there's no frame bulk with this index".to_owned(),
            ));
        }

        self.selected_bulk_idx = Some(bulk_idx);
        Ok(())
    }

    /// Selects the next frame bulk.
    pub fn select_next(&mut self) -> ManualOpResult<()> {
        let bulk_idx = if let Some(bulk_idx) = self.selected_bulk_idx {
            (bulk_idx + 1).min(self.script().frame_bulks().count().saturating_sub(1))
        } else {
            0
        };
        self.select_bulk(bulk_idx)
    }

    /// Selects the previous frame bulk.
    pub fn select_prev(&mut self) -> ManualOpResult<()> {
        let bulk_idx = if let Some(bulk_idx) = self.selected_bulk_idx {
            bulk_idx.saturating_sub(1)
        } else {
            self.script().frame_bulks().count().saturating_sub(1)
        };
        self.select_bulk(bulk_idx)
    }

    /// Deletes the selected line, if any.
    pub fn delete_selected(&mut self) -> ManualOpResult<()> {
        // Don't delete during active adjustments because they store the frame bulk index.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            let Some(line_idx) = self.hovered_line_idx else {
                return Err(ManualOpError::NoHoveredFrame);
            };

            let line = &self.branch().branch.script.lines[line_idx];

            let mut buffer = Vec::new();
            hltas::write::gen_line(&mut buffer, line)
                .expect("writing to an in-memory buffer should never fail");
            let buffer = String::from_utf8(buffer)
                .expect("Line serialization should never produce invalid UTF-8");

            let op = Operation::Delete {
                line_idx,
                line: buffer,
            };
            self.apply_operation(op)?;
            return Ok(());
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Err(ManualOpError::NoSelectedBulk);
        };

        let (line_idx, line) = self
            .branch()
            .branch
            .script
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| matches!(line, Line::FrameBulk(_)))
            .nth(bulk_idx)
            .unwrap();

        let mut buffer = Vec::new();
        hltas::write::gen_line(&mut buffer, line)
            .expect("writing to an in-memory buffer should never fail");
        let buffer = String::from_utf8(buffer)
            .expect("Line serialization should never produce invalid UTF-8");

        let op = Operation::Delete {
            line_idx,
            line: buffer,
        };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Deletes the last frame bulk, if any.
    pub fn delete_last(&mut self) -> ManualOpResult<()> {
        // Don't delete during active adjustments because they store the frame bulk index.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some((line_idx, line)) = self
            .branch()
            .branch
            .script
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| matches!(line, Line::FrameBulk(_)))
            .last()
        else {
            return Err(ManualOpError::UserError(
                "there are no frame bulks to delete".to_owned(),
            ));
        };

        let mut buffer = Vec::new();
        hltas::write::gen_line(&mut buffer, line)
            .expect("writing to an in-memory buffer should never fail");
        let buffer = String::from_utf8(buffer)
            .expect("Line serialization should never produce invalid UTF-8");

        let op = Operation::Delete {
            line_idx,
            line: buffer,
        };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Splits frame bulk at hovered frame.
    pub fn split(&mut self) -> ManualOpResult<()> {
        // Don't split during active adjustments because they store the frame bulk index.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some(frame_idx) = self.hovered_frame_idx else {
            return Err(ManualOpError::NoHoveredFrame);
        };

        let (_line_idx, repeat) =
            line_idx_and_repeat_at_frame(&self.branch().branch.script.lines, frame_idx)
                .expect("invalid frame index");

        // Can't split because this is already a split point.
        if repeat == 0 {
            return Err(ManualOpError::UserError(
                "the script is already split at this point".to_owned(),
            ));
        }

        let op = Operation::Split { frame_idx };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Toggles a key on the selected frame bulk.
    pub fn toggle_key(&mut self, key: Key) -> ManualOpResult<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Err(ManualOpError::NoSelectedBulk);
        };
        let bulk = self
            .branch_mut()
            .branch
            .script
            .frame_bulks_mut()
            .nth(bulk_idx)
            .unwrap();

        let op = Operation::ToggleKey {
            bulk_idx,
            key,
            to: !*key.value_mut(bulk),
        };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Toggles an auto-action on the selected frame bulk.
    pub fn toggle_auto_action(&mut self, target: ToggleAutoActionTarget) -> ManualOpResult<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Err(ManualOpError::NoSelectedBulk);
        };
        let (line_idx, bulk) = self
            .branch()
            .branch
            .script
            .lines
            .iter()
            .enumerate()
            .filter_map(|(line_idx, line)| line.frame_bulk().map(|bulk| (line_idx, bulk)))
            .nth(bulk_idx)
            .unwrap();

        let new_bulk = target.apply(&self.branch().branch.script, bulk_idx);
        if new_bulk == *bulk {
            return Ok(());
        }

        let mut buffer = Vec::new();
        hltas::write::gen_frame_bulk(&mut buffer, bulk)
            .expect("writing to an in-memory buffer should never fail");
        let from = String::from_utf8(buffer)
            .expect("FrameBulk serialization should never produce invalid UTF-8");

        let mut buffer = Vec::new();
        hltas::write::gen_frame_bulk(&mut buffer, &new_bulk)
            .expect("writing to an in-memory buffer should never fail");
        let to = String::from_utf8(buffer)
            .expect("FrameBulk serialization should never produce invalid UTF-8");

        let op = Operation::Replace { line_idx, from, to };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Sets pitch of the selected frame bulk.
    pub fn set_pitch(&mut self, new_pitch: Option<f32>) -> ManualOpResult<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Err(ManualOpError::NoSelectedBulk);
        };
        let (line_idx, bulk) = self
            .branch()
            .branch
            .script
            .lines
            .iter()
            .enumerate()
            .filter_map(|(line_idx, line)| line.frame_bulk().map(|bulk| (line_idx, bulk)))
            .nth(bulk_idx)
            .unwrap();

        if bulk.pitch == new_pitch {
            return Ok(());
        }

        let mut new_bulk = bulk.clone();
        new_bulk.pitch = new_pitch;

        let mut buffer = Vec::new();
        hltas::write::gen_frame_bulk(&mut buffer, bulk)
            .expect("writing to an in-memory buffer should never fail");
        let from = String::from_utf8(buffer)
            .expect("FrameBulk serialization should never produce invalid UTF-8");

        let mut buffer = Vec::new();
        hltas::write::gen_frame_bulk(&mut buffer, &new_bulk)
            .expect("writing to an in-memory buffer should never fail");
        let to = String::from_utf8(buffer)
            .expect("FrameBulk serialization should never produce invalid UTF-8");

        let op = Operation::Replace { line_idx, from, to };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Sets yaw of the selected frame bulk.
    pub fn set_yaw(&mut self, new_yaw: Option<f32>) -> ManualOpResult<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Err(ManualOpError::NoSelectedBulk);
        };
        let (line_idx, bulk) = self
            .branch()
            .branch
            .script
            .lines
            .iter()
            .enumerate()
            .filter_map(|(line_idx, line)| line.frame_bulk().map(|bulk| (line_idx, bulk)))
            .nth(bulk_idx)
            .unwrap();

        let mut new_bulk = bulk.clone();
        match new_yaw {
            Some(new_yaw) => match &mut new_bulk.auto_actions.movement {
                Some(AutoMovement::SetYaw(yaw))
                | Some(AutoMovement::Strafe(StrafeSettings {
                    dir: StrafeDir::Yaw(yaw) | StrafeDir::Line { yaw },
                    ..
                })) => *yaw = new_yaw,
                None => new_bulk.auto_actions.movement = Some(AutoMovement::SetYaw(new_yaw)),
                _ => return Ok(()),
            },
            None => match &mut new_bulk.auto_actions.movement {
                Some(AutoMovement::SetYaw(_)) => new_bulk.auto_actions.movement = None,
                _ => return Ok(()),
            },
        }

        if new_bulk == *bulk {
            return Ok(());
        }

        let mut buffer = Vec::new();
        hltas::write::gen_frame_bulk(&mut buffer, bulk)
            .expect("writing to an in-memory buffer should never fail");
        let from = String::from_utf8(buffer)
            .expect("FrameBulk serialization should never produce invalid UTF-8");

        let mut buffer = Vec::new();
        hltas::write::gen_frame_bulk(&mut buffer, &new_bulk)
            .expect("writing to an in-memory buffer should never fail");
        let to = String::from_utf8(buffer)
            .expect("FrameBulk serialization should never produce invalid UTF-8");

        let op = Operation::Replace { line_idx, from, to };
        self.apply_operation(op)?;

        Ok(())
    }

    fn replace_multiple(
        &mut self,
        first_line_idx: usize,
        count: usize,
        to: &[Line],
    ) -> eyre::Result<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        let from_lines = &self.script().lines[first_line_idx..first_line_idx + count];

        let mut buffer = Vec::new();
        hltas::write::gen_lines(&mut buffer, from_lines)
            .expect("writing to an in-memory buffer should never fail");
        let from = String::from_utf8(buffer)
            .expect("Line serialization should never produce invalid UTF-8");

        let mut buffer = Vec::new();
        hltas::write::gen_lines(&mut buffer, to)
            .expect("writing to an in-memory buffer should never fail");
        let to = String::from_utf8(buffer)
            .expect("Line serialization should never produce invalid UTF-8");

        let op = Operation::ReplaceMultiple {
            first_line_idx,
            from,
            to,
        };
        self.apply_operation(op)
    }

    /// Sets yawspeed of the selected frame bulk.
    pub fn set_yawspeed(&mut self, new_yawspeed: Option<f32>) -> ManualOpResult<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Err(ManualOpError::NoSelectedBulk);
        };

        let new_yawspeed = new_yawspeed.map(|x| x.max(0.));

        let (_, bulk) = self
            .branch()
            .branch
            .script
            .lines
            .iter()
            .enumerate()
            .filter_map(|(line_idx, line)| line.frame_bulk().map(|bulk| (line_idx, bulk)))
            .nth(bulk_idx)
            .unwrap();

        let mut new_bulk = bulk.clone();
        let mut from = 0.;
        let mut to = 0.;

        if let Some(AutoMovement::Strafe(StrafeSettings {
            type_: StrafeType::ConstYawspeed(yawspeed),
            ..
        })) = &mut new_bulk.auto_actions.movement
        {
            if let Some(new_yawspeed) = new_yawspeed {
                from = *yawspeed;
                to = new_yawspeed;
                *yawspeed = new_yawspeed;
            }
        }

        if new_bulk == *bulk {
            return Ok(());
        }

        let op = Operation::SetYawspeed { bulk_idx, from, to };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Sets frame time of the selected bulk.
    pub fn set_frame_time(&mut self, new_frame_time: String) -> ManualOpResult<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Err(ManualOpError::NoSelectedBulk);
        };

        let (_, bulk) = self
            .branch()
            .branch
            .script
            .lines
            .iter()
            .enumerate()
            .filter_map(|(line_idx, line)| line.frame_bulk().map(|bulk| (line_idx, bulk)))
            .nth(bulk_idx)
            .unwrap();

        if bulk.frame_time == new_frame_time {
            return Ok(());
        }

        let op = Operation::SetFrameTime {
            bulk_idx,
            from: bulk.frame_time.clone(),
            to: new_frame_time,
        };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Sets commands of the selected bulk.
    pub fn set_commands(&mut self, new_command: Option<String>) -> ManualOpResult<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.in_camera_editor {
            return Err(ManualOpError::CannotDoInCameraEditor);
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Err(ManualOpError::NoSelectedBulk);
        };

        let (_, bulk) = self
            .branch()
            .branch
            .script
            .lines
            .iter()
            .enumerate()
            .filter_map(|(line_idx, line)| line.frame_bulk().map(|bulk| (line_idx, bulk)))
            .nth(bulk_idx)
            .unwrap();

        if bulk.console_command == new_command {
            return Ok(());
        }

        let op = Operation::SetCommands {
            bulk_idx,
            from: bulk.console_command.clone(),
            to: new_command,
        };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Rewrites the script with a completely new version.
    pub fn rewrite(&mut self, new_script: HLTAS) -> ManualOpResult<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        let script = self.script();
        if new_script == *script {
            return Ok(());
        }

        // Check if we can optimize a full rewrite into a lines replacement.
        if let Some((first_line_idx, count, to)) = replace_multiple_params(script, &new_script) {
            self.replace_multiple(first_line_idx, count, to)?;
            return Ok(());
        }

        let mut buffer = Vec::new();
        script
            .to_writer(&mut buffer)
            .expect("writing to an in-memory buffer should never fail");
        let from = String::from_utf8(buffer)
            .expect("HLTAS serialization should never produce invalid UTF-8");

        let mut buffer = Vec::new();
        new_script
            .to_writer(&mut buffer)
            .expect("writing to an in-memory buffer should never fail");
        let to = String::from_utf8(buffer)
            .expect("HLTAS serialization should never produce invalid UTF-8");

        let op = Operation::Rewrite { from, to };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Applies global smoothing to the entire script.
    pub fn apply_global_smoothing(&mut self) -> ManualOpResult<()> {
        // Don't apply during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        let frame_count = self
            .branch()
            .branch
            .script
            .frame_bulks()
            .map(|bulk| bulk.frame_count.get() as usize)
            .sum::<usize>();

        // Only smooth when we have all accurate frames.
        if self.branch().first_predicted_frame != frame_count + 1 {
            return Err(ManualOpError::UserError(
                "all frames must be accurate (simulated by the \
                 second game) to apply global smoothing"
                    .to_owned(),
            ));
        }

        let frames = &self.branch().frames;
        let smoothed = smoothed_yaws(
            self.smooth_window_s,
            self.smooth_small_window_s,
            self.smooth_small_window_multiplier,
            &frames[..frames.len()],
        );

        let mut line = "target_yaw_override".to_string();
        // Skip the first frame because it is the initial frame before the start of the TAS.
        for yaw in &smoothed[1..] {
            let yaw = yaw.to_degrees();
            write!(&mut line, " {yaw}").unwrap();
        }

        let op = Operation::Insert { line_idx: 0, line };
        self.apply_operation(op)?;

        Ok(())
    }

    /// Applies smoothing to the segment under cursor.
    pub fn apply_smoothing_to_hovered_segment(&mut self) -> ManualOpResult<()> {
        // Don't apply during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if !self.in_camera_editor {
            return Err(ManualOpError::CannotDoInMovementEditor);
        }

        let Some(hovered_frame_idx) = self.hovered_frame_idx else {
            return Err(ManualOpError::NoHoveredFrame);
        };

        // Find the input region the user is pointing at.
        let frames = &self.branch().frames;
        let Some((start, end)) = self.branch().extra_cam[hovered_frame_idx].smoothing_input_region
        else {
            return Err(ManualOpError::UserError(
                "you need to point the cursor at an orange / blue segment to do this".to_owned(),
            ));
        };

        // Only smooth when we have all accurate frames.
        if self.branch().first_predicted_frame <= end {
            return Err(ManualOpError::UserError(
                "all frames in the segment must be accurate (simulated by the \
                 second game) to apply smoothing"
                    .to_owned(),
            ));
        }

        let mut smoothed = smoothed_yaws(
            self.smooth_window_s,
            self.smooth_small_window_s,
            self.smooth_small_window_multiplier,
            &frames[start..=end],
        );

        // Skip the first frame because it is the initial frame before the start of the TAS.
        let first = start.max(1);
        if start == 0 {
            smoothed.remove(0);
        }

        // Convert to degrees for .hltas.
        for yaw in &mut smoothed {
            *yaw = yaw.to_degrees();
        }

        // Figure out where to insert the line.
        let (line_idx, repeat) =
            line_idx_and_repeat_at_frame(&self.branch().branch.script.lines, first - 1).unwrap();

        if repeat == 0 {
            let mut line = "target_yaw_override".to_string();
            for yaw in smoothed {
                write!(&mut line, " {yaw}").unwrap();
            }

            // There's already a frame bulk edge here, no need to split.
            let op = Operation::Insert { line_idx, line };
            self.apply_operation(op)?;
            Ok(())
        } else {
            let target_yaw_override = Line::TargetYawOverride(smoothed);

            // We need to insert the line in the middle of a frame bulk, so split it.
            let mut line = self.branch().branch.script.lines[line_idx].clone();

            let mut buffer = Vec::new();
            hltas::write::gen_lines(&mut buffer, [&line])
                .expect("writing to an in-memory buffer should never fail");
            let from = String::from_utf8(buffer)
                .expect("Line serialization should never produce invalid UTF-8");

            let mut new_line = line.clone();

            let bulk = line.frame_bulk_mut().unwrap();
            let new_bulk = new_line.frame_bulk_mut().unwrap();

            bulk.frame_count = NonZeroU32::new(repeat).unwrap();
            new_bulk.frame_count = NonZeroU32::new(new_bulk.frame_count.get() - repeat).unwrap();

            let mut buffer = Vec::new();
            hltas::write::gen_lines(&mut buffer, [&line, &target_yaw_override, &new_line])
                .expect("writing to an in-memory buffer should never fail");
            let to = String::from_utf8(buffer)
                .expect("Line serialization should never produce invalid UTF-8");

            let op = Operation::ReplaceMultiple {
                first_line_idx: line_idx,
                from,
                to,
            };
            self.apply_operation(op)?;
            Ok(())
        }
    }

    /// Hides frames before the hovered frame, or shows all frames if there's no hovered frame.
    pub fn hide_frames_up_to_hovered(&mut self) -> ManualOpResult<()> {
        // Don't apply during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        match self.hovered_frame_idx {
            None => self.first_shown_frame_idx = 0,
            // If we're pressing hide again on the first visible frame, unhide instead. This is
            // nicer than struggling to look away to unhide.
            Some(frame_idx) if frame_idx == self.first_shown_frame_idx => {
                self.first_shown_frame_idx = 0;
            }
            Some(frame_idx) => {
                let frame_count = self
                    .branch()
                    .branch
                    .script
                    .frame_bulks()
                    .map(|bulk| bulk.frame_count.get() as usize)
                    .sum::<usize>();

                self.first_shown_frame_idx = min(frame_idx, frame_count.saturating_sub(1));

                // Check if we need to unselect or unhover anything now hidden.
                let hovered_frame_bulk_idx =
                    bulk_idx_and_repeat_at_frame(self.script(), self.first_shown_frame_idx)
                        .unwrap()
                        .0;

                if let Some(selected_bulk_idx) = self.selected_bulk_idx {
                    if selected_bulk_idx < hovered_frame_bulk_idx {
                        // All frames of the selected bulk got hidden, so unselect it.
                        self.selected_bulk_idx = None;
                    }
                }

                if let Some(hovered_bulk_idx) = self.hovered_bulk_idx {
                    if hovered_bulk_idx < hovered_frame_bulk_idx {
                        // All frames of the hovered bulk got hidden, so unhover it.
                        self.hovered_bulk_idx = None;
                    }
                }
            }
        }

        Ok(())
    }

    // You MUST check and recompute `extra_cam` after calling this.
    pub fn apply_accurate_frame(
        &mut self,
        frame: AccurateFrame,
        truncate_on_mismatch: bool,
    ) -> Option<PlayRequest> {
        if frame.generation != self.generation {
            return None;
        }

        // TODO: make this nicer somehow maybe?
        if frame.frame_idx == 0 {
            // Initial frame is the same for all branches and between smoothed/unsmoothed.
            for branch_idx in 0..self.branches.len() {
                let branch = &mut self.branches[branch_idx];

                if branch.auto_smoothing.frames.is_empty() {
                    branch.auto_smoothing.frames.push(frame.frame.clone());
                }

                if branch.frames.is_empty() {
                    branch.frames.push(frame.frame.clone());
                    branch.first_predicted_frame = 1;
                    branch.extra_cam.clear();
                }
            }

            return None;
        }

        let branch = &mut self.branches[frame.branch_idx];

        if frame.frame_idx > branch.first_predicted_frame {
            // TODO: we can still use newer frames.
            return None;
        }

        if frame.is_smoothed {
            if !self.auto_smoothing {
                return None;
            }

            let frames = &mut branch.auto_smoothing.frames;

            if frames.len() == frame.frame_idx {
                frames.push(frame.frame);
            } else {
                let current_frame = &mut frames[frame.frame_idx];
                if *current_frame != frame.frame {
                    *current_frame = frame.frame;
                    frames.truncate(frame.frame_idx + 1);
                }
            }

            return None;
        }

        branch.first_predicted_frame = max(frame.frame_idx + 1, branch.first_predicted_frame);

        if branch.frames.len() == frame.frame_idx {
            branch.frames.push(frame.frame);
            branch.extra_cam.clear();
        } else {
            let current_frame = &mut branch.frames[frame.frame_idx];
            if *current_frame != frame.frame {
                *current_frame = frame.frame;

                branch.first_predicted_frame =
                    min(branch.first_predicted_frame, frame.frame_idx + 1);
                branch.extra_cam.clear();

                if truncate_on_mismatch {
                    branch.frames.truncate(frame.frame_idx + 1);
                }
            }
        }

        if self.auto_smoothing {
            let branch = &mut self.branches[frame.branch_idx];
            let frame_count = branch
                .branch
                .script
                .frame_bulks()
                .map(|bulk| bulk.frame_count.get() as usize)
                .sum::<usize>();

            if frame.frame_idx + 1 == frame_count {
                let mut smoothed_script = branch.branch.script.clone();

                // Enable vectorial strafing if it wasn't enabled.
                smoothed_script
                    .lines
                    .insert(0, Line::VectorialStrafing(true));
                smoothed_script.lines.insert(
                    1,
                    Line::VectorialStrafingConstraints(
                        VectorialStrafingConstraints::VelocityYawLocking { tolerance: 0. },
                    ),
                );

                // Compute and insert the smoothed TargetYawOverride line.
                let mut smoothed = smoothed_yaws(
                    self.smooth_window_s,
                    self.smooth_small_window_s,
                    self.smooth_small_window_multiplier,
                    &branch.frames,
                );
                // First yaw corresponds to the initial frame, which is not controlled by the TAS.
                smoothed.remove(0);
                for yaw in &mut smoothed {
                    *yaw = yaw.to_degrees();
                }
                let line = Line::TargetYawOverride(smoothed);
                smoothed_script.lines.insert(2, line);

                // Remove all lines disabling vectorial strafing.
                let mut i = 0;
                while i < smoothed_script.lines.len() {
                    if matches!(smoothed_script.lines[i], Line::VectorialStrafing(false)) {
                        smoothed_script.lines.remove(i);
                    } else {
                        i += 1;
                    }
                }

                branch.auto_smoothing.script = Some(smoothed_script.clone());
                return Some(PlayRequest {
                    script: smoothed_script,
                    generation: self.generation,
                    branch_idx: self.branch_idx,
                    is_smoothed: true,
                });
            }
        }

        None
    }

    pub fn set_stop_frame(&mut self, stop_frame: u32) -> ManualOpResult<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        self.branch_mut().branch.stop_frame = stop_frame;
        self.db.update_branch(&self.branch().branch)?;

        Ok(())
    }

    pub fn set_stop_frame_to_hovered(&mut self) -> ManualOpResult<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        let Some(frame_idx) = self.hovered_frame_idx else {
            return Err(ManualOpError::NoHoveredFrame);
        };
        self.set_stop_frame(frame_idx.try_into().unwrap())?;

        Ok(())
    }

    pub fn branch_clone(&mut self) -> ManualOpResult<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        let mut new_branch = self.branch().clone();
        self.db.insert_branch(&mut new_branch.branch)?;
        self.undo_log.push(Action {
            branch_id: new_branch.branch.branch_id,
            kind: if new_branch.branch.is_hidden {
                ActionKind::Hide
            } else {
                ActionKind::Show
            },
        });
        self.redo_log.clear();
        self.branches.push(new_branch);

        // Switch to the cloned branch.
        self.branch_focus(self.branches.len() - 1)?;

        Ok(())
    }

    pub fn branch_focus(&mut self, branch_idx: usize) -> ManualOpResult<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        if self.branch_idx == branch_idx {
            return Err(ManualOpError::UserError(
                "this branch is already focused".to_owned(),
            ));
        }

        let Some(branch) = self.branches.get(branch_idx) else {
            return Err(ManualOpError::BranchDoesNotExist);
        };

        self.branch_idx = branch_idx;
        self.selected_bulk_idx = None;
        self.hovered_bulk_idx = None;
        self.hovered_frame_idx = None;
        self.db.switch_to_branch(&branch.branch)?;

        if let Some((last_bulk, frame_idx)) = bulk_and_first_frame_idx(self.script()).last() {
            let last_frame_idx = frame_idx + last_bulk.frame_count.get() as usize;
            if self.first_shown_frame_idx + 1 >= last_frame_idx {
                // The whole branch would be hidden, so show all frames instead.
                self.first_shown_frame_idx = 0
            }
        }

        Ok(())
    }

    pub fn branch_focus_next(&mut self) -> ManualOpResult<()> {
        let Some(branch_idx) = (self.branch_idx + 1..self.branches.len())
            .chain(0..self.branch_idx)
            .find(|&idx| !self.branches[idx].branch.is_hidden)
        else {
            return Err(ManualOpError::UserError(
                "there are no other visible branches".to_owned(),
            ));
        };

        self.branch_focus(branch_idx)
    }

    pub fn branch_hide(&mut self, branch_idx: usize) -> ManualOpResult<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        // Don't try to hide the current branch on its own: this causes it to hide only when we
        // switch from it subsequently, which is confusing.
        if self.branch_idx == branch_idx {
            return Err(ManualOpError::UserError(
                "you cannot hide the currently focused branch".to_owned(),
            ));
        }

        let Some(branch) = self.branches.get_mut(branch_idx) else {
            return Err(ManualOpError::BranchDoesNotExist);
        };

        if branch.branch.is_hidden {
            return Ok(());
        }

        branch.branch.is_hidden = true;
        self.db.hide_branch(&branch.branch)?;
        self.undo_log.push(Action {
            branch_id: branch.branch.branch_id,
            kind: ActionKind::Hide,
        });
        self.redo_log.clear();

        Ok(())
    }

    pub fn branch_hide_and_focus_next(&mut self) -> ManualOpResult<()> {
        let Some(next_branch_idx) = (self.branch_idx + 1..self.branches.len())
            .chain(0..self.branch_idx)
            .find(|&idx| !self.branches[idx].branch.is_hidden)
        else {
            return Err(ManualOpError::UserError(
                "there are no other visible branches".to_owned(),
            ));
        };

        let curr_branch_idx = self.branch_idx;
        self.branch_focus(next_branch_idx)?;
        self.branch_hide(curr_branch_idx)
    }

    pub fn branch_show(&mut self, branch_idx: usize) -> ManualOpResult<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Err(ManualOpError::CannotDoDuringAdjustment);
        }

        let Some(branch) = self.branches.get_mut(branch_idx) else {
            return Err(ManualOpError::BranchDoesNotExist);
        };

        if !branch.branch.is_hidden {
            return Ok(());
        }

        branch.branch.is_hidden = false;
        self.db.show_branch(&branch.branch)?;
        self.undo_log.push(Action {
            branch_id: branch.branch.branch_id,
            kind: ActionKind::Show,
        });
        self.redo_log.clear();

        Ok(())
    }

    fn draw_current_branch(&self, mut draw: impl FnMut(DrawLine)) {
        let branch = self.branch();

        let smoothing_input_region = if self.in_camera_editor {
            self.hovered_frame_idx
                .and_then(|idx| branch.extra_cam[idx].smoothing_input_region)
        } else {
            None
        };

        // Draw regular frames.
        let mut collided_this_bulk = false;

        // For drawing small camera lines in the movement editor.
        let mut last_camera_line_origin_vector = None;

        // Note: there's no iterator cloning, which means all values are computed once, in one go.
        let iter = iter::zip(
            // Pairs of frames: (0, 1), (1, 2), (2, 3) and so on.
            branch.frames.iter().tuple_windows(),
            // For second frame in pair: its frame bulk index and whether it's last in its bulk.
            bulk_idx_and_is_last(&branch.branch.script.lines),
        )
        .enumerate();
        for (prev_idx, ((prev, frame), (bulk_idx, bulk, is_last_in_bulk))) in iter {
            let idx = prev_idx + 1;

            // Figure out if we had a collision this frame.
            let mut collided_this_frame = false;
            for trace in &frame.state.move_traces {
                // If we bumped into something along the way...
                if trace.fraction == 1. {
                    break;
                }

                // And it wasn't a ground or a ceiling...
                let n = trace.plane_normal.z;
                if n != -1. && n != 1. {
                    // We have a collision.
                    collided_this_frame = true;
                    break;
                }
            }

            if collided_this_frame {
                collided_this_bulk = true;
            }

            // If frame is predicted (inaccurate).
            let is_predicted = idx >= branch.first_predicted_frame;
            // If frame is part of selected frame bulk.
            let is_selected_bulk = self.selected_bulk_idx == Some(bulk_idx);
            // If frame is part of hovered frame bulk.
            let is_hovered_bulk = self.hovered_bulk_idx == Some(bulk_idx);
            // If frame is hovered.
            let is_hovered = self.hovered_frame_idx == Some(idx);
            // If frame is the stop frame.
            let is_stop_frame = branch.branch.stop_frame as usize == idx;
            // If frame is the end of norefresh for norefresh until stop frame.
            let is_norefresh_until_stop_frame =
                !is_stop_frame && self.norefresh_until_stop_frame_frame_idx == idx;

            // If frame is in the smoothing input region.
            let in_smoothing_input_region =
                smoothing_input_region.map_or(false, |(start, end)| idx >= start && idx <= end);

            // How many frames until the visible part, clamped in a way to allow for smooth dimming.
            let frames_until_hidden = self.first_shown_frame_idx.saturating_sub(idx - 1).min(20);

            // Inaccurate frames get dimmed.
            let dim_inaccurate = if is_predicted { 0.6 } else { 1. };
            // Unhovered bulks get dimmed.
            let dim_unhovered = if is_hovered_bulk || self.in_camera_editor {
                1.
            } else {
                0.7
            };
            // Hidden frames become invisible, smoothly transition into visible.
            let dim_hidden = if frames_until_hidden == 0 {
                1.
            } else {
                (20 - frames_until_hidden) as f32 / 40.
            };
            let dim = dim_inaccurate * dim_unhovered * dim_hidden;

            const WHITE: Vec3 = Vec3::new(1., 1., 1.);
            let color = if self.in_camera_editor {
                WHITE * dim * 0.5
            } else {
                // Deselected bulks get desaturated.
                let saturation = if is_selected_bulk { 1. } else { 0.3 };

                let hue = if collided_this_frame {
                    // Collided frames are red.
                    Vec3::new(1., 0., 0.)
                } else if collided_this_bulk {
                    // Non-collided frames in collided bulk are pink.
                    Vec3::new(1., 0.6, 0.6)
                } else {
                    // Other frames are green.
                    Vec3::new(0., 1., 0.)
                };

                WHITE.lerp(hue, saturation) * dim
            };

            let prev_pos = prev.state.player.pos;
            let pos = frame.state.player.pos;

            // Line from previous to this frame position.
            draw(DrawLine {
                start: prev_pos,
                end: pos,
                color,
            });

            let camera_pitch = frame.state.prev_frame_input.pitch;
            let camera_yaw = frame.state.prev_frame_input.yaw;
            let camera_vector = forward(camera_pitch, camera_yaw);

            if self.in_camera_editor {
                let extra_cam = &branch.extra_cam[idx];

                // Draw camera angle line.
                let hue = if in_smoothing_input_region {
                    Vec3::new(1., 0.75, 0.5)
                } else if extra_cam.in_smoothing_idempotent_region {
                    Vec3::new(0., 1., 0.)
                } else {
                    Vec3::new(0.5, 0.5, 1.)
                };

                draw(DrawLine {
                    start: pos,
                    end: pos + camera_vector * 5.,
                    color: hue * dim_inaccurate * dim_hidden,
                });

                for &line_idx in &extra_cam.change_line_that_ends_here {
                    let perp = perpendicular(prev_pos, pos) * 5.;
                    let diff = (pos - prev_pos).normalize_or_zero() * 5.;

                    let dim_unhovered = if self.hovered_line_idx == Some(line_idx) {
                        1.
                    } else {
                        0.7
                    };

                    // Draw the arrow.
                    draw(DrawLine {
                        start: pos - perp - diff,
                        end: pos,
                        color: WHITE * dim_hidden * dim_unhovered,
                    });
                    draw(DrawLine {
                        start: pos,
                        end: pos + perp - diff,
                        color: WHITE * dim_hidden * dim_unhovered,
                    });

                    // Draw the target angle.
                    let Line::Change(Change {
                        target,
                        final_value,
                        ..
                    }) = branch.branch.script.lines[line_idx]
                    else {
                        unreachable!()
                    };

                    let target_vector = match target {
                        ChangeTarget::Yaw | ChangeTarget::VectorialStrafingYaw => {
                            forward(0., final_value.to_radians())
                        }
                        ChangeTarget::Pitch => forward(final_value.to_radians(), camera_yaw),
                        // TODO: how to draw this? Maybe skip it?
                        ChangeTarget::VectorialStrafingYawOffset => camera_vector,
                    };

                    draw(DrawLine {
                        start: pos,
                        end: pos + target_vector * 20.,
                        color: Vec3::new(1., 1., 0.) * dim_hidden * dim_unhovered,
                    });
                }

                for &camera_line_idx in &extra_cam.camera_line_that_starts_here {
                    let camera_line = &branch.branch.script.lines[camera_line_idx];
                    let perp = perpendicular(prev_pos, pos) * 5.;

                    let hue = match camera_line {
                        Line::TargetYawOverride { .. } => Vec3::new(1., 0.75, 0.5),
                        Line::RenderYawOverride { .. } => Vec3::new(1., 0., 0.),
                        _ => WHITE,
                    };

                    let dim_unhovered = if self.hovered_line_idx == Some(camera_line_idx) {
                        1.
                    } else {
                        0.7
                    };

                    if let Line::Change(Change { target, .. }) = camera_line {
                        let diff = (pos - prev_pos).normalize_or_zero() * 5.;

                        // Draw the arrow.
                        draw(DrawLine {
                            start: prev_pos - perp + diff,
                            end: prev_pos,
                            color: hue * dim_hidden * dim_unhovered,
                        });
                        draw(DrawLine {
                            start: prev_pos,
                            end: prev_pos + perp + diff,
                            color: hue * dim_hidden * dim_unhovered,
                        });

                        // Draw the starting angle.
                        let target_vector = match target {
                            ChangeTarget::Yaw | ChangeTarget::VectorialStrafingYaw => {
                                forward(0., camera_yaw)
                            }
                            ChangeTarget::Pitch => camera_vector,
                            // TODO: how to draw this? Maybe skip it?
                            ChangeTarget::VectorialStrafingYawOffset => camera_vector,
                        };

                        draw(DrawLine {
                            start: prev_pos,
                            end: prev_pos + target_vector * 20.,
                            color: Vec3::new(1., 0., 0.) * dim_hidden * dim_unhovered,
                        });
                    } else {
                        draw(DrawLine {
                            start: prev_pos - perp,
                            end: prev_pos + perp,
                            color: hue * dim_hidden * dim_unhovered,
                        });

                        if let Line::VectorialStrafingConstraints(constraints) = camera_line {
                            let hue = match constraints {
                                VectorialStrafingConstraints::VelocityYaw { .. }
                                | VectorialStrafingConstraints::AvgVelocityYaw { .. }
                                | VectorialStrafingConstraints::VelocityYawLocking { .. } => {
                                    Vec3::new(0., 1., 0.)
                                }
                                // TODO: for Yaw we can draw the Yaw itself, for YawRange we can
                                // draw the range too.
                                VectorialStrafingConstraints::Yaw { .. }
                                | VectorialStrafingConstraints::YawRange { .. } => {
                                    Vec3::new(0., 1., 1.)
                                }
                                VectorialStrafingConstraints::LookAt { .. } => {
                                    Vec3::new(1., 0., 1.)
                                }
                            };

                            draw(DrawLine {
                                start: prev_pos,
                                end: prev_pos + camera_vector * 20.,
                                color: hue * dim_hidden * dim_unhovered,
                            });
                        }
                    }
                }
            } else {
                // If this frame is last in its frame bulk, draw a frame bulk handle.
                if is_last_in_bulk {
                    let perp = perpendicular(prev_pos, pos) * 5.;

                    draw(DrawLine {
                        start: pos - perp,
                        end: pos + perp,
                        color,
                    });
                }

                // If it's selected and last and the frame bulk has a yaw, draw that.
                if is_selected_bulk && is_last_in_bulk {
                    if let Some(yaw) = bulk.yaw() {
                        let yaw_dir = Vec2::from_angle(yaw.to_radians()).extend(0.);

                        draw(DrawLine {
                            start: pos - yaw_dir * 5.,
                            end: pos + yaw_dir * 20.,
                            color: Vec3::new(0.5, 0.5, 1.) * dim,
                        });
                    }
                }

                // Draw camera angle line if it's different enough from the last one.
                if last_camera_line_origin_vector
                    .map(|(origin, angle)| {
                        pos.distance(origin) > 50. || camera_vector.dot(angle) < 0.98
                    })
                    .unwrap_or(true)
                {
                    last_camera_line_origin_vector = Some((pos, camera_vector));

                    draw(DrawLine {
                        start: pos,
                        end: pos + camera_vector * 5.,
                        color: Vec3::new(0.3, 0.3, 1.) * dim_inaccurate * dim_hidden,
                    });
                }
            }

            // If the frame is hovered and not last in bulk, draw a splitting guide.
            if is_hovered && (!is_last_in_bulk || self.in_camera_editor) {
                let perp = perpendicular(prev_pos, pos) * 2.;

                let splitting_guide_color = if self.in_camera_editor {
                    color
                } else {
                    color * 0.5
                };

                draw(DrawLine {
                    start: pos - perp,
                    end: pos + perp,
                    color: splitting_guide_color,
                });
            }

            // If the frame is hovered, draw the player bbox.
            if is_hovered && self.show_player_bbox {
                let hull = frame.state.player.hull();

                const HALF_SIZE: f32 = 16.;

                let half_height = match hull {
                    Hull::Standing => 36.,
                    Hull::Ducked => 18.,
                    Hull::Point => unreachable!(),
                };

                let offset = Vec3::new(HALF_SIZE, HALF_SIZE, half_height);

                let mut draw_aa_cuboid = |corner1: Vec3, corner2: Vec3, color: Vec3| {
                    let delta = corner2 - corner1;
                    let dx = delta * Vec3::X;
                    let dy = delta * Vec3::Y;
                    let dz = delta * Vec3::Z;

                    let lines = [
                        // Bottom.
                        (corner1, corner1 + dx),
                        (corner1, corner1 + dy),
                        (corner1 + dx, corner1 + dx + dy),
                        (corner1 + dy, corner1 + dx + dy),
                        // Top.
                        (corner1 + dz, corner1 + dx + dz),
                        (corner1 + dz, corner1 + dy + dz),
                        (corner1 + dx + dz, corner1 + dx + dy + dz),
                        (corner1 + dy + dz, corner1 + dx + dy + dz),
                        // Sides.
                        (corner1, corner1 + dz),
                        (corner1 + dx, corner1 + dx + dz),
                        (corner1 + dy, corner1 + dy + dz),
                        (corner1 + dx + dy, corner1 + dx + dy + dz),
                    ];

                    for (start, end) in lines {
                        draw(DrawLine { start, end, color });
                    }
                };

                draw_aa_cuboid(pos - offset, pos + offset, color);
            }

            // If this is the stop frame, draw an indicator.
            if is_stop_frame {
                let perp = perpendicular(prev_pos, pos) * 2.;

                draw(DrawLine {
                    start: pos - perp,
                    end: pos + perp,
                    color: Vec3::new(1., 1., 0.5),
                });
            }

            // If bxt_tas_studio_norefresh_until_stop_frame is set, draw another indicator.
            if is_norefresh_until_stop_frame {
                let perp = perpendicular(prev_pos, pos) * 2.;
                let diff = (pos - prev_pos).normalize_or_zero() * 2.;

                // Draw the arrow.
                draw(DrawLine {
                    start: pos - perp + diff,
                    end: pos,
                    color: Vec3::new(1., 1., 0.5),
                });
                draw(DrawLine {
                    start: pos,
                    end: pos + perp + diff,
                    color: Vec3::new(1., 1., 0.5),
                });
            }

            if is_last_in_bulk {
                // Reset flag for next iteration.
                collided_this_bulk = false;
            }
        }
    }

    fn draw_auto_smoothing(&self, mut draw: impl FnMut(DrawLine)) {
        if !self.auto_smoothing {
            return;
        }

        for (prev_idx, (prev, frame)) in self
            .branch()
            .auto_smoothing
            .frames
            .iter()
            .tuple_windows()
            .enumerate()
        {
            let idx = prev_idx + 1;

            // How many frames until the visible part, clamped in a way to allow for smooth dimming.
            let frames_until_hidden = self.first_shown_frame_idx.saturating_sub(idx - 1).min(20);
            // Hidden frames become invisible, smoothly transition into visible.
            let dim = if frames_until_hidden == 0 {
                1.
            } else {
                (20 - frames_until_hidden) as f32 / 40.
            };

            let prev_pos = prev.state.player.pos;
            let pos = frame.state.player.pos;

            // Line from previous to this frame position.
            draw(DrawLine {
                start: prev_pos,
                end: pos,
                color: Vec3::new(1., 0.75, 0.5) * dim,
            });
        }
    }

    fn draw_other_branches(&self, mut draw: impl FnMut(DrawLine)) {
        let total_time = self
            .branch()
            .frames
            .iter()
            .skip(1)
            .map(|f| f.parameters.frame_time)
            .sum::<f32>();

        for (idx, branch) in self.branches.iter().enumerate() {
            if idx == self.branch_idx {
                continue;
            }

            if branch.branch.is_hidden {
                // Skip hidden branches.
                continue;
            }

            let mut time = 0.;
            for (prev_idx, (prev, frame)) in branch.frames.iter().tuple_windows().enumerate() {
                let idx = prev_idx + 1;

                // How many frames until the visible part, clamped in a way to allow for smooth
                // dimming.
                let frames_until_hidden =
                    self.first_shown_frame_idx.saturating_sub(idx - 1).min(20);
                // Hidden frames become invisible, smoothly transition into visible.
                let dim = if frames_until_hidden == 0 {
                    1.
                } else {
                    (20 - frames_until_hidden) as f32 / 40.
                };

                let prev_pos = prev.state.player.pos;
                let pos = frame.state.player.pos;

                // Line from previous to this frame position.
                draw(DrawLine {
                    start: prev_pos,
                    end: pos,
                    color: Vec3::ONE * 0.5 * dim,
                });

                time += frame.parameters.frame_time;
                let next_frame = branch.frames.get(idx + 1).unwrap_or(frame);
                let next_time = time + next_frame.parameters.frame_time;
                if (time - total_time).abs() < (next_time - total_time).abs() {
                    // Draw other branches up to the length of the current branch.
                    break;
                }
            }
        }
    }

    fn draw_inner(&self, mut draw: impl FnMut(DrawLine)) {
        // At least on my machine, things that are drawn later visually appear over things that are
        // drawn earlier. Therefore, the drawing order should be from the least to the most
        // important.
        self.draw_other_branches(&mut draw);
        self.draw_auto_smoothing(&mut draw);
        self.draw_current_branch(&mut draw);
    }

    /// Draws the editor UI.
    #[instrument("Editor::draw", skip_all)]
    pub fn draw(&self, tri: &TriangleApi) {
        tri.render_mode(RenderMode::TransColor);

        tri.begin(Primitive::Lines);

        self.draw_inner(|DrawLine { start, end, color }| {
            tri.color(color.x, color.y, color.z, 1.);
            tri.vertex(start);
            tri.vertex(end);
        });

        tri.end();
    }
}

fn perpendicular(prev: Vec3, next: Vec3) -> Vec3 {
    let line = (next - prev).normalize_or_zero();

    let rv = if line.x == 0. {
        Vec3::X
    } else if line.y == 0. {
        Vec3::Y
    } else {
        Vec3::new(1., -line.x / line.y, 0.).normalize()
    };

    // Make sure it's oriented in a particular way: this makes right-drag to change yaw behave as
    // expected (the yaw will change in the direction where you move the mouse).
    if rv.x * line.y - rv.y * line.x > 0. {
        -rv
    } else {
        rv
    }
}

fn forward(pitch: f32, yaw: f32) -> Vec3 {
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    Vec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, -sin_pitch)
}

fn unwrap_angles(xs: impl Iterator<Item = f32>) -> impl Iterator<Item = f32> {
    use std::f32::consts::PI;

    xs.scan((0., 0.), |(prev, offset), curr| {
        let mut diff = curr - *prev + *offset;
        while diff >= PI {
            diff -= 2. * PI;
            *offset -= 2. * PI;
        }
        while diff <= -PI {
            diff += 2. * PI;
            *offset += 2. * PI;
        }

        *prev += diff;
        Some(*prev)
    })
}

fn smoothed_yaws(
    window_size: f32,
    small_window_size: f32,
    small_window_multiplier: f32,
    frames: &[Frame],
) -> Vec<f32> {
    if frames.is_empty() {
        return vec![];
    }

    let yaws = frames.iter().map(|f| f.state.prev_frame_input.yaw);
    let unwrapped: Vec<f32> = unwrap_angles(yaws).collect();
    let mut rv = Vec::with_capacity(unwrapped.len());

    fn frame_time(frame: &Frame) -> f32 {
        frame.parameters.frame_time
    }

    let repeat_first = iter::repeat((frame_time(&frames[0]), unwrapped[0]));
    let repeat_last = iter::repeat((
        frame_time(frames.last().unwrap()),
        *unwrapped.last().unwrap(),
    ));

    // The smoothing window is centered at the center of each yaw.
    for i in 0..unwrapped.len() {
        let mut total_yaw = 0.;
        let mut total_weight = 0.;

        let mut process_frame =
            |(mut rem_win_size, mut rem_small_win_size), (mut frame_time, yaw): (f32, f32)| {
                // If there's any small window zone left to cover, do so.
                if rem_small_win_size > 0. {
                    let dt = frame_time.min(rem_small_win_size);
                    let weight = dt * small_window_multiplier;

                    total_yaw += yaw * weight;
                    total_weight += weight;

                    rem_win_size -= dt;
                    rem_small_win_size -= dt;
                    frame_time -= dt;
                }

                if frame_time <= 0. {
                    // Ran out of frame time in the branch above (entire frame was covered by the
                    // small window).
                    return ControlFlow::Continue((rem_win_size, rem_small_win_size));
                }

                if rem_win_size <= 0. {
                    // Ran out of smoothing window, break.
                    return ControlFlow::Break(());
                }

                // If there's any regular window zone left to cover, do so.
                let dt = frame_time.min(rem_win_size);
                let weight = dt;

                total_yaw += yaw * weight;
                total_weight += weight;

                rem_win_size -= dt;
                // No need to decrease rem_small_win_size as it is already == 0 here.

                if rem_win_size <= 0. {
                    // Ran out of smoothing window, break.
                    ControlFlow::Break(())
                } else {
                    // Still have smoothing window remaining, continue.
                    ControlFlow::Continue((rem_win_size, rem_small_win_size))
                }
            };

        let rem_win_size = window_size / 2.;
        let rem_small_win_size = small_window_size / 2.;

        // Start from the middle frame.
        let middle_frame_half = iter::once((frames[i].parameters.frame_time / 2., unwrapped[i]));

        // Walk back half an interval.
        middle_frame_half
            .clone()
            .chain(
                zip(
                    frames[..i].iter().map(frame_time),
                    unwrapped[..i].iter().copied(),
                )
                .rev(),
            )
            .chain(repeat_first.clone())
            .try_fold((rem_win_size, rem_small_win_size), &mut process_frame);

        // Walk forward half an interval.
        middle_frame_half
            .chain(zip(
                frames[i + 1..].iter().map(frame_time),
                unwrapped[i + 1..].iter().copied(),
            ))
            .chain(repeat_last.clone())
            .try_fold((rem_win_size, rem_small_win_size), &mut process_frame);

        rv.push(total_yaw / total_weight);
    }

    rv
}

fn replace_multiple_params<'a>(
    old_script: &HLTAS,
    new_script: &'a HLTAS,
) -> Option<(usize, usize, &'a [Line])> {
    if new_script.properties != old_script.properties {
        return None;
    }

    // Most manual edits will probably modify, add or delete a few lines close together, leaving
    // everything before and after the same. We find this region by discarding all consecutive lines
    // that remained the same at the beginning and at the end of the two scripts.

    let new_len = new_script.lines.len();
    let old_len = old_script.lines.len();

    // Find the index of the first non-matching line. It is the same between the two scripts.
    let matching_lines_from_start = zip(&new_script.lines, &old_script.lines)
        .find_position(|(new, old)| new != old)
        .map(|(idx, _)| idx)
        // If we exhaust one of the iterators, return the current index, which is equal to the
        // length of the shorter iterator.
        .unwrap_or(min(new_len, old_len));
    let first_line_idx = matching_lines_from_start;

    // Operate on the non-matching part to avoid the next search from stepping on the same lines.
    //
    // For example, consider a test case like this:
    //
    // ```
    // version 1
    // frames
    // ----------|------|------|0.001|-|-|1
    // ----------|------|------|0.001|-|-|1
    // ```
    //
    // ```
    // version 1
    // frames
    // ----------|------|------|0.001|-|-|1
    // ```
    //
    // When doing the reverse search below using the original (non-sliced) arrays, it will count the
    // single matching line from both the start and the end, causing an overlap and a logic error.
    // By slicing here we're enforcing that all lines that were counted during the forward search
    // are not counted during the reverse search.
    let new_script_non_matching = &new_script.lines[first_line_idx..];
    let old_script_non_matching = &old_script.lines[first_line_idx..];
    let new_len_non_matching = new_len - matching_lines_from_start;
    let old_len_non_matching = old_len - matching_lines_from_start;

    // Find the index of the first non-matching line from the end. It is the same between the two
    // scripts.
    let matching_lines_from_end = zip(
        new_script_non_matching.iter().rev(),
        old_script_non_matching.iter().rev(),
    )
    .find_position(|(new, old)| new != old)
    .map(|(idx, _)| idx)
    // If we exhaust one of the iterators, return the current index, which is equal to the
    // length of the shorter iterator.
    .unwrap_or(min(new_len_non_matching, old_len_non_matching));

    let count = old_len - matching_lines_from_end - matching_lines_from_start;

    let new_one_past_last_line_idx = new_len - matching_lines_from_end;
    let to = &new_script.lines[first_line_idx..new_one_past_last_line_idx];

    Some((first_line_idx, count, to))
}

#[cfg(test)]
mod tests {
    use bxt_strafe::{Input, Parameters, State};
    use expect_test::{expect, Expect};
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn undo_redo() {
        let script =
            HLTAS::from_str("version 1\nframes\n----------|------|------|0.004|10|-|6").unwrap();
        let mut editor = Editor::create_in_memory(&script).unwrap();

        // Undo with no changes should return an error saying there are no actions to undo.
        assert!(matches!(editor.undo(), Err(ManualOpError::UserError(_))));

        let before_op = editor.branch().branch.script.clone();
        editor
            .apply_operation(Operation::SetFrameCount {
                bulk_idx: 0,
                from: 6,
                to: 10,
            })
            .unwrap();
        let after_op = editor.branch().branch.script.clone();
        assert_ne!(before_op, after_op, "operation should affect the HLTAS");

        editor.undo().unwrap();
        assert_eq!(
            before_op,
            editor.branch().branch.script,
            "undo produced wrong result"
        );

        editor.redo().unwrap();
        assert_eq!(
            after_op,
            editor.branch().branch.script,
            "redo produced wrong result"
        );

        // Redo with no changes should return an error saying there are no actions to redo.
        assert!(matches!(editor.redo(), Err(ManualOpError::UserError(_))));
    }

    #[test]
    fn replace_multiple_optimization_bug_1() {
        let script = HLTAS::from_str(
            "version 1\nframes\n\
                ----------|------|------|0.004|10|-|6\n\
                ----------|------|------|0.004|10|-|6",
        )
        .unwrap();

        let new_script = HLTAS::from_str(
            "version 1\nframes\n\
                ----------|------|------|0.004|10|-|6",
        )
        .unwrap();

        let (first_line_idx, count, to) = replace_multiple_params(&script, &new_script).unwrap();
        assert_eq!(first_line_idx, 1);
        assert_eq!(count, 1);
        assert_eq!(to, []);
    }

    #[test]
    fn replace_multiple_optimization_bug_2() {
        let script = HLTAS::from_str("version 1\nframes\nstrafing vectorial").unwrap();
        let new_script = HLTAS::from_str("version 1\nframes\nstrafing vectorial").unwrap();

        let (first_line_idx, count, to) = replace_multiple_params(&script, &new_script).unwrap();
        assert_eq!(first_line_idx, 1);
        assert_eq!(count, 0);
        assert_eq!(to, []);
    }

    #[test]
    fn selected_bulk_idx_is_not_invalid_after_rewrite() {
        let script = HLTAS::from_str(
            "version 1\nframes\n\
                ----------|------|------|0.004|10|-|6\n\
                ----------|------|------|0.004|20|-|6",
        )
        .unwrap();
        let mut editor = Editor::create_in_memory(&script).unwrap();

        editor.selected_bulk_idx = Some(1);

        let new_script = HLTAS::from_str(
            "version 1\nframes\n\
                ----------|------|------|0.004|10|-|6",
        )
        .unwrap();
        editor.rewrite(new_script).unwrap();

        assert_eq!(editor.selected_bulk_idx, None);
    }

    fn check_unwrap_angles(input: impl IntoIterator<Item = f32>, expect: Expect) {
        let radians = input.into_iter().map(|x| x.to_radians());
        let unwrapped: Vec<f32> = unwrap_angles(radians)
            .map(|x| x.to_degrees().round())
            .collect();
        expect.assert_debug_eq(&unwrapped);
    }

    #[test]
    fn test_unwrap_angles_idempotent() {
        check_unwrap_angles(
            [0., 1., 2., 3.],
            expect![[r#"
            [
                0.0,
                1.0,
                2.0,
                3.0,
            ]
        "#]],
        );
    }

    #[test]
    fn test_unwrap_angles() {
        check_unwrap_angles(
            [0., 170., -170., 160., -160., -165.],
            expect![[r#"
                [
                    0.0,
                    170.0,
                    190.0,
                    160.0,
                    200.0,
                    195.0,
                ]
            "#]],
        );
    }

    #[test]
    fn test_unwrap_angles_multiple_revolutions() {
        check_unwrap_angles(
            [
                0., 120., -120., 0., 120., -120., 0., 120., -120., 120., 0., -120., 120., 0., -120.,
            ],
            expect![[r#"
                [
                    0.0,
                    120.0,
                    240.0,
                    360.0,
                    480.0,
                    600.0,
                    720.0,
                    840.0,
                    960.0,
                    840.0,
                    720.0,
                    600.0,
                    480.0,
                    360.0,
                    240.0,
                ]
            "#]],
        );
    }

    fn check_smoothing(
        input: impl IntoIterator<Item = (f32, f32)>,
        small_window_size: f32,
        expect: Expect,
    ) {
        let frames: Vec<Frame> = input
            .into_iter()
            .map(|(frame_time, yaw)| Frame {
                parameters: Parameters {
                    frame_time,
                    ..Default::default()
                },
                state: State {
                    prev_frame_input: Input {
                        yaw,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            })
            .collect();

        let smoothed = smoothed_yaws(1., small_window_size, 4., &frames);
        expect.assert_debug_eq(&smoothed);
    }

    #[test]
    fn test_smoothing_on_small_input() {
        check_smoothing(
            [(0.1, 2.), (0.1, 2.), (0.1, 2.)],
            0.5,
            expect![[r#"
            [
                2.0,
                2.0,
                2.0,
            ]
        "#]],
        );
    }

    #[test]
    fn test_smoothing_no_small_window() {
        check_smoothing(
            [
                (0.25, -1.),
                (0.25, -1.),
                (0.5, 1.),
                (0.25, -1.),
                (0.25, -1.),
            ],
            0.,
            expect![[r#"
                [
                    -0.75,
                    -0.25,
                    0.0,
                    -0.25,
                    -0.75,
                ]
            "#]],
        );
    }

    #[test]
    fn test_smoothing_only_small_window() {
        check_smoothing(
            [
                (0.25, -1.),
                (0.25, -1.),
                (0.5, 1.),
                (0.25, -1.),
                (0.25, -1.),
            ],
            1.,
            expect![[r#"
                [
                    -0.75,
                    -0.25,
                    0.0,
                    -0.25,
                    -0.75,
                ]
            "#]],
        );
    }

    #[test]
    fn test_smoothing() {
        check_smoothing(
            [
                (0.25, -1.),
                (0.25, -1.),
                (0.5, 1.),
                (0.25, -1.),
                (0.25, -1.),
            ],
            0.5,
            expect![[r#"
                [
                    -0.9,
                    -0.4,
                    0.6,
                    -0.4,
                    -0.9,
                ]
            "#]],
        );
    }

    #[test]
    fn test_smoothing_even() {
        check_smoothing(
            [
                (0.25, -1.),
                (0.25, -1.),
                (0.25, 1.),
                (0.25, 1.),
                (0.25, -1.),
                (0.25, -1.),
            ],
            0.5,
            expect![[r#"
                [
                    -0.9,
                    -0.4,
                    0.3,
                    0.3,
                    -0.4,
                    -0.9,
                ]
            "#]],
        );
    }

    #[test]
    fn test_smoothing_partial_small() {
        check_smoothing(
            [
                (0.25, -1.),
                (0.25, -1.),
                (0.4, 1.),
                (0.25, -1.),
                (0.25, -1.),
            ],
            0.5,
            expect![[r#"
                [
                    -0.9,
                    -0.4,
                    0.28000003,
                    -0.4,
                    -0.9,
                ]
            "#]],
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: if std::env::var_os("RUN_SLOW_TESTS").is_none() {
                eprintln!("ignoring slow test");
                0
            } else {
                ProptestConfig::default().cases
            },
            ..ProptestConfig::default()
        })]

        #[test]
        fn replace_multiple_optimization_is_correct(mut old_script: HLTAS, mut new_script: HLTAS) {
            // Get rid of the non-interesting cases.
            new_script.properties = old_script.properties.clone();
            let (first_line_idx, count, to_lines) =
                replace_multiple_params(&old_script, &new_script).unwrap();

            let from_lines = &old_script.lines[first_line_idx..first_line_idx + count];

            let mut buffer = Vec::new();
            hltas::write::gen_lines(&mut buffer, from_lines)
                .expect("writing to an in-memory buffer should never fail");
            let from = String::from_utf8(buffer)
                .expect("Line serialization should never produce invalid UTF-8");

            let mut buffer = Vec::new();
            hltas::write::gen_lines(&mut buffer, to_lines)
                .expect("writing to an in-memory buffer should never fail");
            let to = String::from_utf8(buffer)
                .expect("Line serialization should never produce invalid UTF-8");

            let op = Operation::ReplaceMultiple {
                first_line_idx,
                from,
                to,
            };

            op.apply(&mut old_script);
            prop_assert_eq!(old_script, new_script);
        }
    }
}
