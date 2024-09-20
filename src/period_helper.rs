use xmrs::prelude::FrequencyType;

#[cfg(feature = "libm")]
use num_traits::float::Float;
#[cfg(feature = "micromath")]
use micromath::F32Ext;

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

    // ==== Linear

    /// return period
    #[inline(always)]
    fn linear_note_to_period(note: f32) -> f32 {
        // 10.0: number of octaves
        // 12.0: halftones
        // 16.0: number of finetune steps
        //  4.0: finetune resolution
        10.0 * 12.0 * 16.0 * 4.0 - note * 16.0 * 4.0
    }

    /// return note
    #[inline(always)]
    fn linear_period_to_note(period: f32) -> f32 {
        (10.0 * 12.0 * 16.0 * 4.0 - period) / (16.0 * 4.0)
    }

    /// return frequency
    #[inline(always)]
    fn linear_period_to_frequency(period: f32) -> f32 {
        // 8363.0 is historical amiga module sample frequency (Paula chipset related)
        //  6: octave center
        // 12: halftones
        // 64: period resolution (16.0 * 4.0)
        //     16.0: number of finetune steps
        //      4.0: finetune step resolution
        8363.0 * (2.0f32).powf((6.0 * 12.0 * 16.0 * 4.0 - period) / (12.0 * 16.0 * 4.0))
    }

    // ==== Amiga

    /// return period
    #[inline(always)]
    fn amiga_note_to_period(note: f32) -> f32 {
        /* found using scipy.optimize.curve_fit */
        6848.0 * (-0.0578 * note).exp() + 0.2782
    }

    /// return note
    #[inline(always)]
    fn amiga_period_to_note(period: f32) -> f32 {
        -f32::ln((period - 0.2782) / 6848.0) / 0.0578
    }

    /// return frequency
    #[inline(always)]
    fn amiga_period_to_frequency(period: f32) -> f32 {
        if period == 0.0 {
            0.0
        } else {
            // 7159090.5 / (period * 2.0) // NTSC
            7093789.2 / (period * 2.0) // PAL
        }
    }

    // ==== Generic (TODO: use a trait any day?)

    pub fn note_to_period(&self, note: f32) -> f32 {
        match self.freq_type {
            FrequencyType::LinearFrequencies => Self::linear_note_to_period(note),
            FrequencyType::AmigaFrequencies => Self::amiga_note_to_period(note),
        }
    }

    pub fn period_to_note(&self, period: f32) -> f32 {
        match self.freq_type {
            FrequencyType::LinearFrequencies => Self::linear_period_to_note(period),
            FrequencyType::AmigaFrequencies => Self::amiga_period_to_note(period),
        }.max(0.0)  // Remove < 0.0 and NaN numbers
    }

    pub fn period_to_frequency(&self, period: f32) -> f32 {
        match self.freq_type {
            FrequencyType::LinearFrequencies => Self::linear_period_to_frequency(period),
            FrequencyType::AmigaFrequencies => Self::amiga_period_to_frequency(period),
        }
    }

    //-----------------------------------------------------

    // new adjust period to arpeggio and finetune delta
    pub fn adjust_period(&self, period: f32, arp_note: f32, finetune: f32, semitone: bool) -> f32 {
        let note_orig: f32 = self.period_to_note(period);
        
        let note = if semitone {
            note_orig.round()
        } else {
            note_orig
        };

        if self.historical && arp_note != 0.0 {
            // From C-0 (0) to B-7 (95) only
            let mut note = note;
            if note.ceil() >= 95.0 {
                note = 95.0;
            }
            self.note_to_period(note + arp_note + finetune)
        } else {
            self.note_to_period(note + arp_note + finetune)
        }        
    }
}
