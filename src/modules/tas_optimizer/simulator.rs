use bxt_ipc_types::Frame;
/// Frame simulator.
use bxt_strafe::Trace;
use hltas::types::Line;

/// Frame simulator.
///
/// This is an [`Iterator`] that outputs simulated frames.
pub struct Simulator<'a, T> {
    /// The tracer.
    tracer: &'a T,
    /// Lines left to simulate.
    lines: &'a [Line],
    /// Current repeat.
    repeat: u32,
    /// Frame to simulate from.
    last_frame: Frame,
}

impl<'a, T> Simulator<'a, T> {
    /// Creates a new [`Simulator`] that will return frames that have not yet been simulated.
    ///
    /// This function takes the entire slice of [`Line`]s that need to be simulated and a slice of
    /// already simulated frames (starting with the initial frame). The [`Simulator`] will then
    /// simulate and return any frames that have no yet been simulated.
    ///
    /// # Panics
    ///
    /// Panics if `existing_frames` is empty (it must contain at least the initial frame) or if
    /// `existing_frames` has more frames than there are in the script (which is a logic error).
    pub fn new(tracer: &'a T, existing_frames: &[Frame], lines: &'a [Line]) -> Self {
        assert!(
            !existing_frames.is_empty(),
            "there should be at least one existing frame (initial)"
        );

        // Find the lines that we need to simulate.
        //
        // If we need to simulate starting from a fresh frame bulk, then the lines should include
        // all preceding non-frame-bulk lines to update the state from the last frame.
        let mut frame = 0;
        for (l, line) in lines.iter().enumerate() {
            if frame == existing_frames.len() - 1 {
                return Self {
                    tracer,
                    lines: &lines[l..],
                    repeat: 0,
                    last_frame: existing_frames.last().unwrap().clone(),
                };
            }

            if let Line::FrameBulk(frame_bulk) = line {
                for repeat in 0..frame_bulk.frame_count.get() {
                    if frame == existing_frames.len() - 1 {
                        return Self {
                            tracer,
                            lines: &lines[l..],
                            repeat,
                            last_frame: existing_frames.last().unwrap().clone(),
                        };
                    }

                    frame += 1;
                }
            }
        }

        // All frames have already been simulated.
        assert_eq!(
            frame,
            existing_frames.len() - 1,
            "there were more existing frames than there are frames in the script"
        );

        Self {
            tracer,
            lines: &[],
            repeat: 0,
            last_frame: existing_frames.last().unwrap().clone(),
        }
    }
}

