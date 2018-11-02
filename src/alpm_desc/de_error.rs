//! Errors for serializing the alpm db format
use std::fmt::{self, Display};
use std::io;
use std::result::Result as StdResult;

use failure::{Compat, Context, Fail};
use serde::{de, ser};

/// The error type for deserialization
#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

/// Errors that can occur during deserialization.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Fail)]
pub enum ErrorKind {
    /// This format does not support the given operation
    #[fail(display = "tried to deserialize an unsupported type/context: {}", _0)]
    Unsupported(&'static str),
    /// The deserializer expected a bool
    #[fail(display = "expected a bool")]
    ExpectedBool,
    /// The deserializer expected a hex-encoded byte
    #[fail(display = "expected a hex-encoded byte")]
    ExpectedByte,
    /// The deserializer expected an unsigned integer
    #[fail(display = "expected an unsigned integer")]
    ExpectedUnsigned,
    /// The deserializer expected a signed integer
    #[fail(display = "expected a signed integer")]
    ExpectedSigned,
    /// The deserializer expected a float
    #[fail(display = "expected a float")]
    ExpectedFloat,
    /// The deserializer expected a char
    #[fail(display = "expected a char")]
    ExpectedChar,
    /// The deserializer expected a key (`%NAME%\n`)
    #[fail(display = "expected a key (`%NAME%\n`)")]
    ExpectedKey,
    /// The deserializer expected an empty string
    #[fail(display = "expected an empty string")]
    ExpectedEmpty,
    /// A Serialize method returned a custom error.
    #[fail(display = "the type being deserialized reported an error: {}", _0)]
    Custom(String),
}

impl Error {
    /// Get the kind of this error
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }

    /// Get a version of this error that implements `Fail`.
    ///
    /// Unfortunately we cannot implement `Fail` for this type because it conflicts with
    /// `std::error::Error`, which we must implement for serde.
    pub fn into_fail(self) -> Context<ErrorKind> {
        self.inner
    }
}

impl ::std::ops::Deref for Error {
    type Target = Context<ErrorKind>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}

impl ::std::error::Error for Error {
    fn description(&self) -> &'static str {
        "unimplemented - use `Display` implementation"
    }

    fn cause(&self) -> Option<&::std::error::Error> {
        let cause = self.inner.cause()?;
        // we can't return this, so dump out some info
        eprintln!("  caused by: {}", cause);
        None
    }
}

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        ErrorKind::Custom(format!("{}", msg)).into()
    }
}

pub type Result<T> = StdResult<T, Error>;
