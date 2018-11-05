// /* This Source Code Form is subject to the terms of the Mozilla Public
//  * License, v. 2.0. If a copy of the MPL was not distributed with this
//  * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use error::KeyValueError;
use libc::{int32_t, uint16_t};
use nserror::NsresultExt;
use nsstring::nsString;
use ordered_float::OrderedFloat;
use rkv::{StoreError, Value};
use storage_variant::{IntoVariant, Variant};
use xpcom::interfaces::nsIVariant;

// These are the relevant parts of the nsXPTTypeTag enum in xptinfo.h,
// which nsIVariant.idl reflects into the nsIDataType struct class and uses
// to constrain the values of nsIVariant::dataType.
#[allow(non_camel_case_types)]
enum DataType {
    INT32 = 2,
    DOUBLE = 9,
    BOOL = 10,
    VOID = 13,
    WSTRING = 21,
    EMPTY = 255,
}

// Per https://github.com/rust-lang/rust/issues/44266, casts aren't allowed
// in match arms, so it isn't possible to cast DataType variants to u16
// in order to match them against the value of nsIVariant::dataType.
// Instead we have to reflect each variant into a constant and then match
// against the values of the constants.
//
// (Alternatively, we could use the enum_primitive crate to convert primitive
// values of nsIVariant::dataType to their enum equivalents.  Or perhaps
// bindgen would convert the nsXPTTypeTag enum in xptinfo.h into something else
// we could use.  Since we currently only accept a small subset of values,
// and since that enum is unlikely to change frequently, this workaround
// seems sufficient.)
//
const DATA_TYPE_INT32: uint16_t = DataType::INT32 as u16;
const DATA_TYPE_DOUBLE: uint16_t = DataType::DOUBLE as u16;
const DATA_TYPE_BOOL: uint16_t = DataType::BOOL as u16;
const DATA_TYPE_VOID: uint16_t = DataType::VOID as u16;
const DATA_TYPE_WSTRING: uint16_t = DataType::WSTRING as u16;
const DATA_TYPE_EMPTY: uint16_t = DataType::EMPTY as u16;

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
    value: Option<Value<'a>>,
) -> Result<OwnedValue, KeyValueError> {
    match value {
        Some(Value::Bool(val)) => Ok(OwnedValue::Bool(val)),
        Some(Value::I64(val)) => Ok(OwnedValue::I64(val)),
        Some(Value::F64(val)) => Ok(OwnedValue::F64(val)),
        Some(Value::Str(val)) => Ok(OwnedValue::Str(val.to_owned())),
        Some(_value) => Err(KeyValueError::UnexpectedValue),
        None => Err(KeyValueError::UnexpectedValue),
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

// pub fn owned_to_value<'a>(
//     owned_value: OwnedValue,
// ) -> Result<Value<'a>, KeyValueError> {
//     match owned_value {
//         OwnedValue::Bool(val) => Ok(Value::Bool(val)),
//         OwnedValue::I64(val) => Ok(Value::I64(val)),
//         OwnedValue::F64(val) => Ok(Value::F64(val)),
//         OwnedValue::Str(val) => Ok(Value::Str(&val)),
//         _value => Err(KeyValueError::UnexpectedValue),
//     }
// }

pub fn variant_to_owned(variant: &nsIVariant) -> Result<Option<OwnedValue>, KeyValueError> {
    let mut data_type: uint16_t = 0;
    unsafe { variant.GetDataType(&mut data_type) }.to_result()?;

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
        DATA_TYPE_EMPTY | DATA_TYPE_VOID => {
            Ok(None)
        }
        _unsupported_type => {
            return Err(KeyValueError::UnsupportedType(data_type));
        }
    }
}