impl<'a, T: Trace> Iterator for Simulator<'a, T> {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        for line in self.lines {
            match line {
                Line::FrameBulk(frame_bulk) => {
                    assert!(self.repeat < frame_bulk.frame_count.get());

                    let Frame { parameters, state } = &mut self.last_frame;

                    // Only set frame-time on the first repeat since subsequent repeats inherit it.
                    if self.repeat == 0 {
                        // TODO: move the truncation to bxt-strafe and add frame-time remainder
                        // handling to it?
                        parameters.frame_time =
                            (frame_bulk.frame_time.parse::<f32>().unwrap_or(0.) * 1000.).trunc()
                                / 1000.;
                    }

                    let (new_state, _input) =
                        state.clone().simulate(self.tracer, *parameters, frame_bulk);

                    *state = new_state;

                    self.repeat += 1;
                    if self.repeat == frame_bulk.frame_count.get() {
                        self.lines = &self.lines[1..];
                        self.repeat = 0;
                    }

                    return Some(self.last_frame.clone());
                }
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

            // Advance to the next line for non-frame-bulks.
            self.lines = &self.lines[1..];
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use bxt_strafe::{DummyTracer, Parameters, Player, State};
    use glam::Vec3;
    use hltas::types::FrameBulk;

    use super::*;

    fn default_parameters() -> Parameters {
        Parameters {
            frame_time: 0.010000001,
            max_velocity: 2000.,
            max_speed: 320.,
            stop_speed: 100.,
            friction: 4.,
            edge_friction: 2.,
            ent_friction: 1.,
            accelerate: 10.,
            air_accelerate: 10.,
            gravity: 800.,
            ent_gravity: 1.,
            step_size: 18.,
            bounce: 1.,
            bhop_cap: false,
            bhop_cap_multiplier: 0.65,
            bhop_cap_max_speed_scale: 1.7,
            use_slow_down: true,
            has_stamina: false,
            duck_animation_slow_down: false,
        }
    }

    fn default_player() -> Player {
        Player {
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            base_vel: Vec3::ZERO,
            ducking: false,
            in_duck_animation: false,
            duck_time: 0,
            stamina_time: 0.0,
            health: 100.,
            armor: 0.,
        }
    }

    fn default_state() -> State {
        State::new(&DummyTracer, default_parameters(), default_player())
    }

    fn default_frame() -> Frame {
        Frame {
            parameters: default_parameters(),
            state: default_state(),
        }
    }

    #[test]
    fn simulator_empty_lines() {
        let mut simulator = Simulator::new(&DummyTracer, &[default_frame()], &[]);
        assert_eq!(simulator.next(), None);
    }

    #[test]
    #[should_panic]
    fn simulator_no_initial_frame() {
        Simulator::new(&DummyTracer, &[], &[]);
    }

    #[test]
    #[should_panic]
    fn simulator_too_many_frames() {
        Simulator::new(&DummyTracer, &[default_frame(), default_frame()], &[]);
    }

    #[test]
    fn simulator_one_frame() {
        let lines = [Line::FrameBulk(FrameBulk::with_frame_time(
            "0.001".to_string(),
        ))];
        let simulator = Simulator::new(&DummyTracer, &[default_frame()], &lines);
        assert_eq!(simulator.count(), 1);
    }

    #[test]
    fn simulator_two_frames() {
        let lines = [
            Line::FrameBulk(FrameBulk::with_frame_time("0.001".to_string())),
            Line::FrameBulk(FrameBulk::with_frame_time("0.001".to_string())),
        ];
        let simulator = Simulator::new(&DummyTracer, &[default_frame()], &lines);
        assert_eq!(simulator.count(), 2);
    }

    #[test]
    fn simulator_two_repeats() {
        let lines = [Line::FrameBulk(FrameBulk {
            frame_count: NonZeroU32::new(2).unwrap(),
            ..FrameBulk::with_frame_time("0.001".to_string())
        })];
        let simulator = Simulator::new(&DummyTracer, &[default_frame()], &lines);
        assert_eq!(simulator.count(), 2);
    }

    #[test]
    fn simulator_starts_with_non_frame_bulk_lines() {
        let lines = [
            Line::FrameBulk(FrameBulk::with_frame_time("0.001".to_string())),
            Line::LGAGSTMinSpeed(0.),
            Line::FrameBulk(FrameBulk::with_frame_time("0.001".to_string())),
        ];
        let simulator = Simulator::new(&DummyTracer, &[default_frame(), default_frame()], &lines);
        assert_eq!(simulator.lines, &lines[1..]);
    }

    #[test]
    fn simulator_middle_of_frame_bulk() {
        let lines = [
            Line::FrameBulk(FrameBulk::with_frame_time("0.001".to_string())),
            Line::LGAGSTMinSpeed(0.),
            Line::FrameBulk(FrameBulk {
                frame_count: NonZeroU32::new(2).unwrap(),
                ..FrameBulk::with_frame_time("0.001".to_string())
            }),
        ];
        let simulator = Simulator::new(
            &DummyTracer,
            &[default_frame(), default_frame(), default_frame()],
            &lines,
        );
        assert_eq!(simulator.lines, &lines[2..]);
        assert_eq!(simulator.repeat, 1);
    }

    #[test]
    fn simulator_advances_on_non_frame_bulks() {
        let lines = [
            Line::FrameBulk(FrameBulk::with_frame_time("0.001".to_string())),
            Line::LGAGSTMinSpeed(0.),
            Line::LGAGSTMinSpeed(0.),
            Line::LGAGSTMinSpeed(0.),
            Line::FrameBulk(FrameBulk::with_frame_time("0.001".to_string())),
        ];
        let simulator = Simulator::new(&DummyTracer, &[default_frame()], &lines);
        assert_eq!(simulator.count(), 2);
    }
}
