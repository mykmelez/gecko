// /* This Source Code Form is subject to the terms of the Mozilla Public
//  * License, v. 2.0. If a copy of the MPL was not distributed with this
//  * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use nsstring::nsString;
use rkv::{StoreError, Value};
use storage_variant::{IntoVariant, Variant};

// This is implemented in rkv but is incomplete there.  We implement a subset
// to give KeyValuePair ownership over its value, so it can #[derive(xpcom)].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedValue {
    Bool(bool),
    I64(i64),
    Str(String),

    // Unexpected means either that the value's type isn't one of the ones
    // we support or that we got a StoreError while retrieving the value.
    // TODO: differentiate between "unexpected type" and StoreError.
    Unexpected,
}

impl<'a> From<Result<Option<Value<'a>>, StoreError>> for OwnedValue {
    fn from(value: Result<Option<Value<'a>>, StoreError>) -> OwnedValue {
        match value {
            Ok(Some(value)) => value.into(),
            _ => OwnedValue::Unexpected,
        }
    }
}

impl<'a> From<Value<'a>> for OwnedValue {
    fn from(value: Value) -> OwnedValue {
        match value {
            Value::Bool(val) => OwnedValue::Bool(val),
            Value::I64(val) => OwnedValue::I64(val),
            Value::Str(val) => OwnedValue::Str(val.to_owned()),
            _ => OwnedValue::Unexpected,
        }
    }
}

impl<'a> IntoVariant for OwnedValue {
    fn into_variant(self) -> Option<Variant> {
        match self {
            OwnedValue::Bool(val) => val.into_variant(),
            OwnedValue::I64(val) => val.into_variant(),
            OwnedValue::Str(val) => nsString::from(&val).into_variant(),
            _ => None,
        }
    }
}
