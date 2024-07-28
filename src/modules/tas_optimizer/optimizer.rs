use std::error::Error;
use std::io::Write;
use std::num::NonZeroU32;
use std::result::Result;
use std::{iter, mem};

use bxt_ipc_types::Frame;
use bxt_strafe::Trace;
use hltas::types::*;
use hltas::HLTAS;
use rand::distributions::Uniform;
use rand::prelude::Distribution;
use rand::seq::SliceRandom;
use rand::Rng;
use tap::{Conv, Pipe, Tap, TryConv};

use super::hltas_ext::HLTASExt;
use super::objective::{AttemptResult, Objective};
use super::remote;
use super::simulator::Simulator;
use crate::modules::triangle_drawing::triangle_api::{Primitive, RenderMode};
use crate::modules::triangle_drawing::TriangleApi;

pub struct Optimizer {
    /// The first part of the script that we're not optimizing.
    prefix: HLTAS,

    /// The original script being optimized, saved to allow resetting to the initial state.
    original_hltas: HLTAS,

    /// The script being optimized.
    hltas: HLTAS,

    /// Movement frames, starting from the initial frame.
    frames: Vec<Frame>,

    /// Movement frames from the last mutation, starting from the initial frame.
    last_mutation_frames: Option<Vec<Frame>>,

    /// Generation of this script for remote simulation.
    generation: u16,

    /// Console command from the first frame of the optimized script, that we erased.
    erased_console_command: Option<String>,
}

impl Optimizer {
    pub fn new(
        mut hltas: HLTAS,
        first_frame: usize,
        initial_frame: Frame,
        generation: u16,
    ) -> Self {
        let (l, _r) = hltas.line_and_repeat_at_frame(first_frame).unwrap();

        let mut prefix = hltas.clone();
        prefix.lines.truncate(l);

        hltas.lines = hltas.lines[l..].to_vec();

        // Erase the console command that contains bxt_tas_optim_init.
        //
        // This is so when the single-frame mutation mode splits that frame bulk, it does not lead
        // to bxt_tas_optim_init and other unwanted commands running in the remote client.
        let erased_console_command = hltas.lines[0]
            .frame_bulk_mut()
            .unwrap()
            .console_command
            .take();

        Self {
            prefix,
            original_hltas: hltas.clone(),
            hltas,
            frames: vec![initial_frame],
            last_mutation_frames: None,
            generation,
            erased_console_command,
        }
    }

    /// Resets the optimizer to the original non-optimized script.
    ///
    /// You need to pass it a new generation value so that frames arriving from remote clients don't
    /// overwrite it again.
    pub fn reset(&mut self, generation: u16) {
        self.hltas = self.original_hltas.clone();
        self.frames.truncate(1);
        self.generation = generation;
    }

    pub fn draw(&self, tri: &TriangleApi) {
        tri.render_mode(RenderMode::TransColor);
        tri.color(0., 1., 1., 1.);

        tri.begin(Primitive::Lines);

        for pair in self.frames.windows(2) {
            let (prev, next) = (&pair[0], &pair[1]);

            tri.vertex(prev.state.player.pos);
            tri.vertex(next.state.player.pos);
        }

        if let Some(frames) = &self.last_mutation_frames {
            tri.color(0., 0.5, 0.5, 1.);

            for pair in frames.windows(2) {
                let (prev, next) = (&pair[0], &pair[1]);

                tri.vertex(prev.state.player.pos);
                tri.vertex(next.state.player.pos);
            }
        }

        tri.end();
    }

    pub fn save<W: Write>(&mut self, writer: W) -> Result<(), Box<dyn Error>> {
        let len = self.prefix.lines.len();
        self.prefix.lines.extend(self.hltas.lines.iter().cloned());

        if let Some(Line::FrameBulk(frame_bulk)) = self.prefix.lines.get_mut(len) {
            assert_eq!(
                frame_bulk.console_command, None,
                "the command was erased in Optimizer::new()"
            );
            frame_bulk.console_command = self.erased_console_command.clone();
        }

        let rv = self.prefix.to_writer(writer);
        self.prefix.lines.truncate(len);
        Ok(rv?)
    }

    pub fn current_best(&self) -> HLTAS {
        let mut rv = self.prefix.clone();

        let len = rv.lines.len();
        rv.lines.extend(self.hltas.lines.iter().cloned());

        if let Some(Line::FrameBulk(frame_bulk)) = rv.lines.get_mut(len) {
            assert_eq!(
                frame_bulk.console_command, None,
                "the command was erased in Optimizer::new()"
            );
            frame_bulk.console_command = self.erased_console_command.clone();
        }

        rv
    }

    pub fn simulate_all<T: Trace>(&mut self, tracer: &T) {
        let simulator = Simulator::new(tracer, &self.frames, &self.hltas.lines);
        self.frames.extend(simulator);
    }

    pub fn optimize<'a, T: Trace>(
        &'a mut self,
        tracer: &'a T,
        frames: usize,
        random_frames_to_change: usize,
        change_single_frames: bool,
        change_pitch: bool,
        objective: &'a Objective,
    ) -> Option<impl Iterator<Item = AttemptResult> + 'a> {
        self.simulate_all(tracer);

        if self.frames.len() == 1 {
            return None;
        }

        let mut high = self.frames.len() - 1;
        if frames > 0 {
            high = high.min(frames);
        }

        let between = Uniform::from(0..high);
        let mut rng = rand::thread_rng();

        Some(iter::from_fn(move || {
            let mut hltas = self.hltas.clone();

            // Change several frames.
            let mut stale_frame = self.frames.len() - 1;
            for _ in 0..random_frames_to_change {
                let frame = if change_single_frames {
                    // Pick a random frame and mutate it.
                    let frame = between.sample(&mut rng);
                    mutate_frame(change_pitch, &mut rng, &mut hltas, frame);
                    frame
                } else {
                    mutate_single_frame_bulk(change_pitch, &mut hltas, &mut rng)
                };

                stale_frame = stale_frame.min(frame);
            }

            let mut frames = Vec::from(&self.frames[..stale_frame + 1]);

            // Simulate the result.
            let simulator = Simulator::new(tracer, &frames, &hltas.lines);
            frames.extend(simulator);

            // Check if we got an improvement.
            let result = objective.eval(&frames, &self.frames);
            if result.is_better() {
                self.hltas = hltas;
                self.frames = frames;
            } else {
                self.last_mutation_frames = Some(frames);
            }

            Some(result)
        }))
    }

    fn prepare_hltas_for_sending(&mut self) -> HLTAS {
        let len = self.prefix.lines.len();
        self.prefix.lines.extend(self.hltas.lines.iter().cloned());

        // Replace the TAS optimizer / TAS optim commands with the start sending frames command.
        self.prefix.lines[len]
            .frame_bulk_mut()
            .unwrap()
            .console_command = Some("_bxt_tas_optim_simulation_start_recording_frames".to_owned());

        // Add a toggleconsole command in the end.
        self.prefix.lines.push(Line::FrameBulk(
            FrameBulk::with_frame_time("0.001".to_owned()).tap_mut(|x| {
                x.console_command = Some("_bxt_tas_optim_simulation_done;toggleconsole".to_owned())
            }),
        ));

        let hltas = self.prefix.clone();
        self.prefix.lines.truncate(len);
        hltas
    }

    pub fn maybe_simulate_all_in_remote_client(&mut self) {
        if self.frames.len() > 1 {
            // Already simulated.
            return;
        }

        // Check if we have already been simulating and it has finished.
        remote::receive_simulation_result_from_clients(|_hltas, generation, mut frames| {
            if generation != self.generation {
                return;
            }

            frames.insert(0, mem::take(&mut self.frames).into_iter().next().unwrap());
            self.frames = frames;
        });

        if self.frames.len() > 1 {
            // Already simulated.
            return;
        }

        if !remote::is_any_client_simulating_generation(self.generation) {
            // Try to send the script for simulation.
            remote::maybe_simulate_in_one_client(|| {
                (self.prepare_hltas_for_sending(), self.generation)
            });
        }
    }

    pub fn poll_remote_clients_when_not_optimizing(&mut self) {
        remote::receive_simulation_result_from_clients(|_hltas, generation, mut frames| {
            if generation != self.generation {
                return;
            }

            if self.frames.len() == 1 {
                // Received the initial simulation result, don't miss it.
                frames.insert(0, mem::take(&mut self.frames).into_iter().next().unwrap());
                self.frames = frames;
            }

            // Otherwise, we have received simulation result that we no longer care about.
        });
    }

    // Yes I know this is not the best structured code at the moment...
    #[allow(clippy::too_many_arguments)]
    pub fn optimize_with_remote_clients(
        &mut self,
        frames: usize,
        random_frames_to_change: usize,
        change_single_frames: bool,
        change_pitch: bool,
        objective: &Objective,
        mut on_improvement: impl FnMut(&str),
    ) {
        self.maybe_simulate_all_in_remote_client();

        if self.frames.len() == 1 {
            // Haven't finished the initial simulation yet...
            return;
        }

        remote::receive_simulation_result_from_clients(|mut hltas, generation, mut frames| {
            if generation != self.generation {
                return;
            }

            frames.insert(0, self.frames[0].clone());
            self.last_mutation_frames = Some(frames.clone());

            if let AttemptResult::Better { value } = objective.eval(&frames, &self.frames) {
                self.hltas.lines = hltas
                    .lines
                    .drain(self.prefix.lines.len()..hltas.lines.len() - 1)
                    .collect();

                // Remove the start sending frames command.
                self.hltas.lines[0]
                    .frame_bulk_mut()
                    .unwrap()
                    .console_command = None;

                self.frames = frames;
                on_improvement(&value);
            }
        });

        let mut high = self.frames.len() - 1;
        if frames > 0 {
            high = high.min(frames);
        }

        let between = Uniform::from(0..high);
        let mut rng = rand::thread_rng();

        remote::simulate_in_available_clients(|| {
            let temp = self.hltas.clone();

            // Change several frames.
            for _ in 0..random_frames_to_change {
                if change_single_frames {
                    let frame = between.sample(&mut rng);
                    let frame_bulk = self.hltas.split_single_at_frame(frame).unwrap();
                    mutate_frame_bulk(change_pitch, &mut rng, frame_bulk);
                } else {
                    mutate_single_frame_bulk(change_pitch, &mut self.hltas, &mut rng);
                }
            }

            let hltas = self.prepare_hltas_for_sending();

            self.hltas = temp;

            (hltas, self.generation)
        });
    }

    pub fn minimize<T: Trace>(&mut self, tracer: &T) {
        // Remove unused keys and actions.
        let mut state = self.frames[0].state.clone();
        let mut parameters = self.frames[0].parameters;
        let mut preferred_leave_ground_action_type = LeaveGroundActionType::Jump;

        for line in &mut self.hltas.lines {
            if let Line::FrameBulk(frame_bulk) = line {
                if let Some(action) = frame_bulk.auto_actions.leave_ground_action {
                    preferred_leave_ground_action_type = action.type_;
                }

                parameters.frame_time =
                    (frame_bulk.frame_time.parse::<f32>().unwrap_or(0.) * 1000.).trunc() / 1000.;

                let simulate = |frame_bulk: &FrameBulk| {
                    let mut state_new = state.clone();
                    for _ in 0..frame_bulk.frame_count.get() {
                        state_new = state_new.clone().simulate(tracer, parameters, frame_bulk).0;
                    }
                    state_new
                };

                let mut state_original = simulate(frame_bulk);

                if frame_bulk.action_keys.use_ {
                    frame_bulk.action_keys.use_ = false;
                    let state_new = simulate(frame_bulk);
                    if state_original.player == state_new.player {
                        state_original = state_new;
                    } else {
                        frame_bulk.action_keys.use_ = true;
                    }
                }

                if let Some(action) = frame_bulk.auto_actions.leave_ground_action {
                    frame_bulk.auto_actions.leave_ground_action = Some(LeaveGroundAction {
                        speed: LeaveGroundActionSpeed::Optimal,
                        times: Times::UnlimitedWithinFrameBulk,
                        type_: preferred_leave_ground_action_type,
                    });
                    let state_new = simulate(frame_bulk);
                    if state_original.player == state_new.player {
                        state_original = state_new;
                    } else {
                        frame_bulk.auto_actions.leave_ground_action = Some(action);
                    }
                }

                if let Some(action) = frame_bulk.auto_actions.duck_before_ground {
                    frame_bulk.auto_actions.duck_before_ground = None;
                    let state_new = simulate(frame_bulk);
                    if state_original.player == state_new.player {
                        state_original = state_new;
                    } else {
                        frame_bulk.auto_actions.duck_before_ground = Some(action);
                    }
                }

                state = state_original;
            }

            match line {
                Line::FrameBulk(_) => (),
                Line::Save(_) => (),
                Line::SharedSeed(_) => (),
                Line::Buttons(_) => (),
                Line::LGAGSTMinSpeed(_) => (),
                Line::Reset { non_shared_seed: _ } => (),
                Line::Comment(_) => (),
                Line::VectorialStrafing(_) => (),
                Line::VectorialStrafingConstraints(_) => (),
                Line::Change(_) => (),
                Line::TargetYawOverride(_) => (),
                Line::RenderYawOverride(_) => (),
                Line::PitchOverride(_) => (),
                Line::RenderPitchOverride(_) => (),
            }
        }

        // Join split frame bulks.
        let mut i = 0;
        let lines = &self.hltas.lines;
        let mut new_lines = Vec::new();

        while i < lines.len() {
            match &lines[i] {
                Line::FrameBulk(frame_bulk) => {
                    let mut frame_bulk = frame_bulk.clone();

                    let mut j = i;
                    loop {
                        j += 1;
                        if j == lines.len() {
                            break;
                        }

                        match &lines[j] {
                            Line::FrameBulk(next_frame_bulk) => {
                                let frame_count = frame_bulk.frame_count;
                                frame_bulk.frame_count = next_frame_bulk.frame_count;
                                if &frame_bulk == next_frame_bulk {
                                    frame_bulk.frame_count = NonZeroU32::new(
                                        frame_count.get() + next_frame_bulk.frame_count.get(),
                                    )
                                    .unwrap();
                                } else {
                                    frame_bulk.frame_count = frame_count;
                                    break;
                                }
                            }
                            _ => break,
                        }
                    }

                    new_lines.push(Line::FrameBulk(frame_bulk));
                    i = j;
                }
                line => {
                    new_lines.push(line.clone());
                    i += 1;
                }
            }
        }

        self.hltas.lines = new_lines;
    }
}

