//! Bounds selector expansion during execution planning.

pub(super) const MAX_INLINE_DEPTH: usize = 64;
pub(super) const MAX_SHARED_TAIL: usize = 8;
pub(super) const SELF_LOOP_CHUNK: usize = 8;
