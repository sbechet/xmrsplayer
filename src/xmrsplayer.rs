use crate::channel::{Channel, TriggerKeep, EMPTY_SLOT};
use crate::helper::*;
use xmrs::prelude::*;

pub struct XmrsPlayer<'m, 'c> {
    module: &'m Module,
    sample_rate: f32,

    tempo: u16,
    bpm: u16,
    global_volume: f32,
    /// Global amplification (default 1/4)
    pub amplification: f32,

    // RAMPING START
    /* How much is a channel final volume allowed to change per
     * sample; this is used to avoid abrubt volume changes which
     * manifest as "clicks" in the generated sound. */
    volume_ramp: f32,
    // RAMPING END
    current_table_index: u16,
    current_row: u8,
    current_tick: u16, /* Can go below 255, with high tempo and a pattern delay */
    remaining_samples_in_tick: f32,
    generated_samples: u64,

    position_jump: bool,
    pattern_break: bool,
    jump_dest: u16,
    jump_row: u8,

    /* Extra ticks to be played before going to the next row -
     * Used for EEy effect */
    extra_ticks: u16,

    channel: Vec<Channel<'c>>,

    row_loop_count: Vec<Vec<u8>>,
    loop_count: u8,
    max_loop_count: u8,

    debug: bool,
}

