use xmrs::module::FrequencyType;

#[cfg(feature = "libm")]
use num_traits::float::Float;
#[cfg(feature = "micromath")]
use micromath::F32Ext;

use crate::effect::*;
use crate::period_helper::PeriodHelper;

#[derive(Default, Clone, Copy, Debug)]
pub struct VibratoTremolo {
    pub waveform: u8,
    speed: f32,
    depth: f32,
}

impl VibratoTremolo {
    pub fn new(waveform: u8, speed: f32, depth: f32) -> Self {
        Self {
            waveform,
            speed,
            depth,
        }
    }

    // return depth * (-1..1)
    fn waveform(&self, pos: f32) -> f32 {
        let value = self.depth
            * match self.waveform {
                0 => -(core::f32::consts::TAU * pos).sin(),
                1 => {
                    // triangle, but ramp down reality
                    if pos < 0.5 {
                        -2.0 * pos
                    } else {
                        -2.0 * pos + 2.0
                    }
                }
                _ => {
                    // square
                    if pos < 0.5 {
                        -1.0
                    } else {
                        1.0
                    }
                }
            };

        value
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct EffectVibratoTremolo {
    pub data: VibratoTremolo,
    multiplier: f32,
    in_progress: bool,
    pos: f32,
    value: f32,
}

impl EffectVibratoTremolo {
    fn new(data: VibratoTremolo, multiplier: f32) -> Self {
        Self {
            data,
            multiplier,
            in_progress: false,
            pos: 0.0,
            value: 0.0,
        }
    }

    pub fn tremolo() -> Self {
        Self::new(VibratoTremolo::default(), 1.0)
    }

    pub fn vibrato(period_helper: &PeriodHelper) -> Self {
        match period_helper.freq_type {
            FrequencyType::LinearFrequencies => Self::new(VibratoTremolo::default(), 2.0 * 4.0),
            FrequencyType::AmigaFrequencies => Self::new(VibratoTremolo::default(), 2.0),
        }
    }
}

impl EffectPlugin for EffectVibratoTremolo {
    /* param1: speed, param2:depth */
    fn tick0(&mut self, param1: f32, param2: f32) -> f32 {
        self.data.speed = param1;
        self.data.depth = param2;
        self.retrigger()
    }

    fn tick(&mut self) -> f32 {
        self.in_progress = true;
        self.value = self.data.waveform(self.pos);
        self.pos += self.data.speed;
        self.pos %= 1.0;
        self.value()
    }

    fn in_progress(&self) -> bool {
        self.in_progress
    }

    fn retrigger(&mut self) -> f32 {
        self.in_progress = false;
        self.pos = 0.0;
        self.value = 0.0;
        self.value
    }

    fn clamp(&self, value: f32) -> f32 {
        value
    }

    fn value(&self) -> f32 {
        self.value * self.multiplier
    }
}

impl EffectXM2EffectPlugin for EffectVibratoTremolo {
    fn xm_convert(param: u8, _special: u8) -> Option<(Option<f32>, Option<f32>)> {
        if param > 0 {
            let depth = (param & 0x0F) as f32 / 16.0;
            let depth = if depth != 0.0 { Some(depth) } else { None };

            // from 0..15 to 0..1.0
            let speed = ((param & 0xF0) >> 4) as f32 / 64.0;
            let speed = if speed != 0.0 { Some(speed) } else { None };

            Some((speed, depth))
        } else {
            None
        }
    }

    fn xm_update_effect(&mut self, param: u8, volcolumn: u8, _special2: f32) {
        if volcolumn == 0 {
            if let Some((sspeed, sdepth)) = EffectVibratoTremolo::xm_convert(param, 0) {
                if let Some(speed) = sspeed {
                    self.data.speed = speed;
                }
                if let Some(depth) = sdepth {
                    self.data.depth = depth;
                }
            }
        } else {
            let vol_data = param as f32 / 64.0;
            if vol_data != 0.0 {
                self.data.speed = vol_data;
            }
        }
    }
}
