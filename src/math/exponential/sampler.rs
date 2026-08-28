use rand::SeedableRng;

use super::sample_clamped_exponential;

/// A trait for sampling from a clamped exponential distribution.
///
/// This trait abstracts the process of generating random values from an
/// exponential distribution and clamping them to a specified range relative
/// to the distribution's mean. It allows for easier testing and potential
/// substitution of different sampling strategies.
///
/// The sampled value should be clamped to the range `[mean * lower_factor, mean * upper_factor]`.
pub trait ClampedExponentialSampler {
    /// Samples a value from an exponential distribution and clamps it.
    ///
    /// Generates a sample from an exponential distribution with the specified `mean`
    /// and then restricts the result to be within `[mean * lower_factor, mean * upper_factor]`.
    ///
    /// # Arguments
    /// * `mean`: The desired mean (average) of the underlying exponential distribution (1/lambda).
    ///   **Must be positive.**
    /// * `lower_factor`: The factor by which the `mean` is multiplied to get the lower clamp bound.
    /// * `upper_factor`: The factor by which the `mean` is multiplied to get the upper clamp bound.
    ///
    /// # Returns
    /// A random `f64` value sampled from the exponential distribution and clamped to the specified range.
    ///
    /// # Panics
    /// Implementations may panic if preconditions are violated, such as `mean <= 0.0`.
    /// Refer to the specific implementation's documentation for details.
    fn next_sample(&mut self, mean: f64, lower_factor: f64, upper_factor: f64) -> f64;
}

/// A sampler that uses the standard `rand` library to generate clamped exponential samples.
///
/// This struct wraps an instance of a `rand::Rng` and implements the
/// [`ClampedExponentialSampler`] trait by delegating to the
/// [`sample_clamped_exponential`] function.
///
/// # Type Parameters
/// * `R`: The type of the underlying random number generator, which must implement `rand::Rng`.
pub struct DefaultExponentialSampler<R> {
    rng: R,
}

impl<R> DefaultExponentialSampler<R> {
    /// Creates a new `DefaultExponentialSampler` with the given random number generator.
    ///
    /// # Arguments
    /// * `rng`: An instance of a random number generator.
    ///
    /// # Returns
    /// A new `DefaultExponentialSampler`.
    pub fn new(rng: R) -> Self {
        Self { rng }
    }

    /// Consumes the sampler and returns the underlying random number generator.
    ///
    /// This can be useful if you need to reuse the RNG for other purposes
    /// after using the sampler.
    ///
    /// # Returns
    /// The `R` instance that was wrapped by this sampler.
    pub fn into_inner(self) -> R {
        self.rng
    }

    /// Gets a reference to the underlying random number generator.
    ///
    /// # Returns
    /// A reference to the `R` instance.
    pub fn inner(&self) -> &R {
        &self.rng
    }

    /// Gets a mutable reference to the underlying random number generator.
    ///
    /// This allows you to reseed or otherwise modify the state of the RNG
    /// if the `R` type supports such operations.
    ///
    /// # Returns
    /// A mutable reference to the `R` instance.
    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.rng
    }
}

impl Default for DefaultExponentialSampler<rand::rngs::ThreadRng> {
    /// Creates a new `DefaultExponentialSampler` using the default [`rand::rngs::ThreadRng`].
    ///
    /// This is a convenient way to get a sampler suitable for general use.
    ///
    /// # Returns
    /// A `DefaultExponentialSampler` wrapping `rand::rng()`.
    fn default() -> Self {
        Self { rng: rand::rng() }
    }
}

impl DefaultExponentialSampler<rand::rngs::StdRng> {
    /// Creates a new `DefaultExponentialSampler` seeded with the given value.
    ///
    /// This is useful for creating reproducible samplers for testing or simulations.
    ///
    /// # Arguments
    /// * `seed`: The seed value for the `StdRng`.
    ///
    /// # Returns
    /// A `DefaultExponentialSampler` wrapping a seeded `StdRng`.
    pub fn seed_from_u64(seed: u64) -> Self {
        let rng = rand::rngs::StdRng::seed_from_u64(seed);
        Self { rng }
    }
}

impl<R: rand::Rng> ClampedExponentialSampler for DefaultExponentialSampler<R> {
    /// Samples a value from an exponential distribution and clamps it using the wrapped RNG.
    ///
    /// Delegates to the [`sample_clamped_exponential`] function, passing the
    /// wrapped `Rng` instance. Refer to that function's documentation for
    /// details on the sampling process, clamping, and panic conditions.
    ///
    /// # Arguments
    /// * `mean`: The desired mean of the distribution. **Must be positive.**
    /// * `lower_factor`: The lower clamp factor.
    /// * `upper_factor`: The upper clamp factor.
    ///
    /// # Returns
    /// A clamped exponential sample.
    ///
    /// # Panics
    /// Panics if `mean` is less than or equal to zero, or if `lower_factor >= upper_factor`,
    /// as defined by [`sample_clamped_exponential`].
    fn next_sample(&mut self, mean: f64, lower_factor: f64, upper_factor: f64) -> f64 {
        sample_clamped_exponential(mean, lower_factor, upper_factor, &mut self.rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_std_rng_exponential_sampler_new() {
        let rng = rand::rngs::StdRng::seed_from_u64(12345);
        let sampler = DefaultExponentialSampler::new(rng);
        let _ = sampler;
    }

    #[test]
    fn test_std_rng_exponential_sampler_default() {
        let sampler = DefaultExponentialSampler::default();
        let _ = sampler;
    }

    #[test]
    fn test_std_rng_exponential_sampler_seed_from_u64() {
        let sampler = DefaultExponentialSampler::seed_from_u64(54321);
        let _ = sampler;
    }

    #[test]
    fn test_std_rng_exponential_sampler_into_inner() {
        let original_seed = 98765u64;
        let rng = rand::rngs::StdRng::seed_from_u64(original_seed);
        let sampler = DefaultExponentialSampler::new(rng);

        let mut recovered_rng = sampler.into_inner();
        let _sample = sample_clamped_exponential(1.0, 0.5, 2.0, &mut recovered_rng);
    }

    #[test]
    fn test_std_rng_exponential_sampler_inner() {
        let rng = rand::rngs::StdRng::seed_from_u64(11111);
        let sampler = DefaultExponentialSampler::new(rng);
        let _rng_ref = sampler.inner();
    }

    #[test]
    fn test_std_rng_exponential_sampler_inner_mut() {
        let rng = rand::rngs::StdRng::seed_from_u64(22222);
        let mut sampler = DefaultExponentialSampler::new(rng);
        let rng_mut_ref = sampler.inner_mut();
        let _sample = sample_clamped_exponential(1.0, 0.5, 2.0, rng_mut_ref);
    }

    #[test]
    fn test_std_rng_exponential_sampler_next_sample() {
        let mut sampler = DefaultExponentialSampler::seed_from_u64(33333);
        let mean = 5.0;
        let lower_factor = 0.5;
        let upper_factor = 1.5;
        let lower_bound = mean * lower_factor;
        let upper_bound = mean * upper_factor;

        let sample = sampler.next_sample(mean, lower_factor, upper_factor);

        assert!(sample >= lower_bound);
        assert!(sample <= upper_bound);
    }

    #[test]
    #[should_panic(expected = "Mean for exponential distribution must be positive")]
    fn test_std_rng_exponential_sampler_next_sample_panic_zero_mean() {
        let mut sampler = DefaultExponentialSampler::seed_from_u64(44444);
        let _ = sampler.next_sample(0.0, 0.1, 2.0);
    }
}
