use std::num::NonZeroU32;

use hltas::types::{FrameBulk, Line};
use hltas::HLTAS;

pub trait HLTASExt {
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
            .filter_map(|(l, line)| line.frame_bulk().map(|bulk| (l, bulk)))
            .flat_map(|(l, frame_bulk)| (0..frame_bulk.frame_count.get()).map(move |r| (l, r)))
            .nth(frame)
    }

    fn split_at_frame(&mut self, frame: usize) -> Option<&mut FrameBulk> {
        let (l, r) = self.line_and_repeat_at_frame(frame)?;

        let frame_bulk = self.lines[l].frame_bulk_mut().unwrap();

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

        Some(self.lines[index].frame_bulk_mut().unwrap())
    }

    fn split_single_at_frame(&mut self, frame: usize) -> Option<&mut FrameBulk> {
        self.split_at_frame(frame + 1);
        self.split_at_frame(frame)
    }
}
