/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use nserror::{
    nsresult, NS_ERROR_FAILURE, NS_ERROR_NOT_IMPLEMENTED, NS_ERROR_NO_INTERFACE,
    NS_ERROR_NULL_POINTER, NS_ERROR_UNEXPECTED,
};
use rkv::StoreError;
use std::{
    string::FromUtf16Error,
    sync::{PoisonError, RwLockReadGuard, RwLockWriteGuard},
};

#[derive(Debug, Fail)]
pub enum KeyValueError {
    #[fail(display = "error converting string: {:?}", _0)]
    ConvertString(FromUtf16Error),

    #[fail(display = "no interface '{}'", _0)]
    NoInterface(&'static str),

    #[fail(display = "result '{}'", _0)]
    Nsresult(nsresult),

    #[fail(display = "arg is null")]
    NullPointer,

    #[fail(display = "error reading key/value pair")]
    Read,

    #[fail(display = "error getting read lock: {}", _0)]
    ReadLock(String),

    #[fail(display = "store error: {:?}", _0)]
    StoreError(StoreError),

    #[fail(display = "error getting write lock: {}", _0)]
    WriteLock(String),

    #[fail(display = "unsupported type")]
    UnsupportedType,
}

impl From<nsresult> for KeyValueError {
    fn from(result: nsresult) -> KeyValueError {
        KeyValueError::Nsresult(result)
    }
}

impl From<KeyValueError> for nsresult {
    fn from(err: KeyValueError) -> nsresult {
        match err {
            KeyValueError::ConvertString(_) => NS_ERROR_FAILURE,
            KeyValueError::NoInterface(_) => NS_ERROR_NO_INTERFACE,
            KeyValueError::Nsresult(result) => result,
            KeyValueError::NullPointer => NS_ERROR_NULL_POINTER,
            KeyValueError::Read => NS_ERROR_FAILURE,
            KeyValueError::ReadLock(_) => NS_ERROR_UNEXPECTED,
            KeyValueError::StoreError(_) => NS_ERROR_FAILURE,
            KeyValueError::WriteLock(_) => NS_ERROR_UNEXPECTED,
            KeyValueError::UnsupportedType => NS_ERROR_NOT_IMPLEMENTED,
        }
    }
}

impl From<StoreError> for KeyValueError {
    fn from(err: StoreError) -> KeyValueError {
        KeyValueError::StoreError(err)
    }
}

impl From<FromUtf16Error> for KeyValueError {
    fn from(err: FromUtf16Error) -> KeyValueError {
        KeyValueError::ConvertString(err)
    }
}

impl<'a, T> From<PoisonError<RwLockReadGuard<'a, T>>> for KeyValueError {
    fn from(err: PoisonError<RwLockReadGuard<'a, T>>) -> KeyValueError {
        KeyValueError::ReadLock(format!("{:?}", err))
    }
}

impl<'a, T> From<PoisonError<RwLockWriteGuard<'a, T>>> for KeyValueError {
    fn from(err: PoisonError<RwLockWriteGuard<'a, T>>) -> KeyValueError {
        KeyValueError::WriteLock(format!("{:?}", err))
    }
}