fn mutate_frame<R: Rng>(change_pitch: bool, rng: &mut R, hltas: &mut HLTAS, frame: usize) {
    if frame > 0 {
        let l = hltas.line_and_repeat_at_frame(frame).unwrap().0;
        let frame_bulk = hltas.split_at_frame(frame).unwrap();
        if l == 0 {
            // If we split the first frame bulk, empty out the console command (which contains
            // optim init and TAS optimizer commands).
            frame_bulk.console_command = None;
        }
    }

    // Split it into its own frame bulk.
    let frame_bulk = hltas.split_single_at_frame(frame).unwrap();

    mutate_frame_bulk(change_pitch, rng, frame_bulk);
}

fn mutate_frame_bulk<R: Rng>(change_pitch: bool, rng: &mut R, frame_bulk: &mut FrameBulk) {
    let p = rng.gen::<f32>();
    let strafe_type = if p < 0.01 {
        StrafeType::MaxDeccel
    } else if p < 0.1 {
        StrafeType::MaxAngle
    } else {
        StrafeType::MaxAccel
    };
    frame_bulk.auto_actions.movement = Some(AutoMovement::Strafe(StrafeSettings {
        type_: strafe_type,
        dir: if strafe_type == StrafeType::MaxDeccel {
            StrafeDir::Best
        } else if rng.gen::<bool>() {
            StrafeDir::Left
        } else {
            StrafeDir::Right
        },
    }));

    mutate_action_keys(rng, frame_bulk);
    mutate_auto_actions(rng, frame_bulk);

    if change_pitch {
        mutate_pitch(rng, frame_bulk);
    }
}

