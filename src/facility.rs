use std::fmt::Debug;
use std::time::Duration;

/// Trait providing insight into electrical power data.
pub trait EnergyMetering: Debug + Send {
    /// Returns a current reading in milliamps.
    fn current(&self) -> f32;

    /// Returns a power draw reading in milliwatts.
    fn power(&self) -> f32;

    /// Returns the minimum amount of time a caller needs to wait before new data is ready.
    fn cooldown_duration(&self) -> Duration {
        Duration::from_millis(0)
    }
}
