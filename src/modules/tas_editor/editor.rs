use std::error::Error;
use std::io::Write;
use std::num::NonZeroU32;
use std::result::Result;
use std::str::FromStr;

use bxt_strafe::{Parameters, State, Trace};
use genevo::ga::genetic_algorithm;
use genevo::genetic::FitnessFunction;
use genevo::operator::{GeneticOperator, MutationOp};
use genevo::population::Population;
use genevo::prelude::*;
use genevo::recombination::discrete::SinglePointCrossBreeder;
use genevo::reinsertion::elitist::ElitistReinserter;
use genevo::selection::proportionate::RouletteWheelSelector;
use genevo::selection::truncation::MaximizeSelector;
use genevo::types::fmt::Display;
use glam::Vec3Swizzles;
use hltas::types::*;
use hltas::HLTAS;
use parking_lot::Mutex;
use rand::distributions::Uniform;
use rand::prelude::Distribution;
use rand::Rng;

use crate::modules::triangle_drawing::triangle_api::{Primitive, RenderMode};
use crate::modules::triangle_drawing::TriangleApi;

/// A movement frame.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Parameters used for simulating this frame.
    pub parameters: Parameters,

    /// Final state after this frame.
    pub state: State,
}

pub struct Editor {
    /// The first part of the script that we're not editing.
    prefix: HLTAS,

    /// The script being edited.
    hltas: HLTAS,

    /// Movement frames, starting from the initial frame.
    frames: Vec<Frame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variable {
    PosX,
    PosY,
    PosZ,
    VelX,
    VelY,
    VelZ,
    Speed,
}

impl Variable {
    fn get(self, state: &State) -> f32 {
        match self {
            Variable::PosX => state.player().pos.x,
            Variable::PosY => state.player().pos.y,
            Variable::PosZ => state.player().pos.z,
            Variable::VelY => state.player().vel.x,
            Variable::VelZ => state.player().vel.y,
            Variable::VelX => state.player().vel.z,
            Variable::Speed => state.player().vel.xy().length(),
        }
    }
}

impl FromStr for Variable {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pos.x" => Ok(Self::PosX),
            "pos.y" => Ok(Self::PosY),
            "pos.z" => Ok(Self::PosZ),
            "vel.x" => Ok(Self::VelX),
            "vel.y" => Ok(Self::VelY),
            "vel.z" => Ok(Self::VelZ),
            "speed" => Ok(Self::Speed),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Maximize,
    Minimize,
}

impl Direction {
    fn is_better(self, new_value: f32, old_value: f32) -> bool {
        match self {
            Direction::Maximize => new_value > old_value,
            Direction::Minimize => new_value < old_value,
        }
    }
}

impl FromStr for Direction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "minimize" => Ok(Self::Minimize),
            "maximize" => Ok(Self::Maximize),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationGoal {
    pub variable: Variable,
    pub direction: Direction,
}

impl OptimizationGoal {
    fn is_better(self, new_state: &State, old_state: &State) -> bool {
        self.direction
            .is_better(self.variable.get(new_state), self.variable.get(old_state))
    }

