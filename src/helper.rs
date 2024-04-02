use xmrs::prelude::FrequencyType;

pub const DEBUG: bool = false;
pub const LINEAR_INTERPOLATION: bool = true;

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

#[inline(always)]
pub fn note_is_valid(n: u8) -> bool {
    n > 0 && n < 97
}

#[inline(always)]
pub fn lerp(u: f32, v: f32, t: f32) -> f32 {
    u + t * (v - u)
}

#[inline(always)]
pub fn inverse_lerp(u: f32, v: f32, lerp: f32) -> f32 {
    (lerp - u) / (v - u)
}

#[inline(always)]
pub fn clamp_up_1f(value: &mut f32, limit: f32) {
    if *value > limit {
        *value = limit;
    }
}

#[inline(always)]
pub fn clamp_up(value: &mut f32) {
    clamp_up_1f(value, 1.0);
}

#[inline(always)]
pub fn clamp_down_1f(value: &mut f32, limit: f32) {
    if *value < limit {
        *value = limit;
    }
}

#[inline(always)]
pub fn clamp_down(value: &mut f32) {
    clamp_down_1f(value, 0.0);
}

#[inline(always)]
pub fn clamp(value: &mut f32) {
    if *value > 1.0 {
        *value = 1.0;
    } else if *value < 0.0 {
        *value = 0.0;
    }
}

#[inline(always)]
pub fn slide_towards(val: &mut f32, goal: f32, incr: f32) {
    if *val > goal {
        *val -= incr;
        clamp_down_1f(val, goal);
    } else if *val < goal {
        *val += incr;
        clamp_up_1f(val, goal);
    }
}

#[inline(always)]
pub fn linear_period(note: f32) -> f32 {
    64.0 * (10.0 * 12.0 - note)
}

#[inline(always)]
pub fn linear_frequency(period: f32) -> f32 {
    // 8363.0 is historical amiga module sample frequency
    8363.0 * (2.0f32).powf((64.0 * 6.0 * 12.0 - period) / (64.0 * 12.0))
}

pub fn amiga_period(note: f32) -> f32 {
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

pub fn period(freq_type: FrequencyType, note: f32) -> f32 {
    match freq_type {
        FrequencyType::LinearFrequencies => linear_period(note),
        FrequencyType::AmigaFrequencies => amiga_period(note),
    }
}

pub fn amiga_frequency(period: f32) -> f32 {
    if period == 0.0 {
        0.0
    } else {
        7093789.2 / (period * 2.0)
    }
}

// TODO: Clamp args like period?
pub fn frequency(freq_type: FrequencyType, period: f32, arp_note: f32, period_offset: f32) -> f32 {
    match freq_type {
        FrequencyType::LinearFrequencies => {
            linear_frequency(period - 64.0 * arp_note - 16.0 * period_offset)
        }
        FrequencyType::AmigaFrequencies => {
            if arp_note == 0.0 {
                /* A chance to escape from insanity */
                return amiga_frequency(period + 16.0 * period_offset);
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
            return amiga_frequency(amiga_period(note + arp_note) + 16.0 * period_offset);
        }
    }
}
