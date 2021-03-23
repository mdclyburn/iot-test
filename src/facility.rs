use std::fmt::Debug;

pub trait EnergyMeter: Debug {
    fn current_draw(&self) -> u32;
}
