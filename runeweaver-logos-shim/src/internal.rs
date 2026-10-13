//! Interfaces used by generated compatibility code.

use crate::{Filter, FilterResult, Logos, Skip};

/// Calls a generated rule callback with its lexer type fixed.
#[doc(hidden)]
pub fn call_callback<'source, Token, Output>(
    callback: impl FnOnce(&mut crate::Lexer<'source, Token>) -> Output,
    lexer: &mut crate::Lexer<'source, Token>,
) -> Output
where
    Token: Logos<'source>,
{
    callback(lexer)
}

/// A normalized callback result.
#[doc(hidden)]
pub enum CallbackResult<'source, Token: Logos<'source>> {
    /// Emits a token.
    Emit(Token),
    /// Emits an explicit error.
    Error(Token::Error),
    /// Emits the default error.
    DefaultError,
    /// Omits the current match.
    Skip,
}

/// Converts a callback value into one lexer action.
#[doc(hidden)]
pub trait CallbackRetVal<'source, Payload, Token: Logos<'source>> {
    /// Constructs the lexer action.
    fn construct<C>(self, constructor: C) -> CallbackResult<'source, Token>
    where
        C: Fn(Payload) -> Token;
}

impl<'source, Token: Logos<'source>, Payload> CallbackRetVal<'source, Payload, Token> for Payload {
    fn construct<C>(self, constructor: C) -> CallbackResult<'source, Token>
    where
        C: Fn(Payload) -> Token,
    {
        CallbackResult::Emit(constructor(self))
    }
}

impl<'source, Token, Payload, Error> CallbackRetVal<'source, Payload, Token>
    for Result<Payload, Error>
where
    Token: Logos<'source>,
    Error: Into<Token::Error>,
{
    fn construct<C>(self, constructor: C) -> CallbackResult<'source, Token>
    where
        C: Fn(Payload) -> Token,
    {
        match self {
            Ok(payload) => CallbackResult::Emit(constructor(payload)),
            Err(error) => CallbackResult::Error(error.into()),
        }
    }
}

impl<'source, Token: Logos<'source>, Payload> CallbackRetVal<'source, Payload, Token>
    for Option<Payload>
{
    fn construct<C>(self, constructor: C) -> CallbackResult<'source, Token>
    where
        C: Fn(Payload) -> Token,
    {
        self.map_or(CallbackResult::DefaultError, |payload| {
            CallbackResult::Emit(constructor(payload))
        })
    }
}

impl<'source, Token: Logos<'source>, Payload> CallbackRetVal<'source, Payload, Token>
    for Filter<Payload>
{
    fn construct<C>(self, constructor: C) -> CallbackResult<'source, Token>
    where
        C: Fn(Payload) -> Token,
    {
        match self {
            Filter::Emit(payload) => CallbackResult::Emit(constructor(payload)),
            Filter::Skip => CallbackResult::Skip,
        }
    }
}

impl<'source, Token, Payload, Error> CallbackRetVal<'source, Payload, Token>
    for FilterResult<Payload, Error>
where
    Token: Logos<'source>,
    Error: Into<Token::Error>,
{
    fn construct<C>(self, constructor: C) -> CallbackResult<'source, Token>
    where
        C: Fn(Payload) -> Token,
    {
        match self {
            FilterResult::Emit(payload) => CallbackResult::Emit(constructor(payload)),
            FilterResult::Skip => CallbackResult::Skip,
            FilterResult::Error(error) => CallbackResult::Error(error.into()),
        }
    }
}

impl<'source, Token: Logos<'source>> CallbackRetVal<'source, (), Token> for bool {
    fn construct<C>(self, constructor: C) -> CallbackResult<'source, Token>
    where
        C: Fn(()) -> Token,
    {
        if self {
            CallbackResult::Emit(constructor(()))
        } else {
            CallbackResult::DefaultError
        }
    }
}

impl<'source, Token: Logos<'source>> CallbackRetVal<'source, (), Token> for Skip {
    fn construct<C>(self, _: C) -> CallbackResult<'source, Token>
    where
        C: Fn(()) -> Token,
    {
        CallbackResult::Skip
    }
}

impl<'source, Token, Error> CallbackRetVal<'source, (), Token> for Result<Skip, Error>
where
    Token: Logos<'source>,
    Error: Into<Token::Error>,
{
    fn construct<C>(self, _: C) -> CallbackResult<'source, Token>
    where
        C: Fn(()) -> Token,
    {
        match self {
            Ok(Skip) => CallbackResult::Skip,
            Err(error) => CallbackResult::Error(error.into()),
        }
    }
}

impl<'source, Token: Logos<'source>> CallbackRetVal<'source, (), Token> for Token {
    fn construct<C>(self, _: C) -> CallbackResult<'source, Token>
    where
        C: Fn(()) -> Token,
    {
        CallbackResult::Emit(self)
    }
}

impl<'source, Token, Error> CallbackRetVal<'source, (), Token> for Result<Token, Error>
where
    Token: Logos<'source>,
    Error: Into<Token::Error>,
{
    fn construct<C>(self, _: C) -> CallbackResult<'source, Token>
    where
        C: Fn(()) -> Token,
    {
        match self {
            Ok(token) => CallbackResult::Emit(token),
            Err(error) => CallbackResult::Error(error.into()),
        }
    }
}

/// A normalized skip callback result.
#[doc(hidden)]
pub enum SkipResult<'source, Token: Logos<'source>> {
    /// Omits the current match.
    Skip,
    /// Emits an error.
    Error(Token::Error),
}

/// Converts a skip callback value into one lexer action.
#[doc(hidden)]
pub trait SkipRetVal<'source, Token: Logos<'source>> {
    /// Constructs the skip action.
    fn construct(self) -> SkipResult<'source, Token>;
}

/// Converts a skip callback with its token type fixed.
#[doc(hidden)]
pub fn construct_skip<'source, Token, Value>(
    value: Value,
    _: &crate::Lexer<'source, Token>,
) -> SkipResult<'source, Token>
where
    Token: Logos<'source>,
    Value: SkipRetVal<'source, Token>,
{
    value.construct()
}

impl<'source, Token: Logos<'source>> SkipRetVal<'source, Token> for () {
    fn construct(self) -> SkipResult<'source, Token> {
        SkipResult::Skip
    }
}

impl<'source, Token: Logos<'source>> SkipRetVal<'source, Token> for Skip {
    fn construct(self) -> SkipResult<'source, Token> {
        SkipResult::Skip
    }
}

impl<'source, Token, Error> SkipRetVal<'source, Token> for Result<(), Error>
where
    Token: Logos<'source>,
    Error: Into<Token::Error>,
{
    fn construct(self) -> SkipResult<'source, Token> {
        match self {
            Ok(()) => SkipResult::Skip,
            Err(error) => SkipResult::Error(error.into()),
        }
    }
}

impl<'source, Token, Error> SkipRetVal<'source, Token> for Result<Skip, Error>
where
    Token: Logos<'source>,
    Error: Into<Token::Error>,
{
    fn construct(self) -> SkipResult<'source, Token> {
        match self {
            Ok(Skip) => SkipResult::Skip,
            Err(error) => SkipResult::Error(error.into()),
        }
    }
}