fn mutate_single_frame_bulk<R: Rng>(change_pitch: bool, hltas: &mut HLTAS, rng: &mut R) -> usize {
    let count = hltas.frame_bulks().count();
    let index = rng.gen_range(0..count);
    let frame_bulk = hltas.frame_bulks_mut().nth(index).unwrap();
    let mut mutated_index = index;

    if let Some(AutoMovement::Strafe(StrafeSettings { type_, dir, .. })) =
        frame_bulk.auto_actions.movement.as_mut()
    {
        // Mutate strafe type.
        *type_ = if let StrafeType::ConstYawspeed(_) = *type_ {
            // Constant yawspeed will not be selected unless specified in bulk.
            *type_
        } else if let StrafeType::MaxAccelYawOffset { .. } = *type_ {
            // Max accel yaw offset will not be selected unless specified in bulk.
            *type_
        } else {
            let p = rng.gen::<f32>();
            if p < 0.01 {
                StrafeType::MaxDeccel
            } else if p < 0.1 {
                StrafeType::MaxAngle
            } else {
                StrafeType::MaxAccel
            }
        };

        if let StrafeType::ConstYawspeed(ref mut yawspeed) = *type_ {
            // Constant yawspeed should always pair with side strafe
            // so there is no need to reassign dir.
            // Yawspeed is not allowed to be negative.
            *yawspeed = (*yawspeed + rng.gen_range(-1f32..1f32)).abs();
        };

        if let StrafeType::MaxAccelYawOffset {
            ref mut start,
            ref mut target,
            ref mut accel,
        } = *type_
        {
            *start += rng.gen_range(-1f32..1f32);
            *target += rng.gen_range(-1f32..1f32);
            *accel += rng.gen_range(-1f32..1f32);
        }

        // Mutate strafe direction.
        match dir {
            StrafeDir::Yaw(yaw) => {
                *yaw += if rng.gen::<f32>() < 0.05 {
                    rng.gen_range(-180f32..180f32)
                } else {
                    rng.gen_range(-1f32..1f32)
                };
            }
            StrafeDir::LeftRight(count) | StrafeDir::RightLeft(count) => {
                *count = NonZeroU32::new(
                    (count.get().conv::<i64>() + rng.gen_range(-10..10))
                        .clamp(1, u32::MAX.into())
                        .try_conv()
                        .unwrap(),
                )
                .unwrap();

                if rng.gen::<f32>() < 0.05 {
                    // Invert the strafe dir.
                    let count = *count;
                    *dir = if matches!(*dir, StrafeDir::LeftRight(_)) {
                        StrafeDir::RightLeft(count)
                    } else {
                        StrafeDir::LeftRight(count)
                    };
                }
            }
            StrafeDir::Left | StrafeDir::Right => {
                if rng.gen::<f32>() < 0.05 {
                    // Invert the strafe dir.
                    *dir = if *dir == StrafeDir::Left {
                        StrafeDir::Right
                    } else {
                        StrafeDir::Left
                    };
                }
            }
            _ => (),
        }
    }

    mutate_action_keys(rng, frame_bulk);
    mutate_auto_actions(rng, frame_bulk);

    if change_pitch {
        mutate_pitch(rng, frame_bulk);
    }

    // Mutate frame count.
    let frame_time = frame_bulk.frame_time.clone();
    let same_frame_time_bulks: Vec<usize> = hltas
        .frame_bulks()
        .enumerate()
        .filter(|&(i, bulk)| i != index && bulk.frame_time == frame_time)
        .map(|(i, _)| i)
        .collect();

    if !same_frame_time_bulks.is_empty() {
        // Frame bulks closer to the current one get weighted more.
        let other_index = *same_frame_time_bulks
            .choose_weighted(rng, |i| i.abs_diff(index))
            .unwrap();

        let other_frame_bulk = hltas.frame_bulks_mut().nth(other_index).unwrap();

        // Can't go below frame count of 1 on the next frame bulk.
        let max_frame_count_difference = (other_frame_bulk.frame_count.get() - 1)
            .conv::<i64>()
            .min(10);

        // Can't go above frame count of u32::MAX on the next frame bulk.
        let min_frame_count_difference =
            (other_frame_bulk.frame_count.get().conv::<i64>() - u32::MAX.conv::<i64>()).max(-10);

        let frame_count_difference_range = min_frame_count_difference..max_frame_count_difference;

        let frame_bulk = hltas.frame_bulks_mut().nth(index).unwrap();
        let difference = frame_bulk.frame_count.pipe_ref_mut(|count| {
            let orig_count = count.get();

            *count = NonZeroU32::new(
                (count.get().conv::<i64>() + rng.gen_range(frame_count_difference_range))
                    .clamp(1, u32::MAX.into())
                    .try_conv()
                    .unwrap(),
            )
            .unwrap();

            orig_count.conv::<i64>() - count.get().conv::<i64>()
        });

        if difference != 0 {
            mutated_index = mutated_index.min(other_index);

            let other_frame_bulk = hltas.frame_bulks_mut().nth(other_index).unwrap();
            other_frame_bulk.frame_count.pipe_ref_mut(|count| {
                *count =
                    NonZeroU32::new((count.get().conv::<i64>() + difference).try_conv().unwrap())
                        .unwrap()
            });
        }
    }

    let frame = hltas
        .frame_bulks_mut()
        .take(mutated_index)
        .map(|frame_bulk| frame_bulk.frame_count.get().try_conv::<usize>().unwrap())
        .sum();

    frame
}

