/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#![allow(non_snake_case)]

#[macro_use]
extern crate failure;
extern crate libc;
#[macro_use]
extern crate log;
extern crate nserror;
extern crate nsstring;
extern crate rkv;
#[macro_use]
extern crate xpcom;

mod error;
mod variant;

use error::{KeyValueError};
use libc::{int32_t, int64_t, uint16_t};
use nserror::{
    nsresult, NsresultExt, NS_ERROR_FAILURE, NS_ERROR_NOT_IMPLEMENTED, NS_ERROR_NO_INTERFACE,
    NS_ERROR_UNEXPECTED, NS_OK,
};
use nsstring::{nsAString, nsString};
use rkv::{Manager, Rkv, Store, StoreError, Value};
use std::{
    cell::RefCell,
    collections::VecDeque,
    path::Path,
    str,
    sync::{Arc, RwLock},
};
use variant::{IntoVariant, Variant};
use xpcom::{
    interfaces::{
        nsIJSEnumerator, nsIKeyValueDatabase, nsIKeyValueService, nsISimpleEnumerator, nsISupports,
        nsIVariant,
    },
    nsIID, RefPtr,
};

fn ensure_ref<'a, T>(ptr: *const T) -> Result<&'a T, KeyValueError> {
    if ptr.is_null() {
        Err(KeyValueError::NullPointer)
    } else {
        Ok(unsafe { &*ptr })
    }
}

// These are the relevant parts of the nsXPTTypeTag enum in xptinfo.h,
// which nsIVariant.idl reflects into the nsIDataType struct class and uses
// to constrain the values of nsIVariant::dataType.
#[allow(non_camel_case_types)]
enum DataType {
    INT32 = 2,
    BOOL = 10,
    WSTRING = 21,
    EMPTY = 255,
}

// Per https://github.com/rust-lang/rust/issues/44266, casts aren't allowed
// in match arms, so it isn't possible to cast nsXPTTypeTag variants to u16
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
const DATA_TYPE_BOOL: uint16_t = DataType::BOOL as u16;
const DATA_TYPE_WSTRING: uint16_t = DataType::WSTRING as u16;
const DATA_TYPE_EMPTY: uint16_t = DataType::EMPTY as u16;

/// Construct an nsIKeyValueService.  This should be called only once
/// from `KeyValueServiceConstructor` in C++, after which the instance
/// is memoized and reused (hence the "service" in its name).  It is
/// the XPCOM equivalent of rkv's Manager singleton.
#[no_mangle]
pub extern "C" fn NewKeyValueService(result: *mut *const nsIKeyValueService) -> nsresult {
    let service = KeyValueService::new();
    match service.query_interface::<nsIKeyValueService>() {
        Some(p) => {
            unsafe { p.forget(&mut *result) }
            NS_OK
        }
        None => NS_ERROR_NO_INTERFACE,
    }
}

// For each public XPCOM method in the nsIKeyValue* interfaces, we implement
// a pair of Rust methods:
//
//   1. a method named after the XPCOM (as modified by the XPIDL parser, i.e.
//      by capitalization of its initial letter) that returns an nsresult;
//
//   2. a method with a Rust-y name that returns a Result<(), KeyValueError>.
//
// XPCOM calls the first method, which is only responsible for calling
// the second one and converting its Result to an nsresult (logging errors
// in the process).  The second method is responsible for doing the work.
//
// For example, given an XPCOM method FooBar, we implement a method FooBar
// that calls a method foo_bar.  foo_bar returns a Result<(), KeyValueError>,
// and FooBar converts that to an nsresult.
//
// This design allows us to use Rust idioms like the question mark operator
// to simplify the implementation in the second method while returning XPCOM-
// compatible nsresult values to XPCOM callers.
//
// I wonder if it'd be possible/useful to make Rust implementations of XPCOM
// methods return Result<nsresult, nsresult> rather than nsresult.  Then we
// might be able to merge these pairs of methods into a single method that can
// use Rust idioms while returning the type of value that XPCOM expects.

#[derive(xpcom)]
#[xpimplements(nsIKeyValueService)]
#[refcnt = "nonatomic"]
pub struct InitKeyValueService {}

