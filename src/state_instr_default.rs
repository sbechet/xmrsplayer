use core::ops::Deref;

/// An InstrDefault State
use crate::helper::*;
use crate::period_helper::PeriodHelper;
use crate::{
    state_auto_vibrato::StateAutoVibrato, state_envelope::StateEnvelope, state_sample::StateSample,
};
use xmrs::prelude::*;

impl<'a> Deref for StateInstrDefault<'a> {
    type Target = InstrDefault;
    fn deref(&self) -> &InstrDefault {
        &self.instr
    }
}

#[derive(Clone)]
pub struct StateInstrDefault<'a> {
    instr: &'a InstrDefault,
    pub num: usize,
    /// Output frequency
    rate: f32,
    period_helper: PeriodHelper,
    /// Sample state
    pub state_sample: Option<StateSample<'a>>,
    /// Vibrato state
    pub state_vibrato: StateAutoVibrato<'a>,
    /// Volume Envelope state
    pub envelope_volume: StateEnvelope<'a>,
    /// Panning Envelope state
    pub envelope_panning: StateEnvelope<'a>,

    // Volume sustained?
    pub sustained: bool,
    /// Volume fadeout value
    pub volume_fadeout: f32,

    /// Current volume
    pub volume: f32,
    /// Original Sample volume
    volume_orig: f32,

    /// Current panning
    pub panning: f32,
}

impl<'a> StateInstrDefault<'a> {
    pub fn new(
        instr: &'a InstrDefault,
        num: usize,
        period_helper: PeriodHelper,
        rate: f32,
    ) -> Self {
        let v = &instr.vibrato;
        let ve = &instr.volume_envelope;
        let pe = &instr.panning_envelope;
        Self {
            instr,
            num,
            rate,
            period_helper: period_helper.clone(),
            state_sample: None,
            state_vibrato: StateAutoVibrato::new(v, period_helper),
            envelope_volume: StateEnvelope::new(ve, 1.0),
            envelope_panning: StateEnvelope::new(pe, 0.5),
            sustained: true,
            volume_fadeout: 1.0,
            volume: 1.0,
            volume_orig: 1.0,
            panning: 0.5,
        }
    }

    pub fn has_volume_envelope(&self) -> bool {
        self.envelope_volume.has_volume_envelope()
    }

    pub fn get_sample_c4_rate(&self) -> Option<f32> {
        match &self.state_sample {
            Some(s) => s.get_sample_c4_rate(&self.period_helper),
            None => None,
        }
    }

    pub fn replace_instr(&mut self, instr: &'a InstrDefault) {
        self.instr = instr;
    }

    pub fn is_enabled(&self) -> bool {
        match &self.state_sample {
            Some(s) => s.is_enabled(),
            None => false,
        }
    }

    pub fn sample_reset(&mut self) {
        match &mut self.state_sample {
            Some(s) => s.reset(),
            None => {}
        }
    }

    pub fn envelopes_reset(&mut self) {
        self.sustained = true;
        self.volume_fadeout = 1.0;
        self.envelope_volume.reset();
        self.envelope_panning.reset();
    }

    pub fn volume_reset(&mut self) {
        self.volume = self.volume_orig;
        self.volume_fadeout = 1.0;
        self.sustained = true;
    }

    pub fn vibrato_reset(&mut self) {
        self.state_vibrato.reset();
    }

    pub fn cut_note(&mut self) {
        self.volume = 0.0;
    }

    pub fn key_off(&mut self) {
        /* Key Off */
        self.sustained = false;

        if !self.envelope_volume.has_volume_envelope() {
            if self.instr.volume_fadeout == 0.0 {
                self.cut_note();
            }
        }
    }

    pub fn get_volume(&self) -> f32 {
        (self.volume_fadeout * self.envelope_volume.value * self.volume)
            / (self.volume_fadeout + self.envelope_volume.value + self.volume)
    }

    fn envelopes(&mut self) {
        // Volume
        if !self.sustained {
            self.volume_fadeout -= self.instr.volume_fadeout;
            clamp_down(&mut self.volume_fadeout);
        }
        if self.instr.volume_envelope.enabled {
            self.envelope_volume.tick(self.sustained);
        }
        // Panning
        if self.instr.panning_envelope.enabled {
            self.envelope_panning.tick(self.sustained);
        }
    }

    pub fn get_finetuned_note(&self) -> f32 {
        match &self.state_sample {
            Some(s) if s.is_enabled() => s.get_finetuned_note(),
            _ => 0.0,
        }
    }

    /// get finetune only
    pub fn get_finetune(&self) -> f32 {
        match &self.state_sample {
            Some(s) if s.is_enabled() => s.get_finetune(),
            _ => 0.0,
        }
    }

    pub fn set_finetune(&mut self, finetune: f32) {
        match &mut self.state_sample {
            Some(s) if s.is_enabled() => s.set_finetune(finetune),
            _ => {}
        }
    }

    pub fn update_frequency(&mut self, period: f32, arp_note: f32, period_offset: f32) {
        match &mut self.state_sample {
            Some(s) => {
                let frequency = self.period_helper.frequency(
                    period,
                    arp_note,
                    period_offset + self.state_vibrato.period_offset,
                );
                s.set_step(frequency)
            }
            None => {}
        }
    }

    pub fn set_note(&mut self, note: Note) -> bool {
        if note.is_valid() {
            let num = self.instr.sample_for_note[note.value() as usize - 1] as usize;
            return self.select_sample(num);
        } else {
            return false;
        }
    }

    fn select_sample(&mut self, num: usize) -> bool {
        if num < self.instr.sample.len() {
            let sample = &self.instr.sample[num];
            let state_sample = StateSample::new(sample, self.rate);
            self.panning = state_sample.get_panning();
            self.volume = state_sample.get_volume();
            self.volume_orig = self.volume;
            self.state_sample = Some(state_sample);
            return true;
        } else {
            self.state_sample = None;
            self.panning = 0.5;
            self.volume = 0.0;
            return false;
        }
    }

    pub fn tick(&mut self) {
        self.envelopes();
        self.state_vibrato.tick(self.sustained);
    }
}

impl<'a> Iterator for StateInstrDefault<'a> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_enabled() {
            match &mut self.state_sample {
                Some(s) => s.next(),
                None => None,
            }
        } else {
            None
        }
    }
}
