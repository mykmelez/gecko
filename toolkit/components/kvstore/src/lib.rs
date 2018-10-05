/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#![allow(non_snake_case)]

#[macro_use]
extern crate failure;
extern crate libc;
extern crate lmdb;
#[macro_use]
extern crate log;
extern crate nserror;
extern crate nsstring;
extern crate rkv;
extern crate storage_variant;
#[macro_use]
extern crate xpcom;

mod error;
mod ownedvalue;

use error::KeyValueError;
use libc::{c_double, c_void, int32_t, int64_t, uint16_t};
use nserror::{
    nsresult, NsresultExt, NS_ERROR_FAILURE, NS_ERROR_NOT_IMPLEMENTED, NS_ERROR_NO_AGGREGATION,
    NS_ERROR_UNEXPECTED, NS_OK,
};
use nsstring::{nsACString, nsAString, nsCString, nsString};
use ownedvalue::OwnedValue;
use rkv::{Manager, Rkv, Store, StoreError, Value};
use std::{
    cell::RefCell,
    collections::VecDeque,
    path::Path,
    ptr, str,
    sync::{Arc, RwLock},
};
use storage_variant::IntoVariant;
use xpcom::{
    interfaces::{
        nsIJSEnumerator, nsIKeyValueDatabase, nsISimpleEnumerator, nsISupports, nsIVariant,
    },
    nsIID, RefPtr,
};

unsafe fn ensure_ref<'a, T>(ptr: *const T) -> Result<&'a T, KeyValueError> {
    if ptr.is_null() {
        Err(KeyValueError::NullPointer)
    } else {
        Ok(&*ptr)
    }
}

