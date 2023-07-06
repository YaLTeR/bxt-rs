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
use glam::{Vec2, Vec3};
use hltas::types::{AutoMovement, Line, StrafeDir, StrafeSettings, VectorialStrafingConstraints};
use hltas::HLTAS;
use itertools::Itertools;

use self::db::{Action, ActionKind, Branch, Db};
use self::operation::{Key, Operation};
use self::toggle_auto_action::ToggleAutoActionTarget;
use self::utils::{
    bulk_and_first_frame_idx, bulk_and_first_frame_idx_mut, bulk_idx_and_is_last,
    bulk_idx_and_repeat_at_frame, bulks_with_non_bulk_lines, line_idx_and_repeat_at_frame,
    FrameBulkExt,
};
use super::remote::{AccurateFrame, PlayRequest};
use crate::hooks::sdl::MouseState;
use crate::modules::tas_optimizer::simulator::Simulator;
use crate::modules::triangle_drawing::triangle_api::{Primitive, RenderMode};
use crate::modules::triangle_drawing::TriangleApi;

mod db;
pub mod operation;
pub mod toggle_auto_action;
pub mod utils;

const SMOOTHING_WINDOW_S: f32 = 0.15;
const SMOOTHING_SMALL_WINDOW_S: f32 = 0.03;
const SMOOTHING_SMALL_WINDOW_MUL: f32 = 3.;

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
    hovered_frame_idx: Option<usize>,

    /// Mouse state from the last time `tick()` was called.
    prev_mouse_state: MouseState,

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
    // ==============================================
    // Camera-editor-specific state.
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
    /// Data for auto-smoothing.
    auto_smoothing: AutoSmoothing,
}

impl BranchData {
    fn new(branch: Branch) -> Self {
        Self {
            branch,
            frames: vec![],
            first_predicted_frame: 0,
            auto_smoothing: AutoSmoothing {
                script: None,
                frames: vec![],
            },
        }
    }
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

#[derive(Debug, Clone, Copy)]
pub struct KeyboardState {
    /// Whether the "faster" key is pressed.
    pub adjust_faster: bool,
    /// Whether the "slower" key is pressed.
    pub adjust_slower: bool,
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
            frame_count_adjustment: None,
            yaw_adjustment: None,
            left_right_count_adjustment: None,
            adjacent_frame_count_adjustment: None,
            adjacent_yaw_adjustment: None,
            adjacent_left_right_count_adjustment: None,
            in_camera_editor: false,
            auto_smoothing: false,
            show_player_bbox: false,
            first_shown_frame_idx: 0,
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

    pub fn set_in_camera_editor(&mut self, value: bool) {
        if self.in_camera_editor == value {
            return;
        }

        self.cancel_ongoing_adjustments();
        self.in_camera_editor = value;
    }

    pub fn set_auto_smoothing(&mut self, value: bool) {
        self.auto_smoothing = value;
    }

    pub fn set_show_player_bbox(&mut self, value: bool) {
        self.show_player_bbox = value;
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

        self.generation = self.generation.wrapping_add(1);
    }

    fn is_any_adjustment_active(&self) -> bool {
        self.frame_count_adjustment.is_some()
            || self.yaw_adjustment.is_some()
            || self.left_right_count_adjustment.is_some()
            || self.adjacent_frame_count_adjustment.is_some()
            || self.adjacent_yaw_adjustment.is_some()
            || self.adjacent_left_right_count_adjustment.is_some()
    }

