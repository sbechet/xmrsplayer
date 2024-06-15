/// Here we concentrate some old bugs
use crate::period_helper::PeriodHelper;

#[cfg(feature = "libm")]
use num_traits::float::Float;
#[cfg(feature = "micromath")]
use micromath::F32Ext;

/// Struct is very small we can clone it everywhere in other structs...

#[derive(Default, Clone)]
pub struct HistoricalHelper {
    pub tempo: u16,
}

impl HistoricalHelper {
    pub fn new(tempo: u16) -> Self {
        Self { tempo }
    }

    pub fn set_tempo(&mut self, tempo: u16) {
        self.tempo = tempo;
    }

    /// Arpeggio
    pub fn arpeggio_tick(&self, tick: u8) -> u8 {
        let tick = tick as u16 % self.tempo;
        let reverse_tick = (self.tempo - tick - 1) as u8;
        match reverse_tick {
            0..=15 => reverse_tick % 3,
            51 | 54 | 60 | 63 | 72 | 78 | 81 | 93 | 99 | 105 | 108 | 111 | 114 | 117 | 120
            | 123 | 126 | 129 | 132 | 135 | 138 | 141 | 144 | 147 | 150 | 153 | 156 | 159 | 165
            | 168 | 171 | 174 | 177 | 180 | 183 | 186 | 189 | 192 | 195 | 198 | 201 | 204 | 207
            | 210 | 216 | 219 | 222 | 225 | 228 | 231 | 234 | 237 | 240 | 243 => 0,
            _ => 2,
        }
    }

    /// Multi Retrig Note
    pub fn value_historical_computers(vol: f32, note_retrig_vol: f32) -> f32 {
        match (16.0 * note_retrig_vol) as u8 {
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

    // Parts from ft2-clone - Copyright (c) 2016-2024, Olav Sørensen - BSD-3-Clause license
    // no way to accept these bugs today!
    pub fn adjust_period_from_note_historical(
        phelper: &PeriodHelper,
        period: u16,
        arp_note: u16,
        finetune: i16,
    ) -> f32 {
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

            if period >= phelper.note_to_period(look_up as f32 / 16.0 - 1.0).round() as u16 {
                hi_period = ((tmp_period - fine_tune) as u16 & 0xFFF0) as i16;
            } else {
                lo_period = ((tmp_period - fine_tune) as u16 & 0xFFF0) as i16;
            }
        }

        let tmp_period =
            (lo_period as f32 / 16.0) + ((fine_tune - 16) as f32 / 16.0) + (arp_note as f32);
        phelper.note_to_period(tmp_period).max(1540.0)
    }
}
