mod core;
mod sampler;

pub use core::{sample_clamped_exponential, sample_exponential};
pub use sampler::{ClampedExponentialSampler, DefaultExponentialSampler};
