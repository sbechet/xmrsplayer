use xmrs::prelude::FrequencyType;
use std::sync::{Arc, Mutex};
use crate::historical_helper::HistoricalHelper;


#[derive(Clone)]
pub struct PeriodHelper {
    pub freq_type: FrequencyType,
    historical: Option<Arc<Mutex<HistoricalHelper>>>,
}

impl Default for PeriodHelper {
    fn default() -> Self {
        Self {
            freq_type: FrequencyType::LinearFrequencies,
            historical: None,
        }
    }
}

impl PeriodHelper {
    pub fn new(freq_type: FrequencyType, historical: Option<Arc<Mutex<HistoricalHelper>>>) -> Self {
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
                    Self::amiga_frequency(
                        Self::amiga_period(note + arp_note) + 16.0 * period_offset,
                    )
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

    /*
        without historical bug
        finetune : [-1.0..1.0[
    */
    fn adjust_period_from_note_new(&self, period: f32, arp_note: f32, finetune: f32) -> f32 {
        let orig_note: f32 = self.period_to_note(period).round();
        self.note_to_period(orig_note + arp_note + finetune)
    }

    /// adjust period to nearest semitones
    pub fn adjust_period_from_note(&self, period: f32, arp_note: f32, finetune: f32) -> f32 {
        match &self.historical {
            Some(_hhelper) => {
                let finetune = (finetune * 127.0) as i16;
                HistoricalHelper::adjust_period_from_note_historical(&self, period as u16, arp_note as u16, finetune)
            },
            None => {
                self.adjust_period_from_note_new(period, arp_note, finetune)
            }
        }
    }
}
