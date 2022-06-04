//! Optimization objective.

use std::str::FromStr;

use bxt_strafe::State;
use glam::Vec3Swizzles;
use rhai::serde::to_dynamic;

use super::editor::Frame;

/// The variable to optimize.
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

/// The optimization direction.
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

/// Type of a constraint on a [`Variable`].
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

/// Constraint on a [`Variable`].
#[derive(Debug)]
pub struct Constraint {
    pub variable: Variable,
    pub type_: ConstraintType,
    pub constraint: f32,
}

impl Constraint {
    /// Returns `true` if `frames` satisfies the constraint.
    pub fn is_valid(&self, frames: &[Frame]) -> bool {
        let value = self.variable.get(&frames.last().unwrap().state);
        self.type_.is_valid(value, self.constraint)
    }
}

/// Result of an optimization attempt.
#[derive(Debug)]
pub enum AttemptResult {
    /// The attempt failed the constraint.
    Invalid,
    /// The attempt was worse than the best so far.
    Worse,
    /// The attempt was an improvement.
    Better {
        /// String representation of the optimized value.
        value: String,
    },
}

impl AttemptResult {
    /// Returns `true` if the attempt result is [`Better`].
    ///
    /// [`Better`]: AttemptResult::Better
    #[must_use]
    pub fn is_better(&self) -> bool {
        matches!(self, Self::Better { .. })
    }
}

/// The optimization objective.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Objective {
    /// Objective set with console variables.
    Console {
        variable: Variable,
        direction: Direction,
        constraint: Option<Constraint>,
    },
    /// Objective defined as a Rhai script.
    Rhai {
        engine: rhai::Engine,
        ast: rhai::AST,
    },
}

impl Objective {
    /// Evaluates the objective for `new_frames` compared to `old_frames`.
    pub fn eval(&self, new_frames: &[Frame], old_frames: &[Frame]) -> AttemptResult {
        match self {
            Objective::Console {
                variable,
                direction,
                constraint,
            } => {
                if let Some(constraint) = constraint {
                    if !constraint.is_valid(new_frames) {
                        return AttemptResult::Invalid;
                    }
                }

                let new_value = variable.get(&new_frames.last().unwrap().state);
                let old_value = variable.get(&old_frames.last().unwrap().state);

                if !direction.is_better(new_value, old_value) {
                    return AttemptResult::Worse;
                }

                AttemptResult::Better {
                    value: new_value.to_string(),
                }
            }
            Objective::Rhai { engine, ast } => {
                let mut scope = rhai::Scope::new();

                if let Err(err) = engine.run_ast_with_scope(&mut scope, ast) {
                    error!("Error running Rhai script: {err:?}");
                }

                let should_pass_all_frames =
                    scope.get_value("should_pass_all_frames").unwrap_or(false);

                let convert = |frames: &[Frame]| {
                    if should_pass_all_frames {
                        frames
                            .iter()
                            .map(|f| to_dynamic(f.state.player()).unwrap())
                            .collect::<rhai::Dynamic>()
                    } else {
                        to_dynamic(frames.last().unwrap().state.player()).unwrap()
                    }
                };

                let new_frames = convert(new_frames);

                match engine
                    .call_fn_raw(
                        &mut scope,
                        ast,
                        false,
                        false,
                        "is_valid",
                        None,
                        [new_frames.clone()],
                    )
                    .as_ref()
                    .map(rhai::Dynamic::as_bool)
                {
                    Ok(Ok(true)) => (),
                    Ok(Ok(false)) => return AttemptResult::Invalid,
                    Ok(Err(err)) => {
                        error!("is_valid() returned an unexpected type: {err}");
                        return AttemptResult::Invalid;
                    }
                    Err(err) => {
                        error!("Call to is_valid() failed: {err:?}");
                        return AttemptResult::Invalid;
                    }
                }

                let old_frames = convert(old_frames);

                match engine
                    .call_fn_raw(
                        &mut scope,
                        ast,
                        false,
                        false,
                        "is_better",
                        None,
                        [new_frames.clone(), old_frames],
                    )
                    .as_ref()
                    .map(rhai::Dynamic::as_bool)
                {
                    Ok(Ok(true)) => (),
                    Ok(Ok(false)) => return AttemptResult::Worse,
                    Ok(Err(err)) => {
                        error!("is_better() returned an unexpected type: {err}");
                        return AttemptResult::Worse;
                    }
                    Err(err) => {
                        error!("Call to is_better() failed: {err:?}");
                        return AttemptResult::Worse;
                    }
                }

                let value = match engine
                    .call_fn_raw(
                        &mut scope,
                        ast,
                        false,
                        false,
                        "to_string",
                        None,
                        [new_frames],
                    )
                    .map(rhai::Dynamic::into_string)
                {
                    Ok(Ok(value)) => value,
                    Ok(Err(err)) => {
                        error!("to_string() returned an unexpected type: {err}");
                        "<error>".to_owned()
                    }
                    Err(err) => {
                        error!("Call to to_string() failed: {err:?}");
                        "<error>".to_owned()
                    }
                };

                AttemptResult::Better { value }
            }
        }
    }
}
