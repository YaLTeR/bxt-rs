use std::f32::consts::{FRAC_PI_2, PI};

use arrayvec::ArrayVec;
use bxt_vct::Vct;
use glam::{Vec2, Vec3, Vec3Swizzles};
use hltas::types::*;
use tap::{Pipe, Tap};

use super::*;

/// One step in the simulation chain.
pub trait Step {
    /// Simulates from this step to the end of the frame and returns the final `State` and `Input`.
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        state: State,
        input: Input,
    ) -> (State, Input);
}

fn fly_move<T: Trace>(tracer: &T, parameters: Parameters, state: &mut State) {
    fn clip_velocity(mut velocity: Vec3, normal: Vec3, overbounce: f32) -> Vec3 {
        let backoff = velocity.dot(normal) * overbounce;
        velocity -= normal * backoff;

        for i in 0..3 {
            if velocity[i] > -0.1 && velocity[i] < 0.1 {
                velocity[i] = 0.
            };
        }

        velocity
    }

    let player = &mut state.player;

    let original_vel = player.vel;
    let mut saved_vel = player.vel;
    let mut time_left = parameters.frame_time;
    let mut total_fraction = 0.;
    let mut planes: ArrayVec<Vec3, 5> = ArrayVec::new();

    state.move_traces.clear();

    for _ in 0..4 {
        if player.vel == Vec3::ZERO {
            break;
        }

        let end = player.pos + time_left * player.vel;
        let tr = tracer.trace(player.pos, end, player.hull());
        state.move_traces.push(tr);

        total_fraction += tr.fraction;

        if tr.all_solid {
            player.vel = Vec3::ZERO;
            break;
        }

        if tr.fraction > 0. {
            player.pos = tr.end_pos;
            saved_vel = player.vel;
            planes.clear();
        }

        if tr.fraction == 1. {
            break;
        }

        time_left -= time_left * tr.fraction;

        if planes.is_full() {
            player.vel = Vec3::ZERO;
            break;
        }

        planes.push(tr.plane_normal);

        if state.place != Place::Ground || parameters.ent_friction != 1. {
            for &plane in &planes {
                let overbounce = if plane.z > 0.7 {
                    1.
                } else {
                    1. + parameters.bounce * (1. - parameters.ent_friction)
                };

                saved_vel = clip_velocity(saved_vel, plane, overbounce);
            }

            player.vel = saved_vel;
        } else {
            let mut i = 0;
            loop {
                if i == planes.len() {
                    break;
                }

                player.vel = clip_velocity(saved_vel, planes[i], 1.);

                if planes
                    .iter()
                    .enumerate()
                    .all(|(j, &plane)| j == i || player.vel.dot(plane) >= 0.)
                {
                    // Moving along all planes so we're good.
                    break;
                }

                i += 1;
            }

            if i == planes.len() {
                if planes.len() != 2 {
                    player.vel = Vec3::ZERO;
                    break;
                }

                let dir = planes[0].cross(planes[1]);
                player.vel = dir * dir.dot(player.vel);
            }

            if player.vel.dot(original_vel) <= 0. {
                player.vel = Vec3::ZERO;
                break;
            }
        }
    }

    if total_fraction == 0. {
        player.vel = Vec3::ZERO;
    }
}

fn clamp_velocity(velocity: Vec3, max: f32) -> Vec3 {
    velocity.clamp(-Vec3::splat(max), Vec3::splat(max))
}

pub struct Move;

impl Step for Move {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        _frame_bulk: &FrameBulk,
        mut state: State,
        input: Input,
    ) -> (State, Input) {
        state.player.vel = clamp_velocity(state.player.vel, parameters.max_velocity);

        // AddCorrectGravity()
        let ent_gravity = parameters
            .ent_gravity
            .pipe(|x| if x == 0. { 1. } else { x });
        state.player.vel.z -= ent_gravity * parameters.gravity * 0.5 * parameters.frame_time;
        state.player.vel.z += state.player.base_vel.z * parameters.frame_time;
        state.player.base_vel.z = 0.;
        state.player.vel = clamp_velocity(state.player.vel, parameters.max_velocity);

        // Move()
        if state.place == Place::Ground {
            state.player.vel.z = 0.;
        }

        // Accelerate()
        let (sy, cy) = input.yaw.sin_cos();
        let forward = Vec2::new(cy, sy);
        let right = Vec2::new(sy, -cy);
        let accel_dir = (forward * input.forward + right * input.side).normalize_or_zero();

        let wish_speed_capped = if state.place == Place::Ground {
            state.wish_speed
        } else {
            30.
        };
        let tmp = wish_speed_capped - state.player.vel.xy().dot(accel_dir);
        if tmp > 0. {
            let accel = if state.place == Place::Ground {
                parameters.accelerate
            } else {
                parameters.air_accelerate
            };

            let accel_speed =
                accel * state.wish_speed * parameters.ent_friction * parameters.frame_time;
            state.player.vel += Vec3::from((accel_dir * tmp.min(accel_speed), 0.));
        }

        state.player.vel += state.player.base_vel;
        match state.place {
            Place::Ground => {
                // WalkMove()
                if state.player.vel.length_squared() < 1. {
                    state.player.vel = Vec3::ZERO;
                } else {
                    // Compute player position when trying to walk up a step.
                    let mut up = state.clone();

                    let tr = tracer.trace(
                        up.player.pos,
                        up.player.pos + Vec3::new(0., 0., parameters.step_size),
                        up.player.hull(),
                    );
                    if !tr.start_solid && !tr.all_solid {
                        up.player.pos = tr.end_pos;
                    }
                    fly_move(tracer, parameters, &mut up);

                    let tr = tracer.trace(
                        up.player.pos,
                        up.player.pos - Vec3::new(0., 0., parameters.step_size),
                        up.player.hull(),
                    );
                    if !tr.start_solid && !tr.all_solid {
                        up.player.pos = tr.end_pos;
                    }

                    // Compute player position when not trying to walk up a step.
                    let mut down = state.clone();
                    fly_move(tracer, parameters, &mut down);

                    // Take whichever went the furthest.
                    let up_dist = state.player.pos.xy().distance_squared(up.player.pos.xy());
                    let down_dist = state.player.pos.xy().distance_squared(down.player.pos.xy());
                    if tr.plane_normal.z < 0.7 || down_dist > up_dist {
                        state = down;
                    } else {
                        state = up;
                        state.player.vel.z = down.player.vel.z;
                    }
                }
            }
            Place::Air => {
                // AirMove()
                fly_move(tracer, parameters, &mut state);
            }
            Place::Water => (),
        }

        state.update_place(tracer);
        state.player.vel -= state.player.base_vel;
        state.player.vel = clamp_velocity(state.player.vel, parameters.max_velocity);

        match state.place {
            Place::Air => {
                // FixupGravityVelocity()
                state.player.vel.z -=
                    ent_gravity * parameters.gravity * 0.5 * parameters.frame_time;
                state.player.vel = clamp_velocity(state.player.vel, parameters.max_velocity);
            }
            Place::Ground => {
                state.player.vel.z = 0.;
            }
            Place::Water => (),
        }

        state.prev_frame_input = input;

        (state, input)
    }
}

fn max_accel_theta(parameters: Parameters, state: &State) -> f32 {
    let accel = if state.place == Place::Ground {
        parameters.accelerate
    } else {
        parameters.air_accelerate
    };

    let accel_speed = accel * state.wish_speed * parameters.ent_friction * parameters.frame_time;
    if accel_speed <= 0. {
        return PI;
    }

    if state.player.vel.xy() == Vec2::ZERO {
        return 0.;
    }

    let wish_speed_capped = if state.place == Place::Ground {
        state.wish_speed
    } else {
        30.
    };

    let tmp = wish_speed_capped - accel_speed;
    if tmp <= 0. {
        return FRAC_PI_2;
    }

    let speed = state.player.vel.xy().length();
    if tmp < speed {
        return (tmp / speed).acos();
    }

    0.
}

fn max_angle_theta(parameters: Parameters, state: &State) -> f32 {
    let accel = if state.place == Place::Ground {
        parameters.accelerate
    } else {
        parameters.air_accelerate
    };

    let accel_speed = accel * state.wish_speed * parameters.ent_friction * parameters.frame_time;
    let speed = state.player.vel.xy().length();

    if accel_speed >= speed {
        PI
    } else {
        (-accel_speed / speed).acos()
    }
}

fn max_accel_into_yaw_theta(parameters: Parameters, state: &State, yaw: f32) -> f32 {
    let vel_yaw = state.player.vel.y.atan2(state.player.vel.x);
    let theta = max_accel_theta(parameters, state);

    // This is not the exact maximum but it works well enough in practice.
    if theta == 0. || theta == PI {
        normalize_rad(yaw - vel_yaw + theta)
    } else {
        theta.copysign(normalize_rad(yaw - vel_yaw))
    }
}

fn max_angle_into_yaw_theta(parameters: Parameters, state: &State, yaw: f32) -> f32 {
    let vel_yaw = state.player.vel.y.atan2(state.player.vel.x);
    let theta = max_angle_theta(parameters, state);
    theta.copysign(normalize_rad(yaw - vel_yaw))
}

pub struct Strafe<S>(pub S);

impl<S: Step> Step for Strafe<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        mut state: State,
        mut input: Input,
    ) -> (State, Input) {
        if state.place != Place::Water {
            if let Some(AutoMovement::Strafe(StrafeSettings { type_, dir })) =
                frame_bulk.auto_actions.movement
            {
                let theta = match type_ {
                    StrafeType::MaxAccel | StrafeType::MaxAccelYawOffset { .. } => match dir {
                        StrafeDir::Left => max_accel_theta(parameters, &state),
                        StrafeDir::Right => -max_accel_theta(parameters, &state),
                        StrafeDir::Yaw(yaw) => {
                            max_accel_into_yaw_theta(parameters, &state, yaw.to_radians())
                        }
                        StrafeDir::LeftRight(count) | StrafeDir::RightLeft(count) => {
                            let count = count.get().min(u32::MAX / 2);

                            if state.strafe_cycle_frame_count >= count * 2 {
                                state.strafe_cycle_frame_count = 0;
                            }

                            let turn_other_way = (state.strafe_cycle_frame_count / count) > 0;
                            state.strafe_cycle_frame_count += 1;

                            let mut angle = max_accel_theta(parameters, &state);
                            if matches!(dir, StrafeDir::RightLeft(_)) {
                                angle = -angle;
                            }
                            if turn_other_way {
                                angle = -angle;
                            }

                            angle
                        }
                        _ => 0.,
                    },
                    StrafeType::MaxAngle => match dir {
                        StrafeDir::Left => max_angle_theta(parameters, &state),
                        StrafeDir::Right => -max_angle_theta(parameters, &state),
                        StrafeDir::Yaw(yaw) => {
                            max_angle_into_yaw_theta(parameters, &state, yaw.to_radians())
                        }
                        StrafeDir::LeftRight(count) | StrafeDir::RightLeft(count) => {
                            let count = count.get().min(u32::MAX / 2);

                            if state.strafe_cycle_frame_count >= count * 2 {
                                state.strafe_cycle_frame_count = 0;
                            }

                            let turn_other_way = (state.strafe_cycle_frame_count / count) > 0;
                            state.strafe_cycle_frame_count += 1;

                            let mut angle = max_angle_theta(parameters, &state);
                            if matches!(dir, StrafeDir::RightLeft(_)) {
                                angle = -angle;
                            }
                            if turn_other_way {
                                angle = -angle;
                            }

                            angle
                        }
                        _ => 0.,
                    },
                    StrafeType::MaxDeccel => PI,
                    _ => 0.,
                };

                let vel_yaw = state.player.vel.y.atan2(state.player.vel.x);

                assert!(
                    parameters.max_speed <= Vct::MAX_SPEED_CAP,
                    "max_speed {} is larger than the maximum allowed value {}",
                    parameters.max_speed,
                    Vct::MAX_SPEED_CAP
                );

                let (camera_yaw, entry) = if let StrafeType::ConstYawspeed(yawspeed) = type_ {
                    let right = matches!(dir, StrafeDir::Right);
                    let yaw_delta = (yawspeed * parameters.frame_time).to_radians();

                    let accel_angle = match state.place {
                        Place::Ground => PI / 4.,
                        _ => PI / 2.,
                    };

                    let (accel_angle, camera_yaw) = if right {
                        (-accel_angle, input.yaw - yaw_delta)
                    } else {
                        (accel_angle, input.yaw + yaw_delta)
                    };

                    let camera_yaw = angle_mod_rad(camera_yaw);
                    let entry = Vct::get().find_best(accel_angle);

                    (camera_yaw, entry)
                } else {
                    // TODO: target_yaw velocity_lock

                    let camera_yaw = angle_mod_rad(vel_yaw);
                    let entry = Vct::get().find_best((vel_yaw + theta) - camera_yaw);

                    (camera_yaw, entry)
                };

                let camera_yaw = if matches!(type_, StrafeType::MaxAccelYawOffset { .. }) {
                    // theta < 0. = is right
                    // If is right then we decreases yaw by offset.
                    // Therefore, positive offset in framebulk mean going more on that side.
                    let offset = state.max_accel_yaw_offset_value.to_radians();
                    let offset = if theta < 0. { -offset } else { offset };

                    camera_yaw + angle_mod_rad(offset)
                } else {
                    camera_yaw
                };

                input.yaw = camera_yaw;
                input.forward = entry.forward as f32;
                input.side = entry.side as f32;
            }
        }

        self.0
            .simulate(tracer, parameters, frame_bulk, state, input)
    }
}

pub struct Friction<S>(pub S);

impl<S: Step> Step for Friction<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        mut state: State,
        input: Input,
    ) -> (State, Input) {
        if state.place == Place::Ground {
            let speed = state.player.vel.length();
            if speed >= 0.1 {
                let mut friction = parameters.friction * parameters.ent_friction;

                let mut start = state.player.pos + state.player.vel / speed * 16.;
                start.z = state.player.pos.z - if state.player.ducking { 18. } else { 36. };
                let mut end = start;
                end.z -= 34.;

                let tr = tracer.trace(start, end, state.player.hull());
                if tr.fraction == 1. {
                    friction *= parameters.edge_friction;
                }

                let control = speed.max(parameters.stop_speed);
                let drop = control * friction * parameters.frame_time;
                let new_speed = (speed - drop).max(0.);

                state.player.vel *= new_speed / speed;
            }

            if parameters.has_stamina {
                // This is part of PM_WalkMove(). We need it in here instead because Strafe step
                // will change player speed.
                let factor = (100. - (state.player.stamina_time / 1000.) * 19.) / 100.;
                state.player.vel.x *= factor;
                state.player.vel.y *= factor;
            }
        }

        state.player.vel = clamp_velocity(state.player.vel, parameters.max_velocity);
        self.0
            .simulate(tracer, parameters, frame_bulk, state, input)
    }
}

pub struct ResetFields<S>(pub S);

impl<S: Step> Step for ResetFields<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        mut state: State,
        mut input: Input,
    ) -> (State, Input) {
        input.jump = frame_bulk.action_keys.jump;
        input.duck = frame_bulk.action_keys.duck;
        input.use_ = frame_bulk.action_keys.use_;

        input.yaw = state.prev_frame_input.yaw;
        input.pitch = state.prev_frame_input.pitch;

        if let Some(AutoMovement::SetYaw(yaw)) = frame_bulk.auto_actions.movement {
            input.yaw = yaw.to_radians();
        }
        if let Some(pitch) = frame_bulk.pitch {
            input.pitch = pitch.to_radians();
        }

        state.wish_speed = parameters.max_speed;
        state.jumped = false;
        state.move_traces = ArrayVec::new();

        if !matches!(
            frame_bulk.auto_actions.movement,
            Some(AutoMovement::Strafe(StrafeSettings {
                dir: StrafeDir::LeftRight(_) | StrafeDir::RightLeft(_),
                ..
            }))
        ) {
            state.strafe_cycle_frame_count = 0;
        }

        // If we have some acceleration, then this kicks in.
        // It will preserve the final value across split segments.
        if let Some(AutoMovement::Strafe(StrafeSettings {
            type_:
                StrafeType::MaxAccelYawOffset {
                    start,
                    target,
                    accel,
                },
            dir,
        })) = frame_bulk.auto_actions.movement
        {
            let right = matches!(dir, StrafeDir::Right);

            // Flip start and target when accel is negative.
            state.max_accel_yaw_offset_value = (state.max_accel_yaw_offset_value + accel)
                .max(start)
                .min(target);

            // Reset value if we have different inputs.
            // This means that if we split a s5x bulk,
            // there won't be any side effects.
            if start != state.prev_max_accel_yaw_offset_start
                || target != state.prev_max_accel_yaw_offset_target
                || accel != state.prev_max_accel_yaw_offset_accel
                || right != state.prev_max_accel_yaw_offset_right
            {
                state.max_accel_yaw_offset_value = if accel.is_sign_negative() {
                    target
                } else {
                    start
                };

                // Update so next time we know what to compare against.
                state.prev_max_accel_yaw_offset_start = start;
                state.prev_max_accel_yaw_offset_target = target;
                state.prev_max_accel_yaw_offset_accel = accel;
                state.prev_max_accel_yaw_offset_right = right;
            };
        }

        self.0
            .simulate(tracer, parameters, frame_bulk, state, input)
    }
}

