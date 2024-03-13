/**
 * In the effects implementation, we don't accept the original buffer overflow bugs
 * The idea here is not a 1:1 equivalence but a quality player.
 */
pub enum GenericEffect<'a> {
    Amplitude(&'a dyn EffectPlugin),
    Period(&'a dyn EffectPlugin),
    Panning(&'a dyn EffectPlugin),
}

pub trait EffectPlugin {
    fn tick0(&mut self, param1: f32, param2: f32) -> f32;
    fn tick(&mut self) -> f32;
    fn in_progress(&self) -> bool;
    fn retrigger(&mut self) -> f32;
    fn value(&self) -> f32;
}