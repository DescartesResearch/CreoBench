use std::time::Duration;

use crate::math::exponential::sample_exponential;
use crate::wire::{LoadProfile, Warmup};

/// A load step with a constant arrival rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScheduledStep {
    /// Index of the step in its phase.
    pub index: usize,
    /// End of the step, relative to the start of the phase.
    pub deadline: Duration,
    /// Arrival rate in requests per second. A rate of zero yields no arrivals.
    pub rate: f64,
}

impl ScheduledStep {
    /// Converts a load profile into steps with constant arrival rates.
    ///
    /// The rate of a step is its request count divided by the time between
    /// the previous deadline (or zero for the first step) and its own deadline.
    /// Steps without a positive duration get a rate of zero.
    pub fn from_profile(profile: &LoadProfile) -> Vec<Self> {
        let mut previous = Duration::ZERO;
        profile
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let deadline = Duration::from_secs_f64(step.deadline.as_f64());
                let rate = Self::rate(step.count, deadline.saturating_sub(previous));
                previous = previous.max(deadline);
                Self {
                    index,
                    deadline,
                    rate,
                }
            })
            .collect()
    }

    /// Converts a warmup into one-second steps at the warmup rate.
    pub fn from_warmup(warmup: &Warmup) -> Vec<Self> {
        (0..warmup.duration)
            .map(|index| Self {
                index: index as usize,
                deadline: Duration::from_secs(u64::from(index) + 1),
                rate: f64::from(warmup.rate),
            })
            .collect()
    }

    fn rate(count: u32, duration: Duration) -> f64 {
        if count == 0 || duration.is_zero() {
            return 0.0;
        }
        f64::from(count) / duration.as_secs_f64()
    }
}

/// A single scheduled arrival.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrival {
    /// Scheduled arrival time, relative to the start of the phase.
    pub offset: Duration,
    /// Index of the step this arrival belongs to.
    pub step: usize,
}

/// Arrival times of a Poisson process with a piecewise-constant rate.
///
/// Within each [`ScheduledStep`], inter-arrival times are drawn from an
/// exponential distribution with the step's rate. Inter-arrival times are
/// never negative, so arrivals are non-decreasing. When the next arrival
/// would fall on or after the step's deadline, the schedule restarts at the
/// deadline with the rate of the next step. Since the exponential distribution
/// is memoryless, this yields an exact Poisson process, and the number of
/// arrivals per step is Poisson-distributed with mean `rate * duration`.
///
/// The schedule does not depend on wall-clock time, so it is fully
/// determined by its steps and the state of its random number generator.
pub struct ArrivalSchedule<R> {
    steps: std::vec::IntoIter<ScheduledStep>,
    current: Option<ScheduledStep>,
    clock: Duration,
    rng: R,
}

impl<R: rand::Rng> ArrivalSchedule<R> {
    pub fn new(steps: Vec<ScheduledStep>, rng: R) -> Self {
        let mut steps = steps.into_iter();
        let current = steps.next();
        Self {
            steps,
            current,
            clock: Duration::ZERO,
            rng,
        }
    }
}

impl<R: rand::Rng> Iterator for ArrivalSchedule<R> {
    type Item = Arrival;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let step = self.current?;
            if step.rate > 0.0 {
                // Always positive and finite, see `sample_exponential`.
                let gap = sample_exponential(1.0 / step.rate, &mut self.rng);
                let candidate = self.clock + Duration::from_secs_f64(gap);
                if candidate < step.deadline {
                    self.clock = candidate;
                    return Some(Arrival {
                        offset: candidate,
                        step: step.index,
                    });
                }
            }
            self.clock = self.clock.max(step.deadline);
            self.current = self.steps.next();
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;
    use crate::wire::{LoadStep, LoadStepDeadline};

    fn step(index: usize, deadline_secs: f64, rate: f64) -> ScheduledStep {
        ScheduledStep {
            index,
            deadline: Duration::from_secs_f64(deadline_secs),
            rate,
        }
    }

    fn schedule(steps: Vec<ScheduledStep>, seed: u64) -> ArrivalSchedule<StdRng> {
        ArrivalSchedule::new(steps, StdRng::seed_from_u64(seed))
    }

