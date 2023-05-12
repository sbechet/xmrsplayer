#![allow(unused_imports)]
use crate::prelude::*;
use rodio::Source;
use xmrs::prelude::*;
use xmrs::xm::xmmodule::XmModule;

pub const BUFFER_SIZE: usize = 2048;

pub struct ModuleSource {
    pub player: XmrsPlayer,
    buffer: [f32; BUFFER_SIZE],
    buffer_index: usize,
    sample_rate: u32,
}

impl ModuleSource {
    pub fn new(player: XmrsPlayer, sample_rate: u32) -> Self {
        ModuleSource {
            player,
            buffer: [0.0; BUFFER_SIZE],
            buffer_index: 0,
            sample_rate,
        }
    }

    pub fn get_loop_count(&self) -> u8 {
        self.player.get_loop_count()
    }
}

impl Source for ModuleSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(BUFFER_SIZE - self.buffer_index)
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

impl Iterator for ModuleSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.player.is_samples() {
            return None;
        }

        self.buffer_index += 1;

        if self.buffer_index >= BUFFER_SIZE {
            self.player.generate_samples(&mut self.buffer);
            self.buffer_index = 0;
        }

        Some(self.buffer[self.buffer_index])
    }
}
