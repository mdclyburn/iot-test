use std::fmt::Debug;

pub trait EnergyMetering: Debug + Send {
    /// Current current draw.
    fn current_draw(&self) -> u32;
}
