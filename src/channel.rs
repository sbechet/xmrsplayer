use bitflags::bitflags;
use std::sync::Arc;

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
    freq_type: FrequencyType,
    rate: f32,

    note: f32,
    orig_note: f32, /* The original note before effect modifications, as read in the pattern. */

    pub current: PatternSlot,

    period: f32,

    volume: f32,  /* Ideally between 0 (muted) and 1 (loudest) */
    panning: f32, /* Between 0 (left) and 1 (right); 0.5 is centered */

    // Instrument
    instr: Option<StateInstrDefault>,

    arp_in_progress: bool,
    arp_note_offset: u8,

    volume_slide_param: u8,
    fine_volume_slide_param: u8,

    panning_slide_param: u8,

    portamento_up_param: u8,
    portamento_down_param: u8,
    fine_portamento_up_param: i8,
    fine_portamento_down_param: i8,
    extra_fine_portamento_up_param: i8,
    extra_fine_portamento_down_param: i8,

    tone_portamento_param: u8,
    tone_portamento_target_period: f32,
    multi_retrig_param: u8,
    note_delay_param: u8,
    /// Where to restart a E6y loop
    pub pattern_loop_origin: u8,
    /// How many loop passes have been done
    pub pattern_loop_count: u8,

    vibrato_in_progress: bool,
    /// True if a new note retriggers the waveform
    vibrato_waveform_retrigger: bool,
    vibrato_waveform: Waveform,
    vibrato_depth: f32,
    vibrato_speed: u16,
    /// Position in the waveform
    vibrato_step: u16,
    vibrato_note_offset: f32,

    tremolo_waveform_retrigger: bool,
    tremolo_waveform: Waveform,
    tremolo_depth: f32,
    tremolo_speed: u16,
    tremolo_counter: u16,
    tremolo_volume: f32,

    tremor_param: u8,
    tremor_on: bool,

    pub muted: bool,

    pub actual_volume: [f32; 2],
}

impl Channel {
    pub fn new(module: Arc<Module>, freq_type: FrequencyType, rate: f32) -> Self {
        Self {
            module,
            freq_type,
            rate,
            vibrato_waveform_retrigger: true,
            tremolo_waveform_retrigger: true,
            volume: 1.0,
            panning: 0.5,
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

    fn vibrato(&mut self) {
        self.vibrato_step = (self.vibrato_step + self.vibrato_speed) & 63;
        self.vibrato_note_offset =
            -2.0 * self.vibrato_waveform.waveform(self.vibrato_step) * self.vibrato_depth;
    }

    fn tremolo(&mut self) {
        let step = (self.tremolo_counter * self.tremolo_speed) & 63;
        /* Not so sure about this, it sounds correct by ear compared with
         * MilkyTracker, but it could come from other bugs */
        self.tremolo_volume = -1.0 * self.tremolo_waveform.waveform(step) * self.tremolo_depth;
        self.tremolo_counter = (self.tremolo_counter + 1) & 63;
    }

    fn arpeggio(&mut self, current_tick: u16, tempo: u16, param: u8) {
        let arp_offset = tempo % 3;
        match current_tick {
            1 if arp_offset == 2 => {
                /* arp_offset==2: 0 -> x -> 0 -> y -> x -> … */
                self.arp_in_progress = true;
                self.arp_note_offset = param >> 4;
            }
            0 if arp_offset >= 1 => {
                /* arp_offset==1: 0 -> 0 -> y -> x -> … */
                self.arp_in_progress = false;
                self.arp_note_offset = 0;
            }
            _ => {
                /* 0 -> y -> x -> … */
                let tick = current_tick - arp_offset;
                match tick % 3 {
                    0 => {
                        self.arp_in_progress = false;
                        self.arp_note_offset = 0;
                    }
                    2 => {
                        self.arp_in_progress = true;
                        self.arp_note_offset = param >> 4;
                    }
                    1 => {
                        self.arp_in_progress = true;
                        self.arp_note_offset = param & 0x0F;
                    }
                    _ => {}
                }
            }
        }
    }

    fn tone_portamento(&mut self) {
        /* 3xx called without a note, wait until we get an actual
         * target note. */
        if self.tone_portamento_target_period == 0.0 {
            return;
        }

        if self.period != self.tone_portamento_target_period {
            let incr: f32 = match self.freq_type {
                FrequencyType::LinearFrequencies => 4.0,
                FrequencyType::AmigaFrequencies => 1.0,
            };
            slide_towards(
                &mut self.period,
                self.tone_portamento_target_period,
                incr * self.tone_portamento_param as f32,
            );
        }
    }

    fn panning_slide(&mut self, rawval: u8) {
        if (rawval & 0xF0 != 0) && (rawval & 0x0F != 0) {
            /* Illegal state */
            return;
        }
        if rawval & 0xF0 != 0 {
            /* Slide right */
            let f = (rawval >> 4) as f32 / 255.0;
            self.panning += f;
            clamp_up(&mut self.panning);
        } else {
            /* Slide left */
            let f = (rawval & 0x0F) as f32 / 255.0;
            self.panning -= f;
            clamp_down(&mut self.panning);
        }
    }

    fn volume_slide(&mut self, rawval: u8) {
        if (rawval & 0xF0 != 0) && (rawval & 0x0F != 0) {
            /* Illegal state */
            return;
        }
        if rawval & 0xF0 != 0 {
            /* Slide up */
            let f = (rawval >> 4) as f32 / 64.0;
            self.volume += f;
            clamp_up(&mut self.volume);
        } else {
            /* Slide down */
            let f = (rawval & 0x0F) as f32 / 64.0;
            self.volume -= f;
            clamp_down(&mut self.volume);
        }
    }

    fn pitch_slide(&mut self, period_offset: i8) {
        /* Don't ask about the 0.4 coefficient. I found mention of it
         * nowhere. Found by ear™. */
        self.period += if let FrequencyType::LinearFrequencies = self.freq_type {
            period_offset as f32 * 4.0
        } else {
            period_offset as f32
        };
        clamp_down(&mut self.period);
        /* XXX: upper bound of period ? */
    }

    pub fn trigger_note(&mut self, flags: TriggerKeep) {
        match &mut self.instr {
            Some(instr) => {
                if !flags.contains(TriggerKeep::SAMPLE_POSITION) {
                    instr.sample_reset();
                }

                if !flags.contains(TriggerKeep::ENVELOPE) {
                    instr.envelopes_reset();
                }

                instr.vibrato_reset();

                match &instr.state_sample {
                    Some(s) => {
                        if !flags.contains(TriggerKeep::VOLUME) {
                            self.volume = s.get_volume();
                        }
                        self.panning = s.get_panning();
                    }
                    None => {}
                }

                if !flags.contains(TriggerKeep::PERIOD) {
                    self.period = period(self.freq_type, self.note);
                    instr.update_frequency(
                        self.period,
                        self.arp_note_offset as f32,
                        self.vibrato_note_offset,
                    );
                }
            }
            None => {}
        }

        self.vibrato_note_offset = 0.0;
        self.tremolo_volume = 0.0;
        self.tremor_on = false;

        if self.vibrato_waveform_retrigger {
            self.vibrato_step = 0; /* XXX: should the waveform itself also
                                    * be reset to sine? */
        }

        if self.tremolo_waveform_retrigger {
            self.tremolo_counter = 0;
        }
    }

    fn tick_effects(&mut self, current_tick: u16, tempo: u16) {
        match self.current.effect_type {
            0 => {
                /* 0xy: Arpeggio */
                if self.current.effect_parameter > 0 {
                    self.arpeggio(current_tick, tempo, self.current.effect_parameter);
                }
            }
            1 if current_tick != 0 => {
                /* 1xx: Portamento up */
                let period_offset = -(self.portamento_up_param as i8);
                self.pitch_slide(period_offset);
            }
            2 if current_tick != 0 => {
                /* 2xx: Portamento down */
                let period_offset = self.portamento_down_param as i8;
                self.pitch_slide(period_offset);
            }
            3 if current_tick != 0 => {
                /* 3xx: Tone portamento */
                self.tone_portamento();
            }
            4 if current_tick != 0 => {
                /* 4xy: Vibrato */
                self.vibrato_in_progress = true;
                self.vibrato();
            }
            5 if current_tick != 0 => {
                /* 5xy: Tone portamento + Volume slide */
                self.tone_portamento();
                let rawval = self.volume_slide_param;
                self.volume_slide(rawval);
            }
            6 if current_tick != 0 => {
                /* 6xy: Vibrato + Volume slide */
                self.vibrato_in_progress = true;
                self.vibrato();
                let rawval = self.volume_slide_param;
                self.volume_slide(rawval);
            }
            7 if current_tick != 0 => {
                /* 7xy: Tremolo */
                self.tremolo();
            }
            0xA if current_tick != 0 => {
                /* Axy: Volume slide */
                let rawval = self.volume_slide_param;
                self.volume_slide(rawval);
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
                        if (self.current.effect_parameter as u16 & 0x0F) == current_tick {
                            self.cut_note();
                        }
                    }
                    0xD => {
                        /* EDy: Note delay */
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
                /* Most documentations will tell you the parameter has no
                 * use. Don't be fooled. */
                if current_tick == self.current.effect_parameter as u16 {
                    self.key_off();
                }
            }
            0x19 if current_tick != 0 => {
                /* Pxy: Panning slide */
                let rawval = self.panning_slide_param;
                self.panning_slide(rawval);
            }
            0x1B if current_tick != 0 => {
                /* Rxy: Multi retrig note */
                if ((self.multi_retrig_param) & 0x0F) != 0 {
                    let r = current_tick % (self.multi_retrig_param as u16 & 0x0F);
                    if r == 0 {
                        self.trigger_note(TriggerKeep::VOLUME | TriggerKeep::ENVELOPE);

                        /* Rxy doesn't affect volume if there's a command in the volume
                        column, or if the instrument has a volume envelope. */
                        match &self.instr {
                            Some(instr) => {
                                if self.current.volume != 0 && !instr.volume_envelope.enabled {
                                    let mut v = self.volume
                                        * MULTI_RETRIG_MULTIPLY
                                            [self.multi_retrig_param as usize >> 4]
                                        + MULTI_RETRIG_ADD[self.multi_retrig_param as usize >> 4]
                                            / 64.0;
                                    clamp(&mut v);
                                    self.volume = v;
                                }
                            }
                            None => {}
                        }
                    }
                }
            }
            0x1D if current_tick != 0 => {
                /* Txy: Tremor */
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
                self.volume_slide(self.current.volume & 0x0F);
            }
            0x7 => {
                /* + - Volume slide up */
                self.volume_slide(self.current.volume << 4);
            }
            0xB => {
                /* V - Vibrato */
                self.vibrato_in_progress = false;
                self.vibrato();
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
                self.tone_portamento();
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, current_tick: u16, tempo: u16) {
        match &mut self.instr {
            Some(instr) => {
                instr.envelopes();
                instr.state_vibrato.tick();
            }
            None => return,
        }

        if self.arp_in_progress && !self.current.has_arpeggio() {
            self.arp_in_progress = false;
            self.arp_note_offset = 0;
        }
        if self.vibrato_in_progress && !self.current.has_vibrato() {
            self.vibrato_in_progress = false;
            self.vibrato_note_offset = 0.0;
        }

        if current_tick != 0 {
            self.tick_volume_effects();
        }

        self.tick_effects(current_tick, tempo);

        match &mut self.instr {
            Some(instr) => {
                let panning: f32 = self.panning
                    + (instr.envelope_panning.value - 0.5)
                        * (0.5 - (self.panning - 0.5).abs())
                        * 2.0;
                let mut volume = 0.0;

                if !self.tremor_on {
                    volume = self.volume + self.tremolo_volume;
                    clamp(&mut volume);
                    volume *= instr.envelope_volume_fadeout * instr.envelope_volume.value * instr.volume;
                }

                self.actual_volume[0] = volume * (1.0 - panning).sqrt();
                self.actual_volume[1] = volume * panning.sqrt();

                instr.update_frequency(
                    self.period,
                    self.arp_note_offset as f32,
                    self.vibrato_note_offset,
                );
            }
            None => {}
        }
    }

    fn tick0_effects(&mut self, noteu8: u8) {
        match self.current.effect_type {
            0x1 => {
                /* 1xx: Portamento up */
                if self.current.effect_parameter > 0 {
                    self.portamento_up_param = self.current.effect_parameter;
                }
            }
            0x2 => {
                /* 2xx: Portamento down */
                if self.current.effect_parameter > 0 {
                    self.portamento_down_param = self.current.effect_parameter;
                }
            }
            0x3 => {
                /* 3xx: Tone portamento */
                if self.current.effect_parameter > 0 {
                    self.tone_portamento_param = self.current.effect_parameter;
                }
            }
            0x4 => {
                /* 4xy: Vibrato */
                if self.current.effect_parameter & 0x0F != 0 {
                    /* Set vibrato depth */
                    self.vibrato_depth = (self.current.effect_parameter & 0x0F) as f32 / 15.0;
                }
                if self.current.effect_parameter >> 4 != 0 {
                    /* Set vibrato speed */
                    self.vibrato_speed = (self.current.effect_parameter >> 4) as u16;
                }
            }
            0x5 => {
                /* 5xy: Tone portamento + Volume slide */
                if self.current.effect_parameter > 0 {
                    self.volume_slide_param = self.current.effect_parameter;
                }
            }
            0x6 => {
                /* 6xy: Vibrato + Volume slide */
                if self.current.effect_parameter > 0 {
                    self.volume_slide_param = self.current.effect_parameter;
                }
            }
            0x7 => {
                /* 7xy: Tremolo */
                if self.current.effect_parameter & 0x0F != 0 {
                    /* Set tremolo depth */
                    self.tremolo_depth = (self.current.effect_parameter & 0x0F) as f32 / 15.0;
                }
                if self.current.effect_parameter >> 4 != 0 {
                    /* Set tremolo speed */
                    self.tremolo_speed = (self.current.effect_parameter >> 4) as u16;
                }
            }
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
                if self.current.effect_parameter > 0 {
                    self.volume_slide_param = self.current.effect_parameter;
                }
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
                        /* E1y: Fine portamento up */
                        if self.current.effect_parameter & 0x0F != 0 {
                            self.fine_portamento_up_param =
                                self.current.effect_parameter as i8 & 0x0F;
                        }
                        self.pitch_slide(-self.fine_portamento_up_param);
                    }
                    0x2 => {
                        /* E2y: Fine portamento down */
                        if self.current.effect_parameter & 0x0F != 0 {
                            self.fine_portamento_down_param =
                                self.current.effect_parameter as i8 & 0x0F;
                        }
                        self.pitch_slide(self.fine_portamento_down_param);
                    }
                    0x4 => {
                        /* E4y: Set vibrato control */
                        self.vibrato_waveform =
                            Waveform::try_from(self.current.effect_parameter & 3).unwrap();
                        self.vibrato_waveform_retrigger =
                            ((self.current.effect_parameter >> 2) & 1) == 0;
                    }
                    0x5 => {
                        /* E5y: Set finetune */
                        if note_is_valid(noteu8) {
                            match &mut self.instr {
                                Some(i) => match &mut i.state_sample {
                                    Some(s) => {
                                        self.note = noteu8 as f32 - 1.0
                                            + s.relative_note as f32
                                            + (((self.current.effect_parameter & 0x0F) as i8 - 8)
                                                << 4)
                                                as f32
                                                / 128.0;
                                        self.period = period(self.module.frequency_type, self.note);
                                    }
                                    None => {}
                                },
                                None => {}
                            }
                        }
                    }
                    0x7 => {
                        /* E7y: Set tremolo control */
                        self.tremolo_waveform =
                            Waveform::try_from(self.current.effect_parameter & 3).unwrap();
                        self.tremolo_waveform_retrigger =
                            ((self.current.effect_parameter >> 2) & 1) == 0;
                    }
                    0xA => {
                        /* EAy: Fine volume slide up */
                        if self.current.effect_parameter & 0x0F != 0 {
                            self.fine_volume_slide_param = self.current.effect_parameter & 0x0F;
                        }
                        self.volume_slide(self.fine_volume_slide_param << 4);
                    }
                    0xB => {
                        /* EBy: Fine volume slide down */
                        if self.current.effect_parameter & 0x0F != 0 {
                            self.fine_volume_slide_param = self.current.effect_parameter & 0x0F;
                        }
                        self.volume_slide(self.fine_volume_slide_param);
                    }
                    0xD => {
                        /* EDy: Note delay */
                        /* XXX: figure this out better. EDx triggers
                         * the note even when there no note and no
                         * instrument. But ED0 acts like like a ghost
                         * note, EDx (x ≠ 0) does not. */
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
                        i.envelope_panning.counter = self.current.effect_parameter as u16;
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
                if self.current.effect_parameter > 0 {
                    if (self.current.effect_parameter >> 4) == 0 {
                        /* Keep previous x value */
                        self.multi_retrig_param = (self.multi_retrig_param & 0xF0)
                            | (self.current.effect_parameter & 0x0F);
                    } else {
                        self.multi_retrig_param = self.current.effect_parameter;
                    }
                }
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
                        if self.current.effect_parameter & 0x0F != 0 {
                            self.extra_fine_portamento_up_param =
                                self.current.effect_parameter as i8 & 0x0F;
                        }
                        self.pitch_slide(-self.extra_fine_portamento_up_param);
                    }
                    2 => {
                        /* X2y: Extra fine portamento down */
                        if self.current.effect_parameter & 0x0F != 0 {
                            self.extra_fine_portamento_down_param =
                                self.current.effect_parameter as i8 & 0x0F;
                        }
                        self.pitch_slide(self.extra_fine_portamento_down_param);
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
            0x8 => self.volume_slide(self.current.volume & 0x0F),
            // U - Fine volume slide up (0..15)
            0x9 => self.volume_slide(self.current.volume << 4),
            // S - Vibrato speed (0..15)
            0xA => self.vibrato_speed = self.current.volume as u16 & 0x0F,
            // V - Vibrato depth (0..15)
            0xB => {} // see tick() fn
            // P - Set panning (2,6,10,14..62)
            0xC => {
                self.panning = (((self.current.volume & 0x0F) << 4) | (self.current.volume & 0x0F))
                    as f32
                    / 255.0
            }
            // L - Pan slide left (0..15)
            0xD => {} // see tick() fn
            // R - Pan slide right (0..15)
            0xE => {} // see tick() fn
            // M - Tone portamento (0..15)
            0xF => {
                if self.current.volume & 0x0F != 0 {
                    self.tone_portamento_param =
                        ((self.current.volume & 0x0F) << 4) | (self.current.volume & 0x0F);
                }
                // see also tick() fn
            }
            _ => {}
        }
    }

    fn tick0_load_note_and_instrument(&mut self) {
        let noteu8: u8 = self.current.note.into();

        // First, load instr
        if self.current.instrument > 0 {
            if self.current.has_tone_portamento() {
                /* Tone portamento in effect, unclear stuff happens */
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
                        let instr = StateInstrDefault::new(id.clone(), self.freq_type, self.rate);
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
                            Some(s) => {
                                if s.is_enabled() {
                                    self.note = noteu8 as f32 - 1.0 + s.get_finetuned_note();
                                    self.tone_portamento_target_period =
                                        period(self.module.frequency_type, self.note);
                                } else {
                                    self.cut_note();
                                }
                            }
                            None => self.cut_note(),
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
            match &mut self.instr {
                Some(instr) => instr.update_frequency(
                    self.period,
                    self.arp_note_offset as f32,
                    self.vibrato_note_offset,
                ),
                None => {}
            }
        } else {
            self.note_delay_param = self.current.effect_parameter & 0x0F;
        }
    }
}

impl Iterator for Channel {
    type Item = f32;

    // Was next_of_sample()
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.instr {
            Some(i) => i.next(),
            None => None,
        }
    }
}
