/// An Instrument Vibrato State
use std::sync::Arc;
use xmrs::instr_vibrato::InstrVibrato;

#[derive(Clone, Default)]
pub struct StateAutoVibrato {
    vibrato: Arc<InstrVibrato>,
    sweep: f32,
    amp: f32,
    pos: f32,
    pub period_offset: f32,
}


impl StateAutoVibrato {
    pub fn new(vibrato: Arc<InstrVibrato>) -> Self {
        let mut sv = Self {
            vibrato: Arc::clone(&vibrato),
            ..Default::default()
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
                self.sweep = self.vibrato.depth / self.vibrato.sweep;
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
        }
    }
}
