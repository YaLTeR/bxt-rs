use std::f32::consts::{PI, TAU};

use arrayvec::ArrayVec;
use glam::Vec3;
use hltas::types::*;
use serde::{Deserialize, Serialize};

mod steps;
use steps::*;

/// Result of a trace operation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TraceResult {
    pub all_solid: bool,
    pub start_solid: bool,
    pub fraction: f32,
    pub end_pos: Vec3,
    pub plane_normal: Vec3,
    pub entity: i32,
}

/// Collision hull type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hull {
    /// Standing player.
    Standing,
    /// Ducked player.
    Ducked,
    /// Point-sized hull, as in tracing a line.
    Point,
}

/// The game world's tracing function.
pub trait Trace {
    /// Traces a line from `start` to `end` according to `hull` and returns the outcome.
    fn trace(&self, start: Vec3, end: Vec3, hull: Hull) -> TraceResult;
}

/// Player data.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Player {
    /// Position.
    pub pos: Vec3,
    /// Velocity.
    pub vel: Vec3,
    /// Base velocity (e.g. when the player is on a moving conveyor belt).
    pub base_vel: Vec3,
    /// Whether the player is fully ducking.
    pub ducking: bool,
    /// Whether the player is in process of ducking down.
    pub in_duck_animation: bool,
    /// Ducking animation timer.
    pub duck_time: i32,
    /// Stamina timer for games like CS1.6.
    pub stamina_time: f32,
    /// Player health.
    pub health: f32,
    /// Player armor.
    pub armor: f32,
}

impl Player {
    /// Returns the collision hull to use for tracing for this player state.
    pub fn hull(&self) -> Hull {
        if self.ducking {
            Hull::Ducked
        } else {
            Hull::Standing
        }
    }
}

/// Movement parameters.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Parameters {
    pub frame_time: f32,
    pub max_velocity: f32,
    pub max_speed: f32,
    pub stop_speed: f32,
    pub friction: f32,
    pub edge_friction: f32,
    pub ent_friction: f32,
    pub accelerate: f32,
    pub air_accelerate: f32,
    pub gravity: f32,
    pub ent_gravity: f32,
    pub step_size: f32,
    pub bounce: f32,
    pub bhop_cap: bool,
    pub bhop_cap_multiplier: f32,
    pub bhop_cap_max_speed_scale: f32,
    pub use_slow_down: bool,
    pub has_stamina: bool,
    pub duck_animation_slow_down: bool,
}

/// The type of player's position in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Place {
    /// The player is on the ground.
    #[default]
    Ground,
    /// The player is in the air.
    Air,
    /// The player is underwater.
    Water,
}

/// Final input that the game will receive.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Input {
    pub jump: bool,
    pub duck: bool,
    pub use_: bool,

    pub pitch: f32,
    pub yaw: f32,
    pub forward: f32,
    pub side: f32,
}

/// The state updated and acted upon by the simulation.
///
/// To simulate the next frame, call [`State::simulate()`] on the previous state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct State {
    pub player: Player,
    pub place: Place,
    pub wish_speed: f32,
    pub prev_frame_input: Input,
    pub jumped: bool,
    pub move_traces: ArrayVec<TraceResult, 4>,
    // Number of frames for [`StrafeDir::LeftRight`] or [`StrafeDir::RightLeft`] which goes from
    // `0` to `count - 1`.
    pub strafe_cycle_frame_count: u32,
    // Accelerated yaw speed specifics
    pub max_accel_yaw_offset_value: f32,
    // These values are to indicate whether we are in a "different" frame bulk.
    pub prev_max_accel_yaw_offset_start: f32,
    pub prev_max_accel_yaw_offset_target: f32,
    pub prev_max_accel_yaw_offset_accel: f32,
    pub prev_max_accel_yaw_offset_right: bool,
    // In case of yaw and pitch override, this might be useful.
    pub rendered_viewangles: Vec3,
}

impl State {
    pub fn new<T: Trace>(tracer: &T, parameters: Parameters, player: Player) -> Self {
        let mut rv = Self {
            player,
            place: Place::Air,
            wish_speed: parameters.max_speed,
            prev_frame_input: Input::default(),
            jumped: false,
            move_traces: ArrayVec::new(),
            strafe_cycle_frame_count: 0,
            max_accel_yaw_offset_value: 0.,
            prev_max_accel_yaw_offset_start: 0.,
            prev_max_accel_yaw_offset_target: 0.,
            prev_max_accel_yaw_offset_accel: 0.,
            prev_max_accel_yaw_offset_right: false,
            rendered_viewangles: Vec3::ZERO,
        };

        rv.update_place(tracer);

        rv
    }

