//! Optimization objective.

use std::rc::Rc;
use std::str::FromStr;

use bxt_strafe::State;
use glam::Vec3Swizzles;
use mlua::{Lua, LuaSerdeExt};

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

/// The optimization goal.
#[derive(Debug)]
pub enum OptimizationGoal {
    Console {
        variable: Variable,
        direction: Direction,
    },
    Lua(Rc<Lua>),
}

impl OptimizationGoal {
    /// Returns `true` if `new_frames` is better than `old_frames` according to this optimization
    /// goal.
    pub fn is_better(&self, new_frames: &[Frame], old_frames: &[Frame]) -> bool {
        match self {
            OptimizationGoal::Console {
                variable,
                direction,
            } => direction.is_better(
                variable.get(&new_frames.last().unwrap().state),
                variable.get(&old_frames.last().unwrap().state),
            ),
            OptimizationGoal::Lua(lua) => {
                let is_better: mlua::Function = lua.globals().get("is_better").unwrap();
                let args = if lua.globals().get("should_pass_all_frames").unwrap() {
                    (
                        lua.to_value(
                            &new_frames
                                .iter()
                                .map(|f| f.state.player())
                                .collect::<Vec<_>>(),
                        )
                        .unwrap(),
                        lua.to_value(
                            &old_frames
                                .iter()
                                .map(|f| f.state.player())
                                .collect::<Vec<_>>(),
                        )
                        .unwrap(),
                    )
                } else {
                    (
                        lua.to_value(&new_frames.last().unwrap().state.player())
                            .unwrap(),
                        lua.to_value(&old_frames.last().unwrap().state.player())
                            .unwrap(),
                    )
                };

                match is_better.call(args) {
                    Ok(x) => x,
                    Err(err) => {
                        eprintln!("Call to is_better () failed: {err}");
                        false
                    }
                }
            }
        }
    }

    /// Returns a string representation of the value of the optimization goal for `frames`.
    pub fn to_string(&self, frames: &[Frame]) -> String {
        match self {
            OptimizationGoal::Console { variable, .. } => {
                variable.get(&frames.last().unwrap().state).to_string()
            }
            OptimizationGoal::Lua(lua) => {
                let to_string: mlua::Function = lua.globals().get("to_string").unwrap();
                let args = if lua.globals().get("should_pass_all_frames").unwrap() {
                    lua.to_value(&frames.iter().map(|f| f.state.player()).collect::<Vec<_>>())
                        .unwrap()
                } else {
                    lua.to_value(&frames.last().unwrap().state.player())
                        .unwrap()
                };
                match to_string.call(args) {
                    Ok(x) => x,
                    Err(err) => {
                        eprintln!("Call to to_string () failed: {err}");
                        "<error>".to_owned()
                    }
                }
            }
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

/// The optimization constraint.
#[derive(Debug)]
pub enum Constraint {
    Console {
        variable: Variable,
        type_: ConstraintType,
        constraint: f32,
    },
    Lua(Rc<Lua>),
}

impl Constraint {
    /// Returns `true` if `frames` satisfies the constraint.
    pub fn is_valid(&self, frames: &[Frame]) -> bool {
        match self {
            &Constraint::Console {
                variable,
                type_,
                constraint,
            } => type_.is_valid(variable.get(&frames.last().unwrap().state), constraint),
            Constraint::Lua(lua) => {
                let is_valid: mlua::Function = lua.globals().get("is_valid").unwrap();
                let args = if lua.globals().get("should_pass_all_frames").unwrap() {
                    lua.to_value(&frames.iter().map(|f| f.state.player()).collect::<Vec<_>>())
                        .unwrap()
                } else {
                    lua.to_value(&frames.last().unwrap().state.player())
                        .unwrap()
                };
                match is_valid.call(args) {
                    Ok(x) => x,
                    Err(err) => {
                        eprintln!("Call to is_valid () failed: {err}");
                        false
                    }
                }
            }
        }
    }
}
