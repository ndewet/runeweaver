//! Converts what a rule converter returns into a token payload.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// A failure produced while converting an accepted lexeme into a payload.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadError {
    message: String,
}

impl PayloadError {
    /// Creates a payload-conversion error with a human-readable message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn into_message(self) -> String {
        self.message
    }
}

impl Display for PayloadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PayloadError {}

/// Parses one owned token payload with its [`FromStr`] implementation.
#[doc(hidden)]
#[inline(always)]
pub fn parse_payload<T>(text: &str) -> Result<T, PayloadError>
where
    T: FromStr,
    T::Err: Display,
{
    text.parse()
        .map_err(|error: T::Err| PayloadError::new(error.to_string()))
}

/// Accepts the return type of a rule converter in generated code.
///
/// A converter names a function with `#[token("pattern", with = path)]`. The
/// function returns the payload when a conversion cannot fail. It returns a
/// `Result` of the payload when a conversion can fail. An `Err` becomes a
/// [`ScanError::InvalidPayload`](crate::ScanError::InvalidPayload), and the
/// `Display` of the `Err` value becomes the message of that error.
///
/// Only a variant with a payload accepts a converter. A unit variant has no
/// payload, and a `skip` rule emits no token.
///
#[doc(hidden)]
pub trait PayloadResult<P> {
    /// Converts this value into a payload, or into a conversion failure.
    ///
    /// # Errors
    ///
    /// Returns an error for an `Err`. The message of the error comes from the
    /// `Display` of the `Err` value.
    ///
    fn into_payload(self) -> Result<P, PayloadError>;
}

impl<P> PayloadResult<P> for P {
    #[inline(always)]
    fn into_payload(self) -> Result<P, PayloadError> {
        Ok(self)
    }
}

impl<P, E: Display> PayloadResult<P> for Result<P, E> {
    #[inline(always)]
    fn into_payload(self) -> Result<P, PayloadError> {
        self.map_err(|error| PayloadError::new(error.to_string()))
    }
}