pub struct Jump<S>(pub S);

impl<S: Step> Step for Jump<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        mut state: State,
        input: Input,
    ) -> (State, Input) {
        if parameters.has_stamina {
            state.player.stamina_time =
                (state.player.stamina_time - (parameters.frame_time * 1000.).trunc()).max(0.);
        }

        if input.jump && !state.prev_frame_input.jump && state.place == Place::Ground {
            state.jumped = true;

            if parameters.bhop_cap {
                let max_scaled_speed = parameters.bhop_cap_max_speed_scale * parameters.max_speed;
                if max_scaled_speed > 0. {
                    let speed = state.player.vel.length();
                    if speed > max_scaled_speed {
                        state.player.vel *=
                            (max_scaled_speed / speed) * parameters.bhop_cap_multiplier;
                    }
                }
            }

            state.player.vel.z = (2f32 * 800. * 45.).sqrt();

            if parameters.has_stamina {
                state.player.vel.z *= (100. - (state.player.stamina_time / 1000.) * 19.) / 100.;
                state.player.stamina_time = 25000f32 / 19.; // 1315.789429
            }

            state.player.vel = clamp_velocity(state.player.vel, parameters.max_velocity);
            state.update_place(tracer);
        }

        self.0
            .simulate(tracer, parameters, frame_bulk, state, input)
    }
}

pub struct Use<S>(pub S);

impl<S: Step> Step for Use<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        mut state: State,
        input: Input,
    ) -> (State, Input) {
        if parameters.use_slow_down && input.use_ && state.place == Place::Ground {
            state.player.vel *= 0.3;
        }

        self.0
            .simulate(tracer, parameters, frame_bulk, state, input)
    }
}

pub struct DuckBeforeGround<S>(pub S);

impl<S: Step> Step for DuckBeforeGround<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        state: State,
        mut input: Input,
    ) -> (State, Input) {
        let do_nothing = self
            .0
            .simulate(tracer, parameters, frame_bulk, state.clone(), input);

        if let Some(hltas::types::DuckBeforeGround {
            times: Times::UnlimitedWithinFrameBulk,
        }) = frame_bulk.auto_actions.duck_before_ground
        {
            if input.duck {
                // Duck is already pressed.
                return do_nothing;
            }

            if do_nothing.0.player.ducking {
                // We will duck anyway.
                return do_nothing;
            }

            if state.place == Place::Ground {
                // Already on ground.
                return do_nothing;
            }

            input.duck = true;
            let do_action = self
                .0
                .simulate(tracer, parameters, frame_bulk, state, input);

            if !do_action.0.player.ducking {
                // We couldn't duck instantly.
                return do_nothing;
            }

            if do_nothing.0.place == Place::Ground {
                // We ended up on ground after doing nothing, so duck.
                return do_action;
            }

            for tr_nothing in &do_nothing.0.move_traces {
                if tr_nothing.plane_normal.z >= 0.7 {
                    // We hit a ground plane along the doing nothing movement, so duck.
                    return do_action;
                }
            }
        }

        do_nothing
    }
}

pub struct DuckBeforeCollision<S>(pub S);