    /// Updates the editor state.
    pub fn tick<T: Trace>(
        &mut self,
        tracer: &T,
        world_to_screen: impl Fn(Vec3) -> Option<Vec2>,
        mouse: MouseState,
        keyboard: KeyboardState,
        deadline: Instant,
    ) -> eyre::Result<()> {
        let _span = info_span!("Editor::tick").entered();

        // Update ongoing adjustments.
        self.tick_frame_count_adjustment(mouse, keyboard)?;
        self.tick_yaw_adjustment(mouse, keyboard)?;
        self.tick_left_right_count_adjustment(mouse, keyboard)?;
        self.tick_adjacent_frame_count_adjustment(mouse, keyboard)?;
        self.tick_adjacent_yaw_adjustment(mouse, keyboard)?;
        self.tick_adjacent_left_right_count_adjustment(mouse, keyboard)?;

        // Predict any frames that need prediction.
        //
        // Do this after adjustment and before computing input to have the most up-to-date data.
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

            // Only update the selected bulk and start the adjustments if the mouse has just been
            // pressed.
            if mouse_became_down {
                // Since all mouse buttons that start adjustments have been down last frame, there
                // cannot be any active adjustments now.
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

                        let dir = match (a_screen, b_screen) {
                            (Some(a), Some(b)) => a - b,
                            // Presumably, one of the points is invisible, so just fall back.
                            _ => Vec2::X,
                        };

                        if let Some(yaw) = bulk.yaw() {
                            self.yaw_adjustment = Some(MouseAdjustment::new(*yaw, mouse_pos, dir));
                        } else if let Some(count) = bulk.left_right_count() {
                            // Make the adjustment face the expected way.
                            let dir = match bulk.auto_actions.movement {
                                Some(AutoMovement::Strafe(StrafeSettings {
                                    dir: StrafeDir::RightLeft(_),
                                    ..
                                })) => -dir,
                                _ => dir,
                            };

                            self.left_right_count_adjustment =
                                Some(MouseAdjustment::new(count.get(), mouse_pos, dir));
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
                        }
                    }
                }
            }
        }

        // Update the hovered frame index.
        self.hovered_frame_idx = self
            .branch()
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
            .map(|(frame_idx, _)| frame_idx);

        self.prev_mouse_state = mouse;

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

        self.store_operation(op)
    }

    /// Undoes the last action if any.
    pub fn undo(&mut self) -> eyre::Result<()> {
        // Don't undo during active adjustments because:
        // 1. adustments store the orginal value, which will potentially change after an undo,
        // 2. what if undo removes the frame bulk being adjusted?
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        let Some(action) = self.undo_log.pop() else {
            return Ok(());
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
    pub fn redo(&mut self) -> eyre::Result<()> {
        // Don't redo during active adjustments because:
        // 1. adustments store the orginal value, which will potentially change after an undo,
        // 2. what if undo removes the frame bulk being adjusted?
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        let Some(action) = self.redo_log.pop() else {
            return Ok(());
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

    /// Deletes the selected frame bulk, if any.
    pub fn delete_selected(&mut self) -> eyre::Result<()> {
        // Don't delete during active adjustments because they store the frame bulk index.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if self.in_camera_editor {
            return Ok(());
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Ok(());
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
        self.apply_operation(op)
    }

    /// Deletes the last frame bulk, if any.
    pub fn delete_last(&mut self) -> eyre::Result<()> {
        // Don't delete during active adjustments because they store the frame bulk index.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if self.in_camera_editor {
            return Ok(());
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
            return Ok(());
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
        self.apply_operation(op)
    }

    /// Splits frame bulk at hovered frame.
    pub fn split(&mut self) -> eyre::Result<()> {
        // Don't split during active adjustments because they store the frame bulk index.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if self.in_camera_editor {
            return Ok(());
        }

        let Some(frame_idx) = self.hovered_frame_idx else {
            return Ok(());
        };

        let total_frames = self
            .branch()
            .branch
            .script
            .frame_bulks()
            .map(|bulk| bulk.frame_count.get() as usize)
            .sum::<usize>();

        // Can't split at the very end of the HLTAS.
        if frame_idx == total_frames {
            return Ok(());
        }

        let (_line_idx, repeat) =
            line_idx_and_repeat_at_frame(&self.branch().branch.script.lines, frame_idx)
                .expect("invalid frame index");

        // Can't split because this is already a split point.
        if repeat == 0 {
            return Ok(());
        }

        let op = Operation::Split { frame_idx };
        self.apply_operation(op)
    }

    /// Toggles a key on the selected frame bulk.
    pub fn toggle_key(&mut self, key: Key) -> eyre::Result<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if self.in_camera_editor {
            return Ok(());
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Ok(());
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
        self.apply_operation(op)
    }

    /// Toggles an auto-action on the selected frame bulk.
    pub fn toggle_auto_action(&mut self, target: ToggleAutoActionTarget) -> eyre::Result<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if self.in_camera_editor {
            return Ok(());
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Ok(());
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
        self.apply_operation(op)
    }

    /// Sets pitch of the selected frame bulk.
    pub fn set_pitch(&mut self, new_pitch: Option<f32>) -> eyre::Result<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if self.in_camera_editor {
            return Ok(());
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Ok(());
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
        self.apply_operation(op)
    }

    /// Sets yaw of the selected frame bulk.
    pub fn set_yaw(&mut self, new_yaw: Option<f32>) -> eyre::Result<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if self.in_camera_editor {
            return Ok(());
        }

        let Some(bulk_idx) = self.selected_bulk_idx else {
            return Ok(());
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
        self.apply_operation(op)
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

    /// Rewrites the script with a completely new version.
    pub fn rewrite(&mut self, new_script: HLTAS) -> eyre::Result<()> {
        // Don't toggle during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        let script = self.script();
        if new_script == *script {
            return Ok(());
        }

        // Check if we can optimize a full rewrite into a lines replacement.
        if let Some((first_line_idx, count, to)) = replace_multiple_params(script, &new_script) {
            return self.replace_multiple(first_line_idx, count, to);
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
        self.apply_operation(op)
    }

    /// Applies global smoothing to the entire script.
    pub fn apply_global_smoothing(&mut self) -> eyre::Result<()> {
        // Don't apply during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        let frame_count = self
            .branch()
            .branch
            .script
            .frame_bulks()
            .map(|bulk| bulk.frame_count.get() as usize)
            .sum::<usize>();

        // Only smooth when we have all accurate frames.
        if self.branch().first_predicted_frame != frame_count {
            return Ok(());
        }

        // TODO: Skip the very last frame because we never have accurate info for it at the moment.
        let frames = &self.branch().frames;
        let smoothed = smoothed_yaws(
            SMOOTHING_WINDOW_S,
            SMOOTHING_SMALL_WINDOW_S,
            SMOOTHING_SMALL_WINDOW_MUL,
            &frames[..frames.len() - 1],
        );

        let mut line = "target_yaw_override".to_string();
        // Skip the first frame because it is the initial frame before the start of the TAS.
        for yaw in &smoothed[1..] {
            let yaw = yaw.to_degrees();
            write!(&mut line, " {yaw}").unwrap();
        }

        let op = Operation::Insert { line_idx: 0, line };
        self.apply_operation(op)
    }

    /// Applies smoothing to the segment under cursor.
    pub fn apply_smoothing_to_hovered_segment(&mut self) -> eyre::Result<()> {
        // Don't apply during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if !self.in_camera_editor {
            return Ok(());
        }

        let Some(hovered_frame_idx) = self.hovered_frame_idx else {
            return Ok(());
        };

        // Find the input region the user is pointing at.
        let frames = &self.branch().frames;
        let Some([start, end]) = smoothing_input_regions(SMOOTHING_WINDOW_S, frames)
            .find(|&[start, end]| hovered_frame_idx >= start && hovered_frame_idx <= end)
        else {
            return Ok(());
        };

        // Only smooth when we have all accurate frames.
        if self.branch().first_predicted_frame <= end {
            return Ok(());
        }

        let mut smoothed = smoothed_yaws(
            SMOOTHING_WINDOW_S,
            SMOOTHING_SMALL_WINDOW_S,
            SMOOTHING_SMALL_WINDOW_MUL,
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
            self.apply_operation(op)
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
            self.apply_operation(op)
        }
    }

    /// Hides frames before the hovered frame, or shows all frames if there's no hovered frame.
    pub fn hide_frames_up_to_hovered(&mut self) -> eyre::Result<()> {
        // Don't apply during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
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

    pub fn apply_accurate_frame(&mut self, frame: AccurateFrame) -> Option<PlayRequest> {
        if frame.generation != self.generation {
            return None;
        }

        // TODO: make this nicer somehow maybe?
        if frame.frame_idx == 0 {
            // Initial frame is the same for all branches and between smoothed/unsmoothed.
            for branch in &mut self.branches {
                if branch.frames.is_empty() {
                    branch.frames.push(frame.frame.clone());
                    branch.first_predicted_frame = 1;
                }

                if branch.auto_smoothing.frames.is_empty() {
                    branch.auto_smoothing.frames.push(frame.frame.clone());
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
        } else {
            let current_frame = &mut branch.frames[frame.frame_idx];
            if *current_frame != frame.frame {
                *current_frame = frame.frame;

                branch.frames.truncate(frame.frame_idx + 1);
                branch.first_predicted_frame =
                    min(branch.first_predicted_frame, frame.frame_idx + 1);
            }
        }

        if self.auto_smoothing {
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
                    SMOOTHING_WINDOW_S,
                    SMOOTHING_SMALL_WINDOW_S,
                    SMOOTHING_SMALL_WINDOW_MUL,
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

    pub fn set_stop_frame(&mut self, stop_frame: u32) -> eyre::Result<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        self.branch_mut().branch.stop_frame = stop_frame;
        self.db.update_branch(&self.branch().branch)?;

        Ok(())
    }

    pub fn set_stop_frame_to_hovered(&mut self) -> eyre::Result<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        let Some(frame_idx) = self.hovered_frame_idx else {
            return Ok(());
        };
        self.set_stop_frame(frame_idx.try_into().unwrap())
    }

    pub fn branch_clone(&mut self) -> eyre::Result<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
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

    pub fn branch_focus(&mut self, branch_idx: usize) -> eyre::Result<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        if self.branch_idx == branch_idx {
            return Ok(());
        }

        let Some(branch) = self.branches.get(branch_idx) else {
            return Ok(());
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

    pub fn branch_focus_next(&mut self) -> eyre::Result<()> {
        let Some(branch_idx) = (self.branch_idx + 1..self.branches.len())
            .chain(0..self.branch_idx)
            .find(|&idx| !self.branches[idx].branch.is_hidden)
        else {
            return Ok(());
        };

        self.branch_focus(branch_idx)
    }

    pub fn branch_hide(&mut self, branch_idx: usize) -> eyre::Result<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        // Don't try to hide the current branch on its own: this causes it to hide only when we
        // switch from it subsequently, which is confusing.
        if self.branch_idx == branch_idx {
            return Ok(());
        }

        let Some(branch) = self.branches.get_mut(branch_idx) else {
            return Ok(());
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

    pub fn branch_hide_and_focus_next(&mut self) -> eyre::Result<()> {
        let Some(next_branch_idx) = (self.branch_idx + 1..self.branches.len())
            .chain(0..self.branch_idx)
            .find(|&idx| !self.branches[idx].branch.is_hidden)
        else {
            return Ok(());
        };

        let curr_branch_idx = self.branch_idx;
        self.branch_focus(next_branch_idx)?;
        self.branch_hide(curr_branch_idx)
    }

    pub fn branch_show(&mut self, branch_idx: usize) -> eyre::Result<()> {
        // Don't do this during active adjustments for consistency with other operations.
        if self.is_any_adjustment_active() {
            return Ok(());
        }

        let Some(branch) = self.branches.get_mut(branch_idx) else {
            return Ok(());
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

        let mut is_idempotent_smoothing_region = false;
        let in_idempotent_smoothing_region =
            smoothing_idempotent_regions(SMOOTHING_WINDOW_S, &branch.frames)
                .flatten()
                .tuple_windows()
                .flat_map(|(start, end)| {
                    is_idempotent_smoothing_region = !is_idempotent_smoothing_region;
                    (start..end).map(move |_| is_idempotent_smoothing_region)
                });

        // This does the same loop as `smoothing_idempotent_regions()` internally resulting in
        // duplicate computation. This could be improved.
        let smoothing_input_region = self.hovered_frame_idx.and_then(|hovered| {
            smoothing_input_regions(SMOOTHING_WINDOW_S, &branch.frames)
                .find(|&[start, end]| hovered >= start && hovered <= end)
        });

        let non_bulk_lines = bulks_with_non_bulk_lines(&branch.branch.script.lines)
            .flat_map(|lines| {
                let (end, empties) = if let Some(frame_bulk) = lines.last().unwrap().frame_bulk() {
                    (lines.len() - 1, frame_bulk.frame_count.get() - 1)
                } else {
                    (lines.len(), 0)
                };
                iter::once(&lines[0..end]).chain((0..empties).map(|_| &[][..]))
            })
            // When the script ends with a frame bulk, this iterator will be missing the last frame.
            // To fix this, append an empty element unconditionally. Since zip cuts off the iterator
            // when any of the iterators ends, this element should not pose a problem when the
            // script ends with a non-bulk frame.
            .chain([&[][..]]);

        // Draw regular frames.
        //
        // Note: there's no iterator cloning, which means all values are computed once, in one go.
        let mut collided_this_bulk = false;
        let iter = iter::zip(
            // Pairs of frames: (0, 1), (1, 2), (2, 3) and so on.
            branch.frames.iter().tuple_windows(),
            // For second frame in pair: its frame bulk index and whether it's last in its bulk.
            bulk_idx_and_is_last(&branch.branch.script.lines),
        )
        // For second frame in pair: whether it's in an idempotent smoothing region.
        .zip(in_idempotent_smoothing_region.skip(1))
        .zip(non_bulk_lines.skip(1))
        .enumerate();
        for (
            prev_idx,
            (
                (
                    ((prev, frame), (bulk_idx, bulk, is_last_in_bulk)),
                    in_idempotent_smoothing_region,
                ),
                non_bulk_lines,
            ),
        ) in iter
        {
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

            // If frame is in the smoothing input region.
            let in_smoothing_input_region =
                smoothing_input_region.map_or(false, |[start, end]| idx >= start && idx <= end);

            // How many frames until the visible part, clamped in a way to allow for smooth dimming.
            let frames_until_hidden = self.first_shown_frame_idx.saturating_sub(idx - 1).min(20);

            // Inaccurate frames get dimmed.
            let dim_inaccurate = if is_predicted { 0.6 } else { 1. };
            // Unhovered bulks get dimmed.
            let dim_unhovered = if self.in_camera_editor {
                // TODO: fill this in when the camera editor gets editing.
                1.
            } else if is_hovered_bulk {
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

            if self.in_camera_editor {
                // Draw camera angle line.
                let camera_pitch = frame.state.prev_frame_input.pitch;
                let camera_yaw = frame.state.prev_frame_input.yaw;
                let camera_vector = forward(camera_pitch, camera_yaw);

                let hue = if in_smoothing_input_region {
                    Vec3::new(1., 0.75, 0.5)
                } else if in_idempotent_smoothing_region {
                    Vec3::new(0., 1., 0.)
                } else {
                    Vec3::new(0.5, 0.5, 1.)
                };

                draw(DrawLine {
                    start: pos,
                    end: pos + camera_vector * 5.,
                    color: hue * dim_inaccurate * dim_hidden,
                });

                // Show the last camera line, if any.
                if let Some(camera_line) = non_bulk_lines.iter().rfind(|line| {
                    matches!(
                        line,
                        Line::Change(_)
                            | Line::TargetYawOverride { .. }
                            | Line::RenderYawOverride { .. }
                    )
                }) {
                    let perp = perpendicular(prev_pos, pos) * 5.;

                    let hue = match camera_line {
                        Line::TargetYawOverride { .. } => Vec3::new(1., 0.75, 0.5),
                        Line::RenderYawOverride { .. } => Vec3::new(1., 0., 0.),
                        _ => WHITE,
                    };

                    draw(DrawLine {
                        start: pos - perp,
                        end: pos + perp,
                        color: hue * dim,
                    });
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
        for (idx, branch) in self.branches.iter().enumerate() {
            if idx == self.branch_idx {
                continue;
            }

            if branch.branch.is_hidden {
                // Skip hidden branches.
                continue;
            }

            for (prev_idx, (prev, frame)) in branch.frames.iter().tuple_windows().enumerate() {
                let idx = prev_idx + 1;
                if idx >= self.branch().frames.len() {
                    // Draw other branches up to the length of the current branch.
                    // TODO: this should use time, not frame number.
                    break;
                }

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

/// Returns an iterator of pairs of start and one-past-end frame indices of regions where smoothing
/// is idempotent.
fn smoothing_idempotent_regions(
    smoothing_window_size: f32,
    frames: &[Frame],
) -> impl Iterator<Item = [usize; 2]> + '_ {
    // TODO: This should check the differences between successive yaws, not the yaws themselves.
    //
    // Our smoothing infinitely pads the input frames from both sides.
    let mut idx = 1;
    let mut same_yaw_duration = f32::INFINITY;
    let mut same_yaw_started_at = 0;
    let mut done = false;
    iter::from_fn(move || {
        if done {
            return None;
        }

        while idx < frames.len() {
            let prev = &frames[idx - 1];
            let frame = &frames[idx];

            let prev_yaw = prev.state.prev_frame_input.yaw;
            let yaw = frame.state.prev_frame_input.yaw;

            let mut starting_idx_to_return = None;
            if yaw != prev_yaw {
                // TODO: check for off-by-ones.
                if same_yaw_duration >= smoothing_window_size {
                    starting_idx_to_return = Some(same_yaw_started_at);
                }

                same_yaw_duration = 0.;

                // Rememeber the first frame of the same-yaw region.
                same_yaw_started_at = idx;

                // We're starting a new window of yaws, and this frame is already included. So, let
                // the += below run.
            }

            same_yaw_duration += frame.parameters.frame_time;

            idx += 1;

            if let Some(starting_idx) = starting_idx_to_return {
                // We detect when we go from a long-enough same-yaw region to a changing-yaw region.
                // Therefore, we need to return both the index of the start of the same-yaw region,
                // and the current index.
                return Some([starting_idx, idx - 1]);
            }
        }

        done = true;

        // Since our smoothing infinitely pads input frames from both sides, we can return the last
        // region regardless of how many frames it contains.
        Some([same_yaw_started_at, frames.len()])
    })
}

/// Returns an iterator of pairs of start and end frame indices of input regions for smoothing.
///
/// Note: unlike `smoothing_idempotent_regions()`, this function returns inclusive end indices.
fn smoothing_input_regions(
    smoothing_window_size: f32,
    frames: &[Frame],
) -> impl Iterator<Item = [usize; 2]> + '_ {
    smoothing_idempotent_regions(smoothing_window_size, frames)
        .flatten()
        .skip(1)
        .tuples()
        .map(move |(start, one_past_end)| {
            let walk_back = move |mut idx: usize| {
                let mut rem_win_size =
                    smoothing_window_size / 2. - frames[idx].parameters.frame_time / 2.;

                while idx > 0 && rem_win_size >= 0. {
                    idx -= 1;
                    rem_win_size -= frames[idx].parameters.frame_time;
                }

                idx
            };

            let walk_forward = move |mut idx: usize| {
                let mut rem_win_size =
                    smoothing_window_size / 2. - frames[idx].parameters.frame_time / 2.;

                while idx + 1 < frames.len() && rem_win_size >= 0. {
                    idx += 1;
                    rem_win_size -= frames[idx].parameters.frame_time;
                }

                idx
            };

            [walk_back(start), walk_forward(one_past_end - 1)]
        })
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

        // Undo with no changes should do nothing.
        editor.undo().unwrap();

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

        // Redo with no changes should do nothing.
        editor.redo().unwrap();
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

    fn check_smoothing_idempotent_regions(
        input: impl IntoIterator<Item = (f32, f32)>,
        window_size: f32,
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

        let regions = smoothing_idempotent_regions(window_size, &frames).collect::<Vec<_>>();
        expect.assert_debug_eq(&regions);
    }

    #[test]
    fn test_smoothing_idempotent_regions() {
        check_smoothing_idempotent_regions(
            [
                (0.4, 1.),
                (0.4, 1.),
                (0.4, 3.),
                (0.4, 3.),
                (0.4, 4.),
                (0.4, 4.),
                (0.4, 4.),
                (0.4, 5.),
                (0.4, 5.),
            ],
            1.,
            expect![[r#"
                [
                    [
                        0,
                        2,
                    ],
                    [
                        4,
                        7,
                    ],
                    [
                        7,
                        9,
                    ],
                ]
            "#]],
        );
    }

    fn check_smoothing_input_regions(
        input: impl IntoIterator<Item = (f32, f32)>,
        window_size: f32,
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

        let regions = smoothing_input_regions(window_size, &frames).collect::<Vec<_>>();
        expect.assert_debug_eq(&regions);
    }

    #[test]
    fn test_smoothing_input_regions_1() {
        check_smoothing_input_regions(
            [
                (0.4, 1.),
                (0.4, 1.),
                (0.4, 3.),
                (0.4, 3.),
                (0.4, 4.),
                (0.4, 4.),
                (0.4, 4.),
                (0.4, 5.),
                (0.4, 5.),
            ],
            1.,
            expect![[r#"
                [
                    [
                        1,
                        4,
                    ],
                    [
                        6,
                        7,
                    ],
                ]
            "#]],
        );
    }

    #[test]
    fn test_smoothing_input_regions_2() {
        check_smoothing_input_regions(
            [
                (0.4, 1.),
                (0.4, 1.),
                (0.4, 3.),
                (0.4, 3.),
                (0.4, 4.),
                (0.4, 4.),
                (0.4, 4.),
                (0.4, 5.),
                (0.4, 5.),
                (0.4, 6.),
            ],
            1.,
            expect![[r#"
                [
                    [
                        1,
                        4,
                    ],
                    [
                        6,
                        9,
                    ],
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
