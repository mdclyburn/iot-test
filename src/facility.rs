use std::fmt::Debug;

pub trait EnergyMetering: Debug {
    /// Current current draw.
    fn current_draw(&self) -> u32;
}
