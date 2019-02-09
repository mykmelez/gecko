/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use error::KeyValueError;
use libc::int32_t;
use nserror::NsresultExt;
use nsstring::nsString;
use ordered_float::OrderedFloat;
use rkv::{OwnedValue, Value};
use storage_variant::{
    GetDataType, VariantType, DATA_TYPE_BOOL, DATA_TYPE_DOUBLE, DATA_TYPE_EMPTY, DATA_TYPE_INT32,
    DATA_TYPE_VOID, DATA_TYPE_WSTRING,
};
use xpcom::{interfaces::nsIVariant, RefPtr};

pub fn value_to_owned(value: Option<Value>) -> Result<OwnedValue, KeyValueError> {
    match value {
        Some(Value::Bool(val)) => Ok(OwnedValue::Bool(val)),
        Some(Value::I64(val)) => Ok(OwnedValue::I64(val)),
        Some(Value::F64(val)) => Ok(OwnedValue::F64(val)),
        Some(Value::Str(val)) => Ok(OwnedValue::Str(val.to_owned())),
        Some(_value) => Err(KeyValueError::UnexpectedValue),
        None => Err(KeyValueError::UnexpectedValue),
    }
}

pub fn owned_to_variant(owned: OwnedValue) -> RefPtr<nsIVariant> {
    match owned {
        OwnedValue::Bool(val) => val.into_variant(),
        OwnedValue::I64(val) => val.into_variant(),
        OwnedValue::F64(OrderedFloat(val)) => val.into_variant(),
        OwnedValue::Str(ref val) => nsString::from(val).into_variant(),

        // NB: kvstore doesn't support these types of OwnedValue, but we still
        // have to match them in order to be an exhaustive pattern.
        OwnedValue::Instant(val) => val.into_variant(),
        OwnedValue::Json(ref val) => nsString::from(val).into_variant(),
        OwnedValue::U64(_) => panic!("not supported; shouldn't happen"),
        OwnedValue::Uuid(_) => panic!("not supported; shouldn't happen"),
        OwnedValue::Blob(_) => panic!("not supported; shouldn't happen"),
    }
}

pub fn variant_to_owned(variant: &nsIVariant) -> Result<Option<OwnedValue>, KeyValueError> {
    let data_type = variant.get_data_type();

    match data_type {
        DATA_TYPE_INT32 => {
            let mut val: int32_t = 0;
            unsafe { variant.GetAsInt32(&mut val) }.to_result()?;
            Ok(Some(OwnedValue::I64(val.into())))
        }
        DATA_TYPE_DOUBLE => {
            let mut val: f64 = 0.0;
            unsafe { variant.GetAsDouble(&mut val) }.to_result()?;
            Ok(Some(OwnedValue::F64(val.into())))
        }
        DATA_TYPE_WSTRING => {
            let mut val: nsString = nsString::new();
            unsafe { variant.GetAsAString(&mut *val) }.to_result()?;
            let str = String::from_utf16(&val)?;
            Ok(Some(OwnedValue::Str(str)))
        }
        DATA_TYPE_BOOL => {
            let mut val: bool = false;
            unsafe { variant.GetAsBool(&mut val) }.to_result()?;
            Ok(Some(OwnedValue::Bool(val)))
        }
        DATA_TYPE_EMPTY | DATA_TYPE_VOID => Ok(None),
        unsupported_type => Err(KeyValueError::UnsupportedType(unsupported_type)),
    }
}
