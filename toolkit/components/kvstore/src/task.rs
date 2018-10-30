/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate xpcom;

use error::KeyValueError;
use libc::{int32_t, uint16_t};
use nserror::{nsresult, NsresultExt, NS_OK};
use nsstring::{nsACString, nsCString, nsString};
use rkv::{Manager, Rkv, Store, Value};
use std::{cell::Cell, cell::RefCell, path::Path, ptr, str, sync::{Arc, RwLock}};
use storage_variant::{IntoVariant, Variant};
use xpcom::{
    getter_addrefs,
    interfaces::{nsIEventTarget, nsIKeyValueCallback, nsIRunnable, nsISupports, nsIThread, nsIVariant},
    RefPtr,
};
use KeyValueDatabase;

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

extern "C" {
    fn NS_GetCurrentThreadEventTarget(result: *mut *const nsIThread) -> nsresult;
    fn NS_NewNamedThreadWithDefaultStackSize(
        name: *const nsACString,
        result: *mut *const nsIThread,
        event: *const nsIRunnable,
    ) -> nsresult;
}

pub fn get_current_thread() -> Result<RefPtr<nsIThread>, nsresult> {
    getter_addrefs(|p| unsafe { NS_GetCurrentThreadEventTarget(p) })
}

pub fn create_thread(name: &str) -> Result<RefPtr<nsIThread>, nsresult> {
    getter_addrefs(|p| unsafe {
        NS_NewNamedThreadWithDefaultStackSize(&*nsCString::from(name), p, ptr::null())
    })
}

/// A task is executed asynchronously on a target thread, and passes its
/// result back to the original thread.
pub trait Task {
    fn run(&self) -> Result<Option<RefPtr<nsISupports>>, KeyValueError>;
    fn done(&self, result: Result<Option<RefPtr<nsISupports>>, KeyValueError>) -> Result<(), nsresult>;
}

pub struct GetOrCreateTask {
    callback: RefPtr<nsIKeyValueCallback>,
    thread: RefPtr<nsIThread>,
    path: nsCString,
    name: nsCString,
}

impl GetOrCreateTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueCallback>,
        thread: RefPtr<nsIThread>,
        path: nsCString,
        name: nsCString,
    ) -> GetOrCreateTask {
        GetOrCreateTask {
            callback,
            thread,
            path,
            name,
        }
    }
}

impl Task for GetOrCreateTask {
    fn run(&self) -> Result<Option<RefPtr<nsISupports>>, KeyValueError> {
        let mut writer = Manager::singleton().write()?;
        let rkv = writer.get_or_create(Path::new(str::from_utf8(&self.path)?), Rkv::new)?;
        let store = if self.name.is_empty() {
            rkv.write()?.open_or_create_default()
        } else {
            rkv.write()?
                .open_or_create(Some(str::from_utf8(&self.name)?))
        }?;
        let key_value_db = KeyValueDatabase::new(rkv, store, Some(self.thread.clone()));

        match key_value_db.query_interface::<nsISupports>() {
            Some(db) => Ok(Some(db)),
            None => Err(KeyValueError::NoInterface("nsISupports")),
        }
    }

    fn done(&self, result: Result<Option<RefPtr<nsISupports>>, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(Some(value)) => unsafe { self.callback.HandleResult(value.coerce()) },
            Ok(None) => unsafe { self.callback.HandleResult(ptr::null()) },
            Err(err) => unsafe { self.callback.HandleError(nsresult::from(err)) },
        }.to_result()
    }
}

pub struct PutTask {
    callback: RefPtr<nsIKeyValueCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    value: RefPtr<nsIVariant>,
}

impl PutTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
        value: RefPtr<nsIVariant>,
    ) -> PutTask {
        PutTask {
            callback,
            rkv,
            store,
            key,
            value,
        }
    }
}

impl Task for PutTask {
    fn run(&self) -> Result<Option<RefPtr<nsISupports>>, KeyValueError> {
        let key = str::from_utf8(&self.key)?;

        let mut data_type: uint16_t = 0;
        unsafe { self.value.GetDataType(&mut data_type) }.to_result()?;
        let env = self.rkv.read()?;
        let mut writer = env.write()?;

        match data_type {
            DATA_TYPE_INT32 => {
                let mut value_as_int32: int32_t = 0;
                unsafe { self.value.GetAsInt32(&mut value_as_int32) }.to_result()?;
                writer.put(&self.store, key, &Value::I64(value_as_int32.into()))?;
                writer.commit()?;
            }
            DATA_TYPE_DOUBLE => {
                let mut value_as_double: f64 = 0.0;
                unsafe { self.value.GetAsDouble(&mut value_as_double) }.to_result()?;
                writer.put(&self.store, key, &Value::F64(value_as_double.into()))?;
                writer.commit()?;
            }
            DATA_TYPE_WSTRING => {
                let mut value_as_astring: nsString = nsString::new();
                unsafe { self.value.GetAsAString(&mut *value_as_astring) }.to_result()?;
                let value = String::from_utf16(&value_as_astring)?;
                writer.put(&self.store, key, &Value::Str(&value))?;
                writer.commit()?;
            }
            DATA_TYPE_BOOL => {
                let mut value_as_bool: bool = false;
                unsafe { self.value.GetAsBool(&mut value_as_bool) }.to_result()?;
                writer.put(&self.store, key, &Value::Bool(value_as_bool.into()))?;
                writer.commit()?;
            }
            _unsupported_type => {
                return Err(KeyValueError::UnsupportedType(data_type));
            }
        };

        Ok(None)
    }

    fn done(&self, result: Result<Option<RefPtr<nsISupports>>, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(Some(value)) => unsafe { self.callback.HandleResult(value.coerce()) },
            Ok(None) => unsafe { self.callback.HandleResult(ptr::null()) },
            Err(err) => unsafe { self.callback.HandleError(nsresult::from(err)) },
        }.to_result()
    }
}

#[derive(xpcom)]
#[xpimplements(nsIRunnable, nsINamed)]
#[refcnt = "atomic"]
pub struct InitTaskRunnable {
    name: &'static str,
    source: RefPtr<nsIThread>,

    /// Holds the task, and the result of the task.  The task is created
    /// on the current thread, run on a target thread, and handled again
    /// on the original thread; the result is mutated on the target thread
    /// and accessed on the original thread.
    task: Box<Task>,
    result: Cell<Option<Result<Option<RefPtr<nsISupports>>, KeyValueError>>>,
}

impl TaskRunnable {
    pub fn new(
        name: &'static str,
        source: RefPtr<nsIThread>,
        task: Box<Task>,
        result: Cell<Option<Result<Option<RefPtr<nsISupports>>, KeyValueError>>>,
    ) -> RefPtr<TaskRunnable> {
        TaskRunnable::allocate(InitTaskRunnable {
            name,
            source,
            task,
            result,
        })
    }

    xpcom_method!(Run, run, {});
    fn run(&self) -> Result<(), nsresult> {
        match self.result.take() {
            None => {
                // Run the task on the target thread, store the result,
                // and dispatch the runnable back to the source thread.
                let result = self.task.run();
                self.result.set(Some(result));
                let target = getter_addrefs(|p| unsafe { self.source.GetEventTarget(p) })?;
                unsafe {
                    target.DispatchFromScript(self.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
                }.to_result()
            }
            Some(result) => {
                // Back on the source thread, notify the task we're done.
                self.task.done(result)
            }
        }
    }

    xpcom_method!(GetName, get_name, {}, *mut nsACString);
    fn get_name(&self) -> Result<nsCString, nsresult> {
        Ok(nsCString::from(self.name))
    }
}
