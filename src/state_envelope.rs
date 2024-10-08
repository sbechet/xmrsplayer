/// An Instrument Envelope State
use crate::helper::*;
use xmrs::prelude::*;

#[derive(Clone)]
pub struct StateEnvelope<'a> {
    env: &'a Envelope,
    default_value: f32,
    pub value: f32,
    pub counter: usize,
}

impl<'a> StateEnvelope<'a> {
    // value is volume_envelope_volume=1.0 or volume_envelope_panning=0.5
    pub fn new(env: &'a Envelope, default_value: f32) -> Self {
        Self {
            env,
            default_value,
            value: default_value,
            counter: 0,
        }
    }

    pub fn has_volume_envelope(&self) -> bool {
        self.env.enabled
    }

    pub fn reset(&mut self) {
        self.value = self.default_value;
        self.counter = 0;
    }

    pub fn tick(&mut self, sustained: bool) {
        let num_points = self.env.point.len();

        if num_points == 0 {
            self.value = 0.0;
            return;
        }

        if num_points == 1 {
            self.value = self.env.point[0].value;
            clamp_up(&mut self.value);
            return;
        }

        if self.env.loop_enabled {
            let loop_start = self.env.point[self.env.loop_start_point as usize].frame;
            let loop_end = self.env.point[self.env.loop_end_point as usize].frame;
            if self.counter >= loop_end {
                self.counter -= loop_end - loop_start;
            }
        }

        for i in 1..num_points {
            let prev_point = &self.env.point[i - 1];
            let curr_point = &self.env.point[i];

            if self.counter == prev_point.frame {
                self.value = prev_point.value;
                break;
            }

            if self.counter <= curr_point.frame {
                self.value = EnvelopePoint::lerp(prev_point, curr_point, self.counter);
                break;
            }

            if prev_point.frame >= curr_point.frame {
                self.value = prev_point.value;
                break;
            }
        }

        /* Make sure it is safe to increment frame count */
        if !sustained
            || !self.env.sustain_enabled
            || self.counter != self.env.point[self.env.sustain_point as usize].frame
        {
            self.counter += 1;
        }
    }
}