// These are the relevant parts of the nsXPTTypeTag enum in xptinfo.h,
// which nsIVariant.idl reflects into the nsIDataType struct class and uses
// to constrain the values of nsIVariant::dataType.
#[allow(non_camel_case_types)]
enum DataType {
    INT32 = 2,
    DOUBLE = 9,
    BOOL = 10,
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
const DATA_TYPE_WSTRING: uint16_t = DataType::WSTRING as u16;
const DATA_TYPE_EMPTY: uint16_t = DataType::EMPTY as u16;

#[no_mangle]
pub extern "C" fn KeyValueServiceConstructor(
    outer: *const nsISupports,
    iid: &nsIID,
    result: *mut *mut c_void,
) -> nsresult {
    unsafe { *result = ptr::null_mut() };

    if !outer.is_null() {
        return NS_ERROR_NO_AGGREGATION;
    }

    let service: RefPtr<KeyValueService> = KeyValueService::new();
    unsafe { service.QueryInterface(iid, result) }
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
        path: *const nsACString,
        retval: *mut *const nsIKeyValueDatabase,
    ) -> nsresult {
        match self.get_or_create_default(path) {
            Ok(ptr) => {
                unsafe { ptr.forget(&mut *retval) };
                NS_OK
            }
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn GetOrCreate(
        &self,
        path: *const nsACString,
        name: *const nsACString,
        retval: *mut *const nsIKeyValueDatabase,
    ) -> nsresult {
        match self.get_or_create(path, name) {
            Ok(ptr) => {
                unsafe { ptr.forget(&mut *retval) };
                NS_OK
            }
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
        path: *const nsACString,
    ) -> Result<RefPtr<nsIKeyValueDatabase>, KeyValueError> {
        let path = str::from_utf8(unsafe { ensure_ref(path) }?)?;
        let mut writer = Manager::singleton().write()?;
        let rkv = writer.get_or_create(Path::new(path), Rkv::new)?;
        let store = rkv.write()?.open_or_create_default()?;
        let key_value_db = KeyValueDatabase::new(rkv, store);

        key_value_db
            .query_interface::<nsIKeyValueDatabase>()
            .ok_or(KeyValueError::NoInterface("nsIKeyValueDatabase"))
    }

    fn get_or_create(
        &self,
        path: *const nsACString,
        name: *const nsACString,
    ) -> Result<RefPtr<nsIKeyValueDatabase>, KeyValueError> {
        let path = str::from_utf8(unsafe { ensure_ref(path) }?)?;
        let name = str::from_utf8(unsafe { ensure_ref(name) }?)?;
        let mut writer = Manager::singleton().write()?;
        let rkv = writer.get_or_create(Path::new(path), Rkv::new)?;
        let store = rkv.write()?.open_or_create(Some(name))?;
        let key_value_db = KeyValueDatabase::new(rkv, store);

        key_value_db
            .query_interface::<nsIKeyValueDatabase>()
            .ok_or(KeyValueError::NoInterface("nsIKeyValueDatabase"))
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
    fn Put(&self, key: *const nsACString, value: *const nsIVariant) -> nsresult {
        match self.put(key, value) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn Has(&self, key: *const nsACString, retval: *mut bool) -> nsresult {
        match self.has(key, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn Get(
        &self,
        key: *const nsACString,
        default_value: *const nsIVariant,
        retval: *mut *const nsIVariant,
    ) -> nsresult {
        match self.get(key, default_value) {
            Ok(ptr) => {
                unsafe { ptr.forget(&mut *retval) };
                NS_OK
            }
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn Delete(&self, key: *const nsACString) -> nsresult {
        match self.delete(key) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn GetInt(
        &self,
        key: *const nsACString,
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

    fn GetDouble(
        &self,
        key: *const nsACString,
        default_value: c_double,
        retval: *mut c_double,
    ) -> nsresult {
        match self.get_double(key, default_value, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn GetString(
        &self,
        key: *const nsACString,
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

    fn GetBool(&self, key: *const nsACString, default_value: bool, retval: *mut bool) -> nsresult {
        match self.get_bool(key, default_value, retval) {
            Ok(_) => NS_OK,
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }

    fn Enumerate(
        &self,
        from_key: *const nsACString,
        retval: *mut *const nsISimpleEnumerator,
    ) -> nsresult {
        match self.enumerate(from_key) {
            Ok(ptr) => {
                unsafe { ptr.forget(&mut *retval) };
                NS_OK
            }
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

    fn put(&self, key: *const nsACString, value: *const nsIVariant) -> Result<(), KeyValueError> {
        let key = str::from_utf8(unsafe { ensure_ref(key) }?)?;
        let value = unsafe { ensure_ref(value) }?;

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
                writer.put(&self.store, key, &Value::I64(value_as_int32.into()))?;
                writer.commit()?;
            }
            DATA_TYPE_DOUBLE => {
                info!("nsIVariant type is double");
                let mut value_as_double: f64 = 0.0;
                unsafe { value.GetAsDouble(&mut value_as_double) }.to_result()?;
                writer.put(&self.store, key, &Value::F64(value_as_double.into()))?;
                writer.commit()?;
            }
            DATA_TYPE_WSTRING => {
                info!("nsIVariant type is string");
                let mut value_as_astring: nsString = nsString::new();
                unsafe { value.GetAsAString(&mut *value_as_astring) }.to_result()?;
                let value = String::from_utf16(&value_as_astring)?;
                writer.put(&self.store, key, &Value::Str(&value))?;
                writer.commit()?;
            }
            DATA_TYPE_BOOL => {
                info!("nsIVariant type is bool");
                let mut value_as_bool: bool = false;
                unsafe { value.GetAsBool(&mut value_as_bool) }.to_result()?;
                writer.put(&self.store, key, &Value::Bool(value_as_bool.into()))?;
                writer.commit()?;
            }
            _unsupported_type => {
                return Err(KeyValueError::UnsupportedType(dataType));
            }
        };

        Ok(())
    }

    fn has(&self, key: *const nsACString, retval: *mut bool) -> Result<(), KeyValueError> {
        let key = str::from_utf8(unsafe { ensure_ref(key) }?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, key)?;

        match value {
            Some(_) => unsafe { *retval = true },
            None => unsafe { *retval = false },
        };

        Ok(())
    }

    fn get(
        &self,
        key: *const nsACString,
        default_value: *const nsIVariant,
    ) -> Result<RefPtr<nsIVariant>, KeyValueError> {
        let key = str::from_utf8(unsafe { ensure_ref(key) }?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, key)?;

        match value {
            Some(Value::I64(value)) => Ok(value.into_variant().ok_or(KeyValueError::Read)?.take()),
            Some(Value::F64(value)) => Ok(value.into_variant().ok_or(KeyValueError::Read)?.take()),
            Some(Value::Str(value)) => Ok(nsString::from(value)
                .into_variant()
                .ok_or(KeyValueError::Read)?
                .take()),
            Some(Value::Bool(value)) => Ok(value.into_variant().ok_or(KeyValueError::Read)?.take()),
            None => {
                let default_value = unsafe { ensure_ref(default_value) }?;

                let mut dataType: uint16_t = 0;
                unsafe { default_value.GetDataType(&mut dataType) }.to_result()?;
                info!("get: default value nsIVariant type is {}", dataType);

                match dataType {
                    DATA_TYPE_INT32 => {
                        let mut val: int32_t = 0;
                        unsafe { default_value.GetAsInt32(&mut val) }.to_result()?;
                        Ok(val.into_variant().ok_or(KeyValueError::Read)?.take())
                    }
                    DATA_TYPE_DOUBLE => {
                        let mut val: f64 = 0.0;
                        unsafe { default_value.GetAsDouble(&mut val) }.to_result()?;
                        Ok(val.into_variant().ok_or(KeyValueError::Read)?.take())
                    }
                    DATA_TYPE_WSTRING => {
                        let mut val: nsString = nsString::new();
                        unsafe { default_value.GetAsAString(&mut *val) }.to_result()?;
                        Ok(val.into_variant().ok_or(KeyValueError::Read)?.take())
                    }
                    DATA_TYPE_BOOL => {
                        let mut val: bool = false;
                        unsafe { default_value.GetAsBool(&mut val) }.to_result()?;
                        Ok(val.into_variant().ok_or(KeyValueError::Read)?.take())
                    }
                    DATA_TYPE_EMPTY => {
                        let val = ();
                        Ok(val.into_variant().ok_or(KeyValueError::Read)?.take())
                    }
                    _unsupported_type => {
                        return Err(KeyValueError::UnsupportedType(dataType));
                    }
                }
            }
            Some(value) => return Err(KeyValueError::UnsupportedValue(value.into())),
        }
    }

    fn delete(&self, key: *const nsACString) -> Result<(), KeyValueError> {
        let key = str::from_utf8(unsafe { ensure_ref(key) }?)?;
        let env = self.rkv.read()?;
        let mut writer = env.write()?;

        match writer.delete(&self.store, key) {
            Ok(_) => (),

            // LMDB fails with an error if the key to delete wasn't found,
            // and Rkv returns that error, but we ignore it, as we expect most
            // of our consumers to want this behavior.
            Err(StoreError::LmdbError(lmdb::Error::NotFound)) => (),

            Err(err) => return Err(KeyValueError::StoreError(err)),
        };

        writer.commit()?;

        Ok(())
    }

    fn get_int(
        &self,
        key: *const nsACString,
        default_value: int64_t,
        retval: *mut int64_t,
    ) -> Result<(), KeyValueError> {
        let key = str::from_utf8(unsafe { ensure_ref(key) }?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(Value::I64(value)) => unsafe { *retval = value },
            None => unsafe { *retval = default_value },
            Some(value) => return Err(KeyValueError::UnsupportedValue(value.into())),
        };

        Ok(())
    }

    fn get_double(
        &self,
        key: *const nsACString,
        default_value: c_double,
        retval: *mut c_double,
    ) -> Result<(), KeyValueError> {
        let key = str::from_utf8(unsafe { ensure_ref(key) }?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(Value::F64(value)) => unsafe { *retval = value.into() },
            None => unsafe { *retval = default_value },
            Some(value) => return Err(KeyValueError::UnsupportedValue(value.into())),
        };

        Ok(())
    }

    fn get_string(
        &self,
        key: *const nsACString,
        default_value: *const nsAString,
        retval: *mut nsAString,
    ) -> Result<(), KeyValueError> {
        let key = str::from_utf8(unsafe { ensure_ref(key) }?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(Value::Str(value)) => unsafe { (*retval).assign(&nsString::from(value)) },
            None => unsafe { (*retval).assign(&*default_value) },
            Some(value) => return Err(KeyValueError::UnsupportedValue(value.into())),
        };

        Ok(())
    }

    fn get_bool(
        &self,
        key: *const nsACString,
        default_value: bool,
        retval: *mut bool,
    ) -> Result<(), KeyValueError> {
        let key = str::from_utf8(unsafe { ensure_ref(key) }?)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, &key)?;

        match value {
            Some(Value::Bool(value)) => unsafe { *retval = value },
            None => unsafe { *retval = default_value },
            Some(value) => return Err(KeyValueError::UnsupportedValue(value.into())),
        };

        Ok(())
    }

    fn enumerate(
        &self,
        from_key: *const nsACString,
    ) -> Result<RefPtr<nsISimpleEnumerator>, KeyValueError> {
        let env = self.rkv.read()?;
        let reader = env.read()?;

        // from_key is [optional], and XPConnect maps the absence of a value
        // to an empty string, so we know it isn't a null pointer.
        let from_key = str::from_utf8(unsafe { &*from_key })?;

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

        enumerator
            .query_interface::<nsISimpleEnumerator>()
            .ok_or(KeyValueError::NoInterface("nsISimpleEnumerator"))
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
        match self.get_next() {
            Ok(ptr) => {
                unsafe { ptr.forget(&mut *retval) };
                NS_OK
            }
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
    fn get_next(&self) -> Result<RefPtr<nsISupports>, KeyValueError> {
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

        pair.query_interface::<nsISupports>()
            .ok_or(KeyValueError::NoInterface("nsIKeyValuePair"))
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

    fn GetKey(&self, key: *mut nsACString) -> nsresult {
        unsafe { (*key).assign(&nsCString::from(&self.key)) }
        NS_OK
    }

    fn GetValue(&self, value: *mut *const nsIVariant) -> nsresult {
        match self.get_value() {
            Ok(ptr) => {
                unsafe { ptr.forget(&mut *value) };
                NS_OK
            }
            Err(error) => {
                error!("{}", error);
                error.into()
            }
        }
    }
}

impl KeyValuePair {
    fn get_value(&self) -> Result<RefPtr<nsIVariant>, KeyValueError> {
        Ok(self
            .value
            .clone()
            .into_variant()
            .ok_or(KeyValueError::Nsresult(NS_ERROR_FAILURE))?
            .take())
    }
}
