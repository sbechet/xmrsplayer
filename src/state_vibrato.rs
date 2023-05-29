use crate::helper::*;
/// An Instrument Vibrato State
use std::ops::Deref;
use std::sync::Arc;
use xmrs::vibrato::Vibrato;

#[derive(Clone)]
pub struct StateVibrato {
    vibrato: Arc<Vibrato>,
    pub value: f32, // autovibrato_note_offset
    counter: u16,   // autovibrato_ticks
}

impl Deref for StateVibrato {
    type Target = Vibrato;
    fn deref(&self) -> &Vibrato {
        &self.vibrato
    }
}

impl StateVibrato {
    pub fn new(vibrato: Arc<Vibrato>) -> Self {
        Self {
            vibrato: vibrato,
            value: 0.0,
            counter: 0,
        }
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
        self.counter = 0;
    }

    pub fn tick(&mut self) {
        if self.vibrato.depth == 0.0 && self.value != 0.0 {
            self.value = 0.0;
        } else {
            self.value = self.vibrato.get_value(self.counter);
            self.counter = (self.counter + 1) & 63;
        }
    }
}
