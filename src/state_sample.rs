/// A Sample State
use crate::helper::*;
use xmrs::prelude::*;
use xmrs::sample::Sample;

#[derive(Clone)]
pub struct StateSample<'a> {
    sample: &'a Sample,
    finetune: f32,
    /// current seek position
    position: f32,
    /// step is freq / rate
    step: f32,
    /// For ping-pong samples: true is -->, false is <--
    ping: bool,
    // Output frequency
    rate: f32,
}

impl<'a> StateSample<'a> {
    pub fn new(sample: &'a Sample, rate: f32) -> Self {
        let position = if sample.len() == 0 { -1.0 } else { 0.0 };
        let finetune = sample.finetune;
        Self {
            sample,
            finetune,
            position,
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

    pub fn bits(&self) -> u8 {
        self.sample.bits()
    }

    pub fn get_panning(&self) -> f32 {
        self.sample.panning
    }

    pub fn get_volume(&self) -> f32 {
        self.sample.volume
    }

    /// use sample finetune or force if finetune arg!=0
    pub fn get_finetuned_note(&self) -> f32 {
        self.sample.relative_note as f32 + self.finetune
    }

    /// get finetune only
    pub fn get_finetune(&self) -> f32 {
        self.finetune
    }

    pub fn set_finetune(&mut self, finetune: f32) {
        self.finetune = finetune;
    }

    fn tick(&mut self) -> f32 {
        let a: u32 = self.position as u32;
        let b: u32 = a + 1;
        let t: f32 = self.position - a as f32;

        let mut u: f32 = self.sample.at(a as usize);

        let loop_end = self.sample.loop_start + self.sample.loop_length;

        let v = match self.sample.flags {
            LoopType::No => {
                self.position += self.step;
                if self.position >= self.sample.len() as f32 {
                    self.disable();
                }
                if b < self.sample.len() as u32 {
                    self.sample.at(b as usize)
                } else {
                    0.0
                }
            }
            LoopType::Forward => {
                self.position += self.step;

                // while self.position >= loop_end as f32 {
                //     self.position -= self.loop_length as f32;
                // }

                if self.position >= loop_end as f32 {
                    let delta = (self.position - loop_end as f32) % self.sample.loop_length as f32;
                    self.position = self.sample.loop_start as f32 + delta;
                }
                /* sanity checking */
                // if self.position >= self.sample.len() as f32 {
                //     self.position = self.sample.len() as f32 - 1.0;
                // }

                let seek = if b >= loop_end {
                    self.sample.loop_start
                } else {
                    b
                };
                self.sample.at(seek as usize)
            }
            LoopType::PingPong => {
                if self.ping {
                    self.position += self.step;
                } else {
                    self.position -= self.step;
                }

                if self.ping {
                    if self.position >= loop_end as f32 {
                        self.ping = false;
                        let delta =
                            (self.position - loop_end as f32) % self.sample.loop_length as f32;
                        self.position = loop_end as f32 - delta;
                    }
                    /* sanity checking */
                    if self.position >= self.sample.len() as f32 {
                        self.ping = false;
                        self.position = self.sample.len() as f32 - 1.0;
                    }

                    let seek = if b >= loop_end { a } else { b };
                    self.sample.at(seek as usize)
                } else {
                    if self.position <= self.sample.loop_start as f32 {
                        self.ping = true;
                        let delta = (self.sample.loop_start as f32 - self.position)
                            % self.sample.loop_length as f32;
                        self.position = self.sample.loop_start as f32 + delta;
                    }
                    /* sanity checking */
                    if self.position <= 0.0 {
                        self.ping = true;
                        self.position = 0.0;
                    }

                    let v = u;
                    let seek = if b == 1 || b - 2 <= self.sample.loop_start {
                        a
                    } else {
                        b - 2
                    };
                    u = self.sample.at(seek as usize);
                    v
                }
            }
        };
        lerp(u, v, t)
    }
}

impl<'a> Iterator for StateSample<'a> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= 0.0 {
            Some(self.tick())
        } else {
            None
        }
    }
}
