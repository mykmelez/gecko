/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use libc::uint16_t;
use nserror::{
    nsresult, NS_ERROR_FAILURE, NS_ERROR_NOT_IMPLEMENTED, NS_ERROR_NO_INTERFACE,
    NS_ERROR_NULL_POINTER, NS_ERROR_UNEXPECTED,
};
use rkv::StoreError;
use std::{
    str::Utf8Error,
    string::FromUtf16Error,
    sync::PoisonError,
};
use OwnedValue;

#[derive(Debug, Fail)]
pub enum KeyValueError {
    #[fail(display = "error converting string: {:?}", _0)]
    ConvertBytes(Utf8Error),

    #[fail(display = "error converting string: {:?}", _0)]
    ConvertString(FromUtf16Error),

    #[fail(display = "no interface '{}'", _0)]
    NoInterface(&'static str),

    // TODO: use nsresult.error_name() to convert the number to its name.
    #[fail(display = "error result '{}'", _0)]
    Nsresult(nsresult),

    #[fail(display = "arg is null")]
    NullPointer,

    #[fail(display = "poison error getting read/write lock")]
    PoisonError,

    #[fail(display = "error reading key/value pair")]
    Read,

    #[fail(display = "store error: {:?}", _0)]
    StoreError(StoreError),

    // TODO: convert the number to its name.
    #[fail(display = "unsupported type: {}", _0)]
    UnsupportedType(uint16_t),

    #[fail(display = "unsupported value: {:?}", _0)]
    UnsupportedValue(OwnedValue),
}

impl From<nsresult> for KeyValueError {
    fn from(result: nsresult) -> KeyValueError {
        KeyValueError::Nsresult(result)
    }
}

impl From<KeyValueError> for nsresult {
    fn from(err: KeyValueError) -> nsresult {
        match err {
            KeyValueError::ConvertBytes(_) => NS_ERROR_FAILURE,
            KeyValueError::ConvertString(_) => NS_ERROR_FAILURE,
            KeyValueError::NoInterface(_) => NS_ERROR_NO_INTERFACE,
            KeyValueError::Nsresult(result) => result,
            KeyValueError::NullPointer => NS_ERROR_NULL_POINTER,
            KeyValueError::PoisonError => NS_ERROR_UNEXPECTED,
            KeyValueError::Read => NS_ERROR_FAILURE,
            KeyValueError::StoreError(_) => NS_ERROR_FAILURE,
            KeyValueError::UnsupportedType(_) => NS_ERROR_NOT_IMPLEMENTED,
            KeyValueError::UnsupportedValue(_) => NS_ERROR_NOT_IMPLEMENTED,
        }
    }
}

impl From<StoreError> for KeyValueError {
    fn from(err: StoreError) -> KeyValueError {
        KeyValueError::StoreError(err)
    }
}

impl From<Utf8Error> for KeyValueError {
    fn from(err: Utf8Error) -> KeyValueError {
        KeyValueError::ConvertBytes(err)
    }
}

impl From<FromUtf16Error> for KeyValueError {
    fn from(err: FromUtf16Error) -> KeyValueError {
        KeyValueError::ConvertString(err)
    }
}

impl<T> From<PoisonError<T>> for KeyValueError {
    fn from(err: PoisonError<T>) -> KeyValueError {
        KeyValueError::PoisonError
    }
}
