//! Converts what a rule converter returns into a token payload.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// A failure produced while converting an accepted lexeme into a payload.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadError<E = String> {
    message: E,
}

impl PayloadError {
    /// Creates a payload-conversion error with a human-readable message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl<E> PayloadError<E> {
    /// Retains a typed payload conversion error.
    pub fn from_error(error: E) -> Self {
        Self { message: error }
    }

    pub(crate) fn into_message(self) -> E {
        self.message
    }
}

impl<E: Display> Display for PayloadError<E> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.message, formatter)
    }
}

impl<E: std::fmt::Debug + Display> std::error::Error for PayloadError<E> {}

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
/// A text converter requires a variant with a payload. Scanner callbacks
/// also support unit variants and skip rules. Those callbacks return `()`
/// or `Result<(), E>`.
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

/// Retains converter errors in the declared error type.
#[doc(hidden)]
pub trait TypedPayloadResult<P, E> {
    /// Converts a payload or result without formatting its error.
    fn into_typed_payload(self) -> Result<P, PayloadError<E>>;
}

impl<P, E> TypedPayloadResult<P, E> for P {
    fn into_typed_payload(self) -> Result<P, PayloadError<E>> {
        Ok(self)
    }
}

impl<P, E, F: Into<E>> TypedPayloadResult<P, E> for Result<P, F> {
    fn into_typed_payload(self) -> Result<P, PayloadError<E>> {
        self.map_err(|error| PayloadError::from_error(error.into()))
    }
}

/// Parses a payload while retaining its conversion error.
#[doc(hidden)]
pub fn parse_typed_payload<T: FromStr, E>(text: &str) -> Result<T, PayloadError<E>>
where
    T::Err: Into<E>,
{
    text.parse()
        .map_err(|error: T::Err| PayloadError::from_error(error.into()))
}
