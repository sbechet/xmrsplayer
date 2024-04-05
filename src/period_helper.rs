use xmrs::prelude::FrequencyType;
use crate::helper::*;

pub const AMIGA_PERIOD_SCALE: usize = 1024;

pub static AMIGA_PERIODS: [usize; 13] = [
    1712 * AMIGA_PERIOD_SCALE, // C-2
    1616 * AMIGA_PERIOD_SCALE, // C#2
    1525 * AMIGA_PERIOD_SCALE, // D-2
    1440 * AMIGA_PERIOD_SCALE, // D#2
    1357 * AMIGA_PERIOD_SCALE, // E-2
    1281 * AMIGA_PERIOD_SCALE, // F-2
    1209 * AMIGA_PERIOD_SCALE, // F#2
    1141 * AMIGA_PERIOD_SCALE, // G-2
    1077 * AMIGA_PERIOD_SCALE, // G#2
    1016 * AMIGA_PERIOD_SCALE, // A-2
    961 * AMIGA_PERIOD_SCALE,  // A#2
    907 * AMIGA_PERIOD_SCALE,  // B-2
    856 * AMIGA_PERIOD_SCALE,  // C-3
];

#[derive(Clone)]
pub struct PeriodHelper {
    pub freq_type: FrequencyType
}

impl Default for PeriodHelper {
    fn default() -> Self {
        Self {
            freq_type: FrequencyType::LinearFrequencies
        }
    }
}

impl PeriodHelper {

    pub fn new(freq_type: FrequencyType) -> Self {
        Self {
            freq_type,
        }
    }


    #[inline(always)]
    fn linear_period(note: f32) -> f32 {
        64.0 * (10.0 * 12.0 - note)
    }
    
    #[inline(always)]
    fn linear_frequency(period: f32) -> f32 {
        // 8363.0 is historical amiga module sample frequency
        8363.0 * (2.0f32).powf((64.0 * 12.0 * 6.0 - period) / (64.0 * 12.0))
    }
    
    fn amiga_period(note: f32) -> f32 {
        let intnote = note as i32;
        let a = intnote % 12;
        let octave = intnote / 12 - 2;
        let mut p1 = AMIGA_PERIODS[a as usize];
        let mut p2 = AMIGA_PERIODS[(a + 1) as usize];
    
        if octave > 0 {
            p1 >>= octave;
            p2 >>= octave;
        } else if octave < 0 {
            p1 <<= -octave;
            p2 <<= -octave;
        }
    
        lerp(p1 as f32, p2 as f32, note - intnote as f32) / AMIGA_PERIOD_SCALE as f32
    }
    
    pub fn period(&self, note: f32) -> f32 {
        match self.freq_type {
            FrequencyType::LinearFrequencies => Self::linear_period(note),
            FrequencyType::AmigaFrequencies => Self::amiga_period(note),
        }
    }
    
    fn amiga_frequency(period: f32) -> f32 {
        if period == 0.0 {
            0.0
        } else {
            7093789.2 / (period * 2.0)
        }
    }
    
    // TODO: Clamp args like period?
    pub fn frequency(&self, period: f32, arp_note: f32, period_offset: f32) -> f32 {
        match self.freq_type {
            FrequencyType::LinearFrequencies => {
                Self::linear_frequency(period - 64.0 * arp_note - 16.0 * period_offset)
            }
            FrequencyType::AmigaFrequencies => {
                if arp_note == 0.0 {
                    /* A chance to escape from insanity */
                    return Self::amiga_frequency(period + 16.0 * period_offset);
                }
    
                /* FIXME: this is very crappy at best */
                let mut a = 0;
                let mut octave: i16 = 0;
    
                /* Find the octave of the current period */
                let period = period * AMIGA_PERIOD_SCALE as f32;
                if period > AMIGA_PERIODS[0] as f32 {
                    octave -= 1;
                    while period > (AMIGA_PERIODS[0] << -octave) as f32 {
                        octave -= 1;
                    }
                } else if period < AMIGA_PERIODS[12] as f32 {
                    octave += 1;
                    while period < (AMIGA_PERIODS[12] >> octave) as f32 {
                        octave += 1;
                    }
                }
                /* Find the smallest note closest to the current period */
                let mut p1 = 0;
                let mut p2 = 0;
                for i in 0..12 {
                    p1 = AMIGA_PERIODS[i];
                    p2 = AMIGA_PERIODS[i + 1];
    
                    if octave > 0 {
                        p1 >>= octave;
                        p2 >>= octave;
                    } else if octave < 0 {
                        p1 <<= -octave;
                        p2 <<= -octave;
                    }
    
                    if p2 as f32 <= period && period <= p1 as f32 {
                        a = i;
                        break;
                    }
                }
    
                let note =
                    (12 * (octave + 2)) as f32 + a as f32 + inverse_lerp(p1 as f32, p2 as f32, period);
                return Self::amiga_frequency(Self::amiga_period(note + arp_note) + 16.0 * period_offset);
            }
        }
    }
}


