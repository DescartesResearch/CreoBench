use std::time::Duration;

use crate::math::exponential;

use super::step::LoadInterval;
use super::{LoadStepDuration, LoadStepElapsed, LoadStepRemaining, WaitTime};

/// Calculates the sequence of request batches and their associated wait times
/// for a single [`LoadInterval`].
///
/// The [`LoadIntervalPacer`] takes the specifications of a load interval (number of requests,
/// duration) and generates an iterator-like sequence of `(Batch, WaitTime)` pairs.
/// It uses an exponential distribution to introduce randomness into the inter-batch
/// wait times while aiming to spread the total requests roughly evenly across the
/// interval's duration. It ensures batches are sent with enough time remaining to meet
/// the step's deadline, based on the calculated mean wait time used for the
/// exponential distribution.
pub struct LoadIntervalPacer<S> {
    /// Number of requests yet to be scheduled into batches.
    requests_remaining: u32,
    /// The total duration allocated for this load interval.
    duration: LoadStepDuration,
    /// The effective mean wait time used as the mean for the exponential
    /// distribution (influencing inter-batch wait times) and as the basis
    /// for calculating batch sizes, thereby setting the overall pacing.
    /// This value also serves as a time buffer/reserve to ensure batches
    /// can be sent before the step's deadline.
    effective_mean_wait_time: WaitTime,
    /// The sampler used for generating random wait times according to an exponential distribution.
    sampler: S,
}

impl<S> LoadIntervalPacer<S> {
    /// The maximum default wait time between request batches.
    ///
    /// This acts as a **ceiling** on the default pacing rate to prevent it from
    /// becoming unintentionally slow. If the calculated ideal default pacing
    /// (based on `duration / BASE_BATCHES_PER_INTERVAL`) results in a wait time
    /// *longer* than this limit, this value is used instead, ensuring a minimum
    /// baseline request rate.
    ///
    /// Example:
    /// - If [`LoadStepPacer::BASE_BATCHES_PER_INTERVAL`] suggests a 100ms default wait,
    ///   but this `MAX_DEFAULT_WAIT_TIME` constant is 20ms, the default wait will be **20ms**.
    ///   This 20ms value then also serves as the minimum time buffer reserved for sending
    ///   batches and the threshold for triggering the final batch flush.
    /// - If the suggestion is 5ms, the default wait will be **5ms**
    ///   (as it's faster than this limit), and 5ms becomes the buffer/threshold.
    ///
    /// This helps maintain responsiveness for intervals with longer durations where
    /// the adaptive calculation might otherwise propose a slow default rate.
    const MAX_DEFAULT_WAIT_TIME: WaitTime = WaitTime::new(Duration::from_millis(20));

    /// The base target number of batches used to calculate an initial pacing rate for batch generation.
    ///
    /// This target is used to calculate an initial mean wait time as `duration /
    /// BASE_BATCHES_PER_INTERVAL`. This provides a baseline pacing that adapts to the interval's duration.
    /// However, the actual number of batches sent will also be influenced by
    /// [`LoadStepPacer::MAX_DEFAULT_WAIT_TIME`], which acts as a ceiling on the wait time between
    /// batches. For longer step durations, this ceiling often results in a higher effective batch
    /// rate (batches per second) and thus more total batches than this value might initially suggest.
    ///
    /// Example:
    /// - If the interval duration is 100ms and this value targets 50 batches, while
    ///   [`LoadStepPacer::MAX_DEFAULT_WAIT_TIME`] is 20ms, then the ceiling of
    ///   [`LoadStepPacer::MAX_DEFAULT_WAIT_TIME`] does *not* apply (as `100ms / 50 = 2ms`),
    ///   so ~50 batches are sent.
    /// - If the step duration is 5000ms and this value targets 50 batches, while
    ///   [`LoadStepPacer::MAX_DEFAULT_WAIT_TIME`] is 20ms, then the ceiling of
    ///   [`LoadStepPacer::MAX_DEFAULT_WAIT_TIME`] *does* apply (as `5000ms / 50 = 100ms`).
    ///   This limits the wait time to [`LoadStepPacer::MAX_DEFAULT_WAIT_TIME`], effectively
    ///   aiming for 50 batches per second (as `1000ms / 20ms = 50`), so ~250 batches are sent
    ///   over the 5 second interval (as `50 * 5 = 250`).
    const BASE_BATCHES_PER_INTERVAL: u32 = 50;

