use core::default::Default;

use crate::effect::*;

#[derive(Clone, Default)]
pub struct EffectPortamento {
    speed: f32,
}

impl EffectPlugin for EffectPortamento {
    fn tick0(&mut self, speed: f32, _param2: f32) -> f32 {
        self.speed = speed;
        self.speed
    }

    fn tick(&mut self) -> f32 {
        self.speed
    }

    fn in_progress(&self) -> bool {
        self.speed != 0.0
    }

    fn retrigger(&mut self) -> f32 {
        self.speed
    }

    fn clamp(&self, period: f32) -> f32 {
        let final_period = period + self.value();
        // TODO: maybe clamp can be done elsewhere
        match final_period {
            p if p < 1.0 => 1.0,
            p if p >= 32000.0 => 32000.0 - 1.0,
            _ => final_period,
        }
    }

    fn value(&self) -> f32 {
        self.speed
    }
}

impl EffectXM2EffectPlugin for EffectPortamento {
    // { normal=0, fine=1, extrafine=2 }
    fn xm_convert(param: u8, portype: u8) -> Option<(Option<f32>, Option<f32>)> {
        // assert for all case
        match portype {
            0 => {
                if param == 0 {
                    return None
                }
            }
            1 | 2 => {
                if param & 0x0F == 0 {
                    return None
                }
            }
            _ => {
                return None
            }
        }

        let p = match portype {
            2 => {
                // extra fin portamento (X1x or X2x)
                (param & 0x0F) as f32
            }
            1 => {
                // fine portamento (E1x or E2x)
                (param & 0x0F) as f32 * 4.0
            }
            _ => {
                // portamento (1xx or 2xx)
                param as f32 * 4.0
            }
        };

        Some((Some(p), None))
    }

    fn xm_update_effect(&mut self, param: u8, portype: u8, updown: f32) {
        if let Some((Some(p), None)) = Self::xm_convert(param, portype) {
            let p = if updown == 1.0 { -p } else { p };
            self.tick0(p, 0.0);
        }
        self.retrigger();
    }
}
