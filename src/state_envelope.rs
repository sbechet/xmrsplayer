/// An Instrument Envelope State
use std::sync::Arc;
use xmrs::prelude::*;

#[derive(Clone)]
pub struct StateEnvelope {
    env: Arc<Envelope>,
    default_value: f32,
    pub value: f32,
    pub counter: u16,
}

impl StateEnvelope {
    // value is volume_envelope_volume=1.0 or volume_envelope_panning=0.5
    pub fn new(env: Arc<Envelope>, default_value: f32) -> Self {
        Self {
            env: env,
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

    pub fn tick(&mut self, sustained: bool) -> f32 {
        let num_points = self.env.point.len();

        match num_points {
            0 => self.value = 0.0,
            1 => {
                let outval = self.env.point[0].value as f32 / 64.0;
                self.value = if outval > 1.0 { 1.0 } else { outval };
            }
            _ => {
                if self.env.loop_enabled {
                    let loop_start: u16 = self.env.point[self.env.loop_start_point as usize].frame;
                    let loop_end: u16 = self.env.point[self.env.loop_end_point as usize].frame;
                    let loop_length: u16 = loop_end - loop_start;

                    if self.counter >= loop_end {
                        self.counter -= loop_length;
                    }
                }

                // TODO: cleanup when loading
                let mut j: usize = 0;
                while j < (self.env.point.len() - 2) {
                    if self.env.point[j].frame <= self.counter
                        && self.env.point[j + 1].frame >= self.counter
                    {
                        break;
                    }
                    j += 1;
                }

                self.value =
                    EnvelopePoint::lerp(&self.env.point[j], &self.env.point[j + 1], self.counter)
                        / 64.0;

                /* Make sure it is safe to increment frame count */
                self.counter = if !sustained
                    || !self.env.sustain_enabled
                    || self.counter != self.env.point[self.env.sustain_point as usize].frame
                {
                    self.counter + 1
                } else {
                    self.counter
                };
            }
        }
        return self.value;
    }
}
