use crate::effect::*;

#[derive(Default, Clone, Copy, Debug)]
pub struct VibratoTremolo {
    pub waveform: u8,
    pub speed: f32,
    pub depth: f32,
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
                0 => (std::f32::consts::TAU * pos).sin(),
                1 => {
                    if pos < 0.25 {
                        4.0 * pos
                    } else if pos < 0.75 {
                        -4.0 * (pos - 0.25) + 1.0
                    } else {
                        4.0 * (pos - 0.75) - 1.0
                    }
                }
                _ => {
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
    pub is_tremolo: bool,
    pub in_progress: bool,
    pub pos: f32,
    pub value: f32,
}

impl EffectVibratoTremolo {
    fn new(data: VibratoTremolo, is_tremolo: bool) -> Self {
        Self {
            data,
            is_tremolo,
            in_progress: false,
            pos: 0.0,
            value: 0.0,
        }
    }

    pub fn tremolo() -> Self {
        Self::new(VibratoTremolo::default(), true)
    }

    pub fn vibrato() -> Self {
        Self::new(VibratoTremolo::default(), false)
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
        if self.is_tremolo {
            self.value / 2.0
        } else {
            self.value
        }
    }
}

impl EffectXM2EffectPlugin for EffectVibratoTremolo {
    fn xm_convert(param: u8, _special: u8) -> Option<(Option<f32>, Option<f32>)> {
        if param > 0 {
            let depth = (param & 0x0F) as f32 / 2.0;
            let depth = if depth != 0.0 { Some(depth) } else { None };

            // from 0..63 after shift to 0..1.0
            let speed = ((param & 0xF0) >> 2) as f32 / 64.0 / 4.0;
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
            // from 0..15 to 0..63 then to 0..1.0
            let vol_data = param as f32 * 4.0 / 63.0;
            if vol_data != 0.0 {
                self.data.speed = vol_data;
            }
        }
    }
}
