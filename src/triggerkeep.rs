pub type TriggerKeep = u8;

pub const TRIGGER_KEEP_NONE: TriggerKeep = 0;
pub const TRIGGER_KEEP_VOLUME: TriggerKeep = 1 << 0;
pub const TRIGGER_KEEP_PERIOD: TriggerKeep = 1 << 1;
pub const TRIGGER_KEEP_SAMPLE_POSITION: TriggerKeep = 1 << 2;
pub const TRIGGER_KEEP_ENVELOPE: TriggerKeep = 1 << 3;

pub fn contains(test: TriggerKeep, flag: TriggerKeep) -> bool {
    (test & flag) == flag
}
