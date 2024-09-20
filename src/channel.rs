#[cfg(feature = "libm")]
use num_traits::float::Float;
#[cfg(feature = "micromath")]
use micromath::F32Ext;

use crate::effect::*;
use crate::effect_arpeggio::EffectArpeggio;
use crate::effect_multi_retrig_note::EffectMultiRetrigNote;
use crate::effect_portamento::EffectPortamento;
use crate::effect_toneportamento::EffectTonePortamento;
use crate::effect_vibrato_tremolo::EffectVibratoTremolo;
use crate::effect_volume_panning_slide::EffectVolumePanningSlide;
use crate::historical_helper::HistoricalHelper;
use crate::period_helper::PeriodHelper;
use crate::triggerkeep::*;

use crate::helper::*;
use crate::state_instr_default::StateInstrDefault;
use xmrs::prelude::*;

#[derive(Clone)]
pub struct Channel<'a> {
    module: &'a Module,
    historical: Option<HistoricalHelper>,
    period_helper: PeriodHelper,
    rate: f32,

    note: f32,

    pub current: PatternSlot,

    period: f32,

    volume: f32,  /* Ideally between 0 (muted) and 1 (loudest) */
    panning: f32, /* Between 0 (left) and 1 (right); 0.5 is centered */

    // Instrument
    instr: Option<StateInstrDefault<'a>>,

    arpeggio: EffectArpeggio,
    multi_retrig_note: EffectMultiRetrigNote,
    panning_slide: EffectVolumePanningSlide,

    // one memory for each shift effect
    portamento_up: EffectPortamento,
    portamento_down: EffectPortamento,
    portamento_fine_up: EffectPortamento,
    portamento_fine_down: EffectPortamento,
    portamento_extrafine_up: EffectPortamento,
    portamento_extrafine_down: EffectPortamento,

    tone_portamento: EffectTonePortamento,
    tremolo: EffectVibratoTremolo,
    volume_slide: EffectVolumePanningSlide,
    volume_slide_tick0: EffectVolumePanningSlide,
    vibrato: EffectVibratoTremolo,

    semitone: bool,

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

