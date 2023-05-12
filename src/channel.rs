use bitflags::bitflags;
use std::sync::Arc;

use crate::helper::*;
use xmrs::prelude::*;

lazy_static! {
    pub static ref EMPTY_SLOT: PatternSlot = PatternSlot::default();
}

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
    pub module: Arc<Module>,
    pub note: f32,
    pub orig_note: f32, /* The original note before effect modifications, as read in the pattern. */
    pub instrnr: Option<usize>,
    pub sample: Option<usize>,
    pub current: PatternSlot,

    pub sample_position: f32, // TODO: Bug with rustification (using index not memory seek)
    pub period: f32,
    pub frequency: f32,
    pub step: f32,
    pub ping: bool, /* For ping-pong samples: true is -->, false is <-- */

    pub volume: f32,  /* Ideally between 0 (muted) and 1 (loudest) */
    pub panning: f32, /* Between 0 (left) and 1 (right); 0.5 is centered */

    pub autovibrato_ticks: u16,

    pub sustained: bool,
    pub fadeout_volume: f32,
    pub volume_envelope_volume: f32,
    pub panning_envelope_panning: f32,
    pub volume_envelope_frame_count: u16,
    pub panning_envelope_frame_count: u16,

    pub autovibrato_note_offset: f32,

    pub arp_in_progress: bool,
    pub arp_note_offset: u8,
    pub volume_slide_param: u8,
    pub fine_volume_slide_param: u8,
    pub global_volume_slide_param: u8,
    pub panning_slide_param: u8,
    pub portamento_up_param: u8,
    pub portamento_down_param: u8,
    pub fine_portamento_up_param: i8,
    pub fine_portamento_down_param: i8,
    pub extra_fine_portamento_up_param: i8,
    pub extra_fine_portamento_down_param: i8,
    pub tone_portamento_param: u8,
    pub tone_portamento_target_period: f32,
    pub multi_retrig_param: u8,
    pub note_delay_param: u8,
    pub pattern_loop_origin: u8, /* Where to restart a E6y loop */
    pub pattern_loop_count: u8,  /* How many loop passes have been done */

    pub vibrato_in_progress: bool,
    pub vibrato_waveform: Waveform,
    pub vibrato_waveform_retrigger: bool, /* True if a new note retriggers the waveform */
    pub vibrato_param: u8,
    pub vibrato_ticks: u16, /* Position in the waveform */
    pub vibrato_note_offset: f32,

    pub tremolo_waveform: Waveform,
    pub tremolo_waveform_retrigger: bool,
    pub tremolo_param: u8,
    pub tremolo_ticks: u16,
    pub tremolo_volume: f32,

    pub tremor_param: u8,
    pub tremor_on: bool,

    pub muted: bool,

    pub actual_volume: [f32; 2],

    // RAMPING START
    /* These values are updated at the end of each tick, to save
     * a couple of float operations on every generated sample. */
    pub target_volume: [f32; 2],
    pub frame_count: usize,
    pub end_of_previous_sample: [f32; SAMPLE_RAMPING_POINTS],
    // RAMPING END
    pub freq_type: ModuleFlag,
    pub rate: f32,
}

impl Channel {
    pub fn new(module: Arc<Module>, freq_type: ModuleFlag, rate: f32) -> Self {
        Self {
            module,
            ping: true,
            vibrato_waveform_retrigger: true,
            tremolo_waveform_retrigger: true,
            volume: 1.0,
            volume_envelope_volume: 1.0,
            fadeout_volume: 1.0,
            panning: 0.5,
            panning_envelope_panning: 0.5,
            freq_type,
            rate,
            ..Default::default()
        }
    }

    pub fn cut_note(&mut self) {
        /* NB: this is not the same as Key Off */
        self.volume = 0.0;
    }

    pub fn key_off(&mut self) {
        /* Key Off */
        self.sustained = false;

        /* If no volume envelope is used, also cut the note */
        if self.instrnr.is_none() {
            self.cut_note();
        } else {
            match &self.module.instrument[self.instrnr.unwrap()].instr_type {
                InstrumentType::Default(instr) => {
                    if !instr.volume_envelope.enabled {
                        self.cut_note();
                    }
                }
                _ => {
                    println!("XXX last case");
                    self.cut_note();
                } // TODO
            }
        }
    }

    pub fn autovibrato(&mut self) {
        if self.instrnr.is_some() {
            let chinstr = &self.module.instrument[self.instrnr.unwrap()].instr_type;
            match chinstr {
                InstrumentType::Default(instr) => {
                    if instr.vibrato.depth == 0.0 && self.autovibrato_note_offset != 0.0 {
                        self.autovibrato_note_offset = 0.0;
                    } else {
                        let sweep = if self.autovibrato_ticks < instr.vibrato.sweep as u16 {
                            /* No idea if this is correct, but it sounds close enough… */
                            lerp(
                                0.0,
                                1.0,
                                self.autovibrato_ticks as f32 / instr.vibrato.sweep as f32,
                            )
                        } else {
                            1.0
                        };
                        let step = (self.autovibrato_ticks * instr.vibrato.speed as u16) >> 2;
                        self.autovibrato_ticks = (self.autovibrato_ticks + 1) & 63;
                        self.autovibrato_note_offset = 0.25
                            * instr.vibrato.waveform.waveform(step)
                            * instr.vibrato.depth
                            * sweep;
                    }
                }
                _ => {} // TODO
            }
        } else {
            if self.autovibrato_note_offset != 0.0 {
                self.autovibrato_note_offset = 0.0;
            }
        }
        self.update_frequency();
    }

    pub fn vibrato(&mut self) {
        self.vibrato_ticks = (self.vibrato_ticks + (self.vibrato_param as u16 >> 4)) & 63;
        self.vibrato_note_offset = -2.0
            * self.vibrato_waveform.waveform(self.vibrato_ticks)
            * (self.vibrato_param & 0x0F) as f32
            / 15.0;
        self.update_frequency();
    }

    pub fn tremolo(&mut self) {
        let step = (self.tremolo_ticks * (self.tremolo_param as u16 >> 4)) & 63;
        /* Not so sure about this, it sounds correct by ear compared with
         * MilkyTracker, but it could come from other bugs */
        self.tremolo_volume =
            -1.0 * self.tremolo_waveform.waveform(step) * (self.tremolo_param & 0x0F) as f32 / 15.0;
        self.tremolo_ticks = (self.tremolo_ticks + 1) & 63;
    }