fn mutate_action_keys<R: Rng>(rng: &mut R, frame_bulk: &mut FrameBulk) {
    if rng.gen::<f32>() < 0.05 {
        frame_bulk.action_keys.use_ = !frame_bulk.action_keys.use_;
    }
}

fn mutate_auto_actions<R: Rng>(rng: &mut R, frame_bulk: &mut FrameBulk) {
    if rng.gen::<f32>() < 0.05 {
        frame_bulk.auto_actions.duck_before_ground =
            if frame_bulk.auto_actions.duck_before_ground.is_some() {
                None
            } else {
                Some(DuckBeforeGround {
                    times: Times::UnlimitedWithinFrameBulk,
                })
            };
    }

    if rng.gen::<f32>() < 0.1 {
        let p = rng.gen::<f32>();
        frame_bulk.auto_actions.leave_ground_action = if p < 1. / 3. {
            None
        } else if p < 2. / 3. {
            Some(LeaveGroundAction {
                speed: if rng.gen::<bool>() {
                    LeaveGroundActionSpeed::Any
                } else {
                    LeaveGroundActionSpeed::Optimal
                },
                times: Times::UnlimitedWithinFrameBulk,
                type_: LeaveGroundActionType::DuckTap {
                    zero_ms: frame_bulk.frame_time == "0.001",
                },
            })
        } else {
            Some(LeaveGroundAction {
                speed: if rng.gen::<bool>() {
                    LeaveGroundActionSpeed::Any
                } else {
                    LeaveGroundActionSpeed::Optimal
                },
                times: Times::UnlimitedWithinFrameBulk,
                type_: LeaveGroundActionType::Jump,
            })
        };
    }
}

fn mutate_pitch<R: Rng>(rng: &mut R, frame_bulk: &mut FrameBulk) {
    if let Some(pitch) = frame_bulk.pitch.as_mut() {
        if rng.gen::<f32>() < 0.05 {
            *pitch = rng.gen_range(-89f32..89f32)
        } else {
            *pitch += rng.gen_range(-1f32..1f32)
        };
    }
}

// proptest: after simulating, self.frames.len() = frame count + 1
