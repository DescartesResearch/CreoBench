//! This module provides traits for random number generation.

/// A trait for generating random numbers within a range.
pub trait RangeRNG {
    fn random_range(&mut self, range: std::ops::Range<usize>) -> usize;
}

impl<T: rand::RngExt> RangeRNG for T {
    /// Generate a random `usize` within the specified range.
    ///
    /// Returns a random number where `range.start <= n < range.end`.
    ///
    /// # Arguments
    ///
    /// * `range` - The range within which to generate a random number. The range is half-open: `[start, end)`.
    ///
    /// # Returns
    ///
    /// A random `usize` within the specified range.
    ///
    /// # Panics
    ///
    /// Implementations may panic if `range.start >= range.end`, though
    /// this is not required by the trait.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use creo_bench::math::rng::RangeRNG;
    /// use rand::{SeedableRng, rngs::StdRng};
    ///
    /// let mut rng: StdRng = rand::make_rng();
    /// let random_index = rng.random_range(0..10);
    /// assert!(random_index >= 0);
    /// assert!(random_index < 10);
    /// ```
    fn random_range(&mut self, range: std::ops::Range<usize>) -> usize {
        use rand::RngExt;
        RngExt::random_range(self, range)
    }
}
