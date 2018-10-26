/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[macro_use]
extern crate failure;
extern crate libc;
extern crate lmdb;
#[macro_use]
extern crate log;
extern crate nserror;
extern crate nsstring;
extern crate ordered_float;
extern crate rkv;
extern crate storage_variant;
#[macro_use]
extern crate xpcom;

mod error;
mod ownedvalue;
mod task;

use error::KeyValueError;
use libc::{c_double, c_void, int32_t, int64_t, uint16_t};
use nserror::{
    nsresult, NsresultExt, NS_ERROR_FAILURE, NS_ERROR_NOT_IMPLEMENTED, NS_ERROR_NO_AGGREGATION,
    NS_OK,
};
use nsstring::{nsACString, nsCString, nsString};
use ownedvalue::{value_to_owned, OwnedValue};
use rkv::{Manager, Rkv, Store, StoreError, Value};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    ptr, str,
    sync::{Arc, RwLock},
    vec::IntoIter,
};
use storage_variant::{IntoVariant, Variant};
use task::{create_thread, GetOrCreateTask, get_current_thread, TaskRunnable};
use xpcom::{
    interfaces::{
        nsIEventTarget, nsIJSEnumerator, nsIKeyValueCallback, nsIKeyValueDatabase,
        nsISimpleEnumerator, nsISupports, nsIThread, nsIVariant,
    },
    nsIID, Ensure, RefPtr,
};

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

macro_rules! get_method {
    ($name:ident, $default:ty, $variant:ident, $result:ty) => {
        fn $name(&self, key: &nsACString, default_value: $default) -> Result<$result, KeyValueError> {
            let key = str::from_utf8(key)?;
            let env = self.rkv.read()?;
            let reader = env.read()?;
            let value = reader.get(&self.store, &key)?;

            match value {
                Some(Value::$variant(value)) => Ok(value.into()),
                Some(_value) => Err(KeyValueError::UnexpectedValue),
                None => Ok(default_value.into()),
            }
        }
    };
}

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
// The XPCOM methods are implemented using the xpcom_method! declarative macro
// from the xpcom crate.

#[derive(xpcom)]
#[xpimplements(nsIKeyValueService)]
#[refcnt = "atomic"]
pub struct InitKeyValueService {}

impl KeyValueService {
    fn new() -> RefPtr<KeyValueService> {
        KeyValueService::allocate(InitKeyValueService {})
    }

    xpcom_method!(
        GetOrCreate,
        get_or_create,
        { path: *const nsACString, name: *const nsACString },
        *mut *const nsIKeyValueDatabase
    );

    xpcom_method!(
        GetOrCreateAsync,
        get_or_create_async,
        { callback: *const nsIKeyValueCallback, path: *const nsACString, name: *const nsACString }
    );

    fn get_or_create(
        &self,
        path: &nsACString,
        name: &nsACString,
    ) -> Result<RefPtr<nsIKeyValueDatabase>, KeyValueError> {
        let path = str::from_utf8(path)?;
        let name = str::from_utf8(name)?;
        let mut writer = Manager::singleton().write()?;
        let rkv = writer.get_or_create(Path::new(path), Rkv::new)?;
        let store = if name.is_empty() {
            rkv.write()?.open_or_create_default()
        } else {
            rkv.write()?.open_or_create(Some(name))
        }?;
        let key_value_db = KeyValueDatabase::new(rkv, store, None);

        key_value_db
            .query_interface::<nsIKeyValueDatabase>()
            .ok_or(KeyValueError::NoInterface("nsIKeyValueDatabase"))
    }

    fn get_or_create_async(
        &self,
        callback: &nsIKeyValueCallback,
        path: &nsACString,
        name: &nsACString,
    ) -> Result<(), KeyValueError> {
        let source = get_current_thread()?;
        let target = create_thread("KeyValDB")?;
        let task = Box::new(GetOrCreateTask::new(RefPtr::new(callback), target.clone(), path, name));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::GetOrCreateAsync",
            source,
            task,
            Cell::default(),
        );

        let rv = unsafe {
            target.DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        };

        if rv.succeeded() {
            Ok(())
        } else {
            Err(KeyValueError::Nsresult(rv.error_name(), rv))
        }
    }
}

#[derive(xpcom)]
#[xpimplements(nsIKeyValueDatabase)]
#[refcnt = "nonatomic"]
pub struct InitKeyValueDatabase {
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    // TODO: require this rather than making it optional.
    thread: Option<RefPtr<nsIThread>>,
}

