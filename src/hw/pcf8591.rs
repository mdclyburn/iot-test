use rppal::i2c::I2c;

/// Driver for the Adafruit PCF8591 ADC/DAC board.
#[derive(Debug)]
pub struct PCF8591 {
    i2c: I2c
}

impl PCF8591 {
    /// Create a new instance of the driver.
    pub fn new(i2c: I2c) -> PCF8591 {
        PCF8591 {
            i2c,
        }
    }
}
