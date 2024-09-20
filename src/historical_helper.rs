#[cfg(feature = "micromath")]
use micromath::F32Ext;
#[cfg(feature = "libm")]
use num_traits::float::Float;

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
}