impl KeyValueService {
    fn GetOrCreateDefault(
        &self,
        path: *const nsAString,
        retval: *mut *const nsIKeyValueDatabase,
    ) -> nsresult {
        match self.get_or_create_default(path, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn GetOrCreate(
        &self,
        path: *const nsAString,
        name: *const nsAString,
        retval: *mut *const nsIKeyValueDatabase,
    ) -> nsresult {
        match self.get_or_create(path, name, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }
}

impl KeyValueService {
    fn new() -> RefPtr<KeyValueService> {
        KeyValueService::allocate(InitKeyValueService {})
    }

    fn get_or_create_default(
        &self,
        path: *const nsAString,
        retval: *mut *const nsIKeyValueDatabase,
    ) -> Result<(), KeyValueError> {
        let path =
            String::from_utf16(ensure_ref(path)?)?;

        let mut writer = Manager::singleton().write()?;

        let rkv = writer.get_or_create(Path::new(&path), Rkv::new)?;
        let store = rkv
            .write()?
            .open_or_create_default()?;
        let key_value_db = KeyValueDatabase::new(rkv, store);

        match key_value_db.query_interface::<nsIKeyValueDatabase>() {
            Some(db) => unsafe { db.forget(&mut *retval) },
            None => return Err(KeyValueError::NoInterface("nsIKeyValueDatabase").into()),
        };

        Ok(())
    }

    fn get_or_create(
        &self,
        path: *const nsAString,
        name: *const nsAString,
        retval: *mut *const nsIKeyValueDatabase,
    ) -> Result<(), KeyValueError> {
        let path = String::from_utf16(ensure_ref(path)?)?;
        let name = String::from_utf16(ensure_ref(name)?)?;
        let mut writer = Manager::singleton().write()?;
        let rkv = writer.get_or_create(Path::new(&path), Rkv::new)?;
        let store = rkv.write()?.open_or_create(Some(name.as_str()))?;
        let key_value_db = KeyValueDatabase::new(rkv, store);

        match key_value_db.query_interface::<nsIKeyValueDatabase>() {
            Some(db) => unsafe { db.forget(&mut *retval) },
            None => return Err(KeyValueError::NoInterface("nsIKeyValueDatabase")),
        };

        Ok(())
    }
}

#[derive(xpcom)]
#[xpimplements(nsIKeyValueDatabase)]
#[refcnt = "nonatomic"]
pub struct InitKeyValueDatabase {
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
}

impl KeyValueDatabase {
    fn Put(&self, key: *const nsAString, value: *const nsIVariant) -> nsresult {
        match self.put(key, value) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn Get(
        &self,
        key: *const nsAString,
        default_value: *const nsIVariant,
        retval: *mut *const nsIVariant,
    ) -> nsresult {
        match self.get(key, default_value, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn GetInt(
        &self,
        key: *const nsAString,
        default_value: int64_t,
        retval: *mut int64_t,
    ) -> nsresult {
        match self.get_int(key, default_value, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn GetString(
        &self,
        key: *const nsAString,
        default_value: *const nsAString,
        retval: *mut nsAString,
    ) -> nsresult {
        match self.get_string(key, default_value, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn GetBool(&self, key: *const nsAString, default_value: bool, retval: *mut bool) -> nsresult {
        match self.get_bool(key, default_value, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn Has(&self, key: *const nsAString, retval: *mut bool) -> nsresult {
        match self.has(key, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn Delete(&self, key: *const nsAString) -> nsresult {
        match self.delete(key) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn Enumerate(
        &self,
        from_key: *const nsAString,
        retval: *mut *const nsISimpleEnumerator,
    ) -> nsresult {
        match self.enumerate(from_key, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }
}

impl KeyValueDatabase {
    fn new(rkv: Arc<RwLock<Rkv>>, store: Store) -> RefPtr<KeyValueDatabase> {
        KeyValueDatabase::allocate(InitKeyValueDatabase { rkv, store })
    }

    fn put(&self, key: *const nsAString, value: *const nsIVariant) -> Result<(), KeyValueError> {
        let key = String::from_utf16(ensure_ref(key)?)?;
        let value = ensure_ref(value)?;

        let mut dataType: uint16_t = 0;
        unsafe { value.GetDataType(&mut dataType) }.to_result()?;
        info!("nsIVariant type is {}", dataType);

        let env = self.rkv.read()?;
        let mut writer = env.write()?;

        match dataType {
            DATA_TYPE_INT32 => {
                info!("nsIVariant type is int32");
                let mut value_as_int32: int32_t = 0;
                unsafe { value.GetAsInt32(&mut value_as_int32) }.to_result()?;
                writer.put(&self.store, &key, &Value::I64(value_as_int32.into()))?;
                writer.commit()?;
            }
            DATA_TYPE_WSTRING => {
                info!("nsIVariant type is string");
                let mut value_as_astring: nsString = nsString::new();
                unsafe { value.GetAsAString(&mut *value_as_astring) }.to_result()?;
                let value = String::from_utf16(&value_as_astring)?;
                writer.put(&self.store, &key, &Value::Str(&value))?;
                writer.commit()?;
            }
            DATA_TYPE_BOOL => {
                info!("nsIVariant type is bool");
                let mut value_as_bool: bool = false;
                unsafe { value.GetAsBool(&mut value_as_bool) }.to_result()?;
                writer.put(&self.store, &key, &Value::Bool(value_as_bool.into()))?;
                writer.commit()?;
            }
            _unsupported_type => {
                return Err(KeyValueError::Nsresult(NS_ERROR_NOT_IMPLEMENTED));
            }
        };

        Ok(())
    }

    fn has(&self, key: *const nsAString, retval: *mut bool) -> Result<(), KeyValueError> {
        let key = String::from_utf16(ensure_ref(key)?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(_) => unsafe { *retval = true },
            None => unsafe { *retval = false },
        };

        Ok(())
    }

    fn get(
        &self,
        key: *const nsAString,
        default_value: *const nsIVariant,
        retval: *mut *const nsIVariant,
    ) -> Result<(), KeyValueError> {
        let key = String::from_utf16(ensure_ref(key)?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(Value::I64(value)) => {
                let variant = value.into_variant().ok_or(KeyValueError::Read)?;
                unsafe { variant.take().forget(&mut *retval) };
            }
            Some(Value::Str(value)) => {
                let variant = nsString::from(value)
                    .into_variant()
                    .ok_or(KeyValueError::Read)?;
                unsafe { variant.take().forget(&mut *retval) };
            }
            Some(Value::Bool(value)) => {
                let variant = value.into_variant().ok_or(KeyValueError::Read)?;
                unsafe { variant.take().forget(&mut *retval) };
            }
            None => {
                let default_value = ensure_ref(default_value)?;

                let mut dataType: uint16_t = 0;
                unsafe { default_value.GetDataType(&mut dataType) }.to_result()?;
                info!("get: default value nsIVariant type is {}", dataType);

                match dataType {
                    DATA_TYPE_INT32 => {
                        let mut val: int32_t = 0;
                        unsafe { default_value.GetAsInt32(&mut val) }.to_result()?;
                        let variant = val.into_variant().ok_or(KeyValueError::Read)?;
                        unsafe { variant.take().forget(&mut *retval) };
                    }
                    DATA_TYPE_WSTRING => {
                        let mut val: nsString = nsString::new();
                        unsafe { default_value.GetAsAString(&mut *val) }.to_result()?;
                        let variant = val.into_variant().ok_or(KeyValueError::Read)?;
                        unsafe { variant.take().forget(&mut *retval) };
                    }
                    DATA_TYPE_BOOL => {
                        let mut val: bool = false;
                        unsafe { default_value.GetAsBool(&mut val) }.to_result()?;
                        println!("boolean val: {:?}", val);
                        let variant = (val as bool)
                            .into_variant()
                            .ok_or(KeyValueError::Read)?;
                        unsafe { variant.take().forget(&mut *retval) };
                    }
                    DATA_TYPE_EMPTY => {
                        let val = ();
                        let variant = val.into_variant().ok_or(KeyValueError::Read)?;
                        unsafe { variant.take().forget(&mut *retval) };
                    }
                    _unsupported_type => {
                        return Err(KeyValueError::UnsupportedType);
                    }
                };
            }
            _unsupported_type => return Err(KeyValueError::UnsupportedType),
        };

        Ok(())
    }

    fn delete(&self, key: *const nsAString) -> Result<(), KeyValueError> {
        let key = String::from_utf16(ensure_ref(key)?)?;
        let env = self.rkv.read()?;
        let mut writer = env.write()?;

        writer.delete(&self.store, &key)?;
        writer.commit()?;

        Ok(())
    }

    fn get_int(
        &self,
        key: *const nsAString,
        default_value: int64_t,
        retval: *mut int64_t,
    ) -> Result<(), KeyValueError> {
        let key = String::from_utf16(ensure_ref(key)?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(Value::I64(value)) => unsafe { *retval = value },
            None => unsafe { *retval = default_value },
            _unsupported_type => return Err(KeyValueError::UnsupportedType),
        };

        Ok(())
    }

    fn get_string(
        &self,
        key: *const nsAString,
        default_value: *const nsAString,
        retval: *mut nsAString,
    ) -> Result<(), KeyValueError> {
        let key = String::from_utf16(ensure_ref(key)?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(Value::Str(value)) => unsafe { (*retval).assign(&nsString::from(value)) },
            None => unsafe { (*retval).assign(&*default_value) },
            _unsupported_type => return Err(KeyValueError::UnsupportedType),
        };

        Ok(())
    }

    fn get_bool(
        &self,
        key: *const nsAString,
        default_value: bool,
        retval: *mut bool,
    ) -> Result<(), KeyValueError> {
        let key = String::from_utf16(ensure_ref(key)?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(Value::Bool(value)) => unsafe { *retval = value },
            None => unsafe { *retval = default_value },
            _unsupported_type => return Err(KeyValueError::UnsupportedType),
        };

        Ok(())
    }

    fn enumerate(
        &self,
        from_key: *const nsAString,
        retval: *mut *const nsISimpleEnumerator,
    ) -> Result<(), KeyValueError> {
        let env = self.rkv.read()?;
        let reader = env.read()?;

        // from_key is [optional], and XPConnect maps the absence of a value
        // to an empty string, so we know it isn't a null pointer.
        let from_key = String::from_utf16(unsafe { &*from_key })?;

        let iterator = if from_key == "" {
            reader.iter_start(&self.store)?
        } else {
            reader.iter_from(&self.store, &from_key)?
        };

        // Ideally, we'd iterate pairs lazily, as the consumer calls
        // nsISimpleEnumerator.getNext().  But SimpleEnumerator can't reference
        // the Iter because Rust "cannot #[derive(xpcom)] on a generic type,"
        // and the Iter requires a lifetime parameter, which would make
        // SimpleEnumerator generic.
        //
        // Our fallback approach is to collect the iterator into a collection
        // that SimpleEnumerator owns.
        //
        let pairs: VecDeque<(String, OwnedValue)> = iterator
            .map(|(key, val)| {
                (
                    unsafe { str::from_utf8_unchecked(&key) }.to_owned(),
                    val.into(),
                )
            }).collect();

        let enumerator = SimpleEnumerator::new(pairs);

        match enumerator.query_interface::<nsISimpleEnumerator>() {
            Some(interface) => unsafe { interface.forget(&mut *retval) },
            None => return Err(KeyValueError::NoInterface("nsISimpleEnumerator")),
        };

        Ok(())
    }
}

// This is implemented in rkv but is incomplete there.  We implement a subset
// to give KeyValuePair ownership over its value, so it can #[derive(xpcom)].
#[derive(Clone, Debug, Eq, PartialEq)]
enum OwnedValue {
    Bool(bool),
    I64(i64),
    Str(String),
    // Unexpected means either that the value's type isn't one of the ones
    // we expect or that we got a StoreError while retrieving the value.
    // We should consider differentiating between these types of results.
    Unexpected,
}

impl<'a> From<Result<Option<Value<'a>>, StoreError>> for OwnedValue {
    fn from(value: Result<Option<Value<'a>>, StoreError>) -> OwnedValue {
        match value {
            Ok(Some(Value::Bool(val))) => OwnedValue::Bool(val),
            Ok(Some(Value::I64(val))) => OwnedValue::I64(val),
            Ok(Some(Value::Str(val))) => OwnedValue::Str(val.to_owned()),
            _ => OwnedValue::Unexpected,
        }
    }
}

impl<'a> IntoVariant for OwnedValue {
    fn into_variant(self) -> Option<Variant> {
        match self {
            OwnedValue::I64(val) => val.into_variant(),
            OwnedValue::Bool(val) => val.into_variant(),
            OwnedValue::Str(val) => nsString::from(&val).into_variant(),
            _ => None,
        }
    }
}

#[derive(xpcom)]
#[xpimplements(nsISimpleEnumerator)]
#[refcnt = "nonatomic"]
pub struct InitSimpleEnumerator {
    pairs: RefCell<VecDeque<(String, OwnedValue)>>,
}

impl SimpleEnumerator {
    fn new(pairs: VecDeque<(String, OwnedValue)>) -> RefPtr<SimpleEnumerator> {
        SimpleEnumerator::allocate(InitSimpleEnumerator {
            pairs: RefCell::new(pairs),
        })
    }

    fn HasMoreElements(&self, retval: *mut bool) -> nsresult {
        unsafe { *retval = !self.pairs.borrow().is_empty() };
        NS_OK
    }

    fn GetNext(&self, retval: *mut *const nsISupports) -> nsresult {
        match self.get_next(retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    // The nsISimpleEnumeratorBase methods iterator() and entries() depend on
    // nsIJSEnumerator, which requires jscontext, which is unsupported for Rust.
    fn Iterator(&self, _retval: *mut *const nsIJSEnumerator) -> nsresult {
        NS_ERROR_NOT_IMPLEMENTED
    }
    fn Entries(&self, _aIface: *const nsIID, _retval: *mut *const nsIJSEnumerator) -> nsresult {
        NS_ERROR_NOT_IMPLEMENTED
    }
}

impl SimpleEnumerator {
    fn get_next(&self, retval: *mut *const nsISupports) -> Result<(), KeyValueError> {
        let mut pairs = self.pairs.borrow_mut();
        let (key, value) = pairs
            .pop_front()
            .ok_or(KeyValueError::Nsresult(NS_ERROR_FAILURE))?;

        // Perhaps we should never fail if the value was unexpected and instead
        // return a null or undefined variant.
        //
        // Alternately, we could fail eagerly—when instantiating the enumerator;
        // or even more lazily—on nsIKeyValuePair.getValue().  But eagerly seems
        // too soon, since it exposes the implementation detail that we eagerly
        // collect the results of the cursor iterator (which ideally we'll stop
        // doing in the future).  And lazily would hide errors when the consumer
        // enumerates pairs but doesn't access all values.
        //
        if value == OwnedValue::Unexpected {
            return Err(KeyValueError::Nsresult(NS_ERROR_UNEXPECTED));
        }

        let pair = KeyValuePair::new(key, value);

        match pair.query_interface::<nsISupports>() {
            Some(interface) => unsafe { interface.forget(&mut *retval) },
            None => return Err(KeyValueError::NoInterface("nsIKeyValuePair")),
        };

        Ok(())
    }
}

#[derive(xpcom)]
#[xpimplements(nsIKeyValuePair)]
#[refcnt = "nonatomic"]
pub struct InitKeyValuePair {
    key: String,
    value: OwnedValue,
}

impl KeyValuePair {
    fn new(key: String, value: OwnedValue) -> RefPtr<KeyValuePair> {
        KeyValuePair::allocate(InitKeyValuePair { key, value })
    }

    fn GetKey(&self, key: *mut nsAString) -> nsresult {
        unsafe { (*key).assign(&nsString::from(&self.key)) }
        NS_OK
    }

    fn GetValue(&self, value: *mut *const nsIVariant) -> nsresult {
        match self.get_value(value) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }
}

impl KeyValuePair {
    fn get_value(&self, value: *mut *const nsIVariant) -> Result<(), KeyValueError> {
        let variant = self
            .value
            .clone()
            .into_variant()
            .ok_or(KeyValueError::Nsresult(NS_ERROR_FAILURE))?;
        unsafe { variant.take().forget(&mut *value) };
        Ok(())
    }
}
