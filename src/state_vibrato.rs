/// An Instrument Vibrato State
use crate::helper::*;
use xmrs::prelude::*;

#[derive(Clone, Default)]
pub struct StateVibrato {
    pub value: f32, // autovibrato_note_offset
    pub counter: u16, // autovibrato_ticks
}

impl StateVibrato {

    pub fn new() -> Self {
        Self {
            value: 0.0,
            counter: 0,
        }
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
        self.counter = 0;
}

    pub fn tick(&mut self, instr: &InstrDefault) {
        if instr.vibrato.depth == 0.0 && self.value != 0.0 {
            self.value = 0.0;
        } else {
            let sweep = if self.counter < instr.vibrato.sweep as u16 {
                /* No idea if this is correct, but it sounds close enough… */
                lerp(
                    0.0,
                    1.0,
                    self.counter as f32 / instr.vibrato.sweep as f32,
                )
            } else {
                1.0
            };
            let step = (self.counter * instr.vibrato.speed as u16) >> 2;
            self.counter = (self.counter + 1) & 63;
            self.value = 0.25
                * instr.vibrato.waveform.waveform(step)
                * instr.vibrato.depth
                * sweep;
        }
    }

}