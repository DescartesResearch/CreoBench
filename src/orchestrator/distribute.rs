use crate::config::{LoadProfileConfig, LoadStepConfig};

pub fn distribute_count(total: u32, divisor: usize) -> Vec<u32> {
    let base = total / divisor as u32;
    let remainder = total % divisor as u32;
    (0..divisor)
        .map(|i| {
            if (i as u32) < remainder {
                base + 1
            } else {
                base
            }
        })
        .collect()
}

pub fn distribute_profile(
    profile: &LoadProfileConfig,
    num_generators: usize,
) -> Vec<LoadProfileConfig> {
    let mut generator_steps: Vec<Vec<LoadStepConfig>> =
        (0..num_generators).map(|_| Vec::new()).collect();

    for step in &profile.steps {
        let counts = distribute_count(step.count, num_generators);
        for (steps, count) in generator_steps.iter_mut().zip(counts) {
            steps.push(LoadStepConfig {
                deadline: step.deadline,
                count,
            });
        }
    }

    generator_steps
        .into_iter()
        .map(|steps| LoadProfileConfig { steps })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::config::DeadlineConfig;

    use super::*;

    #[test]
    fn distributes_evenly_when_exact_multiple() {
        let counts = distribute_count(100, 5);
        assert_eq!(counts.len(), 5);
        assert!(counts.iter().all(|&c| c == 20));
        assert_eq!(counts.iter().sum::<u32>(), 100);
    }

    #[test]
    fn distributes_remainder_across_first_generators() {
        let counts = distribute_count(103, 5);
        assert_eq!(counts.len(), 5);
        assert_eq!(counts, [21, 21, 21, 20, 20]);
        assert_eq!(counts.iter().sum::<u32>(), 103);
    }

    #[test]
    fn handles_single_generator() {
        let counts = distribute_count(50, 1);
        assert_eq!(counts, vec![50]);
    }

    #[test]
    fn sum_equals_total() {
        for total in [1, 50, 99, 100, 101, 1000] {
            for n in [1, 2, 5, 10, 100] {
                if n as u32 > total {
                    continue;
                }
                let counts = distribute_count(total, n);
                assert_eq!(counts.iter().sum::<u32>(), total);
            }
        }
    }

    #[test]
    fn output_count_matches_num_generators() {
        let profile = LoadProfileConfig {
            steps: vec![LoadStepConfig {
                deadline: DeadlineConfig::new(1.0),
                count: 100,
            }],
        };
        let profiles = distribute_profile(&profile, 3);
        assert_eq!(profiles.len(), 3);
    }

    #[test]
    fn sum_of_per_step_counts_across_profiles_equals_original() {
        let profile = LoadProfileConfig {
            steps: vec![
                LoadStepConfig {
                    deadline: DeadlineConfig::new(1.0),
                    count: 100,
                },
                LoadStepConfig {
                    deadline: DeadlineConfig::new(2.0),
                    count: 50,
                },
                LoadStepConfig {
                    deadline: DeadlineConfig::new(3.0),
                    count: 200,
                },
            ],
        };
        let profiles = distribute_profile(&profile, 4);
        for (step_idx, original_step) in profile.steps.iter().enumerate() {
            let sum: u32 = profiles.iter().map(|p| p.steps[step_idx].count).sum();
            assert_eq!(sum, original_step.count);
        }
    }

    #[test]
    fn deadlines_are_preserved() {
        let profile = LoadProfileConfig {
            steps: vec![
                LoadStepConfig {
                    deadline: DeadlineConfig::new(5.0),
                    count: 100,
                },
                LoadStepConfig {
                    deadline: DeadlineConfig::new(10.0),
                    count: 50,
                },
            ],
        };
        let profiles = distribute_profile(&profile, 3);
        for p in &profiles {
            for (step_idx, original_step) in profile.steps.iter().enumerate() {
                assert!(
                    (p.steps[step_idx].deadline.as_f64() - original_step.deadline.as_f64()).abs()
                        < f64::EPSILON
                );
            }
        }
    }

    #[test]
    fn single_generator_returns_original_profile() {
        let profile = LoadProfileConfig {
            steps: vec![
                LoadStepConfig {
                    deadline: DeadlineConfig::new(1.0),
                    count: 100,
                },
                LoadStepConfig {
                    deadline: DeadlineConfig::new(2.0),
                    count: 50,
                },
            ],
        };
        let profiles = distribute_profile(&profile, 1);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0], profile);
    }

    #[test]
    fn zero_count_steps_are_distributed() {
        let profile = LoadProfileConfig {
            steps: vec![LoadStepConfig {
                deadline: DeadlineConfig::new(1.0),
                count: 0,
            }],
        };
        let profiles = distribute_profile(&profile, 3);
        assert_eq!(profiles.len(), 3);
        for p in &profiles {
            assert_eq!(p.steps[0].count, 0);
            assert!((p.steps[0].deadline.as_f64() - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn more_generators_than_requests() {
        let profile = LoadProfileConfig {
            steps: vec![LoadStepConfig {
                deadline: DeadlineConfig::new(1.0),
                count: 3,
            }],
        };
        let profiles = distribute_profile(&profile, 5);
        assert_eq!(profiles.len(), 5);
        let sum: u32 = profiles.iter().map(|p| p.steps[0].count).sum();
        assert_eq!(sum, 3);
    }
}
