#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Command {
    Start,
    Abort,
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::test_utils::{impl_proptest_arbitrary, proptest_strategy, round_trip_proptest};

    proptest_strategy! {
        command_strategy: Command => {
            prop_oneof![
                Just(Command::Start),
                Just(Command::Abort),
            ]
        }
    }

    impl_proptest_arbitrary!(Command, command_strategy);

    round_trip_proptest! {
        Command,
        command_round_trip_single,
        command_round_trip_multi,
        command_round_trip_stream,
    }
}
