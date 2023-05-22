/// An Instrument Envelope State
use xmrs::prelude::*;

#[derive(Clone, Default)]
pub struct StateEnvelope {
    default_value: f32,
    pub value: f32,
    pub counter: u16,
}

impl StateEnvelope {
    // value is volume_envelope_volume=1.0 or volume_envelope_panning=0.5
    pub fn new(value: f32) -> Self {
        Self {
            default_value: value,
            value,
            counter: 0,
        }
    }

    pub fn reset(&mut self) {
        self.value = self.default_value;
        self.counter = 0;
    }

    pub fn tick(&mut self, env: &Envelope, sustained: bool) -> f32 {
        let num_points = env.point.len();

        match num_points {
            0 => self.value = 0.0,
            1 => {
                let outval = env.point[0].value as f32 / 64.0;
                self.value = if outval > 1.0 { 1.0 } else { outval };
            }
            _ => {
                if env.loop_enabled {
                    let loop_start: u16 = env.point[env.loop_start_point as usize].frame;
                    let loop_end: u16 = env.point[env.loop_end_point as usize].frame;
                    let loop_length: u16 = loop_end - loop_start;

                    if self.counter >= loop_end {
                        self.counter -= loop_length;
                    }
                }

                let mut j: usize = 0;
                while j < (env.point.len() - 2) {
                    if env.point[j].frame <= self.counter && env.point[j + 1].frame >= self.counter
                    {
                        break;
                    }
                    j += 1;
                }

                self.value =
                    EnvelopePoint::lerp(&env.point[j], &env.point[j + 1], self.counter) / 64.0;

                /* Make sure it is safe to increment frame count */
                self.counter = if !sustained
                    || !env.sustain_enabled
                    || self.counter != env.point[env.sustain_point as usize].frame
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
