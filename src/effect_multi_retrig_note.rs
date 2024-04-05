use crate::effect::*;
use crate::helper::*;
use core::default::Default;

#[derive(Clone, Default)]
pub struct MultiRetrigNote {
    note_retrig_speed: f32,
    note_retrig_vol: f32,
}

impl MultiRetrigNote {
    // for historical purpose
    fn value_historical_computers(&self, vol: f32) -> f32 {
        match (16.0 * self.note_retrig_vol) as u8 {
            0 | 8 => vol,
            rv @ (1 | 2 | 3 | 4 | 5) => vol - ((1 << rv) - 1) as f32,
            6 => vol * 2.0 / 3.0,
            7 => vol / 2.0,
            rv @ (9 | 10 | 11 | 12 | 13) => vol + ((1 << rv) - 9) as f32,
            14 => vol * 3.0 / 2.0,
            15 => vol * 2.0,
            _ => 0.0,
        }
    }

    fn value_new_computers(&self, vol: f32) -> f32 {
        vol * if self.note_retrig_vol <= 0.5 {
            std::f32::consts::FRAC_PI_2
                + self.note_retrig_vol * (f32::asin(0.5) - std::f32::consts::FRAC_PI_2)
        } else {
            std::f32::consts::PI * (1.0 + self.note_retrig_vol)
        }
    }
}

#[derive(Clone, Default)]
pub struct EffectMultiRetrigNote {
    data: MultiRetrigNote,
    historical: bool,
    tick: f32,
}

impl EffectMultiRetrigNote {
    pub fn new(historical: bool, speed: f32, vol: f32) -> Self {
        Self {
            data: MultiRetrigNote {
                note_retrig_speed: speed,
                note_retrig_vol: vol,
            },
            historical,
            ..Default::default()
        }
    }
}

impl EffectPlugin for EffectMultiRetrigNote {
    fn tick0(&mut self, note_retrig_speed: f32, note_retrig_vol: f32) -> f32 {
        self.data.note_retrig_speed = note_retrig_speed;
        if note_retrig_vol != 0.0 {
            self.data.note_retrig_vol = note_retrig_vol;
        }
        self.tick = 1.0;
        self.value()
    }

    fn tick(&mut self) -> f32 {
        self.tick += 1.0;
        self.tick %= self.data.note_retrig_speed;
        self.tick
    }

    fn in_progress(&self) -> bool {
        self.data.note_retrig_speed != 0.0
    }

    fn retrigger(&mut self) -> f32 {
        self.tick = 0.0;
        0.0
    }

    fn clamp(&self, vol: f32) -> f32 {
        if self.tick as f32 >= self.data.note_retrig_speed {
            vol
        } else {
            let mut v = if self.historical {
                self.data.value_historical_computers(vol)
            } else {
                self.data.value_new_computers(vol)
            };
            clamp(&mut v);
            v
        }
    }

    fn value(&self) -> f32 {
        0.0
    }
}

impl EffectXM2EffectPlugin for EffectMultiRetrigNote {
    fn xm_convert(param: u8, _special: u8) -> Option<(Option<f32>, Option<f32>)> {
        let note_retrig_speed = if param & 0x0F == 0 {
            None
        } else {
            Some((param & 0x0F) as f32)
        };

        let note_retrig_vol = if param >> 4 == 0 {
            None
        } else {
            Some((param >> 4) as f32 / 16.0)
        };

        if note_retrig_speed != None || note_retrig_vol != None {
            Some((note_retrig_speed, note_retrig_vol))
        } else {
            None
        }
    }

    fn xm_update_effect(&mut self, param: u8, _special1: u8, _special2: f32) {
        match EffectMultiRetrigNote::xm_convert(param, 0) {
            Some(elem) => {
                if let Some(speed) = elem.0 {
                    self.data.note_retrig_speed = speed;
                }
                if let Some(vol) = elem.1 {
                    self.data.note_retrig_vol = vol;
                }
            }
            None => {}
        }
    }
}
