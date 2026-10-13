use super::{ByteRange, Encoding};
use crate::regex::CharSet;

/// Lowers character sets into single bytes in the range 0 through 255.
pub(crate) struct Bytes;

impl Encoding for Bytes {
    type Label = ByteRange;
    type Sequence = [ByteRange; 1];

    fn encode(&self, set: &CharSet) -> Vec<Self::Sequence> {
        set.ranges()
            .filter_map(|(low, high)| {
                let low = u8::try_from(low as u32).ok()?;
                let high = (high as u32).min(255) as u8;
                Some([ByteRange::new(low, high)])
            })
            .collect()
    }
}
