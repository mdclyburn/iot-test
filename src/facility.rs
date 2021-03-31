use std::fmt::Debug;

/// Trait providing insight into electrical power data.
pub trait EnergyMetering: Debug + Send {
    /// Returns a current reading.
    fn current(&self) -> f32;
}
