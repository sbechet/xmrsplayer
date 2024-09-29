#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(not(any(feature = "std", feature = "libm", feature = "micromath")))]
::core::compile_error!("Must enable at least one of features `std`, `libm`, or `micromath`");

pub(crate) mod effect;
pub(crate) mod triggerkeep;

pub(crate) mod effect_arpeggio;
pub(crate) mod effect_multi_retrig_note;
pub(crate) mod effect_portamento;
pub(crate) mod effect_toneportamento;
//pub(crate) mod effect_tremor;
pub(crate) mod effect_vibrato_tremolo;
pub(crate) mod effect_volume_panning_slide;

pub mod channel;
pub(crate) mod helper;
pub(crate) mod historical_helper;
pub mod prelude;
pub(crate) mod state_auto_vibrato;
pub(crate) mod state_envelope;
pub(crate) mod state_instr_default;
pub(crate) mod state_sample;

pub mod xmrsplayer;
