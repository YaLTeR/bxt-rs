//! Macros.

/// Creates a `&'static [Option<u8>]` byte pattern.
///
/// # Examples
///
/// ```
/// let patterns = Patterns(&[
///     // 6153
///     pattern!(55 8B EC 56 57 8B 7D ?? 57 E8 ?? ?? ?? ?? 8A 08),
/// ]);
/// ```
macro_rules! pattern {
    ($($value:tt)*) => (
        {
            #[bxt_macros::pattern_const($($value)*)]
            const PATTERN: &[Option<u8>] = &[];
            PATTERN
        }
    )
}
