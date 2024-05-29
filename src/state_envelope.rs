/// An Instrument Envelope State
use crate::helper::*;
use xmrs::prelude::*;

#[derive(Clone)]
pub struct StateEnvelope<'a> {
    env: &'a Envelope,
    default_value: f32,
    pub value: f32,
    pub counter: u16,
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

    pub fn tick(&mut self, sustained: bool) -> f32 {
        let num_points = self.env.point.len();
        match num_points {
            0 => self.value = 0.0,
            1 => {
                self.value = self.env.point[0].value as f32 / 64.0;
                clamp_up(&mut self.value);
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

                for i in 1..self.env.point.len() {
                    if self.counter == self.env.point[i - 1].frame {
                        self.value = self.env.point[i - 1].value as f32 / 64.0;
                        break;
                    }

                    if self.counter <= self.env.point[i].frame {
                        self.value = EnvelopePoint::lerp(
                            &self.env.point[i - 1],
                            &self.env.point[i],
                            self.counter,
                        ) / 64.0;
                        break;
                    }

                    if self.env.point[i - 1].frame >= self.env.point[i].frame {
                        self.value = self.env.point[i - 1].value as f32 / 64.0;
                        break;
                    }
                }

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
