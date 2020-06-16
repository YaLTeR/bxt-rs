//! Pattern matching in memory.
//!
//! This is extracted into a separate crate to be able to compile it with optimizations even in
//! debug builds. Searching memory for patterns is really slow otherwise.

/// Set of byte patterns.
///
/// Each pattern can either match bytes (`Some(x)`) or skip them (`None`). For a memory location to
/// match the pattern, all bytes which aren't skipped in the pattern must be equal to the
/// corresponding memory bytes.
#[derive(Clone, Copy)]
pub struct Patterns(pub &'static [&'static [Option<u8>]]);

impl Patterns {
    /// Finds a unique pattern occurrence in memory. Returns a tuple of (byte offset, pattern
    /// index).
    ///
    /// If multiple patterns were found, or if a pattern was found in multiple places, `None` is
    /// returned, as if nothing was found.
    pub fn find(self, memory: &[u8]) -> Option<(usize, usize)> {
        if self.0.is_empty() {
            return None;
        }

        let min_len = self.0.iter().map(|pattern| pattern.len()).min().unwrap();
        if memory.len() < min_len {
            return None;
        }

        let mut match_offset = None;

        // This is the fastest naive solution I could come up with after profiling debug and
        // release builds with hawktracer.

        // Try to match every pattern.
        for (index, pattern) in self.0.iter().enumerate() {
            // Go through every pattern-sized window of memory.
            'next_offset: for (offset, window) in memory.windows(pattern.len()).enumerate() {
                // Check each byte of the window.
                for (&mem, &pat) in window.iter().zip(pattern.iter()) {
                    // If a pattern byte isn't equal to the memory byte,
                    if matches!(pat, Some(byte) if byte != mem) {
                        // try the next memory offset.
                        continue 'next_offset;
                    }
                }

                // We have found a match.

                if match_offset.is_some() {
                    // Duplicate match.
                    return None;
                }

                match_offset = Some((offset, index))
            }
        }

        match_offset
    }
}
