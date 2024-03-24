use xmrs::module::FrequencyType;

use crate::effect::*;
use crate::helper::*;
use core::default::Default;

#[derive(Clone, Default)]
pub struct TonePortamento {
    pub speed: f32,
    pub goal: f32,
}

#[derive(Clone, Default)]
pub struct EffectTonePortamento {
    pub data: TonePortamento,
    value: f32,
}

impl EffectPlugin for EffectTonePortamento {
    fn tick0(&mut self, speed: f32, goal: f32) -> f32 {
        self.data.speed = speed;
        self.data.goal = goal;
        self.value = 0.0;
        self.value()
    }

    fn tick(&mut self) -> f32 {
        if self.data.goal != 0.0 {
            self.value += self.data.speed;
        }
        self.value()
    }

    fn in_progress(&self) -> bool {
        self.data.speed != 0.0
    }

    fn retrigger(&mut self) -> f32 {
        self.value = 0.0;
        self.value()
    }

    fn clamp(&self, period: f32) -> f32 {
        if self.data.goal == 0.0 {
            return period;
        }

        let mut final_period = period;
        if period != self.data.goal {
            slide_towards(&mut final_period, self.data.goal, self.data.speed);
        }
        final_period
    }

    fn value(&self) -> f32 {
        self.value
    }
}

impl EffectXM2EffectPlugin for EffectTonePortamento {
    // 0b0000_0001 : linear frequency
    // 0b1111_1110 : multiplier { 16 }
    fn xm_convert(speed: u8, multiplier: u8) -> Option<(Option<f32>, Option<f32>)> {
        let speed: f32 = if multiplier & 1 == 1 {
            speed as f32 * 4.0
        } else {
            speed as f32
        };
        let mult = multiplier & 0b1111_1110;
        let mult = if mult == 0 { 1 } else { mult } as f32;
        let speed: f32 = speed * mult;
        if speed != 0.0 {
            Some((Some(speed), None))
        } else {
            None
        }
    }

    fn xm_update_effect(&mut self, param: u8, multiplier: u8, note: f32) {
        let freq_type = if multiplier & 1 == 1 {
            FrequencyType::LinearFrequencies
        } else {
            FrequencyType::AmigaFrequencies
        };
        if note != 0.0 {
            self.data.goal = period(freq_type, note as f32);
        }
        if let Some((Some(speed), None)) = Self::xm_convert(param, multiplier) {
            self.data.speed = speed;
        }
        self.retrigger();
    }
}
