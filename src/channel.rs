use bitflags::bitflags;
use std::sync::Arc;

use crate::effect::*;
use crate::effect_arpeggio::EffectArpeggio;
use crate::effect_multi_retrig_note::EffectMultiRetrigNote;
use crate::effect_portamento::EffectPortamento;
use crate::effect_toneportamento::EffectTonePortamento;
use crate::effect_vibrato_tremolo::EffectVibratoTremolo;
use crate::effect_volume_slide::EffectVolumeSlide;

use crate::helper::*;
use crate::state_instr_default::StateInstrDefault;
use xmrs::prelude::*;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TriggerKeep: u8 {
        const NONE = 0;
        const VOLUME = (1 << 0);
        const PERIOD = (1 << 1);
        const SAMPLE_POSITION = (1 << 2);
        const ENVELOPE = (1 << 3);
    }
}

#[derive(Clone, Default)]
pub struct Channel {
    module: Arc<Module>,
    rate: f32,

    note: f32,
    orig_note: f32, /* The original note before effect modifications, as read in the pattern. */

    pub current: PatternSlot,

    period: f32,

    volume: f32,  /* Ideally between 0 (muted) and 1 (loudest) */
    panning: f32, /* Between 0 (left) and 1 (right); 0.5 is centered */

    // Instrument
    instr: Option<StateInstrDefault>,

    panning_slide_param: u8,

    arpeggio: EffectArpeggio,
    multi_retrig_note: EffectMultiRetrigNote,
    portamento: EffectPortamento,
    portamento_fine: EffectPortamento,
    portamento_extrafine: EffectPortamento,
    tone_portamento: EffectTonePortamento,
    tremolo: EffectVibratoTremolo,
    volume_slide: EffectVolumeSlide,
    volume_slide_tick0: EffectVolumeSlide,
    vibrato: EffectVibratoTremolo,

    porta_semitone_slides: bool,

    note_delay_param: u8,
    /// Where to restart a E6y loop
    pub pattern_loop_origin: u8,
    /// How many loop passes have been done
    pub pattern_loop_count: u8,

    tremor_param: u8,
    tremor_on: bool,

    pub muted: bool,

    pub actual_volume: [f32; 2],
}

impl Channel {
    pub fn new(module: Arc<Module>, rate: f32) -> Self {
        Self {
            module,
            rate,
            volume: 1.0,
            panning: 0.5,
            vibrato: EffectVibratoTremolo::vibrato(),
            tremolo: EffectVibratoTremolo::tremolo(),
            multi_retrig_note: EffectMultiRetrigNote::new(false, 0.0, 0.0),
            ..Default::default()
        }
    }

    pub fn is_muted(&self) -> bool {
        let midi_mute = if let Some(i) = &self.instr {
            i.midi_mute_computer
        } else {
            false
        };
        self.muted || midi_mute
    }

    fn cut_note(&mut self) {
        /* NB: this is not the same as Key Off */
        self.volume = 0.0;
    }

    fn key_off(&mut self) {
        match &mut self.instr {
            Some(i) => {
                i.key_off();
            }
            None => self.cut_note(),
        }
    }

    // 0xRL
    fn panning_slide(&mut self, rawval: u8) {
        if (rawval & 0xF0 != 0) && (rawval & 0x0F != 0) {
            /* Illegal state */
            return;
        }
        if rawval & 0xF0 != 0 {
            /* Slide right */
            let f = (rawval >> 4) as f32 / 16.0;
            self.panning += f;
            clamp_up(&mut self.panning);
        } else {
            /* Slide left */
            let f = (rawval & 0x0F) as f32 / 16.0;
            self.panning -= f;
            clamp_down(&mut self.panning);
        }
    }

