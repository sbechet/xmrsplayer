use xmrs::prelude::FrequencyType;

#[derive(Clone)]
pub struct PeriodHelper {
    pub freq_type: FrequencyType,
    historical: bool,
}

impl Default for PeriodHelper {
    fn default() -> Self {
        Self {
            freq_type: FrequencyType::LinearFrequencies,
            historical: false,
        }
    }
}

impl PeriodHelper {
    pub fn new(freq_type: FrequencyType, historical: bool) -> Self {
        Self { 
            freq_type,
            historical,
        }
    }

    /// return period
    #[inline(always)]
    fn linear_period(note: f32) -> f32 {
        // 10.0: number of octaves
        // 12.0: halftones
        // 16.0: number of finetune steps
        //  4.0: finetune resolution
        10.0 * 12.0 * 16.0 * 4.0 - note * 16.0 * 4.0
    }

    /// return frequency
    #[inline(always)]
    fn linear_frequency(period: f32) -> f32 {
        // 8363.0 is historical amiga module sample frequency (Paula chipset related)
        //  6: octave center
        // 12: halftones
        // 64: period resolution (16.0 * 4.0)
        //     16.0: number of finetune steps
        //      4.0: finetune step resolution
        let p = 8363.0 * (2.0f32).powf((6.0 * 12.0 * 16.0 * 4.0 - period) / (12.0 * 16.0 * 4.0));
        return p;
    }

    /// return note
    #[inline(always)]
    fn linear_note(period: f32) -> f32 {
        (10.0 * 12.0 * 16.0 * 4.0 - period) / (16.0 * 4.0)
    }

    /// return period
    #[inline(always)]
    fn amiga_period(note: f32) -> f32 {
        /* found using scipy.optimize.curve_fit */
        6848.0 * (-0.0578 * note).exp() + 0.2782
    } 

    /// return frequency
    fn amiga_frequency(period: f32) -> f32 {
        if period == 0.0 {
            0.0
        } else {
            // PAL Value = 7093789.2, NTSC Value = 7159090.5
            7093789.2 / (period * 2.0)
        }
    }

    /// return note
    #[inline(always)]
    fn amiga_note(period: f32) -> f32 {
        -f32::ln((period - 0.2782) / 6848.0) / 0.0578
    }

    pub fn note_to_period(&self, note: f32) -> f32 {
        match self.freq_type {
            FrequencyType::LinearFrequencies => Self::linear_period(note),
            FrequencyType::AmigaFrequencies => Self::amiga_period(note),
        }
    }

    pub fn period_to_note(&self, period: f32) -> f32 {
        match self.freq_type {
            FrequencyType::LinearFrequencies => Self::linear_note(period),
            FrequencyType::AmigaFrequencies => Self::amiga_note(period),
        }
    }

    pub fn frequency(&self, period: f32, arp_note: f32, period_offset: f32) -> f32 {
        if arp_note == 0.0 {
            match self.freq_type {
                FrequencyType::LinearFrequencies => {
                    Self::linear_frequency(period - 64.0 * arp_note - 16.0 * period_offset)
                }
                FrequencyType::AmigaFrequencies => {
                    let note = Self::amiga_note(period);
                    Self::amiga_frequency(Self::amiga_period(note + arp_note) + 16.0 * period_offset)
                }
            }
        } else {
            let period = self.adjust_period_from_note(period, arp_note, period_offset);
            match self.freq_type {
                FrequencyType::LinearFrequencies => Self::linear_frequency(period),
                FrequencyType::AmigaFrequencies => Self::amiga_frequency(period),
            }
        }
    }

    // Parts from ft2-clone - Copyright (c) 2016-2024, Olav Sørensen - BSD-3-Clause license
    // no way to accept these bugs today!
    fn adjust_period_from_note_historical(&self, period: u16, arp_note: u16, finetune: i16) -> f32 {
        let fine_tune: i16 = (finetune / 8 + 16) as i16;

        // FT2 bug, should've been 10*12*16. Notes above B-7 (95) will have issues.
        // You can only achieve such high notes by having a high relative note setting.
        let mut hi_period: i16 = 8 * 12 * 16;
        let mut lo_period: i16 = 0;

        for _i in 0..8 {
            let tmp_period = (((lo_period + hi_period) >> 1) as u16 & 0xFFF0) as i16 + fine_tune;
            let mut look_up = tmp_period as i32 - 8;
            if look_up < 0 {
                look_up = 0; // safety fix (C-0 w/ f.tune <= -65). This seems to result in 0 in FT2 (TODO: verify)
            }

            if period >= self.note_to_period(look_up as f32 / 16.0 - 1.0).round() as u16 {
                hi_period = ((tmp_period - fine_tune) as u16 & 0xFFF0) as i16;
            } else {
                lo_period = ((tmp_period - fine_tune) as u16 & 0xFFF0) as i16;
            }
        }

        let tmp_period = (lo_period as f32 / 16.0) + ((fine_tune - 16) as f32 / 16.0) + (arp_note as f32);
        self.note_to_period(tmp_period).max(1540.0)
    }

    /*
        without historical bug
        finetune : [-1.0..1.0[
    */
    fn adjust_period_from_note_new(&self, period: f32, arp_note: f32, finetune: f32) -> f32 {
        let orig_note: f32 = self.period_to_note(period).round();
        self.note_to_period(orig_note + arp_note + finetune)
    }

    /// adjust period to nearest semitones
    fn adjust_period_from_note(&self, period: f32, arp_note: f32, finetune: f32) -> f32 {
        if self.historical {
            let finetune = (finetune * 127.0) as i16;
            self.adjust_period_from_note_historical(period as u16, arp_note as u16, finetune)
        } else {
            self.adjust_period_from_note_new(period, arp_note, finetune)
        }
    }

}
