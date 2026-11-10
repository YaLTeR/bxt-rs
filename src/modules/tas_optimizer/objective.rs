//! Optimization objective.

use std::str::FromStr;

use bxt_ipc_types::Frame;
use bxt_strafe::{Player, State};
use glam::{Vec3, Vec3Swizzles};
use rhai::serde::to_dynamic;

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
            Variable::PosX => state.player.pos.x,
            Variable::PosY => state.player.pos.y,
            Variable::PosZ => state.player.pos.z,
            Variable::VelX => state.player.vel.x,
            Variable::VelY => state.player.vel.y,
            Variable::VelZ => state.player.vel.z,
            Variable::Speed => state.player.vel.xy().length(),
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

/// Box the player should reach as early as possible.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    mins: Vec3,
    maxs: Vec3,
}

impl FromStr for Region {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let values: Vec<f32> = s
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|_| ())?;
        let [x1, y1, z1, x2, y2, z2] = values[..] else {
            return Err(());
        };

        let a = Vec3::new(x1, y1, z1);
        let b = Vec3::new(x2, y2, z2);
        Ok(Self {
            mins: a.min(b),
            maxs: a.max(b),
        })
    }
}

impl Region {
    /// Returns `true` if the player's bounding box touches the region, like a trigger.
    fn is_touched_by(&self, player: &Player) -> bool {
        let half_size = if player.ducking {
            Vec3::new(16., 16., 18.)
        } else {
            Vec3::new(16., 16., 36.)
        };

        (player.pos - half_size).cmple(self.maxs).all()
            && (player.pos + half_size).cmpge(self.mins).all()
    }

    /// Returns the first frame where the player touches the region, and the speed on that frame.
    fn first_touch(&self, frames: &[Frame]) -> Option<(usize, f32)> {
        frames
            .iter()
            .position(|frame| self.is_touched_by(&frame.state.player))
            .map(|i| (i, Variable::Speed.get(&frames[i].state)))
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
    /// Reach a region as early as possible, then as fast as possible.
    Reach {
        region: Region,
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
            Objective::Reach { region, constraint } => {
                if let Some(constraint) = constraint {
                    if !constraint.is_valid(new_frames) {
                        return AttemptResult::Invalid;
                    }
                }

                let Some((new_frame, new_speed)) = region.first_touch(new_frames) else {
                    return AttemptResult::Invalid;
                };

                if let Some((old_frame, old_speed)) = region.first_touch(old_frames) {
                    if new_frame > old_frame || (new_frame == old_frame && new_speed <= old_speed) {
                        return AttemptResult::Worse;
                    }
                }

                AttemptResult::Better {
                    value: format!("frame {new_frame}, speed {new_speed}"),
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
                            .map(|f| to_dynamic(f.state.player).unwrap())
                            .collect::<rhai::Dynamic>()
                    } else {
                        to_dynamic(frames.last().unwrap().state.player).unwrap()
                    }
                };

                let new_frames = convert(new_frames);

                let options = rhai::CallFnOptions::new()
                    .eval_ast(false)
                    .rewind_scope(false);
                match engine
                    .call_fn_with_options(
                        options,
                        &mut scope,
                        ast,
                        "is_valid",
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

                let options = rhai::CallFnOptions::new()
                    .eval_ast(false)
                    .rewind_scope(false);
                match engine
                    .call_fn_with_options(
                        options,
                        &mut scope,
                        ast,
                        "is_better",
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

                let options = rhai::CallFnOptions::new()
                    .eval_ast(false)
                    .rewind_scope(false);
                let value = match engine
                    .call_fn_with_options(options, &mut scope, ast, "to_string", [new_frames])
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

#[cfg(test)]
mod tests {
    use super::*;

    fn player_at(pos: Vec3, ducking: bool) -> Player {
        Player {
            pos,
            ducking,
            ..Default::default()
        }
    }

    #[test]
    fn region_parses_corners_in_any_order() {
        let region: Region = "10 20 30 -10 -20 -30".parse().unwrap();
        assert_eq!(region.mins, Vec3::new(-10., -20., -30.));
        assert_eq!(region.maxs, Vec3::new(10., 20., 30.));

        assert!("1 2 3 4 5".parse::<Region>().is_err());
        assert!("1 2 3 4 5 x".parse::<Region>().is_err());
    }

    #[test]
    fn region_is_touched_by_player_bounding_box() {
        let region: Region = "100 -10 -10 200 10 10".parse().unwrap();

        // The standing hull is 32 units wide, so its edge touches the box at x = 84.
        assert!(region.is_touched_by(&player_at(Vec3::new(84., 0., 0.), false)));
        assert!(!region.is_touched_by(&player_at(Vec3::new(83., 0., 0.), false)));

        // The standing hull reaches 36 units up, the ducked one only 18.
        assert!(region.is_touched_by(&player_at(Vec3::new(150., 0., -40.), false)));
        assert!(!region.is_touched_by(&player_at(Vec3::new(150., 0., -40.), true)));
    }
}
