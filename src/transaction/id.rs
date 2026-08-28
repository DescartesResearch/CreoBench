use core::fmt;

/// A unique identifier for a load spec.
///
/// `SpecId` is a thin newtype around `u16`. It identifies a load spec —
/// a named, reusable workload definition — within a single load test run.
///
/// # Examples
/// ```
/// use creo_bench::transaction::SpecId;
///
/// let id = SpecId::new(10);
/// assert_eq!(id.get(), 10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SpecId(usize);

impl fmt::Display for SpecId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SpecId {
    /// Creates a new `SpecId` from a raw `u16` value.
    ///
    /// # Arguments
    ///
    /// * `value` - The identifier value to wrap.
    ///
    /// # Returns
    ///
    /// A new `SpecId` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use creo_bench::transaction::SpecId;
    ///
    /// let id = SpecId::new(3);
    /// ```
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the underlying `u16` value of this `SpecId`.
    ///
    /// # Returns
    ///
    /// The wrapped `u16` value.
    ///
    /// # Examples
    ///
    /// ```
    /// use creo_bench::transaction::SpecId;
    ///
    /// let id = SpecId::new(10);
    /// assert_eq!(id.get(), 10);
    /// ```
    pub const fn get(self) -> usize {
        self.0
    }
}

impl PartialEq<usize> for SpecId {
    fn eq(&self, other: &usize) -> bool {
        self.0 == *other
    }
}

/// A unique identifier for a load generator process within a load test run.
///
/// `LoadGeneratorId` is a thin newtype around `u8`. It identifies the
/// generator process that actually issued a given request and, together
/// with a [`VirtualUserId`][`crate::virtual_user::VirtualUserId`], forms a
/// globally unique request identifier.
///
/// # Examples
/// ```
/// use creo_bench::transaction::LoadGeneratorId;
///
/// let id = LoadGeneratorId::new(3);
/// assert_eq!(id.get(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LoadGeneratorId(u8);

impl LoadGeneratorId {
    /// Creates a new `LoadGeneratorId` from a raw `u8` value.
    ///
    /// # Arguments
    ///
    /// * `value` - The identifier value to wrap.
    ///
    /// # Returns
    ///
    /// A new `LoadGeneratorId` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use creo_bench::transaction::LoadGeneratorId;
    ///
    /// let id = LoadGeneratorId::new(1);
    /// ```
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Returns the underlying `u8` value of this `LoadGeneratorId`.
    ///
    /// # Returns
    ///
    /// The wrapped `u8` value.
    ///
    /// # Examples
    ///
    /// ```
    /// use creo_bench::transaction::LoadGeneratorId;
    ///
    /// let id = LoadGeneratorId::new(3);
    /// assert_eq!(id.get(), 3);
    /// ```
    pub const fn get(self) -> u8 {
        self.0
    }
}
