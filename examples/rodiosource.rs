use rodio::Source;
use xmrsplayer::xmrsplayer::XmrsPlayer;

impl Source for XmrsPlayer {
    fn current_frame_len(&self) -> Option<usize> {
        Some(1)
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.get_sample_rate() as u32
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}