    fn fitness(self, state: &State) -> f32 {
        let value = self.variable.get(state);

        match self.direction {
            Direction::Maximize => value,
            Direction::Minimize => -value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintType {
    GreaterThan,
    LessThan,
}

impl FromStr for ConstraintType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ">" => Ok(Self::GreaterThan),
            "<" => Ok(Self::LessThan),
            _ => Err(()),
        }
    }
}

impl ConstraintType {
    fn is_valid(self, value: f32, constraint: f32) -> bool {
        match self {
            ConstraintType::GreaterThan => value > constraint,
            ConstraintType::LessThan => value < constraint,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Constraint {
    pub variable: Variable,
    pub type_: ConstraintType,
    pub constraint: f32,
}

impl Constraint {
    fn is_valid(self, state: &State) -> bool {
        self.type_
            .is_valid(self.variable.get(state), self.constraint)
    }
}

trait HLTASExt {
    /// Returns the line index and the repeat for the given `frame`.
    ///
    /// Returns [`None`] if `frame` is bigger than the number of frames in the [`HLTAS`].
    fn line_and_repeat_at_frame(&self, frame: usize) -> Option<(usize, u32)>;

    /// Splits the [`HLTAS`] at `frame` if needed and returns a reference to the frame bulk that
    /// starts at `frame`.
    ///
    /// Returns [`None`] if `frame` is bigger than the number of frames in the [`HLTAS`].
    fn split_at_frame(&mut self, frame: usize) -> Option<&mut FrameBulk>;

    /// Splits the [`HLTAS`] at `frame` if needed and returns a reference to the frame bulk that
    /// starts at `frame` and lasts a single repeat.
    ///
    /// Returns [`None`] if `frame` is bigger than the number of frames in the [`HLTAS`].
    fn split_single_at_frame(&mut self, frame: usize) -> Option<&mut FrameBulk>;
}

impl HLTASExt for HLTAS {
    fn line_and_repeat_at_frame(&self, frame: usize) -> Option<(usize, u32)> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(l, line)| {
                if let Line::FrameBulk(frame_bulk) = line {
                    Some((l, frame_bulk))
                } else {
                    None
                }
            })
            .flat_map(|(l, frame_bulk)| (0..frame_bulk.frame_count.get()).map(move |r| (l, r)))
            .nth(frame)
    }

    fn split_at_frame(&mut self, frame: usize) -> Option<&mut FrameBulk> {
        let (l, r) = self.line_and_repeat_at_frame(frame)?;

        let mut frame_bulk = if let Line::FrameBulk(frame_bulk) = &mut self.lines[l] {
            frame_bulk
        } else {
            unreachable!()
        };

        let index = if r == 0 {
            // The frame bulk already starts here.
            l
        } else {
            let mut new_frame_bulk = frame_bulk.clone();
            new_frame_bulk.frame_count = NonZeroU32::new(frame_bulk.frame_count.get() - r).unwrap();
            frame_bulk.frame_count = NonZeroU32::new(r).unwrap();

            self.lines.insert(l + 1, Line::FrameBulk(new_frame_bulk));

            l + 1
        };

        if let Line::FrameBulk(frame_bulk) = &mut self.lines[index] {
            Some(frame_bulk)
        } else {
            unreachable!()
        }
    }

    fn split_single_at_frame(&mut self, frame: usize) -> Option<&mut FrameBulk> {
        self.split_at_frame(frame + 1);
        self.split_at_frame(frame)
    }
}

impl Editor {
    pub fn new(mut hltas: HLTAS, first_frame: usize, initial_frame: Frame) -> Self {
        let (l, _r) = hltas.line_and_repeat_at_frame(first_frame).unwrap();

        let mut prefix = hltas.clone();
        prefix.lines.truncate(l);

        hltas.lines = hltas.lines[l..].to_vec();

        Self {
            prefix,
            hltas,
            frames: vec![initial_frame],
        }
    }

    pub fn draw(&self, tri: &TriangleApi) {
        tri.render_mode(RenderMode::TransColor);
        tri.color(1., 1., 1., 1.);

        tri.begin(Primitive::Lines);

        for pair in self.frames.windows(2) {
            let (prev, next) = (&pair[0], &pair[1]);

            tri.vertex(prev.state.player().pos);
            tri.vertex(next.state.player().pos);
        }

        tri.end();
    }

    /// Marks all frames starting from `frame` as stale, causing their re-simulation.
    fn mark_as_stale(&mut self, frame: usize) {
        self.frames.truncate(frame + 1);
    }

    pub fn save<W: Write>(&mut self, writer: W) -> Result<(), Box<dyn Error>> {
        let len = self.prefix.lines.len();
        self.prefix.lines.extend(self.hltas.lines.iter().cloned());
        let rv = self.prefix.to_writer(writer);
        self.prefix.lines.truncate(len);
        Ok(rv?)
    }

    pub fn simulate_all<T: Trace>(&mut self, tracer: &T) {
        let mut frame = 0;
        for line in &self.hltas.lines {
            if let Line::FrameBulk(frame_bulk) = line {
                for repeat in 0..frame_bulk.frame_count.get() {
                    frame += 1;

                    if frame < self.frames.len() {
                        continue;
                    }

                    let Frame {
                        state,
                        mut parameters,
                    } = self
                        .frames
                        .last()
                        .expect("there should always be at least one state (initial)")
                        .clone();

                    // Only set frame-time on the first repeat since subsequent repeats inherit it.
                    if repeat == 0 {
                        // TODO: move the truncation to bxt-strafe and add frame-time remainder
                        // handling to it?
                        parameters.frame_time =
                            (frame_bulk.frame_time.parse::<f32>().unwrap_or(0.) * 1000.).trunc()
                                / 1000.;
                    }

                    let (state, _input) = state.clone().simulate(tracer, parameters, frame_bulk);

                    self.frames.push(Frame { state, parameters });
                }
            }

            if frame < self.frames.len() {
                continue;
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
            }
        }
    }

    pub fn optimize<T: Trace>(
        &mut self,
        tracer: &T,
        frames: usize,
        random_frames_to_change: usize,
        goal: OptimizationGoal,
        constraint: Option<Constraint>,
    ) {
        self.simulate_all(tracer);

        let mut best_hltas = self.hltas.clone();
        let mut best_state = self.frames.last().unwrap().state.clone();

        let mut high = self.frames.len() - 1;
        if frames > 0 {
            high = high.min(frames);
        }

        let between = Uniform::from(0..high);
        let mut rng = rand::thread_rng();
        // Do several attempts per optimize() call.
        for _ in 0..20 {
            // Change several frames.
            for _ in 0..random_frames_to_change {
                // Pick a random frame and mutate it.
                let frame = between.sample(&mut rng);
                self.mutate_frame(&mut rng, frame);
            }

            let valid_frames = self.frames.len() - 1;
            // Simulate the result.
            self.simulate_all(tracer);

            // Check if we got an improvement.
            let state = &self.frames.last().unwrap().state;
            if constraint.map(|c| c.is_valid(state)).unwrap_or(true)
                && goal.is_better(state, &best_state)
            {
                best_hltas = self.hltas.clone();
                best_state = state.clone();
                eprintln!("found new best value: {}", goal.variable.get(&best_state));
            } else {
                // Restore the script before the changes.
                self.hltas = best_hltas.clone();
                self.mark_as_stale(valid_frames);
            }
        }
    }

    pub fn optimize_genetic<T: Trace + Clone>(
        &mut self,
        tracer: &T,
        frames: usize,
        goal: OptimizationGoal,
        constraint: Option<Constraint>,
    ) {
        self.simulate_all(tracer);

        let mut high = self.frames.len() - 1;
        if frames > 0 {
            high = high.min(frames);
        }

        let between = Uniform::from(0..high);
        let mut rng = rand::thread_rng();

        #[derive(Debug, Clone, Copy, PartialEq)]
        struct SimpleFrame {
            auto_actions: AutoActions,
            action_keys: ActionKeys,
        }

        impl From<&FrameBulk> for SimpleFrame {
            fn from(frame_bulk: &FrameBulk) -> Self {
                SimpleFrame {
                    auto_actions: frame_bulk.auto_actions,
                    action_keys: frame_bulk.action_keys,
                }
            }
        }

        type ScriptGenotype = Vec<SimpleFrame>;

        #[derive(Clone)]
        struct FitnessCalc<'a, T: Trace + Clone> {
            original_frame_bulks: Vec<FrameBulk>,
            tracer: &'a Mutex<&'a T>,
            parameters: Parameters,
            initial_state: State,
            goal: OptimizationGoal,
            constraint: Option<Constraint>,
        }

        impl<'a, T: Trace + Clone> std::fmt::Debug for FitnessCalc<'a, T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("FitnessCalc")
                    .field("original_frame_bulks", &self.original_frame_bulks)
                    .field("parameters", &self.parameters)
                    .field("initial_state", &self.initial_state)
                    .field("goal", &self.goal)
                    .field("constraint", &self.constraint)
                    .finish()
            }
        }

        unsafe impl<'a, T: Trace + Clone> Sync for FitnessCalc<'a, T> {}

        impl<'a, T: Trace + Clone> FitnessFunction<ScriptGenotype, i64> for FitnessCalc<'a, T> {
            fn fitness_of(&self, genotype: &ScriptGenotype) -> i64 {
                let mut parameters = self.parameters;
                let mut state = self.initial_state.clone();

                let tracer = self.tracer.lock();

                for (simple_frame, frame_bulk) in genotype.iter().zip(&self.original_frame_bulks) {
                    let mut frame_bulk = frame_bulk.clone();

                    frame_bulk.auto_actions = simple_frame.auto_actions;
                    frame_bulk.action_keys = simple_frame.action_keys;

                    parameters.frame_time =
                        (frame_bulk.frame_time.parse::<f32>().unwrap_or(0.) * 1000.).trunc()
                            / 1000.;

                    state = state.simulate(*tracer, parameters, &frame_bulk).0;
                }

                if !self.constraint.map(|c| c.is_valid(&state)).unwrap_or(true) {
                    return self.lowest_possible_fitness();
                }

                (self.goal.fitness(&state) as f64 * 100_000.) as i64 + 16_384_000_000
            }

            fn average(&self, values: &[i64]) -> i64 {
                (values.iter().sum::<i64>() as f32 / values.len() as f32 + 0.5).floor() as i64
            }

            fn highest_possible_fitness(&self) -> i64 {
                32_768_000_000
            }

            fn lowest_possible_fitness(&self) -> i64 {
                0
            }
        }

        #[derive(Debug, Clone)]
        struct Mutator {
            between: Uniform<usize>,
        }

        impl GeneticOperator for Mutator {
            fn name() -> String {
                "Script-Genotype-Mutation".to_owned()
            }
        }

        impl MutationOp<ScriptGenotype> for Mutator {
            fn mutate<R>(&self, mut genome: ScriptGenotype, rng: &mut R) -> ScriptGenotype
            where
                R: Rng + Sized,
            {
                for _ in 0..6 {
                    let index = self.between.sample(rng);
                    let frame = &mut genome[index];

                    let mut frame_bulk = FrameBulk {
                        auto_actions: frame.auto_actions,
                        action_keys: frame.action_keys,
                        ..FrameBulk::with_frame_time(String::new())
                    };
                    mutate_frame_bulk(rng, &mut frame_bulk);

                    *frame = (&frame_bulk).into();
                }

                genome
            }
        }

