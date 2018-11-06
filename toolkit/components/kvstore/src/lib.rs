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

mod data_type;
mod error;
mod ownedvalue;
mod task;

use data_type::{
    DataType,
    DATA_TYPE_INT32,
    DATA_TYPE_DOUBLE,
    DATA_TYPE_BOOL,
    DATA_TYPE_VOID,
    DATA_TYPE_WSTRING,
    DATA_TYPE_EMPTY,
};
use error::KeyValueError;
use libc::{c_double, c_void, int32_t, int64_t, uint16_t};
use nserror::{
    nsresult, NsresultExt, NS_ERROR_FAILURE, NS_ERROR_NOT_IMPLEMENTED, NS_ERROR_NO_AGGREGATION,
    NS_OK,
};
use nsstring::{nsACString, nsCString, nsString};
use ownedvalue::{value_to_owned, variant_to_owned, OwnedValue};
use rkv::{Manager, Rkv, Store, StoreError, Value};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    ptr, rc::Rc, str,
    sync::{Arc, RwLock},
    vec::IntoIter,
};
use storage_variant::{IntoVariant, Variant};
use task::{create_thread, get_current_thread, DeleteTask, EnumerateTask,
    GetNextTask, GetOrCreateTask, GetTask, HasMoreElementsTask, HasTask, PutTask, TaskRunnable
};
use xpcom::{
    interfaces::{
        nsIEventTarget, nsIJSEnumerator, nsIKeyValueVoidCallback, nsIKeyValueDatabaseCallback, nsIKeyValueEnumeratorCallback, nsIKeyValueVariantCallback, nsIKeyValueDatabase,
        nsIKeyValueEnumerator, nsIKeyValuePairCallback, nsISupports, nsIThread, nsIVariant,
    },
    nsIID, Ensure, RefPtr,
};

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
        GetOrCreateAsync,
        get_or_create_async,
        { callback: *const nsIKeyValueDatabaseCallback, path: *const nsACString, name: *const nsACString }
    );

    fn get_or_create_async(
        &self,
        callback: &nsIKeyValueDatabaseCallback,
        path: &nsACString,
        name: &nsACString,
    ) -> Result<(), nsresult> {
        let source = get_current_thread()?;
        let target = create_thread("KeyValDB")?;
        let task = Box::new(GetOrCreateTask::new(
            RefPtr::new(callback),
            target.clone(),
            nsCString::from(path),
            nsCString::from(name),
        ));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::GetOrCreateAsync",
            source,
            task,
        );

        unsafe {
            target.DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }
}

#[derive(Clone)]
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
    fn new(
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        thread: Option<RefPtr<nsIThread>>,
    ) -> RefPtr<KeyValueDatabase> {
        KeyValueDatabase::allocate(InitKeyValueDatabase { rkv, store, thread })
    }

    xpcom_method!(
        PutAsync,
        put_async,
        { callback: *const nsIKeyValueVoidCallback, key: *const nsACString, value: *const nsIVariant }
    );

    fn put_async(
        &self,
        callback: &nsIKeyValueVoidCallback,
        key: &nsACString,
        value: &nsIVariant,
    ) -> Result<(), nsresult> {
        let source = get_current_thread()?;
        let value = match variant_to_owned(value)? {
            Some(value) => Ok(value),
            None => Err(KeyValueError::UnexpectedValue),
        }?;

        let task = Box::new(PutTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(key),
            value,
        ));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::PutAsync",
            source,
            task,
        );

        unsafe {
            self.thread.as_ref().unwrap().DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }

    xpcom_method!(
        HasAsync,
        has_async,
        { callback: *const nsIKeyValueVariantCallback, key: *const nsACString }
    );

    fn has_async(
        &self,
        callback: &nsIKeyValueVariantCallback,
        key: &nsACString,
    ) -> Result<(), nsresult> {
        let source = get_current_thread()?;
        let task = Box::new(HasTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(key),
        ));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::HasAsync",
            source,
            task,
        );

        unsafe {
            self.thread.as_ref().unwrap().DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }

    xpcom_method!(
        GetAsync,
        get_async,
        { callback: *const nsIKeyValueVariantCallback, key: *const nsACString, default_value: *const nsIVariant }
    );

    fn get_async(
        &self,
        callback: &nsIKeyValueVariantCallback,
        key: &nsACString,
        default_value: &nsIVariant,
    ) -> Result<(), nsresult> {
        let source = get_current_thread()?;
        let task = Box::new(GetTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(key),
            RefPtr::new(default_value),
        ));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::GetAsync",
            source,
            task,
        );

        unsafe {
            self.thread.as_ref().unwrap().DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }

    xpcom_method!(
        DeleteAsync,
        delete_async,
        { callback: *const nsIKeyValueVoidCallback, key: *const nsACString }
    );

    fn delete_async(
        &self,
        callback: &nsIKeyValueVoidCallback,
        key: &nsACString,
    ) -> Result<(), nsresult> {
        let source = get_current_thread()?;
        let task = Box::new(DeleteTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(key),
        ));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::DeleteAsync",
            source,
            task,
        );

        unsafe {
            self.thread.as_ref().unwrap().DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }

    xpcom_method!(
        EnumerateAsync,
        enumerate_async,
        { callback: *const nsIKeyValueEnumeratorCallback, from_key: *const nsACString, to_key: *const nsACString }
    );

    fn enumerate_async(
        &self,
        callback: &nsIKeyValueEnumeratorCallback,
        from_key: &nsACString,
        to_key: &nsACString,
    ) -> Result<(), nsresult> {
        let source = get_current_thread()?;
        let task = Box::new(EnumerateTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(from_key),
            nsCString::from(to_key),
        ));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::EnumerateAsync",
            source,
            task,
        );

        unsafe {
            self.thread.as_ref().unwrap().DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }
}

