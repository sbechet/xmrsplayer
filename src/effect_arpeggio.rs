use crate::effect::{EffectPlugin, EffectXM2EffectPlugin};
use core::default::Default;

#[derive(Clone, Default)]
pub struct Arpeggio {
    offset1: f32,
    offset2: f32,
}

#[derive(Clone, Default)]
pub struct EffectArpeggio {
    data: Arpeggio,
    historical: bool,
    tick: u8,
    in_progress: bool,
}

impl EffectArpeggio {
    pub fn new(historicalft2: bool) -> Self {
        Self {
            historical: historicalft2,
            ..Default::default()
        }
    }

    pub fn historical(&self) -> bool {
        self.historical
    }

    pub fn get_current_tick(&self) -> u8 {
        self.tick
    }

    // No way to accept that bug today!
    fn historical_ft2_tick(&self) -> u8 {
        match self.tick {
            0..=15 => self.tick % 3,
            51 | 54 | 60 | 63 | 72 | 78 | 81 | 93 | 99 | 105 | 108 | 111 | 114 | 117 | 120
            | 123 | 126 | 129 | 132 | 135 | 138 | 141 | 144 | 147 | 150 | 153 | 156 | 159 | 165
            | 168 | 171 | 174 | 177 | 180 | 183 | 186 | 189 | 192 | 195 | 198 | 201 | 204 | 207
            | 210 | 216 | 219 | 222 | 225 | 228 | 231 | 234 | 237 | 240 | 243 => 0,
            _ => 2,
        }
    }
}

impl EffectPlugin for EffectArpeggio {
    fn tick0(&mut self, param1: f32, param2: f32) -> f32 {
        self.data.offset1 = param1;
        self.data.offset2 = param2;
        self.retrigger()
    }

    fn tick(&mut self) -> f32 {
        self.in_progress = true;
        self.tick += 1;
        self.value()
    }

    fn in_progress(&self) -> bool {
        self.in_progress
    }

    fn retrigger(&mut self) -> f32 {
        self.tick = 0;
        self.in_progress = false;
        self.value()
    }

    fn clamp(&self, _value: f32) -> f32 {
        self.value()
    }

    fn value(&self) -> f32 {
        if self.historical {
            match self.historical_ft2_tick() {
                1 => self.data.offset1,
                2 => self.data.offset2,
                _ => 0.0,
            }
        } else {
            match self.tick % 3 {
                1 => self.data.offset1,
                2 => self.data.offset2,
                _ => 0.0,
            }
        }
    }
}

impl EffectXM2EffectPlugin for EffectArpeggio {
    fn xm_convert(param: u8, _special: u8) -> Option<(Option<f32>, Option<f32>)> {
        if param > 0 {
            let v1 = (param >> 4) as f32;
            let v2 = (param & 0x0F) as f32;
            Some((Some(v1), Some(v2)))
        } else {
            None
        }
    }

    fn xm_update_effect(&mut self, param: u8, _special1: u8, _special2: f32) {
        if let Some((v1, v2)) = Self::xm_convert(param, 0) {
            self.tick0(v1.unwrap(), v2.unwrap());
        }
    }
}
