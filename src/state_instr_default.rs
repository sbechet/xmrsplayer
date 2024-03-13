use crate::helper::*;
use crate::{
    state_envelope::StateEnvelope, state_sample::StateSample, state_auto_vibrato::StateAutoVibrato,
};
/// An InstrDefault State
use std::ops::Deref;
use std::sync::Arc;
use xmrs::prelude::*;

impl Deref for StateInstrDefault {
    type Target = Arc<InstrDefault>;
    fn deref(&self) -> &Arc<InstrDefault> {
        &self.instr
    }
}

// impl DerefMut for StateInstrDefault {
//     fn deref_mut(&mut self) -> &mut InstrDefault { &mut self.instr }
// }

#[derive(Clone)]
pub struct StateInstrDefault {
    instr: Arc<InstrDefault>,
    /// Output frequency
    rate: f32,
    /// Frequency type
    freq_type: FrequencyType,
    /// Sample state
    pub state_sample: Option<StateSample>,
    /// Vibrato state
    pub state_vibrato: StateAutoVibrato,
    /// Volume Envelope state
    pub envelope_volume: StateEnvelope,
    // Volume sustained?
    envelope_sustained: bool,
    /// Volume fadeout value
    pub envelope_volume_fadeout: f32,

    /// Panning Envelope state
    pub envelope_panning: StateEnvelope,

    /// Current volume
    pub volume: f32,

    /// Current panning
    pub panning: f32,
}

impl StateInstrDefault {
    pub fn new(instr: Arc<InstrDefault>, freq_type: FrequencyType, rate: f32) -> Self {
        let v = instr.vibrato.clone();
        let ve = instr.volume_envelope.clone();
        let pe = instr.panning_envelope.clone();
        Self {
            instr,
            rate,
            freq_type,
            state_sample: None,
            state_vibrato: StateAutoVibrato::new(v),
            envelope_volume: StateEnvelope::new(ve, 1.0),
            envelope_sustained: true,
            envelope_volume_fadeout: 1.0,
            envelope_panning: StateEnvelope::new(pe, 0.5),
            volume: 1.0,
            panning: 0.5,
        }
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
        self.envelope_sustained = true;
        self.envelope_volume_fadeout = 1.0;
        self.envelope_volume.reset();
        self.envelope_panning.reset();
    }

    pub fn vibrato_reset(&mut self) {
        self.state_vibrato.reset();
    }

    pub fn cut_note(&mut self) {
        self.volume = 0.0;
    }

    pub fn key_off(&mut self) {
        /* Key Off */
        self.envelope_sustained = false;

        /* If no volume envelope is used, also cut the note */
        if !self.instr.volume_envelope.enabled {
            self.cut_note();
        }
    }

    pub fn get_volume(&self) -> f32 {
        self.envelope_volume_fadeout * self.envelope_volume.value * self.volume
    }

    pub fn envelopes(&mut self) {
        // Volume
        if self.volume_envelope.enabled {
            if !self.envelope_sustained {
                self.envelope_volume_fadeout -= self.instr.volume_fadeout;
                clamp_down(&mut self.envelope_volume_fadeout);
            }
            self.envelope_volume.tick(self.envelope_sustained);
        }
        // Panning
        if self.panning_envelope.enabled {
            self.envelope_panning.tick(self.envelope_sustained);
        }
    }

    pub fn update_frequency(&mut self, period: f32, arp_note: f32, period_offset: f32) {
        match &mut self.state_sample {
            Some(s) => {
                let frequency = frequency(
                    self.freq_type,
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
        let noteu8: u8 = note.into();
        if note_is_valid(noteu8) {
            let num = self.instr.sample_for_note[noteu8 as usize - 1] as usize;
            return self.select_sample(num);
        } else {
            return false;
        }
    }

    fn select_sample(&mut self, num: usize) -> bool {
        if num < self.instr.sample.len() {
            let sample = self.instr.sample[num].clone();
            let state_sample = StateSample::new(sample, self.rate);
            self.panning = state_sample.get_panning();
            self.volume = state_sample.get_volume();
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
        self.state_vibrato.tick(self.envelope_sustained);
    }
}

impl Iterator for StateInstrDefault {
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
