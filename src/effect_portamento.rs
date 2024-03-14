use core::default::Default;

use crate::effect::*;

#[derive(Clone, Default)]
pub struct Portamento {
    pub speed: f32,
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
            _ => final_period,
        }
    }
}

impl EffectPlugin for EffectPortamento {
    fn tick0(&mut self, speed: f32, _param2: f32) -> f32 {
        self.data.speed = speed;
        self.value = 0.0;
        self.value()
    }

    fn tick(&mut self) -> f32 {
        self.value += self.data.speed;
        self.value()
    }

    fn in_progress(&self) -> bool {
        self.data.speed != 0.0
    }

    fn retrigger(&mut self) -> f32 {
        self.value = 0.0;
        self.value()
    }

    fn value(&self) -> f32 {
        self.value
    }
}

impl EffectXM2EffectPlugin for EffectPortamento {
    fn convert(param: u8, special: u8) -> Option<(Option<f32>, Option<f32>)> {
        if param == 0 {
            return None;
        }

        let p = match special {
            1 => {
                // fine portamento
                (param & 0x0F) as f32
            }
            2 => {
                // extra fin portamento
                (1.0 / 4.0) * (param & 0x0F) as f32
            }
            _ => param as f32,
        };

        Some((Some(p), None))
    }
}
