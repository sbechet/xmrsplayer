/* Txy: Tremor

Rapidly switches the sample volume on and off on every tick of the row except the first.
Volume is on for x + 1 ticks and off for y + 1 ticks.

tremor_on: bool = [(T-1) % (X+1+Y+1) ] > X
*/
use crate::effect::{EffectPlugin, EffectXM2EffectPlugin};
use core::default::Default;

#[derive(Clone, Default)]
pub struct EffectTremor {
    tick: usize,
    tick_on: usize,
    tick_off: usize,
}

impl EffectPlugin for EffectTremor {
    fn tick0(&mut self, on: f32, off: f32) -> f32 {
        self.tick_on = on as usize;
        self.tick_off = off as usize;
        self.value()
    }

    fn tick(&mut self) -> f32 {
        // tick overflow seems impossible here
        self.tick = self.tick + 1;
        self.value()
    }

    fn in_progress(&self) -> bool {
        (self.tick - 1) % (self.tick_on + 1 + self.tick_off + 1) > self.tick_on
    }

    fn retrigger(&mut self) -> f32 {
        self.tick = 0;
        self.value()
    }

    fn clamp(&self, _value: f32) -> f32 {
        self.value()
    }

    fn value(&self) -> f32 {
        0.0
    }
}

impl EffectXM2EffectPlugin for EffectTremor {
    fn xm_convert(param: u8, _special: u8) -> Option<(Option<f32>, Option<f32>)> {
        if param > 0 {
            let tick_on = param >> 4;
            let tick_off = param & 0x0F;
            Some((Some(tick_on as f32), Some(tick_off as f32)))
        } else {
            None
        }
    }

    fn xm_update_effect(&mut self, param: u8, _special1: u8, _special2: f32) {
        if param > 0 {
            self.tick_on = (param >> 4) as usize;
            self.tick_off = (param & 0x0F) as usize;
        }
    }
}