    fn gaps_in_secs(arrivals: &[Arrival]) -> Vec<f64> {
        let mut previous = Duration::ZERO;
        arrivals
            .iter()
            .map(|arrival| {
                let gap = arrival.offset - previous;
                previous = arrival.offset;
                gap.as_secs_f64()
            })
            .collect()
    }

    fn mean(values: &[f64]) -> f64 {
        values.iter().sum::<f64>() / values.len() as f64
    }

    fn variance(values: &[f64]) -> f64 {
        let mean = mean(values);
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64
    }

    /// Kolmogorov-Smirnov statistic of `samples` against Exp(`rate`).
    fn ks_statistic_exponential(samples: &[f64], rate: f64) -> f64 {
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let n = sorted.len() as f64;
        sorted
            .iter()
            .enumerate()
            .map(|(i, x)| {
                let cdf = 1.0 - (-rate * x).exp();
                let above = (i as f64 + 1.0) / n - cdf;
                let below = cdf - i as f64 / n;
                above.max(below)
            })
            .fold(0.0, f64::max)
    }

    /// Critical value of the Kolmogorov-Smirnov test for significance level 0.01.
    fn ks_critical_value(n: usize) -> f64 {
        1.63 / (n as f64).sqrt()
    }

    #[test]
    fn empty_schedule_yields_no_arrivals() {
        assert_eq!(schedule(vec![], 1).next(), None);
    }

    #[test]
    fn zero_rate_step_yields_no_arrivals() {
        assert_eq!(schedule(vec![step(0, 10.0, 0.0)], 1).next(), None);
    }

    #[test]
    fn zero_rate_step_is_skipped() {
        let arrivals: Vec<_> = schedule(vec![step(0, 1.0, 0.0), step(1, 2.0, 1000.0)], 1).collect();

        assert!(!arrivals.is_empty());
        for arrival in arrivals {
            assert_eq!(arrival.step, 1);
            assert!(arrival.offset >= Duration::from_secs(1));
            assert!(arrival.offset < Duration::from_secs(2));
        }
    }

    #[test]
    fn same_seed_yields_same_schedule() {
        let steps = vec![step(0, 1.0, 5000.0), step(1, 2.0, 100.0)];
        let first: Vec<_> = schedule(steps.clone(), 7).collect();
        let second: Vec<_> = schedule(steps, 7).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn different_seeds_yield_different_schedules() {
        let steps = vec![step(0, 1.0, 5000.0)];
        let first: Vec<_> = schedule(steps.clone(), 7).collect();
        let second: Vec<_> = schedule(steps, 8).collect();
        assert_ne!(first, second);
    }

    proptest! {
        #[test]
        fn arrivals_are_non_decreasing_and_within_their_step(
            seed in any::<u64>(),
            rates in prop::collection::vec(0.0f64..20_000.0, 1..5),
            durations in prop::collection::vec(0.001f64..0.5, 5),
        ) {
            let mut deadline = 0.0;
            let steps: Vec<_> = rates
                .iter()
                .zip(&durations)
                .enumerate()
                .map(|(index, (rate, duration))| {
                    deadline += duration;
                    step(index, deadline, *rate)
                })
                .collect();

            let mut previous = Duration::ZERO;
            for arrival in schedule(steps.clone(), seed) {
                let step = steps[arrival.step];
                let step_start = if arrival.step == 0 {
                    Duration::ZERO
                } else {
                    steps[arrival.step - 1].deadline
                };
                prop_assert!(arrival.offset >= previous);
                prop_assert!(arrival.offset >= step_start);
                prop_assert!(arrival.offset < step.deadline);
                previous = arrival.offset;
            }
        }
    }

    #[test]
    fn arrival_count_matches_rate() {
        let rate = 1000.0;
        let duration = 100.0;
        let count = schedule(vec![step(0, duration, rate)], 11).count() as f64;

        let expected = rate * duration;
        // Poisson standard deviation is sqrt(expected); allow five of them.
        assert!(
            (count - expected).abs() < 5.0 * expected.sqrt(),
            "expected about {expected} arrivals, got {count}"
        );
    }

    #[test]
    fn arrival_counts_follow_rate_changes() {
        let steps = vec![step(0, 10.0, 100.0), step(1, 11.0, 10_000.0)];
        let arrivals: Vec<_> = schedule(steps, 13).collect();

        let low = arrivals.iter().filter(|a| a.step == 0).count() as f64;
        let high = arrivals.iter().filter(|a| a.step == 1).count() as f64;

        assert!((low - 1000.0).abs() < 5.0 * 1000f64.sqrt(), "got {low}");
        assert!(
            (high - 10_000.0).abs() < 5.0 * 10_000f64.sqrt(),
            "got {high}"
        );
    }

    #[test]
    fn inter_arrival_times_have_unit_coefficient_of_variation() {
        let arrivals: Vec<_> = schedule(vec![step(0, 10.0, 10_000.0)], 17).collect();
        let gaps = gaps_in_secs(&arrivals);

        let cv = variance(&gaps).sqrt() / mean(&gaps);

        assert!((cv - 1.0).abs() < 0.02, "coefficient of variation was {cv}");
    }

    #[test]
    fn inter_arrival_times_are_exponentially_distributed() {
        let rate = 50_000.0;
        let arrivals: Vec<_> = schedule(vec![step(0, 2.0, rate)], 19).collect();
        let gaps = gaps_in_secs(&arrivals);

        let d = ks_statistic_exponential(&gaps, rate);

        assert!(
            d < ks_critical_value(gaps.len()),
            "KS statistic {d} exceeds critical value {}",
            ks_critical_value(gaps.len())
        );
    }

    #[test]
    fn arrival_counts_per_step_have_unit_index_of_dispersion() {
        let steps: Vec<_> = (0..1000).map(|i| step(i, (i + 1) as f64, 100.0)).collect();
        let arrivals: Vec<_> = schedule(steps, 23).collect();

        let mut counts = vec![0.0; 1000];
        for arrival in arrivals {
            counts[arrival.step] += 1.0;
        }
        let dispersion = variance(&counts) / mean(&counts);

        assert!(
            (dispersion - 1.0).abs() < 0.2,
            "index of dispersion was {dispersion}"
        );
    }

    #[test]
    fn superposition_of_independent_schedules_is_poisson() {
        let rate = 20_000.0;
        let workers = 4;
        let mut merged: Vec<_> = (0..workers)
            .flat_map(|seed| schedule(vec![step(0, 1.0, rate / workers as f64)], seed))
            .collect();
        merged.sort_by_key(|arrival| arrival.offset);
        let gaps = gaps_in_secs(&merged);

        let d = ks_statistic_exponential(&gaps, rate);

        assert!(
            d < ks_critical_value(gaps.len()),
            "KS statistic {d} exceeds critical value {}",
            ks_critical_value(gaps.len())
        );
    }

    #[test]
    fn from_profile_derives_rates_from_step_durations() {
        let profile = LoadProfile {
            steps: vec![
                LoadStep {
                    deadline: LoadStepDeadline::new(2.0),
                    count: 1000,
                },
                LoadStep {
                    deadline: LoadStepDeadline::new(2.5),
                    count: 1000,
                },
                LoadStep {
                    deadline: LoadStepDeadline::new(3.5),
                    count: 0,
                },
            ],
        };

        let steps = ScheduledStep::from_profile(&profile);

        assert_eq!(
            steps,
            vec![step(0, 2.0, 500.0), step(1, 2.5, 2000.0), step(2, 3.5, 0.0),]
        );
    }

    #[test]
    fn from_profile_assigns_zero_rate_to_steps_without_duration() {
        let profile = LoadProfile {
            steps: vec![
                LoadStep {
                    deadline: LoadStepDeadline::new(1.0),
                    count: 10,
                },
                LoadStep {
                    deadline: LoadStepDeadline::new(1.0),
                    count: 10,
                },
            ],
        };

        let steps = ScheduledStep::from_profile(&profile);

        assert_eq!(steps[1].rate, 0.0);
    }

    #[test]
    fn from_warmup_yields_one_second_steps_at_warmup_rate() {
        let warmup = Warmup {
            rate: 50,
            duration: 3,
            pause: 5,
        };

        let steps = ScheduledStep::from_warmup(&warmup);

        assert_eq!(
            steps,
            vec![step(0, 1.0, 50.0), step(1, 2.0, 50.0), step(2, 3.0, 50.0)]
        );
    }
}