        for frame in 0..self.frames.len() {
            self.hltas.split_at_frame(frame);
        }

        let frame_bulks: Vec<_> = self
            .hltas
            .lines
            .iter()
            .filter_map(|line| {
                if let Line::FrameBulk(frame_bulk) = line {
                    Some(frame_bulk)
                } else {
                    None
                }
            })
            .cloned()
            .collect();

        let mut individuals = vec![frame_bulks.iter().map(Into::into).collect()];
        for _ in 0..199 {
            let mut frame_bulks = frame_bulks.clone();
            for _ in 0..6 {
                let frame = between.sample(&mut rng);
                mutate_frame_bulk(&mut rng, &mut frame_bulks[frame]);
            }
            individuals.push(frame_bulks.into_iter().map(|f| (&f).into()).collect())
        }

        let fitness_function = FitnessCalc {
            original_frame_bulks: frame_bulks,
            tracer: &Mutex::new(tracer),
            parameters: self.frames[0].parameters,
            initial_state: self.frames[0].state.clone(),
            goal,
            constraint,
        };

        println!(
            "Initial fitness: {}",
            fitness_function.fitness_of(&individuals[0])
        );

        let initial_population: Population<ScriptGenotype> =
            Population::with_individuals(individuals);

        let mut sim = genevo::prelude::simulate(
            genetic_algorithm()
                .with_evaluation(fitness_function.clone())
                // .with_selection(MaximizeSelector::new(0.7, 3))
                .with_selection(RouletteWheelSelector::new(0.7, 3))
                .with_crossover(SinglePointCrossBreeder::new())
                .with_mutation(Mutator { between })
                .with_reinsertion(ElitistReinserter::new(fitness_function, false, 0.7))
                .with_initial_population(initial_population)
                .build(),
        )
        .until(GenerationLimit::new(20))
        .build();

        loop {
            match sim.step() {
                Ok(SimResult::Intermediate(step)) => {
                    let evaluated_population = step.result.evaluated_population;
                    let best_solution = step.result.best_solution;
                    println!(
                        "Step: generation: {}, average_fitness: {}, \
                     best fitness: {}, duration: {}, processing_time: {}",
                        step.iteration,
                        evaluated_population.average_fitness(),
                        best_solution.solution.fitness,
                        step.duration.fmt(),
                        step.processing_time.fmt()
                    );
                }
                Ok(SimResult::Final(step, processing_time, duration, stop_reason)) => {
                    let best_solution = step.result.best_solution;
                    println!("{}", stop_reason);
                    println!(
                        "Final result after {}: generation: {}, \
                         best solution with fitness {} found in generation {}, processing_time: {}",
                        duration.fmt(),
                        step.iteration,
                        best_solution.solution.fitness,
                        best_solution.generation,
                        processing_time.fmt()
                    );

                    for (frame_bulk, simple_frame) in self
                        .hltas
                        .lines
                        .iter_mut()
                        .filter_map(|line| {
                            if let Line::FrameBulk(frame_bulk) = line {
                                Some(frame_bulk)
                            } else {
                                None
                            }
                        })
                        .zip(best_solution.solution.genome.into_iter())
                    {
                        frame_bulk.auto_actions = simple_frame.auto_actions;
                        frame_bulk.action_keys = simple_frame.action_keys;
                    }

                    break;
                }
                Err(err) => {
                    error!("{}", err);
                    break;
                }
            }
        }

        self.mark_as_stale(0);
    }

    fn mutate_frame<R: Rng>(&mut self, rng: &mut R, frame: usize) {
        // Split it into its own frame bulk.
        let frame_bulk = self.hltas.split_single_at_frame(frame).unwrap();

        mutate_frame_bulk(rng, frame_bulk);

        self.mark_as_stale(frame);
    }
}

fn mutate_frame_bulk<R: Rng>(rng: &mut R, frame_bulk: &mut FrameBulk) {
    frame_bulk.auto_actions.movement = Some(AutoMovement::Strafe(StrafeSettings {
        type_: if rng.gen::<f32>() < 0.1 {
            StrafeType::MaxAngle
        } else {
            StrafeType::MaxAccel
        },
        dir: if rng.gen::<bool>() {
            StrafeDir::Left
        } else {
            StrafeDir::Right
        },
    }));

    if rng.gen::<f32>() < 0.05 {
        frame_bulk.action_keys.use_ = !frame_bulk.action_keys.use_;
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
                type_: LeaveGroundActionType::DuckTap { zero_ms: false },
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

// proptest: after simulating, self.frames.len() = frame count + 1
