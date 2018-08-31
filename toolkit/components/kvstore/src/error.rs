/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use failure::{Backtrace, Context, Fail};
use nserror::{
    nsresult, NS_ERROR_FAILURE, NS_ERROR_INVALID_ARG, NS_ERROR_NOT_IMPLEMENTED,
    NS_ERROR_NO_INTERFACE, NS_ERROR_NULL_POINTER, NS_ERROR_UNEXPECTED,
};
use std::{fmt, fmt::Display};

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum KeyValueErrorKind {
    #[fail(display = "error committing transaction")]
    Commit,

    #[fail(display = "error converting '{}' arg from UTF-16", _0)]
    Convert(&'static str),

    #[fail(display = "error deleting key/value pair")]
    Delete,

    #[fail(display = "error getting or creating environment")]
    GetCreateEnv,

    #[fail(display = "error obtaining lock")]
    Lock,

    #[fail(display = "no interface '{}'", _0)]
    NoInterface(&'static str),

    #[fail(display = "result '{}'", _0)]
    Nsresult(nsresult),

    #[fail(display = "arg is null")]
    NullPointer,

    #[fail(display = "error opening or creating database")]
    OpenCreateDB,

    #[fail(display = "error reading key/value pair")]
    Read,

    #[fail(display = "unexpected result")]
    Unexpected,

    #[fail(display = "unsupported type")]
    UnsupportedType,

    #[fail(display = "error writing key/value pair")]
    Write,
}

#[derive(Debug)]
pub struct KeyValueError {
    inner: Context<KeyValueErrorKind>,
}

impl KeyValueError {
    pub fn kind(&self) -> KeyValueErrorKind {
        *self.inner.get_context()
    }
}

impl Fail for KeyValueError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for KeyValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<Context<KeyValueErrorKind>> for KeyValueError {
    fn from(inner: Context<KeyValueErrorKind>) -> KeyValueError {
        KeyValueError { inner }
    }
}

impl From<KeyValueErrorKind> for KeyValueError {
    fn from(kind: KeyValueErrorKind) -> KeyValueError {
        KeyValueError {
            inner: Context::new(kind),
        }
    }
}

impl From<nsresult> for KeyValueError {
    fn from(result: nsresult) -> KeyValueError {
        KeyValueErrorKind::Nsresult(result).into()
    }
}

impl From<KeyValueErrorKind> for nsresult {
    fn from(kind: KeyValueErrorKind) -> nsresult {
        KeyValueError::from(kind).into()
    }
}

impl From<KeyValueError> for nsresult {
    fn from(err: KeyValueError) -> nsresult {
        match err.kind() {
            KeyValueErrorKind::Commit => NS_ERROR_FAILURE,
            KeyValueErrorKind::Convert(_) => NS_ERROR_INVALID_ARG,
            KeyValueErrorKind::Delete => NS_ERROR_FAILURE,

            // Perhaps we should return NS_ERROR_FILE_NOT_FOUND, although that
            // isn't the only possible reason for failure to get/create an env.
            KeyValueErrorKind::GetCreateEnv => NS_ERROR_FAILURE,

            KeyValueErrorKind::OpenCreateDB => NS_ERROR_FAILURE,
            KeyValueErrorKind::NoInterface(_) => NS_ERROR_NO_INTERFACE,
            KeyValueErrorKind::Lock => NS_ERROR_UNEXPECTED,
            KeyValueErrorKind::NullPointer => NS_ERROR_NULL_POINTER,
            KeyValueErrorKind::Nsresult(result) => result,
            KeyValueErrorKind::Read => NS_ERROR_FAILURE,
            KeyValueErrorKind::Unexpected => NS_ERROR_UNEXPECTED,
            KeyValueErrorKind::UnsupportedType => NS_ERROR_NOT_IMPLEMENTED,
            KeyValueErrorKind::Write => NS_ERROR_FAILURE,
        }
    }
}