impl<S: Step> Step for DuckBeforeCollision<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        state: State,
        mut input: Input,
    ) -> (State, Input) {
        let do_nothing = self
            .0
            .simulate(tracer, parameters, frame_bulk, state.clone(), input);

        if let Some(hltas::types::DuckBeforeCollision {
            times: Times::UnlimitedWithinFrameBulk,
            including_ceilings,
        }) = frame_bulk.auto_actions.duck_before_collision
        {
            if input.duck {
                // Duck is already pressed.
                return do_nothing;
            }

            if do_nothing.0.player.ducking {
                // We will duck anyway.
                return do_nothing;
            }

            input.duck = true;
            let do_action = self
                .0
                .simulate(tracer, parameters, frame_bulk, state, input);

            if !do_action.0.player.ducking {
                // We couldn't duck instantly.
                return do_nothing;
            }

            for (tr_nothing, tr_action) in do_nothing
                .0
                .move_traces
                .iter()
                .zip(&do_action.0.move_traces)
            {
                if tr_nothing.plane_normal.z >= 0.7 {
                    // We hit a ground plane, which is not something duck-before-collision handles.
                    return do_nothing;
                }

                if tr_nothing.fraction > tr_action.fraction {
                    // We went further without ducking than after ducking.
                    return do_nothing;
                }

                if tr_nothing.fraction < tr_action.fraction {
                    // We went further after ducking than without ducking.
                    if tr_nothing.plane_normal.z == -1. && !including_ceilings {
                        // We hit a ceiling, but duck-before-collision with ceilings was disabled.
                        return do_nothing;
                    }

                    return do_action;
                }
            }
        }

        do_nothing
    }
}

pub struct Duck<S>(pub S);

impl<S: Step> Step for Duck<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        mut state: State,
        input: Input,
    ) -> (State, Input) {
        // ReduceTimers()
        state.player.duck_time =
            (state.player.duck_time - (parameters.frame_time * 1000.) as i32).max(0);

        // Duck()
        if state.player.ducking
            || (parameters.duck_animation_slow_down
                && (state.player.in_duck_animation || input.duck))
        {
            state.wish_speed *= 0.333;
        }

        if input.duck || state.player.ducking || state.player.in_duck_animation {
            if input.duck {
                if !state.prev_frame_input.duck && !state.player.ducking {
                    state.player.duck_time = 1000;
                    state.player.in_duck_animation = true;
                }

                if state.player.in_duck_animation
                    && (state.player.duck_time <= 600 || state.place != Place::Ground)
                {
                    state.player.ducking = true;
                    state.player.in_duck_animation = false;
                    if state.place == Place::Ground {
                        state.player.pos.z -= 18.;
                        state.update_place(tracer);
                    }
                }
            } else {
                let mut new_pos = state.player.pos;
                if state.place == Place::Ground {
                    new_pos.z += 18.;
                }

                let tr = tracer.trace(new_pos, new_pos, state.player.hull());
                if !tr.start_solid {
                    let tr = tracer.trace(new_pos, new_pos, Hull::Standing);
                    if !tr.start_solid {
                        state.player.ducking = false;
                        state.player.in_duck_animation = false;
                        state.player.duck_time = 0;
                        state.player.pos = new_pos;
                        state.update_place(tracer);
                    }
                }
            }
        }

        self.0
            .simulate(tracer, parameters, frame_bulk, state, input)
    }
}

pub struct LeaveGround<S>(pub S);

