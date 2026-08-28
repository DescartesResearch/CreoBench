/// Errors that can occur when calling [`linspace`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LinspaceError {
    /// The number of points must be at least 2.
    #[error("`num` must be at least 2, but was `{0}`")]
    InvalidNumArgument(usize),

    /// The start value must not be greater than the end value.
    #[error("`start` must be less than or equal to `end`, but was `{start}` to `{end}`")]
    InvalidRangeArgument { start: usize, end: usize },
}

/// Generates `num` evenly spaced integer values from `start` to `end`, inclusive.
///
/// This is a pure integer version of a `linspace`, and ensures:
/// - The first value is `start`,
/// - The last value is `end`,
/// - The intermediate values are as evenly spaced as possible,
///
/// # Arguments
/// - `start`: The starting value (inclusive).
/// - `end`: The ending value (inclusive).
/// - `num`: The number of values to generate (must be at least 2).
///
/// # Returns
/// A `Vec<usize>` of `num` values from `start` to `end`.
///
/// # Errors
/// - [`LinspaceError::InvalidNumArgument`] if `num < 2`.
/// - [`LinspaceError::InvalidRangeArgument`] if `start > end`.
///
/// # Examples
/// ```
/// # use creo_bench::math::linspace::linspace;
///
/// let values = linspace(0, 10, 3).unwrap();
/// assert_eq!(values, vec![0, 5, 10]);
/// ```
pub fn linspace(start: usize, end: usize, num: usize) -> Result<Vec<usize>, LinspaceError> {
    if num < 2 {
        return Err(LinspaceError::InvalidNumArgument(num));
    }

    if start > end {
        return Err(LinspaceError::InvalidRangeArgument { start, end });
    }
    let mut values = Vec::with_capacity(num);

    let delta = end - start;
    let n = num - 1;
    let base_increase = delta / n;

    let extra_increase = delta % n;

    let mut current = start;
    for i in 0..(num - 1) {
        values.push(current);
        let increase = if i < extra_increase {
            base_increase + 1
        } else {
            base_increase
        };
        current += increase;
    }
    values.push(end);

    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linspace_basic() {
        let values = linspace(0, 10, 6).unwrap();
        assert_eq!(values, vec![0, 2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_linspace_exact_division() {
        let values = linspace(10, 20, 6).unwrap();
        assert_eq!(values, vec![10, 12, 14, 16, 18, 20]);
    }

    #[test]
    fn test_linspace_with_remainder_distribution() {
        let values = linspace(3, 10, 4).unwrap();
        // delta = 7, steps = 3 => base = 2, remainder = 1
        assert_eq!(values, vec![3, 6, 8, 10]);
    }

    #[test]
    fn test_linspace_insufficient_range_for_num() {
        let values = linspace(0, 1, 4).unwrap();
        assert_eq!(values, vec![0, 1, 1, 1]);
    }

    #[test]
    fn test_linspace_consecutive_integers() {
        let values = linspace(1, 4, 4).unwrap();
        assert_eq!(values, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_linspace_more_points_than_range() {
        let values = linspace(10, 12, 6).unwrap();
        assert_eq!(values, vec![10, 11, 12, 12, 12, 12]);
    }

    #[test]
    fn test_linspace_start_equals_end_num_min() {
        let values = linspace(5, 5, 2).unwrap();
        assert_eq!(values, vec![5, 5]);
    }

    #[test]
    fn test_linspace_start_equals_end() {
        let values = linspace(5, 5, 3).unwrap();
        assert_eq!(values, vec![5, 5, 5]);
    }

    #[test]
    fn test_linspace_invalid_num_argument() {
        let err = linspace(0, 10, 1).unwrap_err();
        assert_eq!(err, LinspaceError::InvalidNumArgument(1));
    }

    #[test]
    fn test_linspace_invalid_range_argument() {
        let err = linspace(10, 5, 4).unwrap_err();
        assert_eq!(
            err,
            LinspaceError::InvalidRangeArgument { start: 10, end: 5 }
        )
    }

    #[test]
    fn test_linspace_min_num() {
        let values = linspace(0, 5, 2).unwrap();
        assert_eq!(values, vec![0, 5]);
    }

    #[test]
    fn test_linspace_large_range_small_num_exact_division() {
        let values = linspace(0, 100, 3).unwrap();
        assert_eq!(values, vec![0, 50, 100]);
    }

    #[test]
    fn test_linspace_large_range_small_num_with_remainder() {
        let values = linspace(0, 109, 3).unwrap();
        assert_eq!(values, vec![0, 55, 109]);
    }
}
