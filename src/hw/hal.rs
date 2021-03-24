use std::fmt::Debug;

pub trait ADC: Debug {
    /// Retrieve an ADC channel.
    fn get_channel(&self, channel_no: u8) -> ADCChannel;
}

#[derive(Debug)]
pub struct ADCChannel<'a> {
    adc: &'a dyn ADC,
    channel: u8,
}

impl<'a> ADCChannel<'a> {
    pub fn new(adc: &'a dyn ADC, channel: u8) -> ADCChannel<'a> {
        ADCChannel {
            adc,
            channel,
        }
    }

    fn sample(&self) -> u32 {
        0
    }
}
