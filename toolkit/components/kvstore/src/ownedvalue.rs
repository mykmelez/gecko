// /* This Source Code Form is subject to the terms of the Mozilla Public
//  * License, v. 2.0. If a copy of the MPL was not distributed with this
//  * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use error::KeyValueError;
use nsstring::nsString;
use ordered_float::OrderedFloat;
use rkv::{StoreError, Value};
use storage_variant::{IntoVariant, Variant};

// This is implemented in rkv but is incomplete there.  We implement a subset
// to give KeyValuePair ownership over its value, so it can #[derive(xpcom)].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedValue {
    Bool(bool),
    I64(i64),
    F64(OrderedFloat<f64>),
    Str(String),
}

pub fn value_to_owned<'a>(
    value: Result<Option<Value<'a>>, StoreError>,
) -> Result<OwnedValue, KeyValueError> {
    match value {
        Ok(Some(Value::Bool(val))) => Ok(OwnedValue::Bool(val)),
        Ok(Some(Value::I64(val))) => Ok(OwnedValue::I64(val)),
        Ok(Some(Value::F64(val))) => Ok(OwnedValue::F64(val)),
        Ok(Some(Value::Str(val))) => Ok(OwnedValue::Str(val.to_owned())),
        Ok(Some(_value)) => Err(KeyValueError::UnexpectedValue),
        Ok(None) => Err(KeyValueError::UnexpectedValue),
        Err(err) => Err(KeyValueError::StoreError(err)),
    }
}

impl<'a> IntoVariant for OwnedValue {
    fn into_variant(self) -> Option<Variant> {
        match self {
            OwnedValue::Bool(val) => val.into_variant(),
            OwnedValue::I64(val) => val.into_variant(),
            OwnedValue::F64(OrderedFloat(val)) => val.into_variant(),
            OwnedValue::Str(val) => nsString::from(&val).into_variant(),
        }
    }
}
