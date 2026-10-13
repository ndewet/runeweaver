//! Executes byte automata independently of a source representation.

/// Reports the longest accepted prefix and the traversed bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefixMatch {
    /// The winning rule, or no rule when the prefix does not match.
    pub rule: Option<usize>,
    /// The end of the accepted prefix, or the starting offset when none accepts.
    pub end: usize,
    /// The end reached before a missing transition or end of input.
    pub traversed: usize,
}

/// Supplies a DFA to the shared byte matcher.
///
/// Adapters select priorities when they supply each state's accepting rule.
/// Each rule must consume at least one byte.
pub trait ByteMatcher {
    /// Returns the start state identifier.
    fn start() -> usize;
    /// Returns the transition target for a state and byte.
    fn transition(state: usize, byte: u8) -> Option<usize>;
    /// Returns the winning rule at a state.
    fn accept(state: usize) -> Option<usize>;

    /// Matches the longest prefix of a byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::bytes::ByteMatcher;
    /// struct Rule;
    /// impl ByteMatcher for Rule {
    ///     fn start() -> usize { 0 }
    ///     fn transition(state: usize, byte: u8) -> Option<usize> {
    ///         (state == 0 && byte == 255).then_some(1)
    ///     }
    ///     fn accept(state: usize) -> Option<usize> { (state == 1).then_some(0) }
    /// }
    /// let matched = Rule::match_bytes(b"\xff!");
    /// assert_eq!(matched.rule, Some(0));
    /// assert_eq!(matched.end, 1);
    /// ```
    fn match_bytes(input: &[u8]) -> PrefixMatch {
        Self::match_source(|offset| input.get(offset).copied(), 0)
    }

    /// Matches bytes read from a custom source at an absolute offset.
    ///
    /// The reader returns `None` at end of input. No match consumes no bytes.
    /// `traversed` lets an adapter select its own error recovery policy.
    ///
    /// # Panics
    ///
    /// This function panics if the reader supplies a byte at `usize::MAX`.
    ///
    /// # Examples
    ///
    /// ```
    /// use runeweaver::bytes::ByteMatcher;
    /// struct Rule;
    /// impl ByteMatcher for Rule {
    ///     fn start() -> usize { 0 }
    ///     fn transition(_: usize, byte: u8) -> Option<usize> {
    ///         (byte == b'a').then_some(1)
    ///     }
    ///     fn accept(state: usize) -> Option<usize> { (state == 1).then_some(0) }
    /// }
    /// let input = b"!aaa!";
    /// let matched = Rule::match_source(|offset| input.get(offset).copied(), 1);
    /// assert_eq!(matched.end, 4);
    /// ```
    fn match_source(mut read: impl FnMut(usize) -> Option<u8>, offset: usize) -> PrefixMatch {
        let mut state = Self::start();
        let mut matched = PrefixMatch {
            rule: None,
            end: offset,
            traversed: offset,
        };
        while let Some(byte) = read(matched.traversed) {
            let Some(next) = Self::transition(state, byte) else {
                break;
            };
            state = next;
            matched.traversed = matched
                .traversed
                .checked_add(1)
                .expect("a source byte offset fits in usize");
            if let Some(rule) = Self::accept(state) {
                matched.rule = Some(rule);
                matched.end = matched.traversed;
            }
        }
        matched
    }
}
