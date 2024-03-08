/// Effect Vibrato
#[derive(Default, Clone, Copy, Debug)]
pub struct StateVibratoTremolo {
    pub waveform: u8,
    pub speed: f32,
    pub depth: f32,
    pub in_progress: bool,
    pub retrigger: bool,
    pub pos: f32,
    pub offset: f32,
}

impl StateVibratoTremolo {
    pub fn new(waveform: u8, speed: f32, depth: f32) -> Self {
        Self {
            waveform,
            speed,
            depth,
            in_progress: false,
            retrigger: false,
            pos: 0.0,
            offset: 0.0,
        }
    }

    fn value(&self) -> f32 {
        match self.waveform {
            0 => {
                if self.pos < 1.0/3.0 {
                    3.0 * self.pos
                } else {
                    (std::f32::consts::FRAC_PI_2 * (4.0/3.0 * (self.pos - 1.0/3.0))).sin() - 1.0
                }
            },
            1 => {
                if self.pos < 0.5 {
                    2.0 * self.pos
                } else {
                    1.0 - 2.0 * (self.pos - 0.5)
                }
            }
            _ => -0.0
        }
    }

    fn tick(&mut self) {
        let offset = self.depth * self.value();
        if self.pos < 0.5 {
            self.offset = offset;
        } else {
            self.offset = -offset;
        }
        self.pos += self.speed;
        self.pos %= 1.0;
    }

    pub fn tick_vibrato(&mut self) {
        self.tick();
    }

    pub fn tick_tremolo(&mut self) {
        self.tick();
        self.offset /= 2.0;
    }


}