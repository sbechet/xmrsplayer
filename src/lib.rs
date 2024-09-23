#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(not(any(feature = "std", feature = "libm", feature = "micromath")))]
::core::compile_error!("Must enable at least one of features `std`, `libm`, or `micromath`");

pub mod effect;
pub mod triggerkeep;

pub mod effect_arpeggio;
pub mod effect_multi_retrig_note;
pub mod effect_portamento;
pub mod effect_toneportamento;
//pub mod effect_tremor;
pub mod effect_vibrato_tremolo;
pub mod effect_volume_panning_slide;

pub mod channel;
pub mod helper;
pub mod historical_helper;
pub mod prelude;
pub mod state_auto_vibrato;
pub mod state_envelope;
pub mod state_instr_default;
pub mod state_sample;

pub mod xmrsplayer;
