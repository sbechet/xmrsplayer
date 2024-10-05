/// An Instrument Vibrato State
use xmrs::instr_vibrato::InstrVibrato;
use xmrs::period_helper::{FrequencyType, PeriodHelper};

#[derive(Clone)]
pub struct StateAutoVibrato<'a> {
    vibrato: &'a InstrVibrato,
    period_helper: PeriodHelper,
    phase: f32,
    pub current_modulation: f32,
}

impl<'a> StateAutoVibrato<'a> {
    pub fn new(vibrato: &'a InstrVibrato, period_helper: PeriodHelper) -> Self {
        let mut sv = Self {
            vibrato,
            period_helper,
            phase: 0.0,
            current_modulation: 0.0,
        };

        sv.reset();

        sv
    }

    pub fn reset(&mut self) {
        self.retrig();
    }

    pub fn retrig(&mut self) {
        self.phase = 0.0;
        self.current_modulation = 0.0;
    }

    pub fn tick(&mut self, sustain: bool) {
        self.phase += self.vibrato.speed;

        let current_depth = if self.phase < self.vibrato.sweep && !sustain {
            // sweep can't be zero
            (self.phase / self.vibrato.sweep) * self.vibrato.depth as f32
        } else {
            self.vibrato.depth
        };

        self.current_modulation = current_depth * self.vibrato.waveform.value(self.phase);

        if let FrequencyType::AmigaFrequencies = self.period_helper.freq_type {
            self.current_modulation /= 4.0;
        }
    }
}
