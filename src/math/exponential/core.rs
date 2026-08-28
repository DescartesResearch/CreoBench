use rand::RngExt;

/// Samples a value from an exponential distribution.
///
/// The exponential distribution is often used to model the time between
/// events in a Poisson process. It produces positive values with a long tail.
///
/// # Arguments
/// * `mean`: The desired mean (average) of the distribution (1/lambda). **Must be positive.**
/// * `rng`: A mutable reference to a random number generator.
///
/// # Returns
/// A random `f64` value sampled from the exponential distribution.
///
/// # Panics
/// Panics if `mean` is less than or equal to zero.
///
/// # Examples
/// ```
/// use creo_bench::math::exponential;
/// use rand::rngs::StdRng;
/// use rand::SeedableRng;
///
/// let mut rng = StdRng::seed_from_u64(42);
/// let mean = 5.0;
/// let sample = exponential::sample_exponential(mean, &mut rng);
///
/// assert!(sample==3.738623088203482);
/// ```
pub fn sample_exponential(mean: f64, rng: &mut impl rand::Rng) -> f64 {
    debug_assert!(
        mean > 0.0,
        "Mean for exponential distribution must be positive, got {}",
        mean
    );
    if mean <= 0.0 {
        panic!(
            "Mean for exponential distribution must be positive, got {}",
            mean
        );
    }
    let lambda = 1.0 / mean;
    // Sample u in (0.0, 1.0]
    let u = 1.0 - rng.random::<f64>().max(f64::EPSILON);
    -u.ln() / lambda
}