impl KeyValueDatabase {
    fn new(rkv: Arc<RwLock<Rkv>>, store: Store, thread: Option<RefPtr<nsIThread>>) -> RefPtr<KeyValueDatabase> {
        KeyValueDatabase::allocate(InitKeyValueDatabase { rkv, store, thread })
    }

    xpcom_method!(Put, put, { key: *const nsACString, value: *const nsIVariant });
    xpcom_method!(Has, has, { key: *const nsACString }, *mut bool);
    xpcom_method!(Get, get, { key: *const nsACString, default_value: *const nsIVariant }, *mut *const nsIVariant);
    xpcom_method!(Delete, delete, { key: *const nsACString });
    xpcom_method!(GetInt, get_int, { key: *const nsACString, default_value: int64_t }, *mut int64_t);
    xpcom_method!(GetDouble, get_double, { key: *const nsACString, default_value: c_double }, *mut c_double);
    xpcom_method!(GetBool, get_bool, { key: *const nsACString, default_value: bool }, *mut bool);
    xpcom_method!(GetString, get_string, { key: *const nsACString, default_value: *const nsACString }, *mut nsACString);
    xpcom_method!(
        Enumerate,
        enumerate,
        { from_key: *const nsACString, to_key: *const nsACString },
        *mut *const nsISimpleEnumerator
    );

    fn put(&self, key: &nsACString, value: &nsIVariant) -> Result<(), KeyValueError> {
        let key = str::from_utf8(key)?;

        let mut data_type: uint16_t = 0;
        unsafe { value.GetDataType(&mut data_type) }.to_result()?;
        info!("nsIVariant type is {}", data_type);

        let env = self.rkv.read()?;
        let mut writer = env.write()?;

        match data_type {
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
                return Err(KeyValueError::UnsupportedType(data_type));
            }
        };