impl<S: Step> Step for LeaveGround<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        state: State,
        mut input: Input,
    ) -> (State, Input) {
        let do_nothing = self
            .0
            .simulate(tracer, parameters, frame_bulk, state.clone(), input);

        let Some(action) = frame_bulk.auto_actions.leave_ground_action else {
            return do_nothing;
        };

        if action.times != Times::UnlimitedWithinFrameBulk {
            return do_nothing;
        }

        if state.place != Place::Ground {
            return do_nothing;
        }

        if action.speed != LeaveGroundActionSpeed::Any && state.player.vel.xy().length() < 30. {
            return do_nothing;
        }

        let (do_action, speed_nothing, speed_action) = match action.type_ {
            LeaveGroundActionType::Jump => {
                input.jump = true;
                let do_action = self
                    .0
                    .simulate(tracer, parameters, frame_bulk, state, input);
                let speed_nothing = do_nothing.0.player.vel.xy().length_squared();
                let speed_action = do_action.0.player.vel.xy().length_squared();

                (do_action, speed_nothing, speed_action)
            }
            LeaveGroundActionType::DuckTap { zero_ms } => {
                if zero_ms {
                    // TODO.
                }

                if state.prev_frame_input.duck {
                    return do_nothing;
                }

                if state.player.ducking {
                    // If the player is already ducking, try to unduck first.
                    input.duck = false;
                    let do_action = self
                        .0
                        .simulate(tracer, parameters, frame_bulk, state, input);

                    if do_action.0.player.ducking {
                        // Unducking didn't work, so do nothing.
                        return do_nothing;
                    } else {
                        // Unducking worked, we'll try to ducktap next frame.
                        return do_action;
                    }
                }

                input.duck = true;
                let do_action =
                    self.0
                        .simulate(tracer, parameters, frame_bulk, state.clone(), input);

                // The ducktap happens one frame later. For simulating the second frame, disable the
                // leave-ground action to prevent a stack overflow caused by the simulation
                // recursively progressing forward.
                let frame_bulk_without_action = frame_bulk
                    .clone()
                    .tap_mut(|f| f.auto_actions.leave_ground_action = None);
                let next_nothing = do_nothing
                    .0
                    .clone()
                    .simulate(tracer, parameters, &frame_bulk_without_action)
                    .0;

                let next_action = if parameters.duck_animation_slow_down {
                    // For games like CS 1.6 with duck slowdown already occurring during the ducking
                    // animation, we need to ignore that slowdown. Otherwise, prediction will
                    // always decide against ducktap due to ducktap speed penalty. At the end, the
                    // unaffected do_action is returned for correct prediction.
                    let mut ignore_duck_animation_slow_parameter = parameters;
                    ignore_duck_animation_slow_parameter.duck_animation_slow_down = false;

                    let do_action_without_duck_animation_slow = self.0.simulate(
                        tracer,
                        ignore_duck_animation_slow_parameter,
                        frame_bulk,
                        state,
                        input,
                    );

                    do_action_without_duck_animation_slow
                        .0
                        .simulate(tracer, parameters, &frame_bulk_without_action)
                        .0
                } else {
                    do_action
                        .0
                        .clone()
                        .simulate(tracer, parameters, &frame_bulk_without_action)
                        .0
                };

                let speed_nothing = next_nothing.player.vel.xy().length_squared();
                let speed_action = next_action.player.vel.xy().length_squared();

                (do_action, speed_nothing, speed_action)
            }
        };

        match action.speed {
            LeaveGroundActionSpeed::Any => do_action,
            LeaveGroundActionSpeed::Optimal => {
                if !matches!(
                    frame_bulk.auto_actions.movement,
                    Some(AutoMovement::Strafe(StrafeSettings {
                        type_: StrafeType::MaxAccel,
                        ..
                    }))
                ) {
                    return do_nothing;
                }

                if speed_action > speed_nothing {
                    do_action
                } else {
                    do_nothing
                }
            }
            LeaveGroundActionSpeed::OptimalWithFullMaxspeed => do_nothing,
        }
    }
}

pub struct JumpBug<S>(pub S);

impl<S: Step> Step for JumpBug<S> {
    fn simulate<T: Trace>(
        &self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
        state: State,
        mut input: Input,
    ) -> (State, Input) {
        let do_nothing = self
            .0
            .simulate(tracer, parameters, frame_bulk, state.clone(), input);

        if !matches!(
            frame_bulk.auto_actions.jump_bug,
            Some(hltas::types::JumpBug {
                times: Times::UnlimitedWithinFrameBulk
            })
        ) {
            return do_nothing;
        }

        if state.place != Place::Air {
            return do_nothing;
        }

        if state.player.ducking {
            if input.duck {
                return do_nothing;
            }

            input.jump = true;
            let do_action = self
                .0
                .simulate(tracer, parameters, frame_bulk, state, input);
            if do_action.0.jumped {
                do_action
            } else {
                do_nothing
            }
        } else {
            input.duck = true;

            // The duck frame.
            let do_action = self
                .0
                .simulate(tracer, parameters, frame_bulk, state, input);
            if do_action.0.place != Place::Air {
                return do_nothing;
            }

            // The unduck + jump frame.
            let next = do_action
                .0
                .clone()
                .simulate(tracer, parameters, frame_bulk)
                .0;
            if next.jumped {
                do_action
            } else {
                do_nothing
            }
        }
    }
}
