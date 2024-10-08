use crate::channel::Channel;
use crate::helper::*;
use crate::historical_helper::HistoricalHelper;
use crate::triggerkeep::*;
use alloc::{vec, vec::Vec};
use xmrs::prelude::*;

pub struct XmrsPlayer<'a> {
    module: &'a Module,
    sample_rate: f32,

    tempo: u16,
    bpm: u16,
    /// Global volume: 0.0 to 1.0
    pub global_volume: f32,
    global_volume_slide_param: u8,
    /// Global amplification (default 1/4)
    pub amplification: f32,
    current_table_index: usize,
    current_row: usize,
    current_tick: u16,
    /// sample rate / (BPM * 0.4)
    remaining_samples_in_tick: f32,
    /// +1 for a (left,right) sample
    pub generated_samples: u64,

    position_jump: bool,
    pattern_break: bool,
    jump_dest: usize,
    jump_row: usize,

    /// Extra ticks to be played before going to the next row - Used for EEy effect
    extra_ticks: u16,

    pub channel: Vec<Channel<'a>>,

    row_loop_count: Vec<Vec<usize>>,
    loop_count: usize,
    max_loop_count: usize,

    /// None if next-one is a left sample, else right sample
    right_sample: Option<f32>,
    #[cfg(feature = "std")]
    debug: bool,
    hhelper: Option<HistoricalHelper>,

    pause: bool,
}

impl<'a> XmrsPlayer<'a> {
    pub fn new(module: &'a Module, sample_rate: f32, historical: bool) -> Self {
        let num_channels = module.get_num_channels();
        let hhelper = if historical {
            Some(HistoricalHelper::new(module.default_tempo))
        } else {
            None
        };
        let mut player = Self {
            module,
            sample_rate,
            tempo: module.default_tempo,
            bpm: module.default_bpm,
            global_volume: 1.0,
            amplification: 0.25,
            row_loop_count: vec![vec![0; MAX_NUM_ROWS]; module.get_song_length()],
            hhelper: hhelper.clone(),
            global_volume_slide_param: 0,
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
            channel: vec![],
            loop_count: 0,
            max_loop_count: 0,
            right_sample: None,
            #[cfg(feature = "std")]
            debug: false,
            pause: false,
        };

        player.channel = vec![Channel::new(module, sample_rate, hhelper.clone()); num_channels];

        player
    }

    #[cfg(feature = "std")]
    pub fn debug(&mut self, debug: bool) {
        self.debug = debug;
    }

    pub fn set_mute_channel(&mut self, channel_num: usize, mute: bool) {
        if channel_num < self.channel.len() {
            self.channel[channel_num].muted = mute;
        }
    }

    pub fn mute_all(&mut self, mute: bool) {
        for c in &mut self.channel {
            c.muted = mute;
        }
    }

    pub fn set_max_loop_count(&mut self, max_loop_count: usize) {
        self.max_loop_count = max_loop_count;
    }

    pub fn get_loop_count(&self) -> usize {
        self.loop_count
    }