    /// Simulates one frame and returns the next `State` and the final `Input`.
    pub fn simulate<T: Trace>(
        self,
        tracer: &T,
        parameters: Parameters,
        frame_bulk: &FrameBulk,
    ) -> (Self, Input) {
        let chain = ResetFields(JumpBug(LeaveGround(DuckBeforeCollision(DuckBeforeGround(
            Duck(Use(Jump(Friction(Strafe(Move))))),
        )))));
        chain.simulate(tracer, parameters, frame_bulk, self, Input::default())
    }

    fn update_place<T: Trace>(&mut self, tracer: &T) {
        self.place = Place::Air;

        if self.player.vel.z > 180. {
            return;
        }

        let tr = tracer.trace(
            self.player.pos,
            self.player.pos - Vec3::new(0., 0., 2.),
            self.player.hull(),
        );
        if tr.entity == -1 || tr.plane_normal.z < 0.7 {
            return;
        }

        self.place = Place::Ground;
        if !tr.start_solid && !tr.all_solid {
            self.player.pos = tr.end_pos;
        }
    }
}

const U_RAD: f32 = PI / 32768.;
const INV_U_RAD: f32 = 32768. / PI;

fn normalize_rad(mut angle: f32) -> f32 {
    angle %= TAU;

    if angle >= PI {
        angle - TAU
    } else if angle < -PI {
        angle + TAU
    } else {
        angle
    }
}

fn angle_mod_rad(angle: f32) -> f32 {
    ((angle * INV_U_RAD) as i32 & 0xFFFF) as f32 * U_RAD
}

/// A dummy tracer that operates as if in an empty world.
pub struct DummyTracer;

