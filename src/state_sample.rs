/// A Sample State
use crate::helper::*;
use xmrs::prelude::*;

#[derive(Clone, Default)]
pub struct StateSample {
    /// current seek position
    pub position: f32,
    /// step is freq / rate
    pub step: f32,
    /// For ping-pong samples: true is -->, false is <--
    pub ping : bool,
}

impl StateSample {

    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        self.position = 0.0;
        self.ping = true;
    }

    pub fn set_step(&mut self, frequency: f32, rate: f32) {
        self.step = frequency / rate;
    }

    pub fn set_position(&mut self, position: f32) {
        self.position = position;
    }

    pub fn is_enabled(&self) -> bool {
        self.position >= 0.0
    }

    pub fn disable(&mut self) {
        self.position = -1.0;
    }

    pub fn tick(&mut self, sample: &Sample) -> f32 {
        if self.position < 0.0 {
            0.0
        } else {
            self.tick_internal(sample)
        }
    }

    fn tick_internal(&mut self, sample: &Sample) -> f32 {
        let a: u32 = self.position as u32;
        // LINEAR_INTERPOLATION START
        let b: u32 = a + 1;
        let t: f32 = self.position - a as f32;
        // LINEAR_INTERPOLATION END
        let mut u: f32 = sample.at(a as usize);

        let loop_end = sample.loop_start + sample.loop_length;

        let v = match sample.flags {
            LoopType::No => {
                self.position += self.step;
                if self.position >= sample.len() as f32 {
                    self.position = -1.0;
                }
                // LINEAR_INTERPOLATION START
                if b < sample.len() as u32 {
                    sample.at(b as usize)
                } else {
                    0.0
                }
                // LINEAR_INTERPOLATION END
            }
            LoopType::Forward => {
                self.position += self.step;
                while self.position as u32 >= loop_end {
                    self.position -= sample.loop_length as f32;
                }

                let seek = if b == loop_end { sample.loop_start } else { b };
                // LINEAR_INTERPOLATION START
                sample.at(seek as usize)
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
                        self.position =
                            (loop_end << 1) as f32 - self.position;
                    }
                    /* sanity self.cking */
                    if self.position as usize >= sample.len() {
                        self.ping = false;
                        self.position -= sample.len() as f32 - 1.0;
                    }

                    let seek = if b >= loop_end { a } else { b };
                    // LINEAR_INTERPOLATION START
                    sample.at(seek as usize)
                    // LINEAR_INTERPOLATION END
                } else {
                    // LINEAR_INTERPOLATION START
                    let v = u;
                    let seek = if b == 1 || b - 2 <= sample.loop_start {
                        a
                    } else {
                        b - 2
                    };
                    u = sample.at(seek as usize);
                    // LINEAR_INTERPOLATION END

                    if self.position as u32 <= sample.loop_start {
                        self.ping = true;
                        self.position =
                            (sample.loop_start << 1) as f32 - self.position;
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
        //         /* Smoothly transition between old and new sample. */
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