    pub fn get_sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Jump to row at index table_position in pattern_order at speed 
    /// if speed == 0, resets to default speed
    pub fn goto(&mut self, table_position: usize, row: usize, speed: u16) -> bool {
        if table_position < self.module.get_song_length() {
            let num_row = self.module.pattern_order[table_position];
            if row < self.module.get_num_rows(num_row) {
                // Create a position jump
                self.jump_dest = table_position;
                self.jump_row = row;
                self.position_jump = true;

                // Cleanup self
                self.tempo = if speed == 0 {
                    self.module.default_tempo
                } else {
                    speed
                };
                self.bpm = self.module.default_bpm;
                self.global_volume = 1.0;

                // Cleanup channels
                let num_channels = self.module.get_num_channels();
                for i in 0..num_channels {
                    self.channel[i].trigger_note(TRIGGER_KEEP_PERIOD); // clean what we can
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

    /// Returns current pattern number in pattern_order
    pub fn get_current_pattern(&self) -> usize {
        self.module.pattern_order[self.current_table_index]
    }

    /// Returns current index in pattern_order
    pub fn get_current_table_index(&self) -> usize {
        self.current_table_index
    }

    /// Returns current row
    pub fn get_current_row(&self) -> usize {
        self.current_row
    }

    /// Force pause, returning (0.0, 0.0) samples
    pub fn pause(&mut self, pause: bool) {
        self.pause = pause;
    }

    fn post_pattern_change(&mut self) {
        /* Loop if necessary */
        if self.current_table_index >= self.module.pattern_order.len() {
            self.current_table_index = self.module.restart_position;
        }

        #[cfg(feature = "std")]
        if self.debug {
            println!(
                "pattern_order[0x{:03x}] = 0x{:03x}",
                self.current_table_index, self.module.pattern_order[self.current_table_index]
            );
        }
    }

    fn tick0_global_effects(&mut self, ch_index: usize) {
        let ch = &mut self.channel[ch_index];
        let pattern_slot = &ch.current;

        match pattern_slot.effect_type {
            0xB => {
                /* Bxx: Position jump */
                if (pattern_slot.effect_parameter as usize) < self.module.pattern_order.len() {
                    self.position_jump = true;
                    self.jump_dest = pattern_slot.effect_parameter as usize;
                    self.jump_row = 0;
                }
            }
            0xD => {
                /* Dxx: Pattern break */
                /* Jump after playing this line */
                self.pattern_break = true;
                self.jump_row = (pattern_slot.effect_parameter >> 4) as usize * 10
                    + (pattern_slot.effect_parameter & 0x0F) as usize;
            }
            0xE => {
                /* EXy: Extended command */
                match pattern_slot.effect_parameter >> 4 {
                    0x6 => {
                        /* E6y: Pattern loop */
                        if pattern_slot.effect_parameter & 0x0F != 0 {
                            if (pattern_slot.effect_parameter & 0x0F) as usize
                                == ch.pattern_loop_count
                            {
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
                            if let Some(_hhelper) = &self.hhelper {
                                // Replicate FT2 E60 bug
                                self.jump_row = ch.pattern_loop_origin;
                            }
                        }
                    }
                    0xE => {
                        /* EEy: Pattern delay */
                        self.extra_ticks =
                            (pattern_slot.effect_parameter & 0x0F) as u16 * self.tempo;
                    }
                    _ => {}
                }
            }
            0xF => {
                /* Fxx: Set tempo/BPM */
                if pattern_slot.effect_parameter < 32 {
                    self.tempo = pattern_slot.effect_parameter as u16;
                } else {
                    self.bpm = pattern_slot.effect_parameter as u16;
                }
            }
            0x10 => {
                /* Gxx: Set global volume */
                self.global_volume = if pattern_slot.effect_parameter > 64 {
                    1.0
                } else {
                    pattern_slot.effect_parameter as f32 / 64.0
                };
            }
            0x11 => {
                /* Hxy: Global volume slide */
                if pattern_slot.effect_parameter > 0 {
                    self.global_volume_slide_param = pattern_slot.effect_parameter;
                }
            }
            _ => {}
        }
    }

    fn tick0(&mut self) {
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

        let pat_idx_temp = self.module.pattern_order[self.current_table_index];
        let pat_idx = if pat_idx_temp < self.module.pattern.len() {
            pat_idx_temp
        } else {
            // empty pattern, returning to zero
            self.current_table_index = 0;
            self.module.pattern_order[self.current_table_index]
        };

        let num_channels = self.module.get_num_channels();
        let mut in_a_loop = false;

        let current_row = self.current_row;
        #[cfg(feature = "std")]
        if self.debug {
            print!("{:03X} ", current_row);
        }
        for ch_index in 0..num_channels {
            let ps = &self.module.pattern[pat_idx][current_row][ch_index];
            #[cfg(feature = "std")]
            if self.debug {
                print!("{:?}", ps);
            }
            self.channel[ch_index].tick0(ps);
            self.tick0_global_effects(ch_index);
            if !in_a_loop && self.channel[ch_index].pattern_loop_count > 0 {
                in_a_loop = true;
            }
        }
        #[cfg(feature = "std")]
        if self.debug {
            println!();
        }

        if !in_a_loop {
            /* No E6y loop is in effect (or we are in the first pass) */
            self.loop_count = self.row_loop_count[self.current_table_index][self.current_row];
            self.row_loop_count[self.current_table_index][self.current_row] += 1;
        }

        self.current_row = self.current_row.wrapping_add(1); /* Maybe this can be an u8 on old computers, this line can
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
        for ch in &mut self.channel {
            ch.tick(self.current_tick);

            // Specific effect to slide global volume
            if ch.current.effect_type == 0x11 && self.current_tick != 0 {
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
    }

    fn step(&mut self) {
        if self.remaining_samples_in_tick <= 0.0 {
            if self.current_tick == 0 {
                self.tick0();
            } else {
                self.tick();
            }

            self.current_tick += 1;
            if self.current_tick >= self.tempo + self.extra_ticks {
                self.current_tick = 0;
                self.extra_ticks = 0;
            }

            if let Some(hhelper) = &mut self.hhelper {
                hhelper.set_tempo(self.tempo);
            }
            /* FT2 manual says number of ticks / second = BPM * 0.4 */
            self.remaining_samples_in_tick += self.sample_rate / (self.bpm as f32 * 0.4);
        }
        self.remaining_samples_in_tick -= 1.0;
    }

    /// Returns samples from each channel before applying global volume and amplification.
    /// If the function returns None, no more samples are available.
    /// 
    /// In conjunction with the samples_apply_volume() function, this function can be used to replace the iterator or the sample() function if you want to control each channel in fine detail, for example, to create beautiful graphic effects.
    pub fn samples_from_channels(&mut self) -> Option<Vec<(f32, f32)>> {
        if self.pause {
            return Some(vec![(0.0, 0.0); self.channel.len()]);
        }

        self.step();

        if self.max_loop_count > 0 && self.loop_count >= self.max_loop_count {
            return None;
        }

        let samples: Vec<(f32, f32)> = self
            .channel
            .iter_mut()
            .map(|ch| match ch.next() {
                Some(fval) => {
                    if ch.is_muted() {
                        (0.0, 0.0)
                    } else {
                        fval
                    }
                }
                None => (0.0, 0.0),
            })
            .collect();

        self.generated_samples += 1;
        Some(samples)
    }

    /// This function applies volume and amplification to the various channel samples. It is applied to the result of the `samples_from_channels()` function.
    pub fn samples_apply_volume(&mut self, samples: &Vec<(f32, f32)>) -> (f32, f32) {
        let fgvol = (self.global_volume * self.amplification)
            / (self.global_volume + self.amplification);
        let sample = samples
            .into_iter()
            .fold((0.0, 0.0), |(acc_left, acc_right), (left, right)| {
                (acc_left + left, acc_right + right)
            });
        return (sample.0 * fgvol, sample.1 * fgvol);
    }

    /// Returns the sum of the samples from the `samples_from_channels()` and `samples_apply_volume()` functions, separating the left channel from the right.
    pub fn sample(&mut self) -> Option<(f32, f32)> {
        if let Some(samples) = self.samples_from_channels() {
            return Some(self.samples_apply_volume(&samples));
        } else {
            return None;
        }
    }

    /// Returns samples one after the other, starting with the left channel.
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

}

impl<'a> Iterator for XmrsPlayer<'a> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.max_loop_count > 0 && self.loop_count >= self.max_loop_count {
            return None;
        } else {
            self.sample_one()
        }
    }
}