impl Trace for DummyTracer {
    fn trace(&self, _start: Vec3, end: Vec3, _hull: Hull) -> TraceResult {
        TraceResult {
            all_solid: false,
            start_solid: false,
            fraction: 1.,
            end_pos: end,
            plane_normal: Vec3::ZERO,
            entity: -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use ncollide3d::na::{self, Isometry3, Unit, Vector3};
    use ncollide3d::query::{time_of_impact, DefaultTOIDispatcher, TOIStatus, TOI};
    use ncollide3d::shape::{Cuboid, Plane};
    use proptest::prelude::*;

    use super::*;

    #[derive(Debug, Clone)]
    struct World {
        floor: Plane<f32>,
    }

    impl World {
        fn new() -> Self {
            Self {
                floor: Plane::new(Unit::new_normalize(Vector3::z())),
            }
        }
    }

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

    impl Trace for World {
        fn trace(&self, start: Vec3, end: Vec3, hull: Hull) -> TraceResult {
            let half_height = match hull {
                Hull::Standing => 36.,
                Hull::Ducked => 18.,
                Hull::Point => unimplemented!(),
            };

            let player = Cuboid::new(Vector3::new(16., 16., half_height));
            let player_pos = Isometry3::translation(start.x, start.y, start.z + half_height);
            let vel = end - start;
            let player_vel = Vector3::new(vel.x, vel.y, vel.z);

            let toi = time_of_impact(
                &DefaultTOIDispatcher,
                &Isometry3::translation(0., 0., 0.),
                &na::zero(),
                &self.floor,
                &player_pos,
                &player_vel,
                &player,
                1.,
                0.,
            )
            .unwrap();

            if let Some(TOI {
                toi,
                normal1,
                status,
                ..
            }) = toi
            {
                let penetrating = status == TOIStatus::Penetrating;

                TraceResult {
                    all_solid: penetrating,
                    start_solid: penetrating,
                    fraction: toi,
                    end_pos: start + (end - start) * toi * 0.999,
                    plane_normal: Vec3::new(normal1.x, normal1.y, normal1.z),
                    entity: 0,
                }
            } else {
                TraceResult {
                    all_solid: false,
                    start_solid: false,
                    fraction: 1.,
                    end_pos: end,
                    plane_normal: Vec3::ZERO,
                    entity: -1,
                }
            }
        }
    }

    #[test]
    fn stand_still_on_ground() {
        let world = World::new();
        let parameters = default_parameters();
        let player = Player {
            pos: Vec3::ZERO,
            ..default_player()
        };
        let state = State::new(&world, parameters, player);

        let new_state = state
            .clone()
            .simulate(
                &world,
                parameters,
                &FrameBulk::with_frame_time("0.010000001".to_owned()),
            )
            .0;

        assert_eq!(state.player, new_state.player);
    }

    #[test]
    fn snap_to_ground_from_one_unit() {
        let world = World::new();
        let parameters = default_parameters();
        let player = Player {
            pos: Vec3::new(0., 0., 1.),
            ..default_player()
        };
        let state = State::new(&world, parameters, player);

        let state = state
            .simulate(
                &world,
                parameters,
                &FrameBulk::with_frame_time("0.010000001".to_owned()),
            )
            .0;

        assert!(state.player.pos.z.abs() < 1e-5);
    }

    #[test]
    fn no_snap_to_ground_if_too_high() {
        let world = World::new();
        let parameters = default_parameters();
        let player = Player {
            pos: Vec3::new(0., 0., 2.1),
            ..default_player()
        };
        let state = State::new(&world, parameters, player);

        let state = state
            .simulate(
                &world,
                parameters,
                &FrameBulk::with_frame_time("0.010000001".to_owned()),
            )
            .0;

        assert!(state.player.pos.z.abs() >= 1e-5);
    }

    #[test]
    fn autojump_works() {
        let world = World::new();
        let parameters = default_parameters();
        let player = Player {
            pos: Vec3::new(0., 0., 2.01),
            ..default_player()
        };
        let state = State::new(&world, parameters, player);

        let frame_bulk = FrameBulk {
            frame_count: NonZeroU32::new(2).unwrap(),
            auto_actions: AutoActions {
                leave_ground_action: Some(LeaveGroundAction {
                    speed: LeaveGroundActionSpeed::Any,
                    times: Times::UnlimitedWithinFrameBulk,
                    type_: LeaveGroundActionType::Jump,
                }),
                ..Default::default()
            },
            ..FrameBulk::with_frame_time("0.010000001".to_owned())
        };

        let (state, input) = state.simulate(&world, parameters, &frame_bulk);
        assert!(!input.jump);
        assert_eq!(state.place, Place::Ground);

        let (state, input) = state.simulate(&world, parameters, &frame_bulk);
        assert!(input.jump);
        assert_eq!(state.place, Place::Air);
    }

    prop_compose! {
        #[allow(clippy::excessive_precision)]
        fn arbitrary_player()(
            pos in (-50000f32..50000., -50000f32..50000., 0f32..50000.).prop_map(|(x, y, z)| Vec3::new(x, y, z)),
            vel in (-50000f32..50000., -50000f32..50000., -50000f32..50000.).prop_map(|(x, y, z)| Vec3::new(x, y, z)),
            base_vel in (-50000f32..50000., -50000f32..50000., -50000f32..50000.).prop_map(|(x, y, z)| Vec3::new(x, y, z)),
            ducking in any::<bool>(),
            in_duck_animation in any::<bool>(),
            duck_time in 0..1000,
            stamina_time in 0f32..1315.789429,
        ) -> Player {
            Player { pos, vel, base_vel, ducking, in_duck_animation, duck_time, stamina_time, health: 100., armor: 0. }
        }
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
        fn simulation_does_not_panic(
            frame_bulks: Vec<FrameBulk>,
            player in arbitrary_player(),
        ) {
            let world = World::new();
            let parameters = default_parameters();

            let mut state = State::new(&world, parameters, player);
            for frame_bulk in frame_bulks {
                state = state.simulate(&world, parameters, &frame_bulk).0;
            }
        }

        #[test]
        fn player_eventually_reaches_zero_velocity(
            // Smaller values for faster convergence.
            pos in (-50000f32..50000., -50000f32..50000., 0f32..500.).prop_map(|(x, y, z)| Vec3::new(x, y, z)),
            vel in (-50000f32..50000., -50000f32..50000., -500f32..500.).prop_map(|(x, y, z)| Vec3::new(x, y, z)),
            ducking in any::<bool>(),
        ) {
            let world = World::new();
            let parameters = default_parameters();
            let player = Player {
                pos, vel, ducking, ..default_player()
            };

            let frame_bulk = FrameBulk::with_frame_time("0.010000001".to_owned());

            let mut state = State::new(&world, parameters, player);
            for _ in 0..1000 {
                state = state.simulate(&world, parameters, &frame_bulk).0;
                if state.player.vel == Vec3::ZERO {
                    break;
                }
            }

            prop_assert_eq!(state.player.vel, Vec3::ZERO);
        }
    }
}
