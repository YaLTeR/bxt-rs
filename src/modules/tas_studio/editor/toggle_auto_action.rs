use std::num::NonZeroU32;

use hltas::types::{
    AutoMovement, DuckBeforeCollision, DuckBeforeGround, DuckWhenJump, FrameBulk, JumpBug,
    LeaveGroundAction, LeaveGroundActionSpeed, LeaveGroundActionType, Line, StrafeDir,
    StrafeSettings, StrafeType, Times,
};
use hltas::HLTAS;

use super::utils::FrameBulkExt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToggleAutoActionTarget {
    Strafe { dir: StrafeDir, type_: StrafeType },
    LeaveGroundAtOptimalSpeed,
    AutoJump,
    DuckTap,
    JumpBug,
    DuckBeforeCollision,
    DuckBeforeCollisionIncludingCeilings,
    DuckBeforeGround,
    DuckWhenJump,
}

impl ToggleAutoActionTarget {
    pub(crate) fn apply(self, script: &HLTAS, bulk_idx: usize) -> FrameBulk {
        let mut bulk = script.frame_bulks().nth(bulk_idx).unwrap().clone();
        let bulk_yaw = bulk.yaw().copied();
        let aa = &mut bulk.auto_actions;

        match self {
            ToggleAutoActionTarget::Strafe { mut dir, type_ } => {
                if matches!(dir, StrafeDir::Yaw(_) | StrafeDir::Line { .. }) {
                    // Figure out the yaw we'll use by walking frame bulks backwards, starting from
                    // the current one, until we find a frame bulk with a yaw
                    // angle set.
                    let line_idx = script
                        .lines
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| matches!(line, Line::FrameBulk(_)))
                        .nth(bulk_idx)
                        .unwrap()
                        .0;

                    let yaw = script
                        .lines
                        .iter()
                        .take(line_idx + 1)
                        .rev()
                        .filter_map(Line::frame_bulk)
                        .filter_map(FrameBulkExt::yaw)
                        .copied()
                        .next()
                        .unwrap_or(0.);

                    dir = match dir {
                        StrafeDir::Yaw(_) => StrafeDir::Yaw(yaw),
                        StrafeDir::Line { .. } => StrafeDir::Line { yaw },
                        _ => unreachable!(),
                    };
                } else if matches!(dir, StrafeDir::LeftRight(_) | StrafeDir::RightLeft(_)) {
                    // Figure out the count we'll use by walking frame bulks backwards, starting
                    // from the current one, until we find a frame bulk with a count.
                    let line_idx = script
                        .lines
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| matches!(line, Line::FrameBulk(_)))
                        .nth(bulk_idx)
                        .unwrap()
                        .0;

                    let count = script
                        .lines
                        .iter()
                        .take(line_idx + 1)
                        .rev()
                        .filter_map(Line::frame_bulk)
                        .filter_map(|bulk| match bulk.auto_actions.movement {
                            Some(AutoMovement::Strafe(StrafeSettings {
                                dir: StrafeDir::LeftRight(count) | StrafeDir::RightLeft(count),
                                ..
                            })) => Some(count),
                            _ => None,
                        })
                        .next()
                        .unwrap_or_else(|| NonZeroU32::new(30).unwrap());

                    dir = match dir {
                        StrafeDir::LeftRight(_) => StrafeDir::LeftRight(count),
                        StrafeDir::RightLeft(_) => StrafeDir::RightLeft(count),
                        _ => unreachable!(),
                    };
                }

                aa.movement = match aa.movement {
                    Some(AutoMovement::Strafe(settings))
                        if settings.dir != dir || settings.type_ != type_ =>
                    {
                        Some(AutoMovement::Strafe(StrafeSettings { type_, dir }))
                    }
                    Some(AutoMovement::SetYaw(_)) | None => {
                        Some(AutoMovement::Strafe(StrafeSettings { type_, dir }))
                    }
                    _ => {
                        // If this bulk has a yaw, don't lose it. This way you can toggle yaw
                        // strafing off and on without losing the angle.
                        bulk_yaw.map(AutoMovement::SetYaw)
                    }
                }
            }
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed => {
                if let Some(action) = aa.leave_ground_action.as_mut() {
                    action.speed = match action.speed {
                        LeaveGroundActionSpeed::Any => LeaveGroundActionSpeed::Optimal,
                        LeaveGroundActionSpeed::Optimal
                        | LeaveGroundActionSpeed::OptimalWithFullMaxspeed => {
                            LeaveGroundActionSpeed::Any
                        }
                    };
                }
            }
            ToggleAutoActionTarget::AutoJump => {
                aa.leave_ground_action = match aa.leave_ground_action {
                    Some(action) => match action.type_ {
                        LeaveGroundActionType::DuckTap { .. } => Some(LeaveGroundAction {
                            type_: LeaveGroundActionType::Jump,
                            ..action
                        }),
                        LeaveGroundActionType::Jump => None,
                    },
                    None => Some(LeaveGroundAction {
                        speed: LeaveGroundActionSpeed::Any,
                        times: Times::UnlimitedWithinFrameBulk,
                        type_: LeaveGroundActionType::Jump,
                    }),
                };
            }
            ToggleAutoActionTarget::DuckTap => {
                aa.leave_ground_action = match aa.leave_ground_action {
                    Some(action) => match action.type_ {
                        LeaveGroundActionType::Jump { .. } => Some(LeaveGroundAction {
                            type_: LeaveGroundActionType::DuckTap {
                                zero_ms: script.properties.frametime_0ms.is_some(),
                            },
                            ..action
                        }),
                        LeaveGroundActionType::DuckTap { .. } => None,
                    },
                    None => Some(LeaveGroundAction {
                        speed: LeaveGroundActionSpeed::Any,
                        times: Times::UnlimitedWithinFrameBulk,
                        type_: LeaveGroundActionType::DuckTap {
                            zero_ms: script.properties.frametime_0ms.is_some(),
                        },
                    }),
                };
            }
            ToggleAutoActionTarget::JumpBug => {
                aa.jump_bug = if aa.jump_bug.is_some() {
                    None
                } else {
                    Some(JumpBug {
                        times: Times::UnlimitedWithinFrameBulk,
                    })
                };
            }
            ToggleAutoActionTarget::DuckBeforeCollision => {
                aa.duck_before_collision = if aa.duck_before_collision.is_some() {
                    None
                } else {
                    Some(DuckBeforeCollision {
                        times: Times::UnlimitedWithinFrameBulk,
                        including_ceilings: false,
                    })
                };
            }
            ToggleAutoActionTarget::DuckBeforeCollisionIncludingCeilings => {
                aa.duck_before_collision = match aa.duck_before_collision {
                    Some(mut action) => {
                        action.including_ceilings = !action.including_ceilings;
                        Some(action)
                    }
                    None => Some(DuckBeforeCollision {
                        times: Times::UnlimitedWithinFrameBulk,
                        including_ceilings: true,
                    }),
                };
            }
            ToggleAutoActionTarget::DuckBeforeGround => {
                aa.duck_before_ground = if aa.duck_before_ground.is_some() {
                    None
                } else {
                    Some(DuckBeforeGround {
                        times: Times::UnlimitedWithinFrameBulk,
                    })
                };
            }
            ToggleAutoActionTarget::DuckWhenJump => {
                aa.duck_when_jump = if aa.duck_when_jump.is_some() {
                    None
                } else {
                    Some(DuckWhenJump {
                        times: Times::UnlimitedWithinFrameBulk,
                    })
                };
            }
        }

        bulk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn check_toggle_auto_action(input: &str, target: ToggleAutoActionTarget, output: &str) {
        let input = HLTAS::from_str(&format!("version 1\nframes\n{input}")).unwrap();
        let output = hltas::read::frame_bulk(output).unwrap().1;

        let modified = target.apply(&input, input.frame_bulks().count() - 1);
        assert_eq!(modified, output, "apply produced wrong result");
    }

    #[track_caller]
    fn check_toggle_auto_action_with_props(
        props: &str,
        input: &str,
        target: ToggleAutoActionTarget,
        output: &str,
    ) {
        let input = HLTAS::from_str(&format!("version 1\n{props}\nframes\n{input}")).unwrap();
        let output = hltas::read::frame_bulk(output).unwrap().1;

        let modified = target.apply(&input, input.frame_bulks().count() - 1);
        assert_eq!(modified, output, "apply produced wrong result");
    }

    #[test]
    fn toggle_strafe() {
        // Helper function to get the arguments on one line in the tests below.
        #[track_caller]
        fn check_strafe(input: &str, (dir, type_): (StrafeDir, StrafeType), output: &str) {
            check_toggle_auto_action(input, ToggleAutoActionTarget::Strafe { dir, type_ }, output);
        }

        let ignored = 0.;

        // Toggle on and off, no yaw.
        check_strafe(
            "----------|------|------|0.004|-|-|1",
            (StrafeDir::Best, StrafeType::MaxDeccel),
            "s22-------|------|------|0.004|-|-|1",
        );
        check_strafe(
            "s22-------|------|------|0.004|-|-|1",
            (StrafeDir::Best, StrafeType::MaxDeccel),
            "----------|------|------|0.004|-|-|1",
        );
        check_strafe(
            "----------|------|------|0.004|10|-|1",
            (StrafeDir::Best, StrafeType::MaxDeccel),
            "s22-------|------|------|0.004|-|-|1",
        );
        // Toggle with yaw preserves yaw, falls back to zero.
        check_strafe(
            "----------|------|------|0.004|10|-|1",
            (StrafeDir::Yaw(ignored), StrafeType::MaxAccel),
            "s03-------|------|------|0.004|10|-|1",
        );
        check_strafe(
            "s03-------|------|------|0.004|10|-|1",
            (StrafeDir::Yaw(ignored), StrafeType::MaxAccel),
            "----------|------|------|0.004|10|-|1",
        );
        check_strafe(
            "----------|------|------|0.004|-|-|1",
            (StrafeDir::Yaw(ignored), StrafeType::MaxAccel),
            "s03-------|------|------|0.004|0|-|1",
        );
        // Toggle between types preserves yaw.
        check_strafe(
            "s03-------|------|------|0.004|10|-|1",
            (StrafeDir::Yaw(ignored), StrafeType::MaxAngle),
            "s13-------|------|------|0.004|10|-|1",
        );
        // Missing yaw falls back to previous bulks.
        check_strafe(
            "----------|------|------|0.004|10|-|1\n----------|------|------|0.004|-|-|1",
            (StrafeDir::Yaw(ignored), StrafeType::MaxAccel),
            "s03-------|------|------|0.004|10|-|1",
        );

        let ignored = NonZeroU32::new(1).unwrap();

        // Toggle from wiggle erases yaw.
        check_strafe(
            "s06-------|------|------|0.004|10|-|1",
            (StrafeDir::LeftRight(ignored), StrafeType::MaxAccel),
            "----------|------|------|0.004|-|-|1",
        );
        // Toggle to wiggle falls back to previous bulks, then falls back to the count of 30.
        check_strafe(
            "s06-------|------|------|0.004|10|-|1\n----------|------|------|0.004|-|-|1",
            (StrafeDir::LeftRight(ignored), StrafeType::MaxAccel),
            "s06-------|------|------|0.004|10|-|1",
        );
        check_strafe(
            "----------|------|------|0.004|-|-|1",
            (StrafeDir::LeftRight(ignored), StrafeType::MaxAccel),
            "s06-------|------|------|0.004|30|-|1",
        );
        // Toggle between wiggles preserves count.
        check_strafe(
            "s07-------|------|------|0.004|10|-|1",
            (StrafeDir::LeftRight(ignored), StrafeType::MaxAccel),
            "s06-------|------|------|0.004|10|-|1",
        );
    }

    #[test]
    fn toggle_leave_ground_at_optimal_speed() {
        check_toggle_auto_action(
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "----------|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "----j-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "---lj-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-----d----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "---l-d----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-----D----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "---l-D----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---lj-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "----j-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---l-d----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "-----d----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---l-D----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "-----D----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---Lj-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "----j-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---L-d----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "-----d----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---L-D----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::LeaveGroundAtOptimalSpeed,
            "-----D----|------|------|0.004|10|-|1",
        );
    }

    #[test]
    fn toggle_auto_jump() {
        check_toggle_auto_action(
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "----j-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "----j-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "----------|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---lj-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "----------|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-----d----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "----j-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---l-d----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "---lj-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---L-d----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "---Lj-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-----D----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "----j-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---l-D----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "---lj-----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---L-D----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::AutoJump,
            "---Lj-----|------|------|0.004|10|-|1",
        );
    }

    #[test]
    fn toggle_duck_tap() {
        check_toggle_auto_action(
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "-----d----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action_with_props(
            "frametime0ms 0.0000000001",
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "-----D----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-----d----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "----------|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-----D----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "----------|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---l-d----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "----------|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---l-D----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "----------|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "----j-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "-----d----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action_with_props(
            "frametime0ms 0.0000000001",
            "----j-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "-----D----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---lj-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "---l-d----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action_with_props(
            "frametime0ms 0.0000000001",
            "---lj-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "---l-D----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---Lj-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "---L-d----|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action_with_props(
            "frametime0ms 0.0000000001",
            "---Lj-----|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckTap,
            "---L-D----|------|------|0.004|10|-|1",
        );
    }

    #[test]
    fn toggle_jump_bug() {
        check_toggle_auto_action(
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::JumpBug,
            "------b---|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "------b---|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::JumpBug,
            "----------|------|------|0.004|10|-|1",
        );
    }

    #[test]
    fn toggle_duck_before_collision() {
        check_toggle_auto_action(
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckBeforeCollision,
            "-------c--|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-------c--|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckBeforeCollision,
            "----------|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-------C--|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckBeforeCollision,
            "----------|------|------|0.004|10|-|1",
        );
    }

    #[test]
    fn toggle_duck_before_collision_including_ceilings() {
        check_toggle_auto_action(
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckBeforeCollisionIncludingCeilings,
            "-------C--|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-------c--|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckBeforeCollisionIncludingCeilings,
            "-------C--|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "-------C--|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckBeforeCollisionIncludingCeilings,
            "-------c--|------|------|0.004|10|-|1",
        );
    }

    #[test]
    fn toggle_duck_before_ground() {
        check_toggle_auto_action(
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckBeforeGround,
            "--------g-|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "--------g-|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckBeforeGround,
            "----------|------|------|0.004|10|-|1",
        );
    }

    #[test]
    fn toggle_duck_when_jump() {
        check_toggle_auto_action(
            "----------|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckWhenJump,
            "---------w|------|------|0.004|10|-|1",
        );
        check_toggle_auto_action(
            "---------w|------|------|0.004|10|-|1",
            ToggleAutoActionTarget::DuckWhenJump,
            "----------|------|------|0.004|10|-|1",
        );
    }
}