impl<'m: 'c, 'c> XmrsPlayer<'m, 'c> {
    pub fn new(module: &'m Module, sample_rate: f32) -> Self {
        let num_channels = module.get_num_channels();
        Self {
            module,
            sample_rate,
            tempo: module.default_tempo,
            bpm: module.default_bpm,
            global_volume: 1.0,
            amplification: 0.25,
            volume_ramp: 1.0 / 128.0,
            current_table_index: 0,
            current_row: 0,
            current_tick: 0,
            remaining_samples_in_tick: 0.0,
            generated_samples: 0,
            position_jump: false,
            pattern_break: false,
            jump_dest: 0,
            jump_row: 0,
            extra_ticks: 0,
            channel: vec![Channel::new(module.flags, sample_rate); num_channels],
            row_loop_count: vec![vec![0; MAX_NUM_ROWS]; module.get_song_length()],
            loop_count: 0,
            max_loop_count: 0,
            debug: false,
        }
    }

    pub fn debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    pub fn set_max_loop_count(&mut self, max_loop_count: u8) {
        self.max_loop_count = max_loop_count;
    }

    pub fn get_loop_count(&self) -> u8 {
        self.loop_count
    }

    pub fn get_sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// do a manual goto
    pub fn goto(&mut self, table_position: usize, row: usize) -> bool {
        if table_position < self.module.get_song_length() {
            let num_row = self.module.pattern_order[table_position] as usize;
            if row < self.module.get_num_rows(num_row) {
                // Create a position jump
                self.jump_dest = table_position as u16;
                self.jump_row = row as u8;
                self.position_jump = true;

                // Cleanup self
                self.tempo = self.module.default_tempo;
                self.bpm = self.module.default_bpm;
                self.global_volume = 1.0;

                // Cleanup channels
                let num_channels = self.module.get_num_channels();
                for i in 0..num_channels {
                    self.channel[i].trigger_note(TriggerKeep::PERIOD); // clean what we can
                }

                // next() must call tick() then row()
                self.remaining_samples_in_tick = 0.0;
                self.current_tick = 0;

                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn post_pattern_change(&mut self) {
        /* Loop if necessary */
        if self.current_table_index as usize >= self.module.pattern_order.len() {
            self.current_table_index = self.module.restart_position;
        }
        if self.debug {
            println!(
                "pattern_order[0x{:02x}] = 0x{:02x}",
                self.current_table_index,
                self.module.pattern_order[self.current_table_index as usize]
            );
        }
    }

    fn handle_note_and_instrument(&mut self, ch_index: usize, s: &PatternSlot) {
        let ch = &mut self.channel[ch_index];
        let noteu8: u8 = s.note.into();
        if s.instrument > 0 {
            if ch.current.unwrap_or(&EMPTY_SLOT).has_tone_portamento()
                && ch.instrument.is_some()
                && ch.sample.is_some()
            {
                /* Tone portamento in effect, unclear stuff happens */
                ch.trigger_note(TriggerKeep::PERIOD | TriggerKeep::SAMPLE_POSITION);
            } else if let Note::None = s.note {
                if ch.sample.is_some() {
                    /* Ghost instrument, trigger note */
                    /* Sample position is kept, but envelopes are reset */
                    ch.trigger_note(TriggerKeep::SAMPLE_POSITION);
                }
            } else if s.instrument as usize > self.module.instrument.len() {
                /* Invalid instrument, Cut current note */
                ch.cut_note();
                ch.instrument = None;
                ch.sample = None;
            } else {
                let instrnr = s.instrument as usize - 1;
                let instr = &self.module.instrument[instrnr];
                if let InstrumentType::Default(id) = &instr.instr_type {
                    // only good instr
                    if id.sample.len() != 0 {
                        ch.instrument = Some(instr);
                    }
                }
            }
        }

        if note_is_valid(noteu8) {
            if ch.instrument.is_some() {
                match &ch.instrument.unwrap().instr_type {
                    InstrumentType::Empty => ch.cut_note(),
                    InstrumentType::Default(instr) => {
                        if ch.current.unwrap_or(&EMPTY_SLOT).has_tone_portamento() {
                            if ch.sample.is_some() {
                                /* Tone portamento in effect */
                                let chsample = ch.sample.unwrap();
                                ch.note = noteu8 as f32 - 1.0
                                    + chsample.relative_note as f32
                                    + chsample.finetune;
                                ch.tone_portamento_target_period =
                                    period(self.module.flags, ch.note);
                            } else if instr.sample.len() == 0 {
                                ch.cut_note();
                            }
                        } else if (instr.sample_for_note[noteu8 as usize - 1] as usize)
                            < instr.sample.len()
                        {
                            if RAMPING {
                                for z in 0..SAMPLE_RAMPING_POINTS {
                                    ch.end_of_previous_sample[z] = ch.next_of_sample();
                                }
                                ch.frame_count = 0;
                            }
                            let chsample: &Sample =
                                &instr.sample[instr.sample_for_note[noteu8 as usize - 1] as usize];
                            ch.sample = Some(chsample);
                            ch.orig_note = noteu8 as f32 - 1.0
                                + chsample.relative_note as f32
                                + chsample.finetune;
                            ch.note = ch.orig_note;

                            if s.instrument > 0 {
                                ch.trigger_note(TriggerKeep::NONE);
                            } else {
                                /* Ghost note: keep old volume */
                                ch.trigger_note(TriggerKeep::VOLUME);
                            }
                        }
                    }
                    _ => {} // TODO
                }
            } else {
                /* Bad instrument */
                ch.cut_note();
            }
        } else if let Note::KeyOff = s.note {
            ch.key_off();
        }

        match s.volume >> 4 {
            0x0 => {} // Nothing
            // V - Set volume (0..63)
            0x1..=0x4 => ch.volume = (s.volume - 0x10) as f32 / 64.0,
            // V - 0x51..0x5F undefined...
            0x5 => ch.volume = 1.0,
            // D - Volume slide down (0..15)
            0x6 => {} // see tick() fn
            // C - Volume slide up (0..15)
            0x7 => {} // see tick() fn
            // B - Fine volume down (0..15)
            0x8 => ch.volume_slide(s.volume & 0x0F),
            // A - Fine volume up (0..15)
            0x9 => ch.volume_slide(s.volume << 4),
            // U - Vibrato speed (0..15)
            0xA => ch.vibrato_param = (ch.vibrato_param & 0x0F) | ((s.volume & 0x0F) << 4),
            // H - Vibrato depth (0..15)
            0xB => {} // see tick() fn
            // P - Set panning (2,6,10,14..62)
            0xC => ch.panning = (((s.volume & 0x0F) << 4) | (s.volume & 0x0F)) as f32 / 255.0,
            // L - Pan slide left (0..15)
            0xD => {} // see tick() fn
            // R - Pan slide right (0..15)
            0xE => {} // see tick() fn
            // G - Tone portamento (0..15)
            0xF => {
                if s.volume & 0x0F != 0 {
                    ch.tone_portamento_param = ((s.volume & 0x0F) << 4) | (s.volume & 0x0F);
                }
                // see also tick() fn
            }
            _ => {}
        }

        match s.effect_type {
            0x1 => {
                /* 1xx: Portamento up */
                if s.effect_parameter > 0 {
                    ch.portamento_up_param = s.effect_parameter;
                }
            }
            0x2 => {
                /* 2xx: Portamento down */
                if s.effect_parameter > 0 {
                    ch.portamento_down_param = s.effect_parameter;
                }
            }
            0x3 => {
                /* 3xx: Tone portamento */
                if s.effect_parameter > 0 {
                    ch.tone_portamento_param = s.effect_parameter;
                }
            }
            0x4 => {
                /* 4xy: Vibrato */
                if s.effect_parameter & 0x0F != 0 {
                    /* Set vibrato depth */
                    ch.vibrato_param = (ch.vibrato_param & 0xF0) | (s.effect_parameter & 0x0F);
                }
                if s.effect_parameter >> 4 != 0 {
                    /* Set vibrato speed */
                    ch.vibrato_param = (s.effect_parameter & 0xF0) | (ch.vibrato_param & 0x0F);
                }
            }
            0x5 => {
                /* 5xy: Tone portamento + Volume slide */
                if s.effect_parameter > 0 {
                    ch.volume_slide_param = s.effect_parameter;
                }
            }
            0x6 => {
                /* 6xy: Vibrato + Volume slide */
                if s.effect_parameter > 0 {
                    ch.volume_slide_param = s.effect_parameter;
                }
            }
            0x7 => {
                /* 7xy: Tremolo */
                if s.effect_parameter & 0x0F != 0 {
                    /* Set tremolo depth */
                    ch.tremolo_param = (ch.tremolo_param & 0xF0) | (s.effect_parameter & 0x0F);
                }
                if s.effect_parameter >> 4 != 0 {
                    /* Set tremolo speed */
                    ch.tremolo_param = (s.effect_parameter & 0xF0) | (ch.tremolo_param & 0x0F);
                }
            }
            0x8 => {
                /* 8xx: Set panning */
                ch.panning = s.effect_parameter as f32 / 255.0;
            }
            0x9 => {
                /* 9xx: Sample offset */
                if ch.sample.is_some() && note_is_valid(noteu8) {
                    let chsample = ch.sample.unwrap();

                    let final_offset = if chsample.bits() == 16 {
                        s.effect_parameter as usize * 256
                    } else {
                        s.effect_parameter as usize * 512
                    };

                    if final_offset >= chsample.len() {
                        /* Pretend the sample doesn't loop and is done playing */
                        ch.sample_position = -1.0;
                    } else {
                        ch.sample_position = final_offset as f32;
                    }
                }
            }
            0xA => {
                /* Axy: Volume slide */
                if s.effect_parameter > 0 {
                    ch.volume_slide_param = s.effect_parameter;
                }
            }
            0xB => {
                /* Bxx: Position jump */
                if (s.effect_parameter as usize) < self.module.pattern_order.len() {
                    self.position_jump = true;
                    self.jump_dest = s.effect_parameter as u16;
                    self.jump_row = 0;
                }
            }
            0xC => {
                /* Cxx: Set volume */
                ch.volume = if s.effect_parameter > 64 {
                    1.0
                } else {
                    s.effect_parameter as f32 / 64.0
                };
            }
            0xD => {
                /* Dxx: Pattern break */
                /* Jump after playing this line */
                self.pattern_break = true;
                self.jump_row = (s.effect_parameter >> 4) * 10 + (s.effect_parameter & 0x0F);
            }
            0xE => {
                /* EXy: Extended command */
                match s.effect_parameter >> 4 {
                    0x1 => {
                        /* E1y: Fine portamento up */
                        if s.effect_parameter & 0x0F != 0 {
                            ch.fine_portamento_up_param = s.effect_parameter as i8 & 0x0F;
                        }
                        ch.pitch_slide(-ch.fine_portamento_up_param);
                    }
                    0x2 => {
                        /* E2y: Fine portamento down */
                        if s.effect_parameter & 0x0F != 0 {
                            ch.fine_portamento_down_param = s.effect_parameter as i8 & 0x0F;
                        }
                        ch.pitch_slide(ch.fine_portamento_down_param);
                    }
                    0x4 => {
                        /* E4y: Set vibrato control */
                        ch.vibrato_waveform = Waveform::try_from(s.effect_parameter & 3).unwrap();
                        ch.vibrato_waveform_retrigger = ((s.effect_parameter >> 2) & 1) == 0;
                    }
                    0x5 => {
                        /* E5y: Set finetune */
                        if note_is_valid(noteu8) && ch.sample.is_some() {
                            let chsample = ch.sample.unwrap();
                            let noteu8: u8 = ch.current.unwrap_or(&EMPTY_SLOT).note.into();
                            ch.note = noteu8 as f32 - 1.0
                                + chsample.relative_note as f32
                                + (((s.effect_parameter & 0x0F) - 8) << 4) as f32 / 128.0;
                            ch.period = period(self.module.flags, ch.note);
                            ch.update_frequency();
                        }
                    }
                    0x6 => {
                        /* E6y: Pattern loop */
                        if s.effect_parameter & 0x0F != 0 {
                            if (s.effect_parameter & 0x0F) == ch.pattern_loop_count {
                                /* Loop is over */
                                ch.pattern_loop_count = 0;
                            } else {
                                /* Jump to the beginning of the loop */
                                ch.pattern_loop_count += 1;
                                self.position_jump = true;
                                self.jump_row = ch.pattern_loop_origin;
                                self.jump_dest = self.current_table_index;
                            }
                        } else {
                            /* Set loop start point */
                            ch.pattern_loop_origin = self.current_row;
                            /* Replicate FT2 E60 bug */
                            self.jump_row = ch.pattern_loop_origin;
                        }
                    }
                    0x7 => {
                        /* E7y: Set tremolo control */
                        ch.tremolo_waveform = Waveform::try_from(s.effect_parameter & 3).unwrap();
                        ch.tremolo_waveform_retrigger = ((s.effect_parameter >> 2) & 1) == 0;
                    }
                    0xA => {
                        /* EAy: Fine volume slide up */
                        if s.effect_parameter & 0x0F != 0 {
                            ch.fine_volume_slide_param = s.effect_parameter & 0x0F;
                        }
                        ch.volume_slide(ch.fine_volume_slide_param << 4);
                    }
                    0xB => {
                        /* EBy: Fine volume slide down */
                        if s.effect_parameter & 0x0F != 0 {
                            ch.fine_volume_slide_param = s.effect_parameter & 0x0F;
                        }
                        ch.volume_slide(ch.fine_volume_slide_param);
                    }
                    0xD => {
                        /* EDy: Note delay */
                        /* XXX: figure this out better. EDx triggers
                         * the note even when there no note and no
                         * instrument. But ED0 acts like like a ghost
                         * note, EDx (x ≠ 0) does not. */
                        if let Note::None = s.note {
                            if s.instrument == 0 {
                                let flags = TriggerKeep::VOLUME;

                                if ch.current.unwrap_or(&EMPTY_SLOT).effect_parameter & 0x0F != 0 {
                                    ch.note = ch.orig_note;
                                    ch.trigger_note(flags);
                                } else {
                                    ch.trigger_note(
                                        flags | TriggerKeep::PERIOD | TriggerKeep::SAMPLE_POSITION,
                                    );
                                }
                            }
                        }
                    }
                    0xE => {
                        /* EEy: Pattern delay */
                        self.extra_ticks = (ch.current.unwrap_or(&EMPTY_SLOT).effect_parameter
                            & 0x0F) as u16
                            * self.tempo;
                    }
                    _ => {}
                }
            }
            0xF => {
                /* Fxx: Set tempo/BPM */
                if s.effect_parameter > 0 {
                    if s.effect_parameter <= 0x1F {
                        self.tempo = s.effect_parameter as u16;
                    } else {
                        self.bpm = s.effect_parameter as u16;
                    }
                }
            }
            0x10 => {
                /* Gxx: Set global volume */
                self.global_volume = if s.effect_parameter > 64 {
                    1.0
                } else {
                    s.effect_parameter as f32 / 64.0
                };
            }
            0x11 => {
                /* Hxy: Global volume slide */
                if s.effect_parameter > 0 {
                    ch.global_volume_slide_param = s.effect_parameter;
                }
            }
            0x12..=0x14 => { /* Unused */ }
            0x15 => {
                /* Lxx: Set envelope position */
                ch.volume_envelope_frame_count = s.effect_parameter as u16;
                ch.panning_envelope_frame_count = s.effect_parameter as u16;
            }
            0x16..=0x18 => { /* Unused */ }
            0x19 => {
                /* Pxy: Panning slide */
                if s.effect_parameter > 0 {
                    ch.panning_slide_param = s.effect_parameter;
                }
            }
            0x1A => { /* Unused */ }
            0x1B => {
                /* Rxy: Multi retrig note */
                if s.effect_parameter > 0 {
                    if (s.effect_parameter >> 4) == 0 {
                        /* Keep previous x value */
                        ch.multi_retrig_param =
                            (ch.multi_retrig_param & 0xF0) | (s.effect_parameter & 0x0F);
                    } else {
                        ch.multi_retrig_param = s.effect_parameter;
                    }
                }
            }
            0x1C => { /* Unused */ }
            0x1D => {
                /* Txy: Tremor */
                if s.effect_parameter > 0 {
                    /* Tremor x and y params do not appear to be separately
                     * kept in memory, unlike Rxy */
                    ch.tremor_param = s.effect_parameter;
                }
            }
            0x1E..=0x20 => { /* Unused */ }
            0x21 => {
                /* Xxy: Extra stuff */
                match s.effect_parameter >> 4 {
                    1 => {
                        /* X1y: Extra fine portamento up */
                        if s.effect_parameter & 0x0F != 0 {
                            ch.extra_fine_portamento_up_param = s.effect_parameter as i8 & 0x0F;
                        }
                        ch.pitch_slide(-ch.extra_fine_portamento_up_param);
                    }
                    2 => {
                        /* X2y: Extra fine portamento down */
                        if s.effect_parameter & 0x0F != 0 {
                            ch.extra_fine_portamento_down_param = s.effect_parameter as i8 & 0x0F;
                        }
                        ch.pitch_slide(ch.extra_fine_portamento_down_param);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn row(&mut self) {
        if self.position_jump {
            self.current_table_index = self.jump_dest;
            self.current_row = self.jump_row;
            self.position_jump = false;
            self.pattern_break = false;
            self.jump_row = 0;
            self.post_pattern_change();
        } else if self.pattern_break {
            self.current_table_index += 1;
            self.current_row = self.jump_row;
            self.pattern_break = false;
            self.jump_row = 0;
            self.post_pattern_change();
        }

        let pat_idx: usize = self.module.pattern_order[self.current_table_index as usize] as usize;
        let num_channels = self.module.get_num_channels();
        let mut in_a_loop = false;

        for i in 0..num_channels {
            let current_row = self.current_row as usize;
            let s = &self.module.pattern[pat_idx][current_row][i];
            self.channel[i].current = Some(s);
            if s.effect_type != 0xE || (s.effect_parameter >> 4) != 0xD {
                self.handle_note_and_instrument(i, s);
            } else {
                self.channel[i].note_delay_param = s.effect_parameter & 0x0F;
            }

            if !in_a_loop && self.channel[i].pattern_loop_count > 0 {
                in_a_loop = true;
            }
        }

        if !in_a_loop {
            /* No E6y loop is in effect (or we are in the first pass) */
            self.loop_count = self.row_loop_count[self.current_table_index as usize]
                [self.current_row as usize] as u8;
            self.row_loop_count[self.current_table_index as usize][self.current_row as usize] += 1;
        }

        self.current_row += 1; /* Since this is an uint8, this line can
                                * increment from 255 to 0, in which case it
                                * is still necessary to go the next
                                * pattern. */
        let pattern_len = self.module.pattern[pat_idx].len();

        if !self.position_jump
            && !self.pattern_break
            && (self.current_row as usize >= pattern_len || self.current_row == 0)
        {
            self.current_table_index += 1;
            self.current_row = self.jump_row; /* This will be 0 most of
                                               * the time, except when E60
                                               * is used */
            self.jump_row = 0;
            self.post_pattern_change();
        }
    }

    fn tick(&mut self) {
        if self.current_tick == 0 {
            self.row();
        }

        for ch_index in 0..self.channel.len() {
            self.channel[ch_index].envelopes();
            self.channel[ch_index].autovibrato();

            let pattern_slot = self.channel[ch_index].current.unwrap_or(&EMPTY_SLOT);

            if self.channel[ch_index].arp_in_progress && !pattern_slot.has_arpeggio() {
                self.channel[ch_index].arp_in_progress = false;
                self.channel[ch_index].arp_note_offset = 0;
                self.channel[ch_index].update_frequency();
            }
            if self.channel[ch_index].vibrato_in_progress && !pattern_slot.has_vibrato() {
                self.channel[ch_index].vibrato_in_progress = false;
                self.channel[ch_index].vibrato_note_offset = 0.0;
                self.channel[ch_index].update_frequency();
            }

            if self.current_tick != 0 {
                match pattern_slot.volume >> 4 {
                    0x6 => {
                        /* Volume slide down */
                        self.channel[ch_index].volume_slide(pattern_slot.volume & 0x0F);
                    }
                    0x7 => {
                        /* Volume slide up */
                        self.channel[ch_index].volume_slide(pattern_slot.volume << 4);
                    }
                    0xB => {
                        /* Vibrato */
                        self.channel[ch_index].vibrato_in_progress = false;
                        self.channel[ch_index].vibrato();
                    }
                    0xD => {
                        /* Panning slide left */
                        self.channel[ch_index].panning_slide(pattern_slot.volume & 0x0F);
                    }
                    0xE => {
                        /* Panning slide right */
                        self.channel[ch_index].panning_slide(pattern_slot.volume << 4);
                    }
                    0xF => {
                        /* Tone portamento */
                        self.channel[ch_index].tone_portamento();
                    }
                    _ => {}
                }
            }

            match pattern_slot.effect_type {
                0 => {
                    /* 0xy: Arpeggio */
                    if pattern_slot.effect_parameter > 0 {
                        let arp_offset = self.tempo % 3;
                        match self.current_tick {
                            1 if arp_offset == 2 => {
                                /* arp_offset==2: 0 -> x -> 0 -> y -> x -> … */
                                self.channel[ch_index].arp_in_progress = true;
                                self.channel[ch_index].arp_note_offset =
                                    pattern_slot.effect_parameter >> 4;
                                self.channel[ch_index].update_frequency();
                                break;
                            }
                            0 if arp_offset >= 1 => {
                                /* arp_offset==1: 0 -> 0 -> y -> x -> … */
                                self.channel[ch_index].arp_in_progress = false;
                                self.channel[ch_index].arp_note_offset = 0;
                                self.channel[ch_index].update_frequency();
                                break;
                            }
                            _ => {
                                /* 0 -> y -> x -> … */
                                self.channel[ch_index].arpeggio(
                                    pattern_slot.effect_parameter,
                                    self.current_tick - arp_offset,
                                );
                            }
                        }
                    }
                }
                1 => {
                    /* 1xx: Portamento up */
                    if self.current_tick != 0 {
                        let period_offset = -(self.channel[ch_index].portamento_up_param as i8);
                        self.channel[ch_index].pitch_slide(period_offset);
                    }
                }
                2 => {
                    /* 2xx: Portamento down */
                    if self.current_tick != 0 {
                        let period_offset = self.channel[ch_index].portamento_down_param as i8;
                        self.channel[ch_index].pitch_slide(period_offset);
                    }
                }
                3 => {
                    /* 3xx: Tone portamento */
                    if self.current_tick != 0 {
                        self.channel[ch_index].tone_portamento();
                    }
                }
                4 => {
                    /* 4xy: Vibrato */
                    if self.current_tick != 0 {
                        self.channel[ch_index].vibrato_in_progress = true;
                        self.channel[ch_index].vibrato();
                    }
                }
                5 => {
                    /* 5xy: Tone portamento + Volume slide */
                    if self.current_tick != 0 {
                        self.channel[ch_index].tone_portamento();
                        let rawval = self.channel[ch_index].volume_slide_param;
                        self.channel[ch_index].volume_slide(rawval);
                    }
                }
                6 => {
                    /* 6xy: Vibrato + Volume slide */
                    if self.current_tick != 0 {
                        self.channel[ch_index].vibrato_in_progress = true;
                        self.channel[ch_index].vibrato();
                        let rawval = self.channel[ch_index].volume_slide_param;
                        self.channel[ch_index].volume_slide(rawval);
                    }
                }
                7 => {
                    /* 7xy: Tremolo */
                    if self.current_tick != 0 {
                        self.channel[ch_index].tremolo();
                    }
                }
                0xA => {
                    /* Axy: Volume slide */
                    if self.current_tick != 0 {
                        let rawval = self.channel[ch_index].volume_slide_param;
                        self.channel[ch_index].volume_slide(rawval);
                    }
                }
                0xE => {
                    /* EXy: Extended command */
                    match pattern_slot.effect_parameter >> 4 {
                        0x9 => {
                            /* E9y: Retrigger note */
                            if self.current_tick != 0 && pattern_slot.effect_parameter & 0x0F != 0 {
                                let r = self.current_tick
                                    % (pattern_slot.effect_parameter as u16 & 0x0F);
                                if r != 0 {
                                    self.channel[ch_index].trigger_note(TriggerKeep::VOLUME);
                                    self.channel[ch_index].envelopes();
                                }
                            }
                        }
                        0xC => {
                            /* ECy: Note cut */
                            if (pattern_slot.effect_parameter as u16 & 0x0F) == self.current_tick {
                                self.channel[ch_index].cut_note();
                            }
                        }
                        0xD => {
                            /* EDy: Note delay */
                            if self.channel[ch_index].note_delay_param as u16 == self.current_tick {
                                self.handle_note_and_instrument(ch_index, pattern_slot);
                                self.channel[ch_index].envelopes();
                            }
                        }
                        _ => {}
                    }
                }
                17 => {
                    /* Hxy: Global volume slide */
                    if self.current_tick != 0 {
                        if (self.channel[ch_index].global_volume_slide_param & 0xF0 != 0)
                            && (self.channel[ch_index].global_volume_slide_param & 0x0F != 0)
                        {
                            /* Illegal state */
                            break;
                        }
                        if self.channel[ch_index].global_volume_slide_param & 0xF0 != 0 {
                            /* Global slide up */
                            let f = (self.channel[ch_index].global_volume_slide_param >> 4) as f32
                                / 64.0;
                            self.global_volume += f;
                            clamp_up(&mut self.global_volume);
                        } else {
                            /* Global slide down */
                            let f = (self.channel[ch_index].global_volume_slide_param & 0x0F)
                                as f32
                                / 64.0;
                            self.global_volume -= f;
                            clamp_down(&mut self.global_volume);
                        }
                    }
                }
                20 => {
                    /* Kxx: Key off */
                    /* Most documentations will tell you the parameter has no
                     * use. Don't be fooled. */
                    if self.current_tick == pattern_slot.effect_parameter as u16 {
                        self.channel[ch_index].key_off();
                    }
                }
                25 => {
                    /* Pxy: Panning slide */
                    if self.current_tick != 0 {
                        let rawval = self.channel[ch_index].panning_slide_param;
                        self.channel[ch_index].panning_slide(rawval);
                    }
                }
                27 => {
                    /* Rxy: Multi retrig note */
                    if self.current_tick != 0 {
                        if ((self.channel[ch_index].multi_retrig_param) & 0x0F) != 0 {
                            let r = self.current_tick
                                % (self.channel[ch_index].multi_retrig_param as u16 & 0x0F);
                            if r == 0 {
                                self.channel[ch_index]
                                    .trigger_note(TriggerKeep::VOLUME | TriggerKeep::ENVELOPE);

                                /* Rxy doesn't affect volume if there's a command in the volume
                                column, or if the instrument has a volume envelope. */
                                if self.channel[ch_index].instrument.is_some() {
                                    if let InstrumentType::Default(instr) =
                                        &self.channel[ch_index].instrument.unwrap().instr_type
                                    {
                                        if pattern_slot.volume != 0
                                            && !instr.volume_envelope.enabled
                                        {
                                            let mut v = self.channel[ch_index].volume
                                                * MULTI_RETRIG_MULTIPLY[self.channel[ch_index]
                                                    .multi_retrig_param
                                                    as usize
                                                    >> 4]
                                                + MULTI_RETRIG_ADD[self.channel[ch_index]
                                                    .multi_retrig_param
                                                    as usize
                                                    >> 4]
                                                    / 64.0;
                                            clamp(&mut v);
                                            self.channel[ch_index].volume = v;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                29 => {
                    /* Txy: Tremor */
                    if self.current_tick != 0 {
                        self.channel[ch_index].tremor_on = (self.current_tick - 1)
                            % ((self.channel[ch_index].tremor_param as u16 >> 4)
                                + (self.channel[ch_index].tremor_param as u16 & 0x0F)
                                + 2)
                            > (self.channel[ch_index].tremor_param as u16 >> 4);
                    }
                }
                _ => {}
            }

            let panning: f32 = self.channel[ch_index].panning
                + (self.channel[ch_index].panning_envelope_panning - 0.5)
                    * (0.5 - (self.channel[ch_index].panning - 0.5).abs())
                    * 2.0;
            let mut volume = 0.0;

            if !self.channel[ch_index].tremor_on {
                volume = self.channel[ch_index].volume + self.channel[ch_index].tremolo_volume;
                clamp(&mut volume);
                volume *= self.channel[ch_index].fadeout_volume
                    * self.channel[ch_index].volume_envelope_volume;
            }

            if RAMPING {
                /* See https://modarchive.org/forums/index.php?topic=3517.0
                 * and https://github.com/Artefact2/libxm/pull/16 */
                self.channel[ch_index].target_volume[0] = volume * (1.0 - panning).sqrt();
                self.channel[ch_index].target_volume[1] = volume * panning.sqrt();
            } else {
                self.channel[ch_index].actual_volume[0] = volume * (1.0 - panning).sqrt();
                self.channel[ch_index].actual_volume[1] = volume * panning.sqrt();
            }
        }

        self.current_tick += 1;
        if self.current_tick >= self.tempo + self.extra_ticks {
            self.current_tick = 0;
            self.extra_ticks = 0;
        }

        /* FT2 manual says number of ticks / second = BPM * 0.4 */
        self.remaining_samples_in_tick += self.sample_rate / (self.bpm as f32 * 0.4);
    }

    // return (left, right) samples
    fn sample(&mut self) -> Option<(f32, f32)> {
        if self.remaining_samples_in_tick <= 0.0 {
            self.tick();
        }
        self.remaining_samples_in_tick -= 1.0;

        let mut left = 0.0;
        let mut right = 0.0;

        if self.max_loop_count > 0 && self.loop_count >= self.max_loop_count {
            return None;
        }

        for ch in &mut self.channel {
            if ch.instrument.is_none() || ch.sample.is_none() || ch.sample_position < 0.0 {
                continue;
            }
            let fval = ch.next_of_sample();

            if !ch.muted && !ch.instrument.unwrap().muted {
                left += fval * ch.actual_volume[0];
                right += fval * ch.actual_volume[1];
            }

            if RAMPING {
                ch.frame_count += 1;
                slide_towards(
                    &mut ch.actual_volume[0],
                    ch.target_volume[0],
                    self.volume_ramp,
                );
                slide_towards(
                    &mut ch.actual_volume[1],
                    ch.target_volume[1],
                    self.volume_ramp,
                );
            }
        }

        let fgvol = self.global_volume * self.amplification;
        left *= fgvol;
        right *= fgvol;

        Some((left, right))
    }

    pub fn generate_samples(&mut self, output: &mut [f32]) {
        let numsamples = output.len() / 2;
        self.generated_samples += numsamples as u64;

        for i in 0..numsamples {
            match self.sample() {
                Some((left, right)) => {
                    output[(2 * i)] = left;
                    output[(2 * i + 1)] = right;
                }
                None => {}
            }
        }
    }
}

impl<'m: 'c, 'c> Iterator for XmrsPlayer<'m, 'c> {
    type Item = (f32, f32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.max_loop_count > 0 && self.loop_count >= self.max_loop_count {
            return None;
        } else {
            self.generated_samples += 1;
            self.sample()
        }
    }
}
