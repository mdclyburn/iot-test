use std::fmt::{Debug, Display};

pub trait EnergyMeter: Debug + Display {
    fn current_draw(&self) -> u32;
}
