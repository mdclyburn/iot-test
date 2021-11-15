use std::time::Duration;

#[allow(unused_imports)]
use clockwise_common::comm::{Direction, Class as SignalClass};
#[allow(unused_imports)]
use clockwise_common::{
    criteria::{
        Criterion,
        GPIOCriterion,
        EnergyCriterion,
        EnergyStat,
        Timing,
        SerialTraceCondition,
        SerialTraceCriterion,
    },
    facility::EnergyMetering,
    hw::INA219,
    input::TestProvider,
    io,
    io::{
        Device,
        Mapping,
        DeviceInputs,
    },
    test::{
        Operation,
        Test,
    },
};

#[derive(Debug)]
pub struct SampleTestProvider {
    tests: Vec<Test>,
}

impl SampleTestProvider {
    fn new() -> SampleTestProvider {
        SampleTestProvider {
            tests: vec![
                // Test::new(
                //     "radio-packet-tx",
                //     (&["radio_send_app"]).into_iter().map(|x| *x),
                //     (&[]).into_iter().copied(),
                //     &[Operation::reset_device(),
                //       Operation::idle_testbed(Duration::from_millis(5000))],
                //     &[Criterion::Energy(EnergyCriterion::new("system-total", EnergyStat::Total)
                //                         .with_max(350.0))]),

                // Test::new(
                //     "no-app-test",
                //     (&[]).into_iter().map(|x| *x),
                //     &[Operation { time: 0, pin_no: 23, input: Signal::Digital(false) },
                //       Operation { time: 200, pin_no: 23, input: Signal::Digital(false) }],
                //     &[Criterion::Energy(EnergyCriterion::new("system", EnergyStat::Average)
                //                         .with_min(10.0))]),

                // Test::new(
                //     "blink-trace",
                //     (&[]).into_iter().copied(),
                //     (&["capsule/led/command/on", "capsule/led/command/off"]).into_iter().copied(),
                //     &[Operation { time: 0, pin_no: 23, input: Signal::Digital(false) },
                //       Operation { time: 3000, pin_no: 23, input: Signal::Digital(true) }],
                //     &[Criterion::ParallelTrace(ParallelTraceCriterion::new(&[ParallelTraceCondition::new(2).with_extra_data(1),
                //                                                              ParallelTraceCondition::new(1).with_timing(Timing::Relative(Duration::from_millis(50)),
                //                                                                                                         Duration::from_millis(5))
                //                                                              .with_extra_data(1)]))]),

                Test::new(
                    "serial-blink-trace",
                    (&[]).into_iter().copied(),
                    (&[]).into_iter().copied(),
                    &[Operation::at(0).idle_sync(Duration::from_millis(3000))],
                    &[Criterion::SerialTrace(
                        SerialTraceCriterion::new(&[
                            SerialTraceCondition::new(&[0x6c, 0x65, 0x64, 0x20, 0x6f, 0x6e]),
                            SerialTraceCondition::new(&[0x6c, 0x65, 0x64, 0x20, 0x6f, 0x6e])
                                .with_timing(Timing::Relative(Duration::from_millis(250)),
                                             Duration::from_millis(25)),
                            SerialTraceCondition::new(&[0x6c, 0x65, 0x64, 0x20, 0x6f, 0x6e])
                                .with_timing(Timing::Relative(Duration::from_millis(0)),
                                             Duration::from_millis(10))]))],
                    true),
            ]
        }
    }
}

impl TestProvider for SampleTestProvider {
    fn tests(&self) -> Box<dyn Iterator<Item = Test> + '_> {
        let it = self.tests.iter()
            .cloned();
        Box::new(it)
    }
}

#[no_mangle]
pub fn get_test_adapter() -> Box<dyn TestProvider> {
    Box::new(SampleTestProvider::new())
}