        Ok(())
    }

    fn has(&self, key: &nsACString) -> Result<bool, KeyValueError> {
        let key = str::from_utf8(key)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, key)?;
        Ok(value.is_some())
    }

    fn get(
        &self,
        key: &nsACString,
        default_value: &nsIVariant,
    ) -> Result<RefPtr<nsIVariant>, KeyValueError> {
        let key = str::from_utf8(key)?;
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
            Some(_value) => Err(KeyValueError::UnexpectedValue),
            None => Ok(into_variant(default_value)?.take()),
        }
    }

    fn delete(&self, key: &nsACString) -> Result<(), KeyValueError> {
        let key = str::from_utf8(key)?;
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

    get_method!(get_int, int64_t, I64, int64_t);
    get_method!(get_double, c_double, F64, c_double);
    get_method!(get_bool, bool, Bool, bool);
    get_method!(get_string, &nsACString, Str, nsCString);

    fn enumerate(
        &self,
        from_key: &nsACString,
        to_key: &nsACString,
    ) -> Result<RefPtr<nsISimpleEnumerator>, KeyValueError> {
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let from_key = str::from_utf8(from_key)?;
        let to_key = str::from_utf8(to_key)?;

        let iterator = if from_key.is_empty() {
            reader.iter_start(&self.store)?
        } else {
            reader.iter_from(&self.store, &from_key)?
        };

        // Ideally, we'd enumerate pairs lazily, as the consumer calls
        // nsISimpleEnumerator.getNext(), which calls our
        // SimpleEnumerator.get_next() implementation.  But SimpleEnumerator
        // can't reference the Iter because Rust "cannot #[derive(xpcom)]
        // on a generic type," and the Iter requires a lifetime parameter,
        // which would make SimpleEnumerator generic.
        //
        // Our fallback approach is to eagerly collect the iterator
        // into a collection that SimpleEnumerator owns.  Fixing this so we
        // enumerate pairs lazily is bug 1499252.
        let pairs: Vec<(
            Result<String, KeyValueError>,
            Result<OwnedValue, KeyValueError>,
        )> = iterator
            // Convert the key to a string so we can compare it to the "to" key.
            // For forward compatibility, we don't fail here if we can't convert
            // a key to UTF-8.  Instead, we store the Err in the collection
            // and fail lazily in SimpleEnumerator.get_next().
            .map(|(key, val)| (str::from_utf8(&key), val))
            .take_while(|(key, _val)| {
                if to_key.is_empty() {
                    true
                } else {
                    match *key {
                        Ok(key) => key <= to_key,
                        Err(_err) => true,
                    }
                }
            }).map(|(key, val)| {
                (
                    match key {
                        Ok(key) => Ok(key.to_owned()),
                        Err(err) => Err(err.into()),
                    },
                    value_to_owned(val),
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
    iter: RefCell<
        IntoIter<(
            Result<String, KeyValueError>,
            Result<OwnedValue, KeyValueError>,
        )>,
    >,
}

impl SimpleEnumerator {
    fn new(
        pairs: Vec<(
            Result<String, KeyValueError>,
            Result<OwnedValue, KeyValueError>,
        )>,
    ) -> RefPtr<SimpleEnumerator> {
        SimpleEnumerator::allocate(InitSimpleEnumerator {
            iter: RefCell::new(pairs.into_iter()),
        })
    }

    xpcom_method!(HasMoreElements, has_more_elements, {}, *mut bool);
    xpcom_method!(GetNext, get_next, {}, *mut *const nsISupports);

    // The nsISimpleEnumeratorBase methods iterator() and entries() depend on
    // nsIJSEnumerator, which requires jscontext, which is unsupported for Rust.
    #[allow(non_snake_case)]
    fn Iterator(&self, _retval: *mut *const nsIJSEnumerator) -> nsresult {
        NS_ERROR_NOT_IMPLEMENTED
    }
    #[allow(non_snake_case)]
    fn Entries(&self, _aIface: *const nsIID, _retval: *mut *const nsIJSEnumerator) -> nsresult {
        NS_ERROR_NOT_IMPLEMENTED
    }

    fn has_more_elements(&self) -> Result<bool, KeyValueError> {
        Ok(!self.iter.borrow().as_slice().is_empty())
    }

    fn get_next(&self) -> Result<RefPtr<nsISupports>, KeyValueError> {
        let mut iter = self.iter.borrow_mut();
        let (key, value) = iter.next().ok_or(KeyValueError::from(NS_ERROR_FAILURE))?;

        // We fail on retrieval of the key/value pair if the key isn't valid
        // UTF-*, if the value is unexpected, or if we encountered a store error
        // while retrieving the pair.
        let pair = KeyValuePair::new(key?, value?);

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

    xpcom_method!(GetKey, get_key, {}, *mut nsACString);
    xpcom_method!(GetValue, get_value, {}, *mut *const nsIVariant);

    fn get_key(&self) -> Result<nsCString, KeyValueError> {
        Ok(nsCString::from(&self.key))
    }

    fn get_value(&self) -> Result<RefPtr<nsIVariant>, KeyValueError> {
        Ok(self
            .value
            .clone()
            .into_variant()
            .ok_or(KeyValueError::from(NS_ERROR_FAILURE))?
            .take())
    }
}

// TODO: consider making this an implementation of the IntoVariant trait
// from storage/variant/src/lib.rs.
fn into_variant(variant: &nsIVariant) -> Result<Variant, KeyValueError> {
    let mut data_type: uint16_t = 0;
    unsafe { variant.GetDataType(&mut data_type) }.to_result()?;

    match data_type {
        DATA_TYPE_INT32 => {
            let mut val: int32_t = 0;
            unsafe { variant.GetAsInt32(&mut val) }.to_result()?;
            Ok(val.into_variant().ok_or(KeyValueError::Read)?)
        }
        DATA_TYPE_DOUBLE => {
            let mut val: f64 = 0.0;
            unsafe { variant.GetAsDouble(&mut val) }.to_result()?;
            Ok(val.into_variant().ok_or(KeyValueError::Read)?)
        }
        DATA_TYPE_WSTRING => {
            let mut val: nsString = nsString::new();
            unsafe { variant.GetAsAString(&mut *val) }.to_result()?;
            Ok(val.into_variant().ok_or(KeyValueError::Read)?)
        }
        DATA_TYPE_BOOL => {
            let mut val: bool = false;
            unsafe { variant.GetAsBool(&mut val) }.to_result()?;
            Ok(val.into_variant().ok_or(KeyValueError::Read)?)
        }
        DATA_TYPE_EMPTY => {
            let val = ();
            Ok(val.into_variant().ok_or(KeyValueError::Read)?)
        }
        _unsupported_type => {
            return Err(KeyValueError::UnsupportedType(data_type));
        }
    }
}
