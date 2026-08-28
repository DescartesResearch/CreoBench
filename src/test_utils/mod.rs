#[cfg(test)]
use bytes::BytesMut;

#[cfg(test)]
use crate::net::MessageFramer;
#[cfg(test)]
use serde::de::DeserializeOwned;
#[cfg(test)]
use tokio_util::codec::{Decoder, Encoder};

mod profile;
mod registry;
mod script;
mod warmup;

#[cfg(test)]
mod mock_client;
#[cfg(test)]
mod pool;
#[cfg(test)]
mod scenario;

pub mod prelude {
    pub use crate::test_utils::profile::ProfileBuilder;
    pub use crate::test_utils::registry::{DEFAULT_TEST_URL, RegistryBuilder};
    pub use crate::test_utils::script::{
        DEFAULT_TEST_METHOD, DEFAULT_TEST_PATH, DEFAULT_TEST_SERVICE, RequestBuilder,
        ScriptBuilder, Static,
    };
    pub use crate::test_utils::warmup::WarmupBuilder;

    #[cfg(test)]
    pub use crate::test_utils::mock_client::{MockClientBuilder, MockHttpClient, MockResponse};
    #[cfg(test)]
    pub use crate::test_utils::pool::PoolBuilder;
    #[cfg(test)]
    pub use crate::test_utils::scenario::{Scenario, ScenarioBuilder};
}

#[cfg(test)]
pub(crate) fn assert_round_trips_single<T>(value: T)
where
    T: serde::Serialize + DeserializeOwned + std::fmt::Debug + PartialEq + Clone,
{
    let mut buf = BytesMut::new();
    let mut framer = MessageFramer::<T, T>::new();
    framer.encode(value.clone(), &mut buf).unwrap();

    let decoded = framer.decode(&mut buf).unwrap().unwrap();

    assert_eq!(value, decoded);

    assert!(buf.is_empty());
}

#[cfg(test)]
pub(crate) fn assert_round_trips_multi<T>(values: &[T])
where
    T: serde::Serialize + DeserializeOwned + std::fmt::Debug + PartialEq + Clone,
{
    let mut buf = BytesMut::new();
    let mut framer = MessageFramer::<T, T>::new();

    for v in values {
        framer.encode(v.clone(), &mut buf).unwrap();
    }

    for v in values {
        let decoded = framer.decode(&mut buf).unwrap().unwrap();

        assert_eq!(*v, decoded);
    }

    assert!(buf.is_empty());
}

#[cfg(test)]
pub(crate) fn assert_round_trips_stream<T>(value: T)
where
    T: serde::Serialize + DeserializeOwned + std::fmt::Debug + PartialEq + Clone,
{
    let mut framer = MessageFramer::<T, T>::new();

    let mut encoded = BytesMut::new();
    framer.encode(value.clone(), &mut encoded).unwrap();

    // simulate network chunks
    let mut buf = BytesMut::new();
    let mut decoded = None;

    for chunk in encoded.chunks(3) {
        buf.extend_from_slice(chunk);

        if let Ok(Some(v)) = framer.decode(&mut buf) {
            decoded = Some(v);
        }
    }

    assert_eq!(decoded.unwrap(), value);
    assert!(buf.is_empty());
}

#[cfg(test)]
macro_rules! round_trip_proptest {
    ($ty:ty, $single:ident, $multi:ident, $stream: ident, ) => {
        ::proptest::proptest! {
            #[test]
            fn $single(value: $ty) {
                crate::test_utils::assert_round_trips_single(value);
            }

            #[test]
            fn $multi(values: Vec<$ty>) {
                crate::test_utils::assert_round_trips_multi(&values);
            }

            #[test]
            fn $stream(value: $ty) {
                crate::test_utils::assert_round_trips_stream(value);
            }
        }
    };
}

#[cfg(test)]
macro_rules! proptest_strategy {
    (
        $name:ident : $ty:ty {
            $($field:ident : $field_strategy:expr),* $(,)?
        }
    ) => {
        fn $name() -> impl proptest::strategy::Strategy<Value = $ty> {
            use proptest::prelude::*;

            (
                $($field_strategy,)*
            )
            .prop_map(|($($field,)*)| {
                $ty {
                    $($field,)*
                }
            })
        }
    };

    (
        $name:ident : $ty:ty => $body:expr
    ) => {
        fn $name() -> impl proptest::strategy::Strategy<Value = $ty> {
            $body
        }
    };
}

#[cfg(test)]
macro_rules! impl_proptest_arbitrary {
    ($ty:ty, $strategy_fn:ident) => {
        impl ::proptest::arbitrary::Arbitrary for $ty {
            type Parameters = ();
            type Strategy = ::proptest::strategy::BoxedStrategy<$ty>;

            fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
                $strategy_fn().boxed()
            }
        }
    };
}

#[cfg(test)]
pub(crate) use impl_proptest_arbitrary;
#[cfg(test)]
pub(crate) use proptest_strategy;
#[cfg(test)]
pub(crate) use round_trip_proptest;
