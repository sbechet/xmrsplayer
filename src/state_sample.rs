use crate::helper::*;
/// A Sample State
use std::ops::Deref;
use std::sync::Arc;
use xmrs::prelude::*;
use xmrs::sample::Sample;

impl Deref for StateSample {
    type Target = Arc<Sample>;
    fn deref(&self) -> &Arc<Sample> {
        &self.sample
    }
}

#[derive(Clone)]
pub struct StateSample {
    pub sample: Arc<Sample>,
    /// current seek position
    position: f32,
    /// step is freq / rate
    step: f32,
    /// For ping-pong samples: true is -->, false is <--
    ping: bool,
    // Output frequency
    rate: f32,
}

impl StateSample {
    pub fn new(sample: Arc<Sample>, rate: f32) -> Self {
        let pos = if sample.len() == 0 { -1.0 } else { 0.0 };

        Self {
            sample,
            position: pos,
            step: 0.0,
            ping: true,
            rate,
        }
    }

    pub fn reset(&mut self) {
        self.position = if self.sample.len() == 0 { -1.0 } else { 0.0 };
        self.ping = true;
    }

    pub fn set_step(&mut self, frequency: f32) {
        self.step = frequency / self.rate;
    }

    pub fn set_position(&mut self, position: usize) {
        if position >= self.sample.len() {
            self.disable();
        } else {
            self.position = position as f32;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.position >= 0.0
    }

    pub fn disable(&mut self) {
        self.position = -1.0;
    }

    pub fn get_finetuned_note(&self) -> f32 {
        self.sample.relative_note as f32 + self.sample.finetune
    }

    fn tick_internal(&mut self) -> f32 {
        let a: u32 = self.position as u32;
        // LINEAR_INTERPOLATION START
        let b: u32 = a + 1;
        let t: f32 = self.position - a as f32;
        // LINEAR_INTERPOLATION END
        let mut u: f32 = self.sample.at(a as usize);

        let loop_end = self.sample.loop_start + self.sample.loop_length;

        let v = match self.sample.flags {
            LoopType::No => {
                self.position += self.step;
                if self.position >= self.sample.len() as f32 {
                    self.position = -1.0;
                }
                // LINEAR_INTERPOLATION START
                if b < self.sample.len() as u32 {
                    self.sample.at(b as usize)
                } else {
                    0.0
                }
                // LINEAR_INTERPOLATION END
            }
            LoopType::Forward => {
                self.position += self.step;
                while self.position as u32 >= loop_end {
                    self.position -= self.sample.loop_length as f32;
                }

                let seek = if b == loop_end {
                    self.sample.loop_start
                } else {
                    b
                };
                // LINEAR_INTERPOLATION START
                self.sample.at(seek as usize)
                // LINEAR_INTERPOLATION END
            }
            LoopType::PingPong => {
                if self.ping {
                    self.position += self.step;
                } else {
                    self.position -= self.step;
                }
                /* XXX: this may not work for very tight ping-pong loops
                 * (ie switself.s direction more than once per sample */
                if self.ping {
                    if self.position as u32 >= loop_end {
                        self.ping = false;
                        self.position = (loop_end << 1) as f32 - self.position;
                    }
                    /* sanity self.cking */
                    if self.position as usize >= self.sample.len() {
                        self.ping = false;
                        self.position -= self.sample.len() as f32 - 1.0;
                    }

                    let seek = if b >= loop_end { a } else { b };
                    // LINEAR_INTERPOLATION START
                    self.sample.at(seek as usize)
                    // LINEAR_INTERPOLATION END
                } else {
                    // LINEAR_INTERPOLATION START
                    let v = u;
                    let seek = if b == 1 || b - 2 <= self.sample.loop_start {
                        a
                    } else {
                        b - 2
                    };
                    u = self.sample.at(seek as usize);
                    // LINEAR_INTERPOLATION END

                    if self.position as u32 <= self.sample.loop_start {
                        self.ping = true;
                        self.position = (self.sample.loop_start << 1) as f32 - self.position;
                    }
                    /* sanity self.cking */
                    if self.position <= 0.0 {
                        self.ping = true;
                        self.position = 0.0;
                    }
                    v
                }
            }
        };

        let endval = if LINEAR_INTERPOLATION {
            lerp(u, v, t)
        } else {
            u
        };

        // if RAMPING {
        //     if self.frame_count < SAMPLE_RAMPING_POINTS {
        //         /* Smoothly transition between old and new self.sample. */
        //         return lerp(
        //             self.end_of_previous_sample[self.frame_count],
        //             endval,
        //             self.frame_count as f32 / SAMPLE_RAMPING_POINTS as f32,
        //         );
        //     }
        // }

        return endval;
    }
}

impl Iterator for StateSample {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= 0.0 {
            Some(self.tick_internal())
        } else {
            None
        }
    }
}
