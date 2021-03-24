use std::fmt::Debug;

pub trait EnergyMeter: Debug {
    /// Current current draw.
    fn current_draw(&self) -> u32;
}
