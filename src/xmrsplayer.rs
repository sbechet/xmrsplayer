use crate::channel::{Channel, TriggerKeep};
use crate::helper::*;
use std::sync::Arc;
use xmrs::prelude::*;

pub struct XmrsPlayer {
    module: Arc<Module>,
    sample_rate: f32,

    tempo: u16,
    bpm: u16,
    global_volume: f32,
    global_volume_slide_param: u8,
    /// Global amplification (default 1/4)
    pub amplification: f32,

    // RAMPING START
    /* How much is a channel final volume allowed to change per
     * sample; this is used to avoid abrubt volume changes which
     * manifest as "clicks" in the generated sound. */
    // volume_ramp: f32,
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

    channel: Vec<Channel>,

    row_loop_count: Vec<Vec<u8>>,
    loop_count: u8,
    max_loop_count: u8,

    right_sample: Option<f32>, // None if next-one is a left sample, else right sample
    debug: bool,
}

impl XmrsPlayer {
    pub fn new(module: Arc<Module>, sample_rate: f32) -> Self {
        let num_channels = module.get_num_channels();
        Self {
            module: module.clone(),
            sample_rate,
            tempo: module.default_tempo,
            bpm: module.default_bpm,
            global_volume: 1.0,
            global_volume_slide_param: 0,
            amplification: 0.25,
            // XXX volume_ramp: 1.0 / 128.0,
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
            channel: vec![
                Channel::new(module.clone(), module.frequency_type, sample_rate);
                num_channels
            ],
            row_loop_count: vec![vec![0; MAX_NUM_ROWS]; module.get_song_length()],
            loop_count: 0,
            max_loop_count: 0,
            right_sample: None,
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

    pub fn is_samples(&self) -> bool {
        self.max_loop_count == 0 || self.loop_count <= self.max_loop_count
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

    pub fn get_current_pattern(&self) -> usize {
        self.module.pattern_order[self.current_table_index as usize] as usize
    }

    pub fn get_current_row(&self) -> usize {
        self.current_row as usize
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

    pub fn tick0_global_effects(&mut self, ch_index: usize) {
        let ch = &mut self.channel[ch_index];

        match ch.current.effect_type {
            0xB => {
                /* Bxx: Position jump */
                if (ch.current.effect_parameter as usize) < self.module.pattern_order.len() {
                    self.position_jump = true;
                    self.jump_dest = ch.current.effect_parameter as u16;
                    self.jump_row = 0;
                }
            }
            0xD => {
                /* Dxx: Pattern break */
                /* Jump after playing this line */
                self.pattern_break = true;
                self.jump_row =
                    (ch.current.effect_parameter >> 4) * 10 + (ch.current.effect_parameter & 0x0F);
            }
            0xE => {
                /* EXy: Extended command */
                match ch.current.effect_parameter >> 4 {
                    0x6 => {
                        /* E6y: Pattern loop */
                        if ch.current.effect_parameter & 0x0F != 0 {
                            if (ch.current.effect_parameter & 0x0F) == ch.pattern_loop_count {
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
                    0xE => {
                        /* EEy: Pattern delay */
                        self.extra_ticks = (ch.current.effect_parameter & 0x0F) as u16 * self.tempo;
                    }
                    _ => {}
                }
            }
            0xF => {
                /* Fxx: Set tempo/BPM */
                if ch.current.effect_parameter > 0 {
                    if ch.current.effect_parameter <= 0x1F {
                        self.tempo = ch.current.effect_parameter as u16;
                    } else {
                        self.bpm = ch.current.effect_parameter as u16;
                    }
                }
            }
            0x10 => {
                /* Gxx: Set global volume */
                self.global_volume = if ch.current.effect_parameter > 64 {
                    1.0
                } else {
                    ch.current.effect_parameter as f32 / 64.0
                };
            }
            0x11 => {
                /* Hxy: Global volume slide */
                if ch.current.effect_parameter > 0 {
                    self.global_volume_slide_param = ch.current.effect_parameter;
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

        let current_row = self.current_row as usize;
        for ch_index in 0..num_channels {
            self.channel[ch_index].tick0(&self.module.pattern[pat_idx][current_row][ch_index]);
            self.tick0_global_effects(ch_index);
            if !in_a_loop && self.channel[ch_index].pattern_loop_count > 0 {
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

        for ch in &mut self.channel {
            ch.tick(self.current_tick, self.tempo);

            // Specific effect to slide global volume
            if ch.current.effect_type == 17 && self.current_tick != 0 {
                /* Hxy: Global volume slide */
                self.global_volume += if (self.global_volume_slide_param & 0xF0 != 0)
                    && (self.global_volume_slide_param & 0x0F != 0)
                {
                    /* Illegal state */
                    0.0
                } else if self.global_volume_slide_param & 0xF0 != 0 {
                    /* Global slide up */
                    (self.global_volume_slide_param >> 4) as f32 / 64.0
                } else {
                    /* Global slide down */
                    -((self.global_volume_slide_param & 0x0F) as f32) / 64.0
                };
            }
            clamp(&mut self.global_volume);
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
            if ch.instrnr.is_none() || ch.sample.is_none() || !ch.state_sample.is_enabled() {
                continue;
            }
            let fval = ch.next_of_sample();

            if !ch.muted && !ch.module.instrument[ch.instrnr.unwrap()].muted {
                left += fval * ch.actual_volume[0];
                right += fval * ch.actual_volume[1];
            }

            // if RAMPING {
            //     ch.frame_count += 1;
            //     slide_towards(
            //         &mut ch.actual_volume[0],
            //         ch.target_volume[0],
            //         self.volume_ramp,
            //     );
            //     slide_towards(
            //         &mut ch.actual_volume[1],
            //         ch.target_volume[1],
            //         self.volume_ramp,
            //     );
            // }
        }

        let fgvol = self.global_volume * self.amplification;
        left *= fgvol;
        right *= fgvol;

        Some((left, right))
    }

    fn sample_one(&mut self) -> Option<f32> {
        match self.right_sample {
            Some(right) => {
                self.right_sample = None;
                return Some(right);
            }
            None => {
                let next_samples = self.sample();
                match next_samples {
                    Some((left, right)) => {
                        self.right_sample = Some(right);
                        return Some(left);
                    }
                    None => return None,
                }
            }
        }
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

impl Iterator for XmrsPlayer {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.max_loop_count > 0 && self.loop_count >= self.max_loop_count {
            return None;
        } else {
            self.generated_samples += 1;
            self.sample_one()
        }
    }
}