/// Samples a value from an exponential distribution and clamps it to a specified range.
///
/// This function generates a sample from an exponential distribution with the given `mean`
/// and then restricts the result to be within `[mean * lower_factor, mean * upper_factor]`.
/// Clamping helps prevent extreme values that might lead to undesirable behavior
/// (e.g., very long or very short wait times).
///
/// # Arguments
/// * `mean`: The desired mean (average) of the underlying exponential distribution (1/lambda). **Must be positive.**
/// * `lower_factor`: The factor by which the `mean` is multiplied to get the lower clamp bound.
/// * `upper_factor`: The factor by which the `mean` is multiplied to get the upper clamp bound.
/// * `rng`: A mutable reference to a random number generator.
///
/// # Returns
/// A random `f64` value sampled from the exponential distribution and clamped to the specified range.
///
/// # Panics
/// - If `mean` is less than or equal to zero
/// - If `lower_factor` is greater than or equal to `upper_factor`
///
/// # Examples
/// ```
/// use creo_bench::math::exponential;
/// use rand::rngs::StdRng;
/// use rand::SeedableRng;
///
/// let mut rng = StdRng::seed_from_u64(123);
/// let mean = 8.0;
/// let lower = 0.5;
/// let upper = 1.5;
/// let clamped_sample = exponential::sample_clamped_exponential(mean, lower, upper, &mut rng);
/// let lower_bound = mean * lower;
/// let upper_bound = mean * upper;
/// assert!(clamped_sample == lower_bound);
/// ```
pub fn sample_clamped_exponential(
    mean: f64,
    lower_factor: f64,
    upper_factor: f64,
    rng: &mut impl rand::Rng,
) -> f64 {
    if lower_factor >= upper_factor {
        panic!(
            "Lower clamping factor should be less than upper clamping factor, but was `{lower_factor}` (lower factor) and `{upper_factor}` (upper factor)"
        )
    }
    let sampled_value = sample_exponential(mean, rng);
    let lower_bound = mean * lower_factor;
    let upper_bound = mean * upper_factor;

    sampled_value.clamp(lower_bound, upper_bound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn test_sample_exponential_basic() {
        let mut rng = StdRng::seed_from_u64(12345);
        let mean = 2.5;
        let sample = sample_exponential(mean, &mut rng);
        assert!(sample > 0.0);
        assert!(sample.is_finite());
    }

    #[test]
    #[should_panic(expected = "Mean for exponential distribution must be positive")]
    fn test_sample_exponential_panic_zero_mean() {
        let mut rng = StdRng::seed_from_u64(54321);
        let _ = sample_exponential(0.0, &mut rng);
    }

    #[test]
    #[should_panic(expected = "Mean for exponential distribution must be positive")]
    fn test_sample_exponential_panic_negative_mean() {
        let mut rng = StdRng::seed_from_u64(98765);
        let _ = sample_exponential(-5.0, &mut rng);
    }

    #[test]
    fn test_sample_exponential_mean() {
        let seed = 112233_u64;
        let mean = 5.0;
        let num_samples = 500_000;
        let tolerance = 0.001;
        let mut rng = StdRng::seed_from_u64(seed);

        let sum: f64 = (0..num_samples)
            .map(|_| {
                // Create a new RNG for each sample to minimize correlation
                // Seed based on the main seed and the iteration index
                let sample = sample_exponential(mean, &mut rng);
                assert!(sample > 0.0, "Sampled value must be positive");
                sample
            })
            .sum();

        let calculated_mean = sum / (num_samples as f64);
        let difference = (calculated_mean - mean).abs();
        let relative_error = difference / mean;

        assert!(
            relative_error <= tolerance,
            "Sampled mean {} is not within {}% of expected mean {}. Relative error: {}",
            calculated_mean,
            tolerance * 100.0,
            mean,
            relative_error * 100.0
        );
    }

    #[test]
    fn test_sample_exponential_very_small_positive_mean() {
        let mut rng = StdRng::seed_from_u64(11111);
        let mean = f64::MIN_POSITIVE;
        let sample = sample_exponential(mean, &mut rng);
        assert!(sample >= 0.0 || sample.is_infinite());
    }

    #[test]
    fn test_sample_exponential_large_mean() {
        let mut rng = StdRng::seed_from_u64(22222);
        let mean = 1_000_000.0;
        let sample = sample_exponential(mean, &mut rng);
        assert!(sample > 0.0);
        assert!(sample.is_finite());
    }

    #[test]
    fn test_sample_exponential_clamped_basic() {
        let mut rng = StdRng::seed_from_u64(33333);
        let mean = 5.0;
        let lower_factor = 0.5;
        let upper_factor = 1.5;

        let lower_bound = mean * lower_factor; // 2.5
        let upper_bound = mean * upper_factor; // 7.5

        let clamped_sample = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng);

        assert!(clamped_sample >= lower_bound);
        assert!(clamped_sample <= upper_bound);
        assert!(clamped_sample > 0.0);
    }

    #[test]
    #[should_panic(expected = "Mean for exponential distribution must be positive")]
    fn test_sample_exponential_clamped_panic_zero_mean() {
        let mut rng = StdRng::seed_from_u64(44444);
        let _ = sample_clamped_exponential(0.0, 0.1, 2.0, &mut rng);
    }

    #[test]
    #[should_panic(expected = "Mean for exponential distribution must be positive")]
    fn test_sample_exponential_clamped_panic_negative_mean() {
        let mut rng = StdRng::seed_from_u64(55555);
        let _ = sample_clamped_exponential(-1.0, 0.1, 2.0, &mut rng);
    }

    #[test]
    #[should_panic(expected = "Lower clamping factor should be less than upper clamping factor")]
    fn test_sample_exponential_clamped_bounds_order() {
        let mut rng = StdRng::seed_from_u64(66666);
        let mean = 4.0;
        let lower_factor = 2.0;
        let upper_factor = 1.0;

        let _ = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng);
    }

    #[test]
    fn test_sample_exponential_clamped_tiny_factors() {
        let mut rng = StdRng::seed_from_u64(77777);
        let mean = 10.0;
        let lower_factor = 0.0001;
        let upper_factor = 0.0002;

        let lower_bound = mean * lower_factor;
        let upper_bound = mean * upper_factor;

        let clamped_sample = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng);

        assert!(clamped_sample > 0.0);
        assert!(clamped_sample >= lower_bound);
        assert!(clamped_sample <= upper_bound);
    }

    #[test]
    fn test_sample_exponential_clamped_large_factors() {
        let mut rng = StdRng::seed_from_u64(88888);
        let mean = 2.0;
        let lower_factor = 1000.0;
        let upper_factor = 2000.0;

        let lower_bound = mean * lower_factor;
        let upper_bound = mean * upper_factor;

        let clamped_sample = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng);

        assert!(clamped_sample > 0.0);
        assert!(clamped_sample >= lower_bound);
        assert!(clamped_sample <= upper_bound);
    }

    #[test]
    fn test_sample_exponential_clamped_fractional_factors() {
        let mut rng = StdRng::seed_from_u64(99999);
        let mean = 100.0;
        let lower_factor = 0.8;
        let upper_factor = 1.2;

        let lower_bound = mean * lower_factor;
        let upper_bound = mean * upper_factor;

        let clamped_sample = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng);

        assert!(clamped_sample > 0.0);
        assert!(clamped_sample >= lower_bound);
        assert!(clamped_sample <= upper_bound);
    }

    #[test]
    fn test_sample_exponential_clamped_extreme_clamp_low() {
        let mut rng = StdRng::seed_from_u64(10101);
        let mean = 1000.0;
        let lower_factor = 0.000001;
        let upper_factor = 0.000002;

        let lower_bound = mean * lower_factor;
        let upper_bound = mean * upper_factor;

        let clamped_sample = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng);

        assert!(clamped_sample > 0.0);
        assert!(clamped_sample >= lower_bound);
        assert!(clamped_sample <= upper_bound);
    }

    #[test]
    fn test_sample_exponential_clamped_extreme_clamp_high() {
        let mut rng = StdRng::seed_from_u64(10202);
        let mean = 0.001;
        let lower_factor = 1000000.0;
        let upper_factor = 2000000.0;

        let lower_bound = mean * lower_factor;
        let upper_bound = mean * upper_factor;

        let clamped_sample = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng);

        assert!(clamped_sample > 0.0);
        assert!(clamped_sample >= lower_bound);
        assert!(clamped_sample <= upper_bound);
    }

    #[test]
    fn test_sample_exponential_reproducibility() {
        let seed = 13579;
        let mean = 7.3;
        let mut rng1 = StdRng::seed_from_u64(seed);
        let mut rng2 = StdRng::seed_from_u64(seed);

        let sample1 = sample_exponential(mean, &mut rng1);
        let sample2 = sample_exponential(mean, &mut rng2);

        assert_eq!(sample1, sample2);
    }

    #[test]
    fn test_sample_exponential_clamped_reproducibility() {
        let seed = 24680;
        let mean = 10.0;
        let lower_factor = 0.7;
        let upper_factor = 1.3;
        let mut rng1 = StdRng::seed_from_u64(seed);
        let mut rng2 = StdRng::seed_from_u64(seed);

        let sample1 = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng1);
        let sample2 = sample_clamped_exponential(mean, lower_factor, upper_factor, &mut rng2);

        assert_eq!(sample1, sample2);
    }
}
