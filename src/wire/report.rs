use crate::tracker::IntervalReport;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GeneratorUpdate {
    IntervalReport(IntervalReport),
    Finished,
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::test_utils::{impl_proptest_arbitrary, proptest_strategy, round_trip_proptest};

    proptest_strategy! {
        finished_strategy: GeneratorUpdate => {
            prop_oneof![
                Just(GeneratorUpdate::Finished),
            ]
        }
    }

    impl_proptest_arbitrary!(GeneratorUpdate, finished_strategy);

    round_trip_proptest! {
        GeneratorUpdate,
        generator_update_round_trip_single,
        generator_update_round_trip_multi,
        generator_update_round_trip_stream,
    }
}
