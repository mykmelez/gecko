/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[macro_use]
extern crate failure;
extern crate libc;
extern crate lmdb;
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
mod owned_value;
mod task;

use error::KeyValueError;
use libc::c_void;
use nserror::{nsresult, NS_ERROR_FAILURE, NS_ERROR_NO_AGGREGATION, NS_OK};
use nsstring::{nsACString, nsCString};
use owned_value::{variant_to_owned, OwnedValue};
use rkv::{Rkv, Store};
use std::{
    cell::RefCell,
    ptr,
    sync::{Arc, RwLock},
    vec::IntoIter,
};
use storage_variant::IntoVariant;
use task::{
    create_thread, DeleteTask, EnumerateTask, GetNextTask, GetOrCreateTask, GetTask,
    HasMoreElementsTask, HasTask, PutTask, TaskRunnable,
};
use xpcom::{
    interfaces::{
        nsIKeyValueDatabaseCallback, nsIKeyValueEnumeratorCallback, nsIKeyValuePairCallback,
        nsIKeyValueVariantCallback, nsIKeyValueVoidCallback, nsISupports, nsIThread, nsIVariant,
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
        GetOrCreate,
        get_or_create,
        { callback: *const nsIKeyValueDatabaseCallback, path: *const nsACString,
            name: *const nsACString }
    );

    fn get_or_create(
        &self,
        callback: &nsIKeyValueDatabaseCallback,
        path: &nsACString,
        name: &nsACString,
    ) -> Result<(), nsresult> {
        let target = create_thread("KeyValDB")?;

        let task = Box::new(GetOrCreateTask::new(
            RefPtr::new(callback),
            target.clone(),
            nsCString::from(path),
            nsCString::from(name),
        ));

        TaskRunnable::new("KVService::GetOrCreate", task)?.dispatch(target)
    }
}

#[derive(Clone, xpcom)]
#[xpimplements(nsIKeyValueDatabase)]
#[refcnt = "atomic"]
pub struct InitKeyValueDatabase {
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    thread: RefPtr<nsIThread>,
}

impl KeyValueDatabase {
    fn new(
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        thread: RefPtr<nsIThread>,
    ) -> RefPtr<KeyValueDatabase> {
        KeyValueDatabase::allocate(InitKeyValueDatabase { rkv, store, thread })
    }

    xpcom_method!(
        Put,
        put,
        { callback: *const nsIKeyValueVoidCallback, key: *const nsACString,
            value: *const nsIVariant }
    );

    fn put(
        &self,
        callback: &nsIKeyValueVoidCallback,
        key: &nsACString,
        value: &nsIVariant,
    ) -> Result<(), nsresult> {
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

        TaskRunnable::new("KVDatabase::Put", task)?.dispatch(self.thread.clone())
    }

    xpcom_method!(
        Get,
        get,
        { callback: *const nsIKeyValueVariantCallback, key: *const nsACString,
            default_value: *const nsIVariant }
    );

    fn get(
        &self,
        callback: &nsIKeyValueVariantCallback,
        key: &nsACString,
        default_value: &nsIVariant,
    ) -> Result<(), nsresult> {
        let task = Box::new(GetTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(key),
            variant_to_owned(default_value)?,
        ));

        TaskRunnable::new("KVDatabase::Get", task)?.dispatch(self.thread.clone())
    }

    xpcom_method!(
        Has,
        has,
        { callback: *const nsIKeyValueVariantCallback, key: *const nsACString }
    );

    fn has(&self, callback: &nsIKeyValueVariantCallback, key: &nsACString) -> Result<(), nsresult> {
        let task = Box::new(HasTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(key),
        ));

        TaskRunnable::new("KVDatabase::Has", task)?.dispatch(self.thread.clone())
    }

    xpcom_method!(
        Delete,
        delete,
        { callback: *const nsIKeyValueVoidCallback, key: *const nsACString }
    );

    fn delete(&self, callback: &nsIKeyValueVoidCallback, key: &nsACString) -> Result<(), nsresult> {
        let task = Box::new(DeleteTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(key),
        ));

        TaskRunnable::new("KVDatabase::Delete", task)?.dispatch(self.thread.clone())
    }

    xpcom_method!(
        Enumerate,
        enumerate,
        { callback: *const nsIKeyValueEnumeratorCallback, from_key: *const nsACString,
            to_key: *const nsACString }
    );

    fn enumerate(
        &self,
        callback: &nsIKeyValueEnumeratorCallback,
        from_key: &nsACString,
        to_key: &nsACString,
    ) -> Result<(), nsresult> {
        let task = Box::new(EnumerateTask::new(
            RefPtr::new(callback),
            Arc::clone(&self.rkv),
            self.store,
            nsCString::from(from_key),
            nsCString::from(to_key),
        ));

        TaskRunnable::new("KVDatabase::Enumerate", task)?.dispatch(self.thread.clone())
    }
}

#[derive(xpcom)]
#[xpimplements(nsIKeyValueEnumerator)]
#[refcnt = "atomic"]
pub struct InitKeyValueEnumerator {
    thread: RefPtr<nsIThread>,
    iter: Arc<
        RefCell<
            IntoIter<(
                Result<String, KeyValueError>,
                Result<OwnedValue, KeyValueError>,
            )>,
        >,
    >,
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
            iter: Arc::new(RefCell::new(pairs.into_iter())),
        })
    }

    xpcom_method!(HasMoreElements, has_more_elements, {
        callback: *const nsIKeyValueVariantCallback
    });

    fn has_more_elements(&self, callback: &nsIKeyValueVariantCallback) -> Result<(), nsresult> {
        let task = Box::new(HasMoreElementsTask::new(
            RefPtr::new(callback),
            self.iter.clone(),
        ));

        TaskRunnable::new("KVEnumerator::HasMoreElements", task)?.dispatch(self.thread.clone())
    }

    xpcom_method!(GetNext, get_next, {
        callback: *const nsIKeyValuePairCallback
    });

    fn get_next(&self, callback: &nsIKeyValuePairCallback) -> Result<(), nsresult> {
        let task = Box::new(GetNextTask::new(RefPtr::new(callback), self.iter.clone()));

        TaskRunnable::new("KVEnumerator::GetNext", task)?.dispatch(self.thread.clone())
    }
}

#[derive(xpcom)]
#[xpimplements(nsIKeyValuePair)]
#[refcnt = "atomic"]
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