    pub fn trigger_note(&mut self, flags: TriggerKeep) {
        self.tremor_on = false;

        match &mut self.instr {
            Some(instr) => {
                if !flags.contains(TriggerKeep::SAMPLE_POSITION) {
                    instr.sample_reset();
                }

                if !flags.contains(TriggerKeep::ENVELOPE) {
                    instr.envelopes_reset();
                }

                instr.vibrato_reset();

                if !flags.contains(TriggerKeep::VOLUME) {
                    self.volume = instr.volume;
                }
                self.panning = instr.panning;

                if !flags.contains(TriggerKeep::PERIOD) {
                    self.period = period(self.module.frequency_type, self.note);
                    instr.update_frequency(
                        self.period,
                        self.arpeggio.value(),
                        self.vibrato.value(),
                    );
                }
            }
            None => {}
        }
    }

    fn tick_effects(&mut self, current_tick: u16) {
        match self.current.effect_type {
            0 => {
                /* 0xy: Arpeggio */
                if self.current.effect_parameter > 0 {
                    self.arpeggio.tick();
                }
            }
            1 => {
                /* 1xx: Portamento up */
                self.portamento.tick();
                self.period = self.portamento.clamp(self.period);
            }
            2 => {
                /* 2xx: Portamento down */
                self.portamento.tick();
                self.period = self.portamento.clamp(self.period);
            }
            3 if current_tick != 0 => {
                /* 3xx: Tone portamento */
                self.tone_portamento.tick();
                self.period = self.tone_portamento.clamp(self.period);
                if self.porta_semitone_slides {
                    // TODO: Tone portamento effects slide by semitones
                }
            }
            4 if current_tick != 0 => {
                /* 4xy: Vibrato */
                self.vibrato.tick();
            }
            5 if current_tick != 0 => {
                /* 5xy: Tone portamento + Volume slide */
                self.tone_portamento.tick();
                self.period = self.tone_portamento.clamp(self.period);
                if self.porta_semitone_slides {
                    // TODO: Tone portamento effects slide by semitones
                }
                // now volume slide
                self.volume += self.volume_slide.tick();
            }
            6 if current_tick != 0 => {
                /* 6xy: Vibrato + Volume slide */
                self.vibrato.tick();
                // now volume slide
                self.volume += self.volume_slide.tick();
            }
            7 if current_tick != 0 => {
                /* 7xy: Tremolo */
                self.tremolo.tick();
            }
            0xA if current_tick != 0 => {
                /* Axy: Volume slide */
                self.volume += self.volume_slide.tick();
            }
            0xE => {
                /* EXy: Extended command */
                match self.current.effect_parameter >> 4 {
                    0x9 if current_tick != 0 => {
                        /* E9y: Retrigger note */
                        if self.current.effect_parameter & 0x0F != 0 {
                            let r = current_tick % (self.current.effect_parameter as u16 & 0x0F);
                            if r != 0 {
                                self.trigger_note(TriggerKeep::VOLUME);
                                match &mut self.instr {
                                    Some(instr) => {
                                        instr.envelopes();
                                    }
                                    None => {}
                                }
                            }
                        }
                    }
                    0xC => {
                        /* ECy: Note cut */
                        //TODO? If y is greater than or equal to the current module Speed, this command is ignored. 
                        if (self.current.effect_parameter as u16 & 0x0F) == current_tick {
                            self.cut_note();
                        }
                    }
                    0xD => {
                        /* EDy: Note delay */
                        //TODO? If y is greater than or equal to the current module Speed, the current pattern cell's contents are never played. 
                        if self.note_delay_param as u16 == current_tick {
                            self.tick0_load_note_and_instrument();
                            match &mut self.instr {
                                Some(instr) => {
                                    instr.envelopes();
                                }
                                None => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            0x14 => {
                /* Kxx: Key off */
                if current_tick == self.current.effect_parameter as u16 {
                    self.key_off();
                }
            }
            0x19 if current_tick != 0 => {
                /* Pxy: Panning slide */
                self.panning_slide(self.panning_slide_param);
            }
            0x1B if current_tick != 0 => {
                /* Rxy: Multi retrig note */
                if self.multi_retrig_note.tick() == 0.0 {
                    self.trigger_note(TriggerKeep::VOLUME | TriggerKeep::ENVELOPE);

                    /* Rxy doesn't affect volume if there's a command in the volume
                    column, or if the instrument has a volume envelope. */
                    match &self.instr {
                        Some(instr) => {
                            if self.current.volume != 0 && !instr.volume_envelope.enabled {
                                self.volume = self.multi_retrig_note.clamp(self.volume);
                            }
                        }
                        None => {}
                    }
                }
            }
            0x1D if current_tick != 0 => {
                /* Txy: Tremor
                    Rapidly switches the sample volume on and off on every tick of the row except the first.
                    Volume is on for x + 1 ticks and off for y + 1 ticks.

                    tremor_on: bool = [(T-1) % (X+1+Y+1) ] > X
                */
                self.tremor_on = (current_tick - 1)
                    % ((self.tremor_param as u16 >> 4) + (self.tremor_param as u16 & 0x0F) + 2)
                    > (self.tremor_param as u16 >> 4);
            }
            _ => {}
        }
    }

    fn tick_volume_effects(&mut self) {
        match self.current.volume >> 4 {
            0x6 => {
                /* - - Volume slide down */
                self.volume_slide.xm_update_effect(self.current.volume, 2, 0.0);
                self.volume += self.volume_slide.tick();
            }
            0x7 => {
                /* + - Volume slide up */
                self.volume_slide.xm_update_effect(self.current.volume, 1, 0.0);
                self.volume += self.volume_slide.tick();
            }
            0xB => {
                /* V - Vibrato */
                self.vibrato.tick();
            }
            0xD => {
                /* L - Panning slide left */
                self.panning_slide(self.current.volume & 0x0F);
            }
            0xE => {
                /* R - Panning slide right */
                self.panning_slide(self.current.volume << 4);
            }
            0xF => {
                /* M - Tone portamento */
                self.tone_portamento.tick();
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, current_tick: u16) {
        match &mut self.instr {
            Some(instr) => {
                instr.tick();
            }
            None => return,
        }

        if current_tick != 0 {
            self.tick_volume_effects();
        }

        self.tick_effects(current_tick);

        match &mut self.instr {
            Some(instr) => {
                let panning: f32 = self.panning
                    + (instr.envelope_panning.value - 0.5)
                        * (0.5 - (self.panning - 0.5).abs())
                        * 2.0;
                let mut volume = 0.0;

                if !self.tremor_on {
                    volume = self.volume + self.tremolo.value();
                    clamp(&mut volume);
                    volume *= instr.get_volume();
                }

                self.actual_volume[0] = volume * panning.sqrt();
                self.actual_volume[1] = volume * (1.0 - panning).sqrt();

                instr.update_frequency(self.period, self.arpeggio.value(), self.vibrato.value());
            }
            None => {}
        }
    }

    fn tick0_effects(&mut self, noteu8: u8) {
        match self.current.effect_type {
            0x0 => self
                .arpeggio
                .xm_update_effect(self.current.effect_parameter, 0, 0.0),
            0x1 => self
                .portamento
                .xm_update_effect(self.current.effect_parameter, 0, 1.1),
            0x2 => self
                .portamento
                .xm_update_effect(self.current.effect_parameter, 0, 0.0),
            0x3 => {
                let mult = if let FrequencyType::LinearFrequencies = self.module.frequency_type {
                    1
                } else {
                    0
                };
                self.tone_portamento.xm_update_effect(
                    self.current.effect_parameter,
                    mult,
                    self.note,
                );
            }
            0x4 => self
                .vibrato
                .xm_update_effect(self.current.effect_parameter, 0, 0.0),
            0x5 => {
                /* 5xy: Tone portamento + Volume slide */
                self.volume_slide.xm_update_effect(self.current.effect_parameter,0, 0.0);
            }
            0x6 => {
                /* 6xy: Vibrato + Volume slide */
                self.volume_slide.xm_update_effect(self.current.effect_parameter,0, 0.0);
            }
            0x7 => self
                .tremolo
                .xm_update_effect(self.current.effect_parameter, 0, 0.0),
            0x8 => {
                /* 8xx: Set panning */
                self.panning = self.current.effect_parameter as f32 / 255.0;
            }
            0x9 => {
                /* 9xx: Sample offset */
                if note_is_valid(noteu8) {
                    match &mut self.instr {
                        Some(i) => match &mut i.state_sample {
                            Some(s) => {
                                let final_offset = if s.bits() == 16 {
                                    self.current.effect_parameter as usize * 256
                                } else {
                                    self.current.effect_parameter as usize * 512
                                };
                                s.set_position(final_offset);
                            }
                            None => {}
                        },
                        None => {}
                    }
                }
            }
            0xA => {
                /* Axy: Volume slide */
                self.volume_slide.xm_update_effect(self.current.effect_parameter,0, 0.0);
            }
            0xC => {
                /* Cxx: Set volume */
                self.volume = if self.current.effect_parameter > 64 {
                    1.0
                } else {
                    self.current.effect_parameter as f32 / 64.0
                };
            }
            0xE => {
                /* EXy: Extended command */
                match self.current.effect_parameter >> 4 {
                    0x1 => {
                        /* E1y: Fine Portamento up */
                        self.portamento_fine.xm_update_effect(
                            self.current.effect_parameter,
                            1,
                            1.0,
                        );
                        self.portamento_fine.tick();
                        self.period = self.portamento_fine.clamp(self.period);
                    }
                    0x2 => {
                        /* E2y: Fine portamento down */
                        self.portamento_fine.xm_update_effect(
                            self.current.effect_parameter,
                            1,
                            0.0,
                        );
                        self.portamento_fine.tick();
                        self.period = self.portamento_fine.clamp(self.period);
                    }
                    0x3 => {
                        /* E3y: Set glissando control */
                        // TODO, see FT2 setPortamentoCtrl
                        self.porta_semitone_slides = self.current.effect_parameter != 0;
                    }
                    0x4 => {
                        /* E4y: Set vibrato control */
                        self.vibrato.data.waveform = self.current.effect_parameter & 3;
                        if ((self.current.effect_parameter >> 2) & 1) == 0 {
                            self.vibrato.retrigger();
                        }
                    }
                    0x5 => {
                        /* E5y: Set finetune */
                        if note_is_valid(noteu8) {
                            match &mut self.instr {
                                Some(i) => match &mut i.state_sample {
                                    Some(s) => {
                                        // replacing state_sample.get_finetuned_note()...
                                        let finetune =
                                            (self.current.effect_parameter & 0x0F) as f32 / 16.0
                                                - 0.5;
                                        let finetuned_note = s.relative_note as f32 + finetune;
                                        self.note = noteu8 as f32 - 1.0 + finetuned_note
                                    }
                                    None => {}
                                },
                                None => {}
                            }
                        }
                    }
                    0x7 => {
                        /* E7y: Set tremolo control */
                        self.tremolo.data.waveform = self.current.effect_parameter & 3;
                        if ((self.current.effect_parameter >> 2) & 1) == 0 {
                            self.tremolo.retrigger();
                        }
                    }
                    0xA => {
                        /* EAy: Fine volume slide up */
                        self.volume_slide_tick0.xm_update_effect(self.current.effect_parameter,1, 0.0);
                        self.volume += self.volume_slide_tick0.tick();
                    }
                    0xB => {
                        /* EBy: Fine volume slide down */
                        self.volume_slide_tick0.xm_update_effect(self.current.effect_parameter,2, 0.0);
                        self.volume += self.volume_slide_tick0.tick();
                    }
                    0xD => {
                        /* EDy: Note delay */
                        // TODO? If y is greater than or equal to the current module Speed, the current pattern cell's contents are never played. 
                        /* XXX: figure this out better. EDy triggers
                         * the note even when there no note and no
                         * instrument. But ED0 acts like like a ghost
                         * note, EDy (y ≠ 0) does not. */
                        if let Note::None = self.current.note {
                            if self.current.instrument == 0 {
                                let flags = TriggerKeep::VOLUME;

                                if self.current.effect_parameter & 0x0F != 0 {
                                    self.note = self.orig_note;
                                    self.trigger_note(flags);
                                } else {
                                    self.trigger_note(
                                        flags | TriggerKeep::PERIOD | TriggerKeep::SAMPLE_POSITION,
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            0x15 => {
                /* Lxx: Set envelope position */
                match &mut self.instr {
                    Some(i) => {
                        i.envelope_volume.counter = self.current.effect_parameter as u16;
                        if i.sustained {
                            i.envelope_panning.counter = self.current.effect_parameter as u16;
                        }
                    }
                    None => {}
                }
            }
            0x19 => {
                /* Pxy: Panning slide */
                if self.current.effect_parameter > 0 {
                    self.panning_slide_param = self.current.effect_parameter;
                }
            }
            0x1B => {
                /* Rxy: Multi retrig note */
                self.multi_retrig_note
                    .xm_update_effect(self.current.effect_parameter, 0, 0.0);
            }
            0x1D => {
                /* Txy: Tremor */
                if self.current.effect_parameter > 0 {
                    /* Tremor x and y params do not appear to be separately
                     * kept in memory, unlike Rxy */
                    self.tremor_param = self.current.effect_parameter;
                }
            }
            0x21 => {
                /* Xxy: Extra stuff */
                match self.current.effect_parameter >> 4 {
                    1 => {
                        /* X1y: Extra fine portamento up */
                        self.portamento_extrafine.xm_update_effect(
                            self.current.effect_parameter,
                            2,
                            1.0,
                        );
                        self.portamento_extrafine.tick();
                        self.period = self.portamento_extrafine.clamp(self.period);
                    }
                    2 => {
                        /* X2y: Extra fine portamento down */
                        self.portamento_extrafine.xm_update_effect(
                            self.current.effect_parameter,
                            2,
                            0.0,
                        );
                        self.portamento_extrafine.tick();
                        self.period = self.portamento_extrafine.clamp(self.period);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn tick0_volume_effects(&mut self) {
        match self.current.volume >> 4 {
            0x0 => {} // Nothing
            // V - Set volume (0..63)
            0x1..=0x4 => self.volume = (self.current.volume - 0x10) as f32 / 64.0,
            // V - 0x51..0x5F undefined...
            0x5 => self.volume = 1.0,
            // - - Volume slide down (0..15)
            0x6 => {} // see tick() fn
            // + - Volume slide up (0..15)
            0x7 => {} // see tick() fn
            // D - Fine volume slide down (0..15)
            0x8 => {
                self.volume_slide.xm_update_effect(self.current.volume, 2, 0.0);
                self.volume += self.volume_slide.tick();
            }
            // U - Fine volume slide up (0..15)
            0x9 => {
                self.volume_slide.xm_update_effect(self.current.volume, 1, 0.0);
                self.volume += self.volume_slide.tick();
            }
            // S - Vibrato speed (0..15)
            0xA => self.vibrato.xm_update_effect(self.current.volume, 1, 0.0),
            // V - Vibrato depth (0..15)
            0xB => {} // see tick() fn
            // P - Set panning
            0xC => self.panning = (self.current.volume << 4) as f32 / 255.0,
            // L - Pan slide left (0..15)
            0xD => {} // see tick() fn
            // R - Pan slide right (0..15)
            0xE => {} // see tick() fn
            // M - Tone portamento (0..15)
            0xF => {
                // TODO: Check that
                // if ! self.current.has_retrigger_note_empty() {
                let mult = if let FrequencyType::LinearFrequencies = self.module.frequency_type {
                    1
                } else {
                    0
                };
                self.tone_portamento.xm_update_effect(
                    self.current.volume & 0x0F,
                    2 | mult,
                    self.note,
                );
                // }
            }
            _ => {}
        }
    }

    fn tick0_load_note_and_instrument(&mut self) {
        let noteu8: u8 = self.current.note.into();

        // First, load instr
        if self.current.instrument > 0 {
            if self.current.has_tone_portamento() {
                self.trigger_note(TriggerKeep::PERIOD | TriggerKeep::SAMPLE_POSITION);
            } else if let Note::None = self.current.note {
                /* Ghost instrument, trigger note */
                /* Sample position is kept, but envelopes are reset */
                self.trigger_note(TriggerKeep::SAMPLE_POSITION);
            } else if self.current.instrument as usize > self.module.instrument.len() {
                /* Invalid instrument, Cut current note */
                self.cut_note();
                self.instr = None;
            } else {
                let instrnr = self.current.instrument as usize - 1;
                if let InstrumentType::Default(id) = &self.module.instrument[instrnr].instr_type {
                    // only good instr
                    if id.sample.len() != 0 {
                        let instr = StateInstrDefault::new(
                            id.clone(),
                            self.module.frequency_type,
                            self.rate,
                        );
                        self.instr = Some(instr);
                    }
                } else {
                    // TODO
                }
            }
        }

        // Next, choose sample from note
        if note_is_valid(noteu8) {
            match &mut self.instr {
                Some(i) => {
                    if self.current.has_tone_portamento() {
                        match &i.state_sample {
                            Some(s) if s.is_enabled() => {
                                self.note = noteu8 as f32 - 1.0 + s.get_finetuned_note()
                            }
                            _ => self.cut_note(),
                        }
                    } else if i.set_note(self.current.note) {
                        if let Some(s) = &i.state_sample {
                            self.orig_note = noteu8 as f32 - 1.0 + s.get_finetuned_note();
                            self.note = self.orig_note;
                        }

                        if self.current.instrument > 0 {
                            self.trigger_note(TriggerKeep::NONE);
                        } else {
                            /* Ghost note: keep old volume */
                            self.trigger_note(TriggerKeep::VOLUME);
                        }
                    } else {
                        self.cut_note();
                    }
                }
                None => self.cut_note(),
            }
        } else if let Note::KeyOff = self.current.note {
            self.key_off();
        }

        // Volume effect
        self.tick0_volume_effects();

        // Effects
        self.tick0_effects(noteu8);
    }

    pub fn tick0(&mut self, pattern_slot: &PatternSlot) {
        self.current = pattern_slot.clone();

        if self.current.effect_type != 0xE || (self.current.effect_parameter >> 4) != 0xD {
            self.tick0_load_note_and_instrument();

            if self.arpeggio.in_progress() && !self.current.has_arpeggio() {
                self.arpeggio.retrigger();
            }

            if self.vibrato.in_progress && !self.current.has_vibrato() {
                self.vibrato.retrigger();
            }

            match &mut self.instr {
                Some(instr) => {
                    instr.update_frequency(self.period, self.arpeggio.value(), self.vibrato.value())
                }
                None => {}
            }
        } else {
            self.note_delay_param = self.current.effect_parameter & 0x0F;
        }
    }
}

impl Iterator for Channel {
    type Item = (f32, f32);

    // Was next_of_sample()
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.instr {
            Some(i) => match i.next() {
                Some(fval) => Some((fval * self.actual_volume[0], fval * self.actual_volume[1])),
                None => None,
            },
            None => None,
        }
    }
}