#[derive(xpcom)]
#[xpimplements(nsIKeyValueEnumerator)]
#[refcnt = "nonatomic"]
pub struct InitKeyValueEnumerator {
    thread: RefPtr<nsIThread>,
    iter: Rc<RefCell<IntoIter<(
        Result<String, KeyValueError>,
        Result<OwnedValue, KeyValueError>,
    )>>>,
}

impl KeyValueEnumerator {
    fn new(
        thread: RefPtr<nsIThread>,
        pairs: Vec<(
            Result<String, KeyValueError>,
            Result<OwnedValue, KeyValueError>,
        )>,
    ) -> RefPtr<KeyValueEnumerator> {
        KeyValueEnumerator::allocate(InitKeyValueEnumerator {
            thread,
            iter: Rc::new(RefCell::new(pairs.into_iter())),
        })
    }

    xpcom_method!(HasMoreElementsAsync, has_more_elements_async, { callback: *const nsIKeyValueVariantCallback });
    // xpcom_method!(GetNextAsync, get_next_async, { callback: *const nsIKeyValueVoidCallback });

    fn has_more_elements_async(
        &self,
        callback: &nsIKeyValueVariantCallback,
    ) -> Result<(), nsresult> {
        let source = get_current_thread()?;
        let task = Box::new(HasMoreElementsTask::new(
            RefPtr::new(callback),
            self.iter.clone(),
        ));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::HasMoreElementsAsync",
            source,
            task,
        );

        unsafe {
            self.thread.DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }

    xpcom_method!(GetNextAsync, get_next_async, { callback: *const nsIKeyValuePairCallback });

    fn get_next_async(
        &self,
        callback: &nsIKeyValuePairCallback,
    ) -> Result<(), nsresult> {
        let source = get_current_thread()?;
        let task = Box::new(GetNextTask::new(
            RefPtr::new(callback),
            self.iter.clone(),
        ));

        let runnable = TaskRunnable::new(
            "KeyValueDatabase::GetNextAsync",
            source,
            task,
        );

        unsafe {
            self.thread.DispatchFromScript(runnable.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }

    // fn has_more_elements(&self) -> Result<bool, KeyValueError> {
    //     Ok(!self.iter.borrow().as_slice().is_empty())
    // }

    // fn get_next(&self) -> Result<RefPtr<nsISupports>, KeyValueError> {
    //     let mut iter = self.iter.borrow_mut();
    //     let (key, value) = iter.next().ok_or(KeyValueError::from(NS_ERROR_FAILURE))?;

    //     // We fail on retrieval of the key/value pair if the key isn't valid
    //     // UTF-*, if the value is unexpected, or if we encountered a store error
    //     // while retrieving the pair.
    //     let pair = KeyValuePair::new(key?, value?);

    //     pair.query_interface::<nsISupports>()
    //         .ok_or(KeyValueError::NoInterface("nsIKeyValuePair"))
    // }
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
        DATA_TYPE_EMPTY | DATA_TYPE_VOID => {
            let val = ();
            Ok(val.into_variant().ok_or(KeyValueError::Read)?)
        }
        _unsupported_type => {
            println!("unsupported variant data type: {:?}", data_type);
            return Err(KeyValueError::UnsupportedType(data_type));
        }
    }
}