    /// Minimum calculated wait time threshold.
    ///
    /// If the randomly sampled wait time falls below this tiny duration,
    /// it's considered effectively zero. In this case, a final batch
    /// with all remaining requests is created to prevent potential
    /// division-by-zero errors or infinite loops in subsequent calculations.
    const MINIMUM_CALCULATED_WAIT_TIME: WaitTime = WaitTime::from_nanos(1000);

    /// Factor to determine the lower bound for clamping the sampled wait time.
    ///
    /// The minimum sampled wait time will be `mean_wait_time * this_value`.
    /// Helps prevent excessively short wait times that could cause bursts.
    const EXP_SAMPLING_LOWER_CLAMP_FACTOR: f64 = 0.5;

    /// Factor to determine the upper bound for clamping the sampled wait time.
    ///
    /// The maximum sampled wait time will be `mean_wait_time * this_value`.
    /// Helps prevent excessively long wait times that could cause underflows
    /// or miss deadlines.
    const EXP_SAMPLING_UPPER_CLAMP_FACTOR: f64 = 1.5;
}

impl<S: exponential::ClampedExponentialSampler> LoadIntervalPacer<S> {
    /// Creates a new `LoadIntervalPacer`.
    ///
    /// # Arguments
    /// * `interval`: The [`LoadInterval`] to be paced.
    /// * `sampler`: The exponential sampler for wait times between batches.
    ///
    /// # Returns
    /// A new instance ready to generate batches.
    pub fn new(interval: LoadInterval, sampler: S) -> Self {
        let duration = interval.duration();
        let request_count = interval.request_count();

        let default_mean_wait_time = Self::default_mean_wait_time(duration);
        let target_pace_wait_time = Self::target_pace_wait_time(duration, request_count);
        let effective_mean_wait_time =
            Self::effective_mean_wait_time(default_mean_wait_time, target_pace_wait_time);

        Self {
            requests_remaining: request_count,
            duration,
            effective_mean_wait_time,
            sampler,
        }
    }

    /// Calculates the default mean wait time for batch generation.
    ///
    /// This default is used as the mean for the exponential distribution.
    /// It aims for a reasonable pacing based on `duration` and
    /// `BASE_BATCHES_PER_INTERVAL`, but is capped by `MAX_DEFAULT_WAIT_TIME` to
    /// prevent overly slow default rates.
    ///
    /// # Arguments
    /// * `duration`: The total duration of the load step.
    ///
    /// # Returns
    /// The calculated default mean wait time.
    fn default_mean_wait_time(duration: LoadStepDuration) -> WaitTime {
        let target_wait_time_based_on_duration =
            duration.as_duration() / Self::BASE_BATCHES_PER_INTERVAL;
        // Use min to enforce MAX_DEFAULT_WAIT_TIME as a ceiling on slowness.
        // If the adaptive target is slower (longer wait) than the limit,
        // use the limit (faster pace). Otherwise, use the adaptive target.
        Self::MAX_DEFAULT_WAIT_TIME.min(WaitTime::new(target_wait_time_based_on_duration))
    }

    /// Generates the next batch of requests and the wait time *after* it.
    ///
    /// This method consumes internal state (`requests_remaining`). It should
    /// be called repeatedly until it returns `None`.
    ///
    /// # Arguments
    /// * `now`: The time elapsed since the start of this step.
    ///
    /// # Returns
    /// * [`Some`]: The next batch to send and the time to wait
    ///   after sending it before the *next* call.
    /// * [`None`]: If all requests for this step have been scheduled.
    ///
    /// # Notes
    /// The method will return a final batch paired with [`WaitTime::ZERO`]
    /// if the remaining step duration is less than the calculated
    /// effective mean wait time, ensuring all requests are flushed before the deadline.
    pub fn next(&mut self, now: LoadStepElapsed) -> Option<(crate::transaction::Batch, WaitTime)> {
        if self.requests_remaining == 0 {
            return None;
        }
        let duration_remaining = self.duration.remaining_after(now);

        if self.is_final_batch_time(duration_remaining) {
            return Some(self.final_batch());
        }

        let wait_time = self.next_wait_time(duration_remaining);

        if self.is_wait_time_negligible(wait_time) {
            return Some(self.final_batch());
        }

        let batch_size = self.next_batch_size(duration_remaining);

        if self.is_batch_size_zero(batch_size) {
            return Some(self.final_batch());
        }

        let batch = self.new_batch(batch_size);
        Some((batch, wait_time))
    }

    /// Calculates the wait time from the current elapsed time to the deadline of this interval.
    ///
    /// This function determines the remaining time until the [`LoadInterval`] associated
    /// with this pacer is due to finish. It is primarily useful after all request
    /// batches have been generated (i.e., after `next` has returned `None`)
    /// to determine how long to wait until the exact deadline if precise timing is required.
    ///
    /// # Arguments
    /// * `now`: The time elapsed since the start of this step.
    ///
    /// # Returns
    /// A [`WaitTime`] representing the duration from `now` to the step's deadline.
    /// If `now` is at or past the deadline, this will return [`WaitTime::ZERO`].
    ///
    /// # Examples
    /// ```
    /// # use creo_bench::load::{LoadInterval, LoadStepDuration, LoadStepElapsed, LoadIntervalPacer};
    /// # use std::time::Duration;
    /// # use creo_bench::math::exponential::DefaultExponentialSampler;
    ///
    /// let interval =
    /// LoadInterval::new(LoadStepDuration::new(Duration::from_millis(1000)), 50);
    /// let pacer = LoadIntervalPacer::new(interval, DefaultExponentialSampler::default());
    ///
    /// // Imagine 800 milliseconds have elapsed
    /// let elapsed = LoadStepElapsed::new(Duration::from_millis(800));
    /// let wait_time_to_deadline = pacer.remaining(elapsed);
    ///
    ///
    /// assert_eq!(wait_time_to_deadline.as_duration(), Duration::from_millis(200));
    /// ```
    pub fn remaining(&self, now: LoadStepElapsed) -> WaitTime {
        let remaining = self.duration.remaining_after(now);
        WaitTime::new(remaining.as_duration())
    }

    /// Checks if the remaining time is too short to allow normal batch processing.
    ///
    /// # Arguments
    /// * `duration_remaining`: The remaining duration for the load interval.
    ///
    /// # Returns
    /// `true` if the remaining time is less than the effective mean wait time, `false` otherwise.
    fn is_final_batch_time(&self, duration_remaining: LoadStepRemaining) -> bool {
        duration_remaining < self.effective_mean_wait_time
    }

    /// Calculates the target pace wait time to meet the target RPS for the interval duration.
    ///
    /// This is the target time per request to achieve the overall request count evenly over
    /// the step duration.
    ///
    /// # Arguments
    /// * `duration`: The duration for the load interval.
    /// * `request_count`: The target request count for the load interval.
    ///
    /// # Returns
    /// The calculated target pace wait time.
    fn target_pace_wait_time(duration: LoadStepDuration, request_count: u32) -> WaitTime {
        WaitTime::new(duration.as_duration() / (request_count + 1))
    }

    /// Determines the effective mean wait time for exponential sampling.
    ///
    /// Uses the *slower* of the default mean wait time and the target pace wait time.
    ///
    /// # Arguments
    /// * `default_mean_wait_time`: The default mean wait time calculated for the step.
    /// * `target_pace_wait_time`: The target pace wait time calculated for the step duration.
    ///
    /// # Returns
    /// The effective mean wait time to use for sampling.
    fn effective_mean_wait_time(
        default_mean_wait_time: WaitTime,
        target_pace_wait_time: WaitTime,
    ) -> WaitTime {
        default_mean_wait_time.max(target_pace_wait_time)
    }

    /// Samples the next wait time from the exponential distribution.
    ///
    /// The sample is clamped using `EXP_SAMPLING_LOWER_CLAMP_FACTOR` and
    /// `EXP_SAMPLING_UPPER_CLAMP_FACTOR`.
    ///
    /// # Arguments
    /// * `mean_wait_time`: The mean wait time to use for the exponential distribution.
    ///
    /// # Returns
    /// The sampled and clamped wait time.
    fn sample_wait_time(&mut self, mean_wait_time: WaitTime) -> WaitTime {
        let wait_time_secs_f64 = self.sampler.next_sample(
            mean_wait_time.as_secs_f64(),
            Self::EXP_SAMPLING_LOWER_CLAMP_FACTOR,
            Self::EXP_SAMPLING_UPPER_CLAMP_FACTOR,
        );

        WaitTime::from_secs_f64(wait_time_secs_f64)
    }

