use core::default::Default;

use crate::effect::*;


#[derive(Clone, Default)]
pub struct Portamento {
    pub period: f32,
}


#[derive(Clone, Default)]
pub struct EffectPortamento {
    pub data: Portamento,
    value: f32,
}

impl EffectPortamento {
    pub fn clamp(&self, period: f32) -> f32 {
        let final_period = period + self.value();
        match final_period {
            p if p < 1.0 => 1.0,
            p if p >= 32000.0 => 32000.0 - 1.0,
            _ => final_period
        }
    }
}

impl EffectPlugin for EffectPortamento {
    fn tick0(&mut self, param1: f32, _param2: f32) -> f32 {
        self.data.period = param1;
        self.value = param1;
        self.value()
    }

    fn tick(&mut self) -> f32 {
        self.value += self.data.period;
        self.value()
    }

    fn in_progress(&self) -> bool {
        self.data.period != 0.0
    }

    fn retrigger(&mut self) -> f32 {
        self.value = self.data.period;
        self.value()
    }

    fn value(&self) -> f32 {
        self.value
    }

}


impl EffectXM2EffectPlugin for EffectPortamento {
    fn convert(param: u8, special: u8) -> Option<(Option<f32>, Option<f32>)> {
        let mut p = param;
        let mut m = 4.0;
        match special {
            1 => p &= 0x0F, // fine portamento
            2 => m = 1.0,   // extra fin portamento
            _ => {},
        }

        if param != 0 {
            Some((Some(m * p as f32), None))
        } else {
            None
        }
    }
}