use bitflags::bitflags;

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
pub struct Channel<'c> {
    pub note: f32,
    pub orig_note: f32, /* The original note before effect modifications, as read in the pattern. */
    pub instrument: Option<&'c Instrument>,
    pub sample: Option<&'c Sample>,
    pub current: Option<&'c PatternSlot>,

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

impl<'c> Channel<'c> {
    pub fn new(freq_type: ModuleFlag, rate: f32) -> Self {
        Self {
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
        if self.instrument.is_none() {
            self.cut_note();
        } else {
            match &self.instrument.unwrap().instr_type {
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
        if self.instrument.is_some() {
            let chinstr = &self.instrument.unwrap().instr_type;
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
        if let Some(instr) = self.instrument {
            match &instr.instr_type {
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
        if self.instrument.is_none() || self.sample.is_none() || self.sample_position < 0.0 {
            if RAMPING {
                if self.frame_count < SAMPLE_RAMPING_POINTS {
                    return lerp(
                        self.end_of_previous_sample[self.frame_count],
                        0.0,
                        self.frame_count as f32 / SAMPLE_RAMPING_POINTS as f32,
                    );
                }
            }
            return 0.0;
        }

        let sample_len = if let Some(s) = self.sample {
            s.len()
        } else {
            return 0.0;
        };

        let sample = self.sample.unwrap();

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
                if self.sample_position >= sample_len as f32 {
                    self.sample_position = -1.0;
                }
                // LINEAR_INTERPOLATION START
                if b < sample_len as u32 {
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
                        self.sample_position = (loop_end << 1) as f32 - self.sample_position;
                    }
                    /* sanity self.cking */
                    if self.sample_position as usize >= sample_len {
                        self.ping = false;
                        self.sample_position -= sample_len as f32 - 1.0;
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

        if RAMPING {
            if self.frame_count < SAMPLE_RAMPING_POINTS {
                /* Smoothly transition between old and new sample. */
                return lerp(
                    self.end_of_previous_sample[self.frame_count],
                    endval,
                    self.frame_count as f32 / SAMPLE_RAMPING_POINTS as f32,
                );
            }
        }

        return endval;
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
            if !flags.contains(TriggerKeep::VOLUME) {
                self.volume = self.sample.unwrap().volume;
            }
            self.panning = self.sample.unwrap().panning;
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
}
