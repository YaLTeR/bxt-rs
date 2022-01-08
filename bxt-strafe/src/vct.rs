use arrayvec::ArrayVec;
use ordered_float::NotNan;
use parking_lot::{const_rwlock, Once, RwLock};

use crate::normalize_rad;

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub forward: i16,
    pub side: i16,
    pub angle: NotNan<f32>,
}

pub struct Vct {
    entries: ArrayVec<Entry, 10196504>,
}

/// The largest max_speed that the VCT is valid for.
///
/// The VCT is exactly the same for any max_speed less than or equal to this value.
pub const MAX_SPEED_CAP: f32 = 1023.;

pub fn get_static() -> &'static RwLock<Vct> {
    static VCT: RwLock<Vct> = const_rwlock(Vct::new());

    static INIT: Once = Once::new();
    INIT.call_once(|| VCT.write().compute());

    &VCT
}

impl Vct {
    pub const fn new() -> Self {
        Self {
            entries: ArrayVec::new_const(),
        }
    }

    fn add_combinations(&mut self, f: i16, s: i16) {
        for (f, s) in [
            (f, s),
            (f, -s),
            (-f, s),
            (-f, -s),
            (s, f),
            (s, -f),
            (-s, f),
            (-s, -f),
        ] {
            self.entries.push(Entry {
                forward: f,
                side: s,
                angle: NotNan::new((-s as f32).atan2(f as f32)).unwrap(),
            })
        }
    }

    pub fn compute(&mut self) {
        eprintln!("Computing the vectorial compensation table.");

        /// Maximal value for forwardmove and sidemove.
        const MAX_MOVE: i16 = 2047;

        // Compute the Farey sequence in ascending order, starting from 0 / 1 and 1 / MAX_MOVE.
        // This produces all co-prime F and S in the first octant (angles from -90 to -45 degrees).
        let mut f = 0;
        let mut s = 1;
        let mut p = 1;
        let mut q = MAX_MOVE;

        while p != 1 || q != 1 {
            let k = (MAX_MOVE + s) / q;
            let tmp_f = f;
            let tmp_s = s;
            f = p;
            s = q;
            p = k * p - tmp_f;
            q = k * q - tmp_s;

            // Scale f and s to be as large as possible.
            let fac = MAX_MOVE / s;
            let scaled_f = f * fac;
            let scaled_s = s * fac;

            self.add_combinations(scaled_f, scaled_s);
        }

        // Add 0 and PI / 4 angles omitted in the loop above.
        self.add_combinations(0, 2047);
        self.add_combinations(2047, 2047);

        self.entries.sort_unstable_by_key(|entry| entry.angle);
    }

    pub fn find_best(&self, accel_angle: f32) -> Entry {
        let accel_angle = NotNan::new(normalize_rad(accel_angle)).unwrap();

        let index = self
            .entries
            .binary_search_by_key(&accel_angle, |entry| entry.angle);

        match index {
            Ok(index) => self.entries[index],
            Err(index) if index == 0 => self.entries[0],
            Err(index) if index == self.entries.len() => self.entries[index - 1],
            Err(index) => {
                let prev = self.entries[index - 1];
                let next = self.entries[index];
                if accel_angle - prev.angle < next.angle - accel_angle {
                    prev
                } else {
                    next
                }
            }
        }
    }
}
