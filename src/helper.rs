use xmrs::prelude::ModuleFlag;

pub const DEBUG: bool = false;
pub const LINEAR_INTERPOLATION: bool = false;
// pub const RAMPING: bool = false;
pub const SAMPLE_RAMPING_POINTS: usize = 32;

pub const AMIGA_FREQ_SCALE: usize = 1024;

pub static AMIGA_FREQUENCIES: [usize; 13] = [
    1712 * AMIGA_FREQ_SCALE,
    1616 * AMIGA_FREQ_SCALE,
    1525 * AMIGA_FREQ_SCALE,
    1440 * AMIGA_FREQ_SCALE, /* C-2, C#2, D-2, D#2 */
    1357 * AMIGA_FREQ_SCALE,
    1281 * AMIGA_FREQ_SCALE,
    1209 * AMIGA_FREQ_SCALE,
    1141 * AMIGA_FREQ_SCALE, /* E-2, F-2, F#2, G-2 */
    1077 * AMIGA_FREQ_SCALE,
    1016 * AMIGA_FREQ_SCALE,
    961 * AMIGA_FREQ_SCALE,
    907 * AMIGA_FREQ_SCALE, /* G#2, A-2, A#2, B-2 */
    856 * AMIGA_FREQ_SCALE,
]; /* C-3 */

pub static MULTI_RETRIG_ADD: [f32; 16] = [
    0.0, -1.0, -2.0, -4.0, /* 0, 1, 2, 3 */
    -8.0, -16.0, 0.0, 0.0, /* 4, 5, 6, 7 */
    0.0, 1.0, 2.0, 4.0, /* 8, 9, A, B */
    8.0, 16.0, 0.0, 0.0, /* C, D, E, F */
];

pub static MULTI_RETRIG_MULTIPLY: [f32; 16] = [
    1.0,
    1.0,
    1.0,
    1.0, /* 0, 1, 2, 3 */
    1.0,
    1.0,
    2.0 / 3.0,
    0.5, /* 4, 5, 6, 7 */
    1.0,
    1.0,
    1.0,
    1.0, /* 8, 9, A, B */
    1.0,
    1.0,
    1.50,
    2.0, /* C, D, E, F */
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
pub fn clamp_up_1f(vol: &mut f32, limit: f32) {
    if *vol > limit {
        *vol = limit;
    }
}

#[inline(always)]
pub fn clamp_up(vol: &mut f32) {
    clamp_up_1f(vol, 1.0);
}

#[inline(always)]
pub fn clamp_down_1f(vol: &mut f32, limit: f32) {
    if *vol < limit {
        *vol = limit;
    }
}

#[inline(always)]
pub fn clamp_down(vol: &mut f32) {
    clamp_down_1f(vol, 0.0);
}

#[inline(always)]
pub fn clamp(vol: &mut f32) {
    if *vol > 1.0 {
        *vol = 1.0;
    } else if *vol < 0.0 {
        *vol = 0.0;
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
    7680.0 - note * 64.0
}

#[inline(always)]
pub fn linear_frequency(period: f32) -> f32 {
    8363.0 * (2.0f32).powf((4608.0 - period) / 768.0)
}

pub fn amiga_period(note: f32) -> f32 {
    let intnote = note as i32;
    let a = intnote % 12;
    let octave = intnote / 12 - 2;
    let mut p1 = AMIGA_FREQUENCIES[a as usize];
    let mut p2 = AMIGA_FREQUENCIES[(a + 1) as usize];

    if octave > 0 {
        p1 >>= octave;
        p2 >>= octave;
    } else if octave < 0 {
        p1 <<= -octave;
        p2 <<= -octave;
    }

    lerp(p1 as f32, p2 as f32, note - intnote as f32) / AMIGA_FREQ_SCALE as f32
}

pub fn period(freq_type: ModuleFlag, note: f32) -> f32 {
    match freq_type {
        ModuleFlag::LinearFrequencies => linear_period(note),
        ModuleFlag::AmigaFrequencies => amiga_period(note),
    }
}

pub fn amiga_frequency(period: f32) -> f32 {
    if period == 0.0 {
        0.0
    } else {
        7093789.2 / (period * 2.0)
    }
}

pub fn frequency(freq_type: ModuleFlag, period: f32, note_offset: f32, period_offset: f32) -> f32 {
    match freq_type {
        ModuleFlag::LinearFrequencies => {
            linear_frequency(period - 64.0 * note_offset - 16.0 * period_offset)
        }
        ModuleFlag::AmigaFrequencies => {
            if note_offset == 0.0 {
                /* A chance to escape from insanity */
                return amiga_frequency(period + 16.0 * period_offset);
            }

            /* FIXME: this is very crappy at best */
            let mut a = 0;
            let mut octave: i8 = 0;

            /* Find the octave of the current period */
            let period = period * AMIGA_FREQ_SCALE as f32;
            if period > AMIGA_FREQUENCIES[0] as f32 {
                octave -= 1;
                while period > (AMIGA_FREQUENCIES[0] << -octave) as f32 {
                    octave -= 1;
                }
            } else if period < AMIGA_FREQUENCIES[12] as f32 {
                octave += 1;
                while period < (AMIGA_FREQUENCIES[12] >> octave) as f32 {
                    octave += 1;
                }
            }
            /* Find the smallest note closest to the current period */
            let mut p1 = 0;
            let mut p2 = 0;
            for i in 0..12 {
                p1 = AMIGA_FREQUENCIES[i];
                p2 = AMIGA_FREQUENCIES[i + 1];

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
            return amiga_frequency(amiga_period(note + note_offset) + 16.0 * period_offset);
        }
    }
}
