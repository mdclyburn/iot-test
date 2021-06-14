use std::collections::HashMap;
use std::time::Instant;

use crate::comm::Signal;
use crate::sw::instrument::Spec;

use super::test::Response;

#[derive(Copy, Clone, Debug)]
pub struct Trace {
    id: u16,
    extra: u16,
    time: Instant,
}

impl Trace {
    fn new(id: u16, extra: u16, time: Instant) -> Trace {
        Trace {
            id,
            extra,
            time,
        }
    }

    pub fn get_id(&self) -> u16 {
        self.id
    }

    pub fn get_extra(&self) -> u16 {
        self.extra
    }

    pub fn get_time(&self) -> Instant {
        self.time
    }
}

pub fn reconstruct<'a, T>(responses: T,
                          test_spec: &Spec,
                          pin_sig: &HashMap<u8, u16>) -> Vec<Trace>
where
    T: IntoIterator<Item = &'a Response>
{
    let last_trace_pin = *pin_sig.iter()
        .reduce(|(pin_no_a, sig_a), (pin_no_b, sig_b)| {
            if sig_a > sig_b {
                (pin_no_a, sig_a)
            } else {
                (pin_no_b, sig_b)
            }
        })
        .unwrap()
        .0;

    let mut traces = Vec::new();
    let mut response_iter = responses.into_iter();
    loop {
        let mut trace_responses: Vec<Response> = Vec::new();
        while let Some(response) = response_iter.next() {
            trace_responses.push(*response);
            if response.get_pin() == last_trace_pin {
                break;
            }
        }
        if trace_responses.is_empty() {
            break;
        }

        // Create Trace from pin responses.
        let mut trace_val: u16 = 0;
        for response in &trace_responses {
            if response.get_output() == Signal::Digital(true) {
                trace_val |= 1 << pin_sig.get(&response.get_pin()).unwrap();
            }
        }

        let trace = Trace::new(
            trace_val & id_mask(test_spec.id_bit_length()),
            (trace_val & extra_mask(test_spec.id_bit_length())) >> test_spec.id_bit_length(),
            trace_responses[0].get_time());

        traces.push(trace);
    }

    traces
}

fn id_mask(len: u8) -> u16 {
    let mut mask = 0;
    for n in 0..len {
        mask |= 1 << n;
    }

    mask
}

fn extra_mask(id_len: u8) -> u16 {
    u16::MAX ^ id_mask(id_len)
}