impl<'a> Channel<'a> {
    pub fn new(module: &'a Module, rate: f32, historical: Option<HistoricalHelper>) -> Self {
        let period_helper = PeriodHelper::new(module.frequency_type, historical.is_some());
        Self {
            module,
            historical: historical.clone(),
            period_helper: period_helper.clone(),
            rate,
            volume: 1.0,
            panning: 0.5,
            arpeggio: EffectArpeggio::new(historical.clone()),
            tone_portamento: EffectTonePortamento::new(period_helper.clone()),
            vibrato: EffectVibratoTremolo::vibrato(&period_helper),
            tremolo: EffectVibratoTremolo::tremolo(),
            multi_retrig_note: EffectMultiRetrigNote::new(historical, 0.0, 0.0),
            note: 0.0,
            current: PatternSlot::default(),
            period: 0.0,
            instr: None,
            panning_slide: EffectVolumePanningSlide::default(),
            portamento_up: EffectPortamento::default(),
            portamento_down: EffectPortamento::default(),
            portamento_fine_up: EffectPortamento::default(),
            portamento_fine_down: EffectPortamento::default(),
            portamento_extrafine_up: EffectPortamento::default(),
            portamento_extrafine_down: EffectPortamento::default(),
            volume_slide: EffectVolumePanningSlide::default(),
            volume_slide_tick0: EffectVolumePanningSlide::default(),
            semitone: false,
            note_delay_param: 0,
            pattern_loop_origin: 0,
            pattern_loop_count: 0,
            tremor_param: 0,
            tremor_on: false,
            muted: false,
            actual_volume: [0.0, 0.0],
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

    pub fn cut_note(&mut self) {
        /* NB: this is not the same as Key Off */
        self.volume = 0.0;
    }

    fn key_off_historical(&mut self, tick: u16) {
        if let Some(i) = &mut self.instr {
            i.key_off();
            // openmpt `key_off.xm`: Key off at tick 0 (K00) is very dodgy command. If there is a note next to it, the note is ignored. If there is a volume column command or instrument next to it and the current instrument has no volume envelope, the note is faded out instead of being cut.
            if (tick == 0
                && (i.has_volume_envelope()
                    || self.current.instrument != 0
                    || self.current.volume != 0))
                || (tick != 0 && i.has_volume_envelope())
            {
                self.trigger_note(
                    TRIGGER_KEEP_VOLUME | TRIGGER_KEEP_PERIOD | TRIGGER_KEEP_ENVELOPE,
                );
            } else {
                self.cut_note();
            }
        } else {
            self.cut_note();
        }
    }

    fn key_off(&mut self, tick: u16) {
        if let Some(_hhelper) = &self.historical {
            self.key_off_historical(tick);
            return;
        }

        if let Some(i) = &mut self.instr {
            i.key_off();
        } else {
            self.cut_note();
        }
    }

    pub fn trigger_note(&mut self, flags: TriggerKeep) {
        self.tremor_on = false;

        match &mut self.instr {
            Some(instr) => {
                if !contains(flags, TRIGGER_KEEP_SAMPLE_POSITION) {
                    instr.sample_reset();
                }

                if !contains(flags, TRIGGER_KEEP_ENVELOPE) {
                    instr.envelopes_reset();
                }

                instr.vibrato_reset();

                if !contains(flags, TRIGGER_KEEP_VOLUME) {
                    instr.volume_reset();
                    self.volume = instr.volume;
                }

                // TODO
                self.panning = instr.panning;

                if !contains(flags, TRIGGER_KEEP_PERIOD) {
                    self.period = self.period_helper.note_to_period(self.note);
                    instr.update_frequency(self.period, 0.0, self.vibrato.value(), self.semitone);
                }
            }
            None => {}
        }
    }

    pub fn tickn_update_instr(&mut self) {
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

                let arp_note = if self.current.has_arpeggio() {
                    self.arpeggio.value()
                } else {
                    0.0
                };

                instr.update_frequency(self.period, arp_note, self.vibrato.value(), self.semitone)
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
            1 if current_tick != 0 => {
                /* 1xx: Portamento up */
                self.portamento_up.tick();
                self.period = self.portamento_up.clamp(self.period);
            }
            2 if current_tick != 0 => {
                /* 2xx: Portamento down */
                self.portamento_down.tick();
                self.period = self.portamento_down.clamp(self.period);
            }
            3 if current_tick != 0 => {
                /* 3xx: Tone portamento */
                self.tone_portamento.tick();
                self.period = self.tone_portamento.clamp(self.period);
            }
            4 if current_tick != 0 => {
                /* 4xy: Vibrato */
                self.vibrato.tick();
            }
            5 if current_tick != 0 => {
                /* 5xy: Tone portamento + Volume slide */
                self.tone_portamento.tick();
                self.period = self.tone_portamento.clamp(self.period);
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
                            if r == 0 {
                                self.trigger_note(TRIGGER_KEEP_VOLUME);
                                match &mut self.instr {
                                    Some(instr) => {
                                        instr.tick();
                                    }
                                    None => {}
                                }
                            }
                        }
                    }
                    0xC => {
                        /* ECy: Note cut */
                        if (self.current.effect_parameter as u16 & 0x0F) == current_tick {
                            self.cut_note();
                        }
                    }
                    0xD => {
                        /* EDy: Note delay */
                        if self.note_delay_param as u16 == current_tick {
                            self.tick0_load_instrument_and_note();
                            // Volume effect
                            self.tick0_volume_effects();
                            // Effects
                            self.tick0_effects();

                            /* Special KeyOff cases */
                            if let Note::KeyOff = self.current.note {
                                if self.current.instrument == 0 {
                                    if let Some(i) = &mut self.instr {
                                        i.volume_reset();
                                    }
                                } else {
                                    self.trigger_note(TRIGGER_KEEP_NONE);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            0x14 => {
                /* Kxx: Key off */
                if current_tick == self.current.effect_parameter as u16 {
                    self.key_off(current_tick);
                }
            }
            0x19 if current_tick != 0 => {
                /* Pxy: Panning slide */
                self.panning += self.panning_slide.tick();
                self.panning = self.panning_slide.clamp(self.panning);
            }
            0x1B if current_tick != 0 => {
                /* Rxy: Multi retrig note */
                if self.multi_retrig_note.tick() == 0.0 {
                    self.trigger_note(TRIGGER_KEEP_VOLUME | TRIGGER_KEEP_ENVELOPE);
                    match &self.instr {
                        Some(instr) => {
                            if self.volume == 0.0 && !instr.volume_envelope.enabled {
                                self.volume = self.multi_retrig_note.clamp(self.volume);
                                // priority on original volume
                                if self.current.volume >= 0x10 && self.current.volume <= 0x50 {
                                    self.volume = (self.current.volume - 0x10) as f32 / 64.0;
                                }
                                // priority on original panning
                                if self.current.volume >= 0xC0 && self.current.volume <= 0xCF {
                                    self.panning = (self.current.volume & 0x0F) as f32 / 16.0;
                                }
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
                self.volume_slide
                    .xm_update_effect(self.current.volume, 2, 64.0);
                self.volume += self.volume_slide.tick();
            }
            0x7 => {
                /* + - Volume slide up */
                self.volume_slide
                    .xm_update_effect(self.current.volume, 1, 64.0);
                self.volume += self.volume_slide.tick();
            }
            0xB => {
                /* V - Vibrato */
                self.vibrato.tick();
            }
            0xF => {
                /* M - Tone portamento */
                self.tone_portamento.tick();
                self.period = self.tone_portamento.clamp(self.period);
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, current_tick: u16) {
        match &mut self.instr {
            Some(instr) => {
                instr.tick();
            }
            None => {
                if self.current.has_note_delay() {
                    self.tick_effects(current_tick);
                    self.tickn_update_instr();
                }
                return;
            }
        }
        self.tick_volume_effects();
        self.tick_effects(current_tick);
        self.tickn_update_instr();
    }

    fn tick0_effects(&mut self) {
        match self.current.effect_type {
            0x0 => self
                .arpeggio
                .xm_update_effect(self.current.effect_parameter, 0, 0.0),
            0x1 => {
                self.portamento_up
                    .xm_update_effect(self.current.effect_parameter, 0, 1.0);
            }
            0x2 => {
                self.portamento_down
                    .xm_update_effect(self.current.effect_parameter, 0, 0.0);
            }
            0x3 => {
                self.tone_portamento
                    .xm_update_effect(self.current.effect_parameter, 1, self.note);
            }
            0x4 => self
                .vibrato
                .xm_update_effect(self.current.effect_parameter, 0, 0.0),
            0x5 => {
                /* 5xy: Tone portamento + Volume slide */
                self.volume_slide
                    .xm_update_effect(self.current.effect_parameter, 0, 64.0);
            }
            0x6 => {
                /* 6xy: Vibrato + Volume slide */
                self.volume_slide
                    .xm_update_effect(self.current.effect_parameter, 0, 64.0);
            }
            0x7 => self
                .tremolo
                .xm_update_effect(self.current.effect_parameter, 0, 0.0),
            0x8 => {
                /* 8xx: Set panning */
                self.panning = self.current.effect_parameter as f32 / 256.0;
            }
            0x9 => {
                /* 9xx: Sample offset */
                if self.current.note.is_valid() {
                    match &mut self.instr {
                        Some(i) => match &mut i.state_sample {
                            Some(s) => {
                                s.set_position(self.current.effect_parameter as usize * 256);
                            }
                            None => {}
                        },
                        None => {}
                    }
                }
            }
            0xA => {
                /* Axy: Volume slide */
                self.volume_slide
                    .xm_update_effect(self.current.effect_parameter, 0, 64.0);
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
                        self.portamento_fine_up.xm_update_effect(
                            self.current.effect_parameter,
                            1,
                            1.0,
                        );
                        self.period = self.portamento_fine_up.clamp(self.period);
                    }
                    0x2 => {
                        /* E2y: Fine portamento down */
                        self.portamento_fine_down.xm_update_effect(
                            self.current.effect_parameter,
                            1,
                            0.0,
                        );
                        self.period = self.portamento_fine_down.clamp(self.period);
                    }
                    0x3 => {
                        /* E3y: Set glissando control */
                        self.semitone = self.current.effect_parameter != 0;
                    }
                    0x4 => {
                        /* E4y: Set vibrato control */
                        // TODO: more abstraction to be done one day here!
                        self.vibrato.data.waveform = self.current.effect_parameter & 3;
                        if ((self.current.effect_parameter >> 2) & 1) == 0 {
                            self.vibrato.retrigger();
                        }
                    }
                    0x5 => {
                        /* E5y: Set finetune */
                        if self.current.note.is_valid() {
                            match &mut self.instr {
                                Some(i) => {
                                    let finetune =
                                        (self.current.effect_parameter & 0x0F) as f32 / 8.0 - 1.0;
                                    i.set_finetune(finetune);
                                    self.note = self.current.note.value() as f32 - 1.0
                                        + i.get_finetuned_note();
                                    self.period = self.period_helper.note_to_period(self.note);
                                }
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
                    0x9 => {
                        /* E90: Retrigger note */
                        if self.current.effect_parameter & 0x0F == 0 {
                            self.trigger_note(TRIGGER_KEEP_VOLUME);
                            match &mut self.instr {
                                Some(instr) => {
                                    instr.tick();
                                }
                                None => {}
                            }
                        }
                    }
                    0xA => {
                        /* EAy: Fine volume slide up */
                        self.volume_slide_tick0.xm_update_effect(
                            self.current.effect_parameter,
                            1,
                            64.0,
                        );
                        self.volume += self.volume_slide_tick0.tick();
                    }
                    0xB => {
                        /* EBy: Fine volume slide down */
                        self.volume_slide_tick0.xm_update_effect(
                            self.current.effect_parameter,
                            2,
                            64.0,
                        );
                        self.volume += self.volume_slide_tick0.tick();
                    }
                    0xD => {
                        /* ED0: Note with no delay */
                        if self.current.effect_parameter & 0xF0 == 0 {
                            match self.current.note {
                                Note::None => {
                                    self.trigger_note(
                                        TRIGGER_KEEP_SAMPLE_POSITION
                                            | TRIGGER_KEEP_VOLUME
                                            | TRIGGER_KEEP_PERIOD,
                                    );
                                }
                                Note::KeyOff => {
                                    if self.current.instrument == 0 {
                                        self.key_off(0);
                                    } else {
                                        self.trigger_note(
                                            TRIGGER_KEEP_PERIOD | TRIGGER_KEEP_ENVELOPE,
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            0x14 => {
                /* Kxx: Key off */
                if 0 == self.current.effect_parameter as u16 {
                    self.key_off(0);
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
                self.panning_slide
                    .xm_update_effect(self.current.effect_parameter, 0, 16.0);
                self.panning = self.panning_slide.clamp(self.panning);
            }
            0x1B => {
                /* Rxy: Multi retrig note */
                self.multi_retrig_note
                    .xm_update_effect(self.current.effect_parameter, 0, 0.0);
            }
            0x1D => {
                /* Txy: Tremor */
                if self.current.effect_parameter > 0 {
                    self.tremor_param = self.current.effect_parameter;
                }
            }
            0x21 => {
                /* Xxy: Extra stuff */
                match self.current.effect_parameter >> 4 {
                    1 => {
                        /* X1y: Extra fine portamento up */
                        self.portamento_extrafine_up.xm_update_effect(
                            self.current.effect_parameter,
                            2,
                            1.0,
                        );
                        self.period = self.portamento_extrafine_up.clamp(self.period);
                    }
                    2 => {
                        /* X2y: Extra fine portamento down */
                        self.portamento_extrafine_down.xm_update_effect(
                            self.current.effect_parameter,
                            2,
                            0.0,
                        );
                        self.period = self.portamento_extrafine_down.clamp(self.period);
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
            0x5 => self.volume = (self.current.volume - 0x20) as f32 / 64.0,
            // - - Volume slide down (0..15)
            0x6 => {} // see tick() fn
            // + - Volume slide up (0..15)
            0x7 => {} // see tick() fn
            // D - Fine volume slide down (0..15)
            0x8 => {
                self.volume_slide
                    .xm_update_effect(self.current.volume, 2, 64.0);
                self.volume += self.volume_slide.tick();
            }
            // U - Fine volume slide up (0..15)
            0x9 => {
                self.volume_slide
                    .xm_update_effect(self.current.volume, 1, 64.0);
                self.volume += self.volume_slide.tick();
            }
            // S - Vibrato speed (0..15)
            0xA => self.vibrato.xm_update_effect(self.current.volume, 1, 0.0),
            // V - Vibrato depth (0..15)
            0xB => {} // see tick() fn
            // P - Set panning
            0xC => self.panning = (self.current.volume & 0x0F) as f32 / 16.0,
            0xD => {
                /* L - Panning slide left */
                self.panning_slide
                    .xm_update_effect(self.current.volume, 2, 16.0);
                self.panning += self.panning_slide.tick();
                self.panning = self.panning_slide.clamp(self.panning);
            }
            0xE => {
                /* R - Panning slide right */
                self.panning_slide
                    .xm_update_effect(self.current.volume, 1, 16.0);
                self.panning += self.panning_slide.tick();
                self.panning = self.panning_slide.clamp(self.panning);
            }
            // M - Tone portamento (0..15)
            0xF => {
                self.tone_portamento
                    .xm_update_effect(self.current.volume & 0x0F, 16, self.note);
            }
            _ => {}
        }
    }

    /// change instr and return true if it was the same
    fn tick0_change_instr(&mut self, sample_only: bool) -> bool {
        let instrnr = self.current.instrument as usize - 1;
        if let InstrumentType::Default(id) = &self.module.instrument[instrnr].instr_type {
            let was_same = if let Some(i) = &mut self.instr {
                i.num == instrnr
            } else {
                false
            };
            // only good instr
            if id.sample.len() != 0 {
                if sample_only {
                    match &mut self.instr {
                        Some(i) => i.replace_instr(id),
                        _ => {}
                    }
                } else {
                    let instr =
                        StateInstrDefault::new(id, instrnr, self.period_helper.clone(), self.rate);
                    self.instr = Some(instr);
                }
            }
            was_same
        } else {
            // TODO
            false
        }
    }

    /// return true if it was the same instrument
    fn tick0_load_instrument(&mut self) -> bool {
        if self.current.instrument > 0 {
            if self.current.instrument as usize > self.module.instrument.len() {
                /* Invalid instrument, Cut current note */
                self.cut_note();
                self.instr = None;
                return false;
            } else if self.current.has_tone_portamento() {
                self.trigger_note(TRIGGER_KEEP_PERIOD | TRIGGER_KEEP_SAMPLE_POSITION);
                return self.tick0_change_instr(true);
            } else if let Note::None = self.current.note {
                /* Ghost instrument, trigger note */
                if self.current.has_volume_slide() {
                    self.trigger_note(TRIGGER_KEEP_SAMPLE_POSITION | TRIGGER_KEEP_PERIOD);
                } else {
                    /* Sample position is kept, but envelopes are reset */
                    self.trigger_note(
                        TRIGGER_KEEP_SAMPLE_POSITION | TRIGGER_KEEP_VOLUME | TRIGGER_KEEP_PERIOD,
                    );
                }
                return self.tick0_change_instr(true);
            } else if !self.current.note.is_keyoff() {
                return self.tick0_change_instr(false);
            } else if self.current.note.is_keyoff() {
                self.trigger_note(TRIGGER_KEEP_PERIOD);
            }
        }
        return true;
    }

    fn tick0_load_note(&mut self, new_instr: bool) {
        if self.current.note.is_valid() {
            match &mut self.instr {
                Some(i) => {
                    if self.current.has_tone_portamento() {
                        match &i.state_sample {
                            Some(s) if s.is_enabled() => {
                                self.note =
                                    self.current.note.value() as f32 - 1.0 + s.get_finetuned_note()
                            }
                            _ => self.cut_note(),
                        }
                    } else if i.set_note(self.current.note) {
                        if let Some(s) = &i.state_sample {
                            self.note =
                                self.current.note.value() as f32 - 1.0 + s.get_finetuned_note();
                        }

                        if self.current.instrument > 0 {
                            self.trigger_note(TRIGGER_KEEP_NONE);
                        } else {
                            /* Ghost note: keep old volume */
                            self.trigger_note(TRIGGER_KEEP_VOLUME);
                        }
                    } else {
                        self.cut_note();
                    }
                }
                None => self.cut_note(),
            }
        } else if let Note::KeyOff = self.current.note {
            if self.current.instrument == 0 || new_instr {
                self.key_off(0);
            } else {
                self.trigger_note(TRIGGER_KEEP_PERIOD | TRIGGER_KEEP_ENVELOPE);
            }
        }
    }

    fn tick0_load_instrument_and_note(&mut self) {
        if let Some(_hhelper) = &self.historical {
            if self.current.effect_type == 0x14 {
                // Historical Kxy effect bug
                return;
            }
        }

        // First, load instr
        let new_instr: bool = self.tick0_load_instrument();
        // Next, choose sample from note
        self.tick0_load_note(new_instr);
    }

    pub fn tick0(&mut self, pattern_slot: &PatternSlot) {
        self.current = pattern_slot.clone();

        if !self.current.has_note_delay()
            || (self.current.has_note_delay() && self.current.effect_parameter & 0x0F == 0)
        {
            /* load instrument then note */
            self.tick0_load_instrument_and_note();
            // Volume effect
            self.tick0_volume_effects();
            // Effects
            self.tick0_effects();

            if self.arpeggio.in_progress() && !self.current.has_arpeggio() {
                self.arpeggio.retrigger();
            }

            if self.vibrato.in_progress() && !self.current.has_vibrato() {
                self.vibrato.retrigger();
            }

            self.tickn_update_instr();
        } else {
            self.note_delay_param = self.current.effect_parameter & 0x0F;
        }
    }
}

impl<'a> Iterator for Channel<'a> {
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
