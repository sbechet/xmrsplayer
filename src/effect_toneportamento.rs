use core::default::Default;
use crate::helper::*;
use crate::effect::*;


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

impl EffectTonePortamento {
    pub fn clamp(&self, period: f32) -> f32 {
        let mut final_period = period;
        slide_towards(
            &mut final_period,
            self.data.goal,
            self.data.speed,
        );
        final_period
    }
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

    fn value(&self) -> f32 {
        self.value
    }

}


impl EffectXM2EffectPlugin for EffectTonePortamento {
    fn convert(param: u8, special: u8) -> Option<(Option<f32>, Option<f32>)> {
        let speed = match special {
            1 => (param<<4) as f32,
            _ => param as f32
        };
        if speed != 0.0 {
            Some((Some(speed), None))
        } else {
            None
        }
    }
}