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
    fn test_sample_exponential_reproducibility() {
        let seed = 13579;
        let mean = 7.3;
        let mut rng1 = StdRng::seed_from_u64(seed);
        let mut rng2 = StdRng::seed_from_u64(seed);

        let sample1 = sample_exponential(mean, &mut rng1);
        let sample2 = sample_exponential(mean, &mut rng2);

        assert_eq!(sample1, sample2);
    }
}
