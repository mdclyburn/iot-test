use std::fmt::Debug;

pub trait ADC: Debug + Send {
    /// Retrieve an ADC channel.
    fn get_channel(&self, channel_no: u8) -> ADCChannel;

    /// Sample a channel's analog signal.
    fn sample(&self, channel_no: u8) -> u32;
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
        self.adc.sample(self.channel)
    }
}