    /// Calculates the wait time before sending the next batch.
    ///
    /// Samples from an exponential distribution (clamped) using the effective mean wait time as
    /// the mean and ensures the wait time does not exceed the safe limit to meet the deadline.
    ///
    /// # Arguments
    /// * `duration_remaining`: The remaining duration for the load step.
    ///
    /// # Returns
    /// The calculated wait time.
    fn next_wait_time(&mut self, duration_remaining: LoadStepRemaining) -> WaitTime {
        let sampled_wait_time = self.sample_wait_time(self.effective_mean_wait_time);

        // Ensure we don't wait so long that we don't have (at least the default amount of)
        // time left to send the subsequent batch.
        let max_wait_time = duration_remaining.max_safe_wait_time(self.effective_mean_wait_time);

        sampled_wait_time.min(max_wait_time)
    }

    /// Checks if the calculated wait time is below the minimum threshold.
    ///
    /// A wait time that is effectively zero can cause issues in subsequent calculations
    /// or indicate that time is running out.
    ///
    /// # Arguments
    /// * `wait_time`: The calculated wait time to check.
    ///
    /// # Returns
    /// `true` if the wait time is below the minimum threshold, `false` otherwise.
    fn is_wait_time_negligible(&self, wait_time: WaitTime) -> bool {
        wait_time < Self::MINIMUM_CALCULATED_WAIT_TIME
    }

    /// Calculates the size of the next batch.
    ///
    /// The size is based on the remaining requests, time, and the target rate
    /// derived from the effective mean wait time.
    ///
    /// # Arguments
    /// * `duration_remaining`: The remaining duration for the load step.
    ///
    /// # Returns
    /// The calculated batch size.
    fn next_batch_size(&self, duration_remaining: LoadStepRemaining) -> u32 {
        let secs_f64 = self.effective_mean_wait_time.as_secs_f64();

        if secs_f64 == 0.0 || !secs_f64.is_finite() {
            // This case should not be possible, but just as a safeguard to prevent infinite loops
            return self.requests_remaining;
        }

        let batches_remaining = duration_remaining.as_secs_f64() / secs_f64;

        // Prevent NaN or infinity in batches_remaining
        if !batches_remaining.is_finite() || batches_remaining <= 0.0 {
            return self.requests_remaining;
        }

        let batch_size = (f64::from(self.requests_remaining) / batches_remaining).ceil() as u32;

        // Ensure batch size doesn't exceed remaining requests.
        batch_size.min(self.requests_remaining)
    }

    /// Checks if the calculated batch size is invalid (i.e., zero).
    ///
    /// # Arguments
    /// * `batch_size`: The calculated batch size to check.
    ///
    /// # Returns
    /// `true` if the batch size is zero, `false` otherwise.
    fn is_batch_size_zero(&self, batch_size: u32) -> bool {
        batch_size == 0
    }

    /// Creates a batch of requests with a specified size.
    ///
    /// # Arguments
    /// * `size`: The number of requests to include in the batch.
    ///
    /// # Returns
    /// A new [`crate::transaction::Batch`] instance.
    fn new_batch(&mut self, size: u32) -> crate::transaction::Batch {
        self.requests_remaining = self.requests_remaining.saturating_sub(size);
        crate::transaction::Batch::new(size)
    }

