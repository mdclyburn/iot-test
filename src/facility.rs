use std::fmt::Debug;

/// Trait providing insight into electrical power data.
pub trait EnergyMetering: Debug + Send {
    /// Returns a current reading in milliamps.
    fn current(&self) -> f32;

    /// Returns a power draw reading in milliwatts.
    fn power(&self) -> f32;
}