    pub fn arpeggio(&mut self, param: u8, tick: u16) {
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

    pub fn tone_portamento(&mut self) {
        /* 3xx called without a note, wait until we get an actual
         * target note. */
        if self.tone_portamento_target_period == 0.0 {
            return;
        }

        if self.period != self.tone_portamento_target_period {
            let incr: f32 = match self.freq_type {
                ModuleFlag::LinearFrequencies => 4.0,
                ModuleFlag::AmigaFrequencies => 1.0,
            };
            slide_towards(
                &mut self.period,
                self.tone_portamento_target_period,
                incr * self.tone_portamento_param as f32,
            );
            self.update_frequency();
        }
    }

    pub fn panning_slide(&mut self, rawval: u8) {
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

    pub fn volume_slide(&mut self, rawval: u8) {
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

    /// return (new_counter, value)
    fn envelope_tick(&self, env: &Envelope, counter: u16) -> (u16, f32) {
        let num_points = env.point.len();
        if num_points < 2 {
            /* Don't really know what to do… */
            if num_points == 1 {
                /* XXX I am pulling this out of my ass */
                let mut outval: f32 = env.point[0].value as f32 / 64.0;
                if outval > 1.0 {
                    outval = 1.0;
                }
                return (counter, outval);
            }
            return (counter, 0.0);
        } else {
            let mut counter = counter;

            if env.loop_enabled {
                let loop_start: u16 = env.point[env.loop_start_point as usize].frame;
                let loop_end: u16 = env.point[env.loop_end_point as usize].frame;
                let loop_length: u16 = loop_end - loop_start;

                if counter >= loop_end {
                    counter -= loop_length;
                }
            }

            let mut j: usize = 0;
            while j < (env.point.len() - 2) {
                if env.point[j].frame <= counter && env.point[j + 1].frame >= counter {
                    break;
                }
                j += 1;
            }

            let outval = EnvelopePoint::lerp(&env.point[j], &env.point[j + 1], counter) / 64.0;

            /* Make sure it is safe to increment frame count */
            let new_counter = if !self.sustained
                || !env.sustain_enabled
                || counter != env.point[env.sustain_point as usize].frame
            {
                counter + 1
            } else {
                counter
            };
            return (new_counter, outval);
        }
    }

    pub fn envelopes(&mut self) {
        if let Some(instr) = self.instrnr {
            match &self.module.instrument[instr].instr_type {
                InstrumentType::Default(instrument) => {
                    if instrument.volume_envelope.enabled {
                        if !self.sustained {
                            self.fadeout_volume -= instrument.volume_fadeout;
                            clamp_down(&mut self.fadeout_volume);
                        }
                        (
                            self.volume_envelope_frame_count,
                            self.volume_envelope_volume,
                        ) = self.envelope_tick(
                            &instrument.volume_envelope,
                            self.volume_envelope_frame_count,
                        );
                    }
                    if instrument.panning_envelope.enabled {
                        (
                            self.panning_envelope_frame_count,
                            self.panning_envelope_panning,
                        ) = self.envelope_tick(
                            &instrument.panning_envelope,
                            self.panning_envelope_frame_count,
                        );
                    }
                }
                _ => {} // TODO
            }
        }
    }

    pub fn next_of_sample(&mut self) -> f32 {
        if self.instrnr.is_none() || self.sample.is_none() || self.sample_position < 0.0 {
            // if RAMPING {
            //     if self.frame_count < SAMPLE_RAMPING_POINTS {
            //         return lerp(
            //             self.end_of_previous_sample[self.frame_count],
            //             0.0,
            //             self.frame_count as f32 / SAMPLE_RAMPING_POINTS as f32,
            //         );
            //     }
            // }
            return 0.0;
        }

        match &self.module.instrument[self.instrnr.unwrap()].instr_type {
            InstrumentType::Default(instr) => {
                let sample = &instr.sample[self.sample.unwrap()];
                let a: u32 = self.sample_position as u32;
                // LINEAR_INTERPOLATION START
                let b: u32 = a + 1;
                let t: f32 = self.sample_position - a as f32;
                // LINEAR_INTERPOLATION END
                let mut u: f32 = sample.at(a as usize);

                let loop_end = sample.loop_start + sample.loop_length;

                let v = match sample.flags {
                    LoopType::No => {
                        self.sample_position += self.step;
                        if self.sample_position >= sample.len() as f32 {
                            self.sample_position = -1.0;
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
                        self.sample_position += self.step;
                        while self.sample_position as u32 >= loop_end {
                            self.sample_position -= sample.loop_length as f32;
                        }

                        let seek = if b == loop_end { sample.loop_start } else { b };
                        // LINEAR_INTERPOLATION START
                        sample.at(seek as usize)
                        // LINEAR_INTERPOLATION END
                    }
                    LoopType::PingPong => {
                        if self.ping {
                            self.sample_position += self.step;
                        } else {
                            self.sample_position -= self.step;
                        }
                        /* XXX: this may not work for very tight ping-pong loops
                         * (ie switself.s direction more than once per sample */
                        if self.ping {
                            if self.sample_position as u32 >= loop_end {
                                self.ping = false;
                                self.sample_position =
                                    (loop_end << 1) as f32 - self.sample_position;
                            }
                            /* sanity self.cking */
                            if self.sample_position as usize >= sample.len() {
                                self.ping = false;
                                self.sample_position -= sample.len() as f32 - 1.0;
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

                            if self.sample_position as u32 <= sample.loop_start {
                                self.ping = true;
                                self.sample_position =
                                    (sample.loop_start << 1) as f32 - self.sample_position;
                            }
                            /* sanity self.cking */
                            if self.sample_position <= 0.0 {
                                self.ping = true;
                                self.sample_position = 0.0;
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
            _ => {
                // TODO
                return 0.0;
            }
        }
    }

    pub fn update_frequency(&mut self) {
        self.frequency = frequency(
            self.freq_type,
            self.period,
            self.arp_note_offset as f32,
            self.vibrato_note_offset + self.autovibrato_note_offset,
        );
        self.step = self.frequency / self.rate;
    }

    pub fn pitch_slide(&mut self, period_offset: i8) {
        /* Don't ask about the 0.4 coefficient. I found mention of it
         * nowhere. Found by ear™. */
        self.period += if let ModuleFlag::LinearFrequencies = self.freq_type {
            period_offset as f32 * 4.0
        } else {
            period_offset as f32
        };
        clamp_down(&mut self.period);
        /* XXX: upper bound of period ? */

        self.update_frequency();
    }

    pub fn trigger_note(&mut self, flags: TriggerKeep) {
        if !flags.contains(TriggerKeep::SAMPLE_POSITION) {
            self.sample_position = 0.0;
            self.ping = true;
        }

        if self.sample.is_some() {
            match &self.module.instrument[self.instrnr.unwrap()].instr_type {
                InstrumentType::Default(instr) => match self.sample {
                    Some(s) => {
                        let sample = &instr.sample[s];
                        if !flags.contains(TriggerKeep::VOLUME) {
                            self.volume = sample.volume;
                        }
                        self.panning = sample.panning;
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        if !flags.contains(TriggerKeep::ENVELOPE) {
            self.sustained = true;
            self.fadeout_volume = 1.0;
            self.volume_envelope_volume = 1.0;
            self.panning_envelope_panning = 0.5;
            self.volume_envelope_frame_count = 0;
            self.panning_envelope_frame_count = 0;
        }
        self.vibrato_note_offset = 0.0;
        self.tremolo_volume = 0.0;
        self.tremor_on = false;

        self.autovibrato_ticks = 0;

        if self.vibrato_waveform_retrigger {
            self.vibrato_ticks = 0; /* XXX: should the waveform itself also
                                     * be reset to sine? */
        }
        if self.tremolo_waveform_retrigger {
            self.tremolo_ticks = 0;
        }

        if !flags.contains(TriggerKeep::PERIOD) {
            self.period = period(self.freq_type, self.note);
            self.update_frequency();
        }
    }

    pub fn tick0(&mut self, pattern_slot: &PatternSlot) {
        self.current = pattern_slot.clone();

        if self.current.effect_type != 0xE || (self.current.effect_parameter >> 4) != 0xD {
            self.handle_note_and_instrument();
        } else {
            self.note_delay_param = self.current.effect_parameter & 0x0F;
        }
    }

    // XXX pub fn not needed with code refactoring
    pub fn handle_note_and_instrument(&mut self) {
        let noteu8: u8 = self.current.note.into();
        if self.current.instrument > 0 {
            if self.current.has_tone_portamento() && self.instrnr.is_some() && self.sample.is_some()
            {
                /* Tone portamento in effect, unclear stuff happens */
                self.trigger_note(TriggerKeep::PERIOD | TriggerKeep::SAMPLE_POSITION);
            } else if let Note::None = self.current.note {
                if self.sample.is_some() {
                    /* Ghost instrument, trigger note */
                    /* Sample position is kept, but envelopes are reset */
                    self.trigger_note(TriggerKeep::SAMPLE_POSITION);
                }
            } else if self.current.instrument as usize > self.module.instrument.len() {
                /* Invalid instrument, Cut current note */
                self.cut_note();
                self.instrnr = None;
                self.sample = None;
            } else {
                let instrnr = self.current.instrument as usize - 1;
                if let InstrumentType::Default(id) = &self.module.instrument[instrnr].instr_type {
                    // only good instr
                    if id.sample.len() != 0 {
                        self.instrnr = Some(instrnr);
                    }
                }
            }
        }

        if note_is_valid(noteu8) {
            if self.instrnr.is_some() {
                match &self.module.instrument[self.instrnr.unwrap()].instr_type {
                    InstrumentType::Empty => self.cut_note(),
                    InstrumentType::Default(instr) => {
                        if self.current.has_tone_portamento() {
                            if self.sample.is_some() {
                                /* Tone portamento in effect */
                                let chsample = &instr.sample[self.sample.unwrap()];
                                self.note = noteu8 as f32 - 1.0
                                    + chsample.relative_note as f32
                                    + chsample.finetune;
                                self.tone_portamento_target_period =
                                    period(self.module.flags, self.note);
                            } else if instr.sample.len() == 0 {
                                self.cut_note();
                            }
                        } else if (instr.sample_for_note[noteu8 as usize - 1] as usize)
                            < instr.sample.len()
                        {
                            // if RAMPING {
                            //     for z in 0..SAMPLE_RAMPING_POINTS {
                            //         self.end_of_previous_sample[z] = self.next_of_sample();
                            //     }
                            //     self.frame_count = 0;
                            // }
                            self.sample = Some(instr.sample_for_note[noteu8 as usize - 1] as usize);
                            let chsample = &instr.sample[self.sample.unwrap()];
                            self.orig_note = noteu8 as f32 - 1.0
                                + chsample.relative_note as f32
                                + chsample.finetune;
                            self.note = self.orig_note;

                            if self.current.instrument > 0 {
                                self.trigger_note(TriggerKeep::NONE);
                            } else {
                                /* Ghost note: keep old volume */
                                self.trigger_note(TriggerKeep::VOLUME);
                            }
                        }
                    }
                    _ => {} // TODO
                }
            } else {
                /* Bad instrument */
                self.cut_note();
            }
        } else if let Note::KeyOff = self.current.note {
            self.key_off();
        }

        match self.current.volume >> 4 {
            0x0 => {} // Nothing
            // V - Set volume (0..63)
            0x1..=0x4 => self.volume = (self.current.volume - 0x10) as f32 / 64.0,
            // V - 0x51..0x5F undefined...
            0x5 => self.volume = 1.0,
            // D - Volume slide down (0..15)
            0x6 => {} // see tick() fn
            // C - Volume slide up (0..15)
            0x7 => {} // see tick() fn
            // B - Fine volume down (0..15)
            0x8 => self.volume_slide(self.current.volume & 0x0F),
            // A - Fine volume up (0..15)
            0x9 => self.volume_slide(self.current.volume << 4),
            // U - Vibrato speed (0..15)
            0xA => {
                self.vibrato_param =
                    (self.vibrato_param & 0x0F) | ((self.current.volume & 0x0F) << 4)
            }
            // H - Vibrato depth (0..15)
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
            // G - Tone portamento (0..15)
            0xF => {
                if self.current.volume & 0x0F != 0 {
                    self.tone_portamento_param =
                        ((self.current.volume & 0x0F) << 4) | (self.current.volume & 0x0F);
                }
                // see also tick() fn
            }
            _ => {}
        }

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
                    self.vibrato_param =
                        (self.vibrato_param & 0xF0) | (self.current.effect_parameter & 0x0F);
                }
                if self.current.effect_parameter >> 4 != 0 {
                    /* Set vibrato speed */
                    self.vibrato_param =
                        (self.current.effect_parameter & 0xF0) | (self.vibrato_param & 0x0F);
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
                    self.tremolo_param =
                        (self.tremolo_param & 0xF0) | (self.current.effect_parameter & 0x0F);
                }
                if self.current.effect_parameter >> 4 != 0 {
                    /* Set tremolo speed */
                    self.tremolo_param =
                        (self.current.effect_parameter & 0xF0) | (self.tremolo_param & 0x0F);
                }
            }
            0x8 => {
                /* 8xx: Set panning */
                self.panning = self.current.effect_parameter as f32 / 255.0;
            }
            0x9 => {
                /* 9xx: Sample offset */
                if self.sample.is_some() && note_is_valid(noteu8) {
                    if let InstrumentType::Default(instr) =
                        &self.module.instrument[self.instrnr.unwrap()].instr_type
                    {
                        let chsample = &instr.sample[self.sample.unwrap()];

                        let final_offset = if chsample.bits() == 16 {
                            self.current.effect_parameter as usize * 256
                        } else {
                            self.current.effect_parameter as usize * 512
                        };

                        if final_offset >= chsample.len() {
                            /* Pretend the sample doesn't loop and is done playing */
                            self.sample_position = -1.0;
                        } else {
                            self.sample_position = final_offset as f32;
                        }
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
                        if note_is_valid(noteu8) && self.sample.is_some() {
                            if let InstrumentType::Default(instr) =
                                &self.module.instrument[self.instrnr.unwrap()].instr_type
                            {
                                let chsample = &instr.sample[self.sample.unwrap()];
                                let noteu8: u8 = self.current.note.into();
                                self.note = noteu8 as f32 - 1.0
                                    + chsample.relative_note as f32
                                    + (((self.current.effect_parameter & 0x0F) - 8) << 4) as f32
                                        / 128.0;
                                self.period = period(self.module.flags, self.note);
                                self.update_frequency();
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
            0x11 => {
                /* Hxy: Global volume slide */
                if self.current.effect_parameter > 0 {
                    self.global_volume_slide_param = self.current.effect_parameter;
                }
            }
            0x12..=0x14 => { /* Unused */ }
            0x15 => {
                /* Lxx: Set envelope position */
                self.volume_envelope_frame_count = self.current.effect_parameter as u16;
                self.panning_envelope_frame_count = self.current.effect_parameter as u16;
            }
            0x16..=0x18 => { /* Unused */ }
            0x19 => {
                /* Pxy: Panning slide */
                if self.current.effect_parameter > 0 {
                    self.panning_slide_param = self.current.effect_parameter;
                }
            }
            0x1A => { /* Unused */ }
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
            0x1C => { /* Unused */ }
            0x1D => {
                /* Txy: Tremor */
                if self.current.effect_parameter > 0 {
                    /* Tremor x and y params do not appear to be separately
                     * kept in memory, unlike Rxy */
                    self.tremor_param = self.current.effect_parameter;
                }
            }
            0x1E..=0x20 => { /* Unused */ }
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
}
