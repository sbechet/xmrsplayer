/// An Instrument Vibrato State
use xmrs::{instr_vibrato::InstrVibrato, module::FrequencyType};

use crate::period_helper::PeriodHelper;

#[derive(Clone)]
pub struct StateAutoVibrato<'a> {
    vibrato: &'a InstrVibrato,
    period_helper: PeriodHelper,
    sweep: f32,
    amp: f32,
    pos: f32,
    pub period_offset: f32,
}

impl<'a> StateAutoVibrato<'a> {
    pub fn new(vibrato: &'a InstrVibrato, period_helper: PeriodHelper) -> Self {
        let mut sv = Self {
            vibrato,
            period_helper,
            sweep: 0.0,
            amp: 0.0,
            pos: 0.0,
            period_offset: 0.0,
        };

        sv.reset();

        sv
    }

    pub fn reset(&mut self) {
        self.pos = 0.0;
        self.period_offset = 0.0;
        self.retrig();
    }

    pub fn retrig(&mut self) {
        if self.vibrato.depth > 0.0 {
            self.pos = 0.0;
            if self.vibrato.sweep > 0.0 {
                self.amp = 0.0;
                self.sweep = self.vibrato.depth / (256.0 * self.vibrato.sweep);
            } else {
                self.amp = self.vibrato.depth;
                self.sweep = 0.0;
            }
        }
    }

    pub fn tick(&mut self, sustained: bool) {
        if self.vibrato.depth > 0.0 {
            self.amp = if sustained {
                if self.amp + self.sweep > self.vibrato.depth {
                    self.sweep = 0.0;
                    self.vibrato.depth
                } else {
                    self.amp + self.sweep
                }
            } else {
                self.amp
            };
            self.pos += self.vibrato.speed;
            self.period_offset = self.amp * self.vibrato.waveform.value(self.pos);
            self.period_offset /=
                if let FrequencyType::LinearFrequencies = self.period_helper.freq_type {
                    1.0
                } else {
                    4.0
                }
        }
    }
}