    /// Creates the final batch containing all remaining requests.
    ///
    /// Consumes all remaining requests and sets the internal counter to zero.
    /// It signals the end of the interval by pairing the batch with [`WaitTime::ZERO`].
    ///
    /// # Returns
    /// A tuple containing the final batch and a zero wait time.
    fn final_batch(&mut self) -> (crate::transaction::Batch, WaitTime) {
        (self.new_batch(self.requests_remaining), WaitTime::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::LoadInterval;
    use crate::math::exponential::{self, DefaultExponentialSampler};
    use std::time::Duration;

    pub struct MockSampler {
        value: f64,
    }

    impl MockSampler {
        fn new(value: f64) -> Self {
            Self { value }
        }
    }

    impl exponential::ClampedExponentialSampler for MockSampler {
        fn next_sample(&mut self, _mean: f64, _lower_factor: f64, _upper_factor: f64) -> f64 {
            self.value
        }
    }

    fn make_step(deadline_ms: u64, requests: u32) -> LoadInterval {
        LoadInterval::new(
            LoadStepDuration::new(Duration::from_millis(deadline_ms)),
            requests,
        )
    }

    fn make_elapsed_from_ms(ms: u64) -> LoadStepElapsed {
        LoadStepElapsed::new(Duration::from_millis(ms))
    }

    #[test]
    fn test_new_valid_interval() {
        let current = make_step(1000, 50);
        // should not panic
        let _ = LoadIntervalPacer::new(current, DefaultExponentialSampler::default());
    }

    #[test]
    fn test_next_no_requests() {
        let current = make_step(1000, 0);
        let mut pacer =
            LoadIntervalPacer::new(current, DefaultExponentialSampler::seed_from_u64(12345));

        let elapsed = make_elapsed_from_ms(100);
        let result = pacer.next(elapsed);

        assert!(result.is_none());
    }

    #[test]
    fn test_next_final_batch_due_to_time_remaining() {
        let current = make_step(1000, 100);
        let mut pacer = LoadIntervalPacer::new(current, DefaultExponentialSampler::default());

        let effective_wait_ms = pacer.effective_mean_wait_time.as_duration().as_millis() as u64;
        let threshold_ms = 1000 - effective_wait_ms;
        // Elapse time just *after* the threshold
        let elapsed = make_elapsed_from_ms(threshold_ms + 1);
        let (batch, wait_time) = pacer.next(elapsed).expect("Should return a final batch");

        assert_eq!(batch.size(), 100);
        assert_eq!(wait_time, WaitTime::ZERO);

        let result2 = pacer.next(make_elapsed_from_ms(threshold_ms + 2));
        assert!(result2.is_none());
    }

    #[test]
    fn test_next_final_batch_due_to_small_wait_time() {
        let current = make_step(5000, 30);
        let min_wait_time_secs_f64 =
            LoadIntervalPacer::<()>::MINIMUM_CALCULATED_WAIT_TIME.as_secs_f64();

        let tiny_wait_time_secs_f64 = 0.5 * min_wait_time_secs_f64;

        let sampler = MockSampler::new(tiny_wait_time_secs_f64);

        let mut pacer = LoadIntervalPacer::new(current, sampler);
        let elapsed = make_elapsed_from_ms(100);

        let (batch, wait_time) = pacer.next(elapsed).expect("Should return final batch");
        assert_eq!(batch.size(), 30,);
        assert_eq!(wait_time, WaitTime::ZERO,);
        // Ensure iterator finishes
        let result2 = pacer.next(make_elapsed_from_ms(101));
        assert!(result2.is_none());
    }

    #[test]
    fn test_next_single_request() {
        let current = make_step(5000, 1);
        let mut pacer =
            LoadIntervalPacer::new(current, DefaultExponentialSampler::seed_from_u64(11111));

        let elapsed = make_elapsed_from_ms(100);
        let result = pacer.next(elapsed);

        let (batch, wait_time) = result.expect("Should return a batch");
        assert_eq!(batch.size(), 1);
        assert!(wait_time.as_secs_f64() > 0.0);

        let result2 = pacer.next(make_elapsed_from_ms(200));
        assert!(result2.is_none());
    }

    fn test_next_multiple_batches_step(current: LoadInterval) {
        let request_count = current.request_count();
        let mut pacer =
            LoadIntervalPacer::new(current, DefaultExponentialSampler::seed_from_u64(33333));

        let elapsed_ms = 0u64;
        let mut total_requests_sent = 0u32;
        let mut batch_count = 0_u32;

        let base_target_batches =
            LoadIntervalPacer::<()>::BASE_BATCHES_PER_INTERVAL.min(request_count);
        let max_rate_batches = (pacer.duration.as_secs_f64()
            / LoadIntervalPacer::<()>::MAX_DEFAULT_WAIT_TIME.as_secs_f64())
        .ceil() as u32;
        let expected_batches = base_target_batches.max(max_rate_batches);
        // Loop until no more batches or a reasonable limit to prevent infinite loops in buggy code
        let mut elapsed = make_elapsed_from_ms(elapsed_ms);
        for _ in 0..expected_batches * 2 {
            let result = pacer.next(elapsed);

            match result {
                Some((batch, wait_time)) => {
                    batch_count += 1;
                    let batch_size = batch.size();
                    total_requests_sent += batch_size;

                    elapsed = LoadStepElapsed::new(elapsed.as_duration() + wait_time.as_duration());
                }
                None => {
                    break;
                }
            }
        }

        assert_eq!(total_requests_sent, request_count);
        let tolerance = (0.2 * expected_batches as f64).ceil() as u32;
        assert!(batch_count.abs_diff(expected_batches) <= tolerance,);
    }

    #[test]
    fn test_next_multiple_batches() {
        let step_durations = [50, 100, 500, 1000, 2000, 5000, 10000];
        let request_counts = [10, 50, 100, 1000, 2000];

        for (duration_ms, request_count) in step_durations.into_iter().zip(request_counts) {
            let current = make_step(duration_ms, request_count);
            test_next_multiple_batches_step(current);
        }
    }

    #[test]
    fn test_next_clamping_buffer() {
        let current = make_step(10_000, 50);
        let mut pacer =
            LoadIntervalPacer::new(current, DefaultExponentialSampler::seed_from_u64(44444));

        let elapsed = make_elapsed_from_ms(9975); // 25ms remaining

        let result = pacer.next(elapsed);
        assert!(result.is_some());
    }

    #[test]
    fn test_next_zero_remaining_duration() {
        let current = make_step(1000, 20);
        let mut pacer =
            LoadIntervalPacer::new(current, DefaultExponentialSampler::seed_from_u64(77777));

        let elapsed = make_elapsed_from_ms(1000);
        let result = pacer.next(elapsed);

        let (batch, wait_time) = result.expect("Should return the final batch");
        assert_eq!(batch.size(), 20);
        assert_eq!(wait_time, WaitTime::ZERO);
    }

    #[test]
    fn test_remaining_at_start() {
        let current = make_step(10_000, 50);
        let pacer = LoadIntervalPacer::new(current, DefaultExponentialSampler::default());

        let elapsed_at_start = make_elapsed_from_ms(0);
        let wait_time = pacer.remaining(elapsed_at_start);

        let expected_wait = WaitTime::new(Duration::from_millis(10_000));
        assert_eq!(wait_time, expected_wait);
    }

    #[test]
    fn test_remaining_mid_interval() {
        let current = make_step(10_000, 50);
        let pacer = LoadIntervalPacer::new(current, DefaultExponentialSampler::default());

        let elapsed_mid = make_elapsed_from_ms(4_000);
        let wait_time = pacer.remaining(elapsed_mid);

        let expected_wait = WaitTime::new(Duration::from_millis(6_000));
        assert_eq!(wait_time, expected_wait);
    }

    #[test]
    fn test_remaining_near_end() {
        let current = make_step(10_000, 50);
        let pacer = LoadIntervalPacer::new(current, DefaultExponentialSampler::default());

        let elapsed_near_end = make_elapsed_from_ms(9_995);
        let wait_time = pacer.remaining(elapsed_near_end);

        let expected_wait = WaitTime::new(Duration::from_millis(5));
        assert_eq!(wait_time, expected_wait);
    }

    #[test]
    fn test_remaining_at_deadline() {
        let current = make_step(5_000, 10);
        let pacer = LoadIntervalPacer::new(current, DefaultExponentialSampler::default());

        let elapsed_at_deadline = make_elapsed_from_ms(5_000);
        let wait_time = pacer.remaining(elapsed_at_deadline);

        assert_eq!(wait_time, WaitTime::ZERO);
    }

    #[test]
    fn test_remaining_past_deadline() {
        let current = make_step(3_000, 5);
        let pacer = LoadIntervalPacer::new(current, DefaultExponentialSampler::default());

        let elapsed_past_deadline = make_elapsed_from_ms(3_500);
        let wait_time = pacer.remaining(elapsed_past_deadline);

        assert_eq!(wait_time, WaitTime::ZERO);
    }

    #[test]
    fn test_integration_next_then_remaining() {
        let current = make_step(1_000, 5);
        let mut pacer =
            LoadIntervalPacer::new(current, DefaultExponentialSampler::seed_from_u64(98765));

        let mut elapsed_ms = 0u64;
        let mut total_requests_sent = 0;

        // Generate all batches
        let elapsed = make_elapsed_from_ms(elapsed_ms);
        while let Some((batch, wait_time)) = pacer.next(elapsed) {
            total_requests_sent += batch.size();
            elapsed_ms += wait_time.as_duration().as_millis() as u64;
        }
        assert_eq!(total_requests_sent, 5);

        let final_elapsed = make_elapsed_from_ms(elapsed_ms);
        let final_wait_time = pacer.remaining(final_elapsed);

        let expected_final_wait_duration =
            Duration::from_millis(1_000).saturating_sub(Duration::from_millis(elapsed_ms));
        let expected_final_wait = WaitTime::new(expected_final_wait_duration);

        assert_eq!(final_wait_time, expected_final_wait);
    }
}
