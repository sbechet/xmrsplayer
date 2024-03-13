use core::default::Default;
use crate::effect::EffectPlugin;


#[derive(Clone, Default)]
pub struct Arpeggio {
    offset1: f32,
    offset2: f32,
}


#[derive(Clone, Default)]
pub struct EffectArpeggio {
    arpeggio: Arpeggio,
    tick: usize,
    in_progress: bool,
}


impl EffectPlugin for EffectArpeggio {
    fn tick0(&mut self, param1: f32, param2: f32) -> f32 {
        self.arpeggio.offset1 = param1;
        self.arpeggio.offset2 = param2;
        self.tick = 0;
        self.value()
    }

    fn tick(&mut self) -> f32 {
        self.in_progress = true;
        self.tick = (self.tick + 1) % 3;
        self.value()
    }

    fn in_progress(&self) -> bool {
        self.in_progress
    }

    fn retrigger(&mut self) -> f32 {
        self.tick=0;
        self.in_progress = false;
        self.value()
    }

    fn value(&self) -> f32 {
        match self.tick {
            1 => self.arpeggio.offset1,
            2 => self.arpeggio.offset2,
            _ => 0.0,
        }
    }

}