/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate xpcom;

use error::KeyValueError;
use into_variant;
use libc::{int32_t, uint16_t};
use nserror::{nsresult, NsresultExt, NS_ERROR_FAILURE, NS_OK};
use nsstring::{nsACString, nsCString, nsString};
use ownedvalue::{value_to_owned, OwnedValue};
use rkv::{Manager, Rkv, Store, StoreError, Value};
use std::{cell::Cell, cell::RefCell, path::Path, ptr, rc::Rc, str, sync::{Arc, RwLock}, vec::IntoIter};
use storage_variant::{IntoVariant, Variant};
use xpcom::{
    getter_addrefs,
    interfaces::{nsIEventTarget, nsIKeyValueCallback, nsIKeyValueDatabase, nsIKeyValueDatabaseCallback,
        nsIKeyValueVariantCallback, nsIKeyValuePairCallback, nsIRunnable, nsISupports, nsIThread, nsIVariant,
    },
    RefPtr,
};
use KeyValueDatabase;
use KeyValueEnumerator;
use KeyValuePair;

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

pub trait DatabaseTask {
    fn run(&self) -> Result<Option<RefPtr<nsIKeyValueDatabase>>, KeyValueError>;
    fn done(&self, result: Result<Option<RefPtr<nsIKeyValueDatabase>>, KeyValueError>) -> Result<(), nsresult>;
}

pub trait VariantTask {
    fn run(&self) -> Result<Option<RefPtr<nsIVariant>>, KeyValueError>;
    fn done(&self, result: Result<Option<RefPtr<nsIVariant>>, KeyValueError>) -> Result<(), nsresult>;
}

/// We can't generalize Task with a type parameter because it's held by
/// the nsIRunnable, which can't be generic, because "cannot #[derive(xpcom)]
/// on a generic type."  So we specialize Task/TaskRunnable by return value.
pub trait BoolTask {
    fn run(&self) -> Result<bool, KeyValueError>;
    fn done(&self, result: Result<bool, KeyValueError>) -> Result<(), nsresult>;
}

pub struct GetOrCreateTask {
    callback: RefPtr<nsIKeyValueDatabaseCallback>,
    thread: RefPtr<nsIThread>,
    path: nsCString,
    name: nsCString,
}

impl GetOrCreateTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueDatabaseCallback>,
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

impl DatabaseTask for GetOrCreateTask {
    fn run(&self) -> Result<Option<RefPtr<nsIKeyValueDatabase>>, KeyValueError> {
        let mut writer = Manager::singleton().write()?;
        let rkv = writer.get_or_create(Path::new(str::from_utf8(&self.path)?), Rkv::new)?;
        let store = if self.name.is_empty() {
            rkv.write()?.open_or_create_default()
        } else {
            rkv.write()?
                .open_or_create(Some(str::from_utf8(&self.name)?))
        }?;
        let db = KeyValueDatabase::new(rkv, store, Some(self.thread.clone()));

        match db.query_interface::<nsIKeyValueDatabase>() {
            Some(db) => Ok(Some(db)),
            None => Err(KeyValueError::NoInterface("nsISupports")),
        }
    }

    fn done(&self, result: Result<Option<RefPtr<nsIKeyValueDatabase>>, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(Some(value)) => unsafe { self.callback.HandleResult(value.coerce()) },
            Ok(None) => unsafe { self.callback.HandleResult(ptr::null()) },
            Err(err) => unsafe { self.callback.HandleError(&*nsCString::from(err.to_string())) },
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

    fn run(&self) -> Result<(), KeyValueError> {
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

        Ok(())
    }

    fn done(&self, result: Result<(), KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(()) => unsafe { self.callback.HandleResult(ptr::null()) },
            Err(err) => unsafe { self.callback.HandleError(&*nsCString::from(err.to_string())) },
        }.to_result()
    }
}

pub struct HasTask {
    callback: RefPtr<nsIKeyValueVariantCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
}

impl HasTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVariantCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
    ) -> HasTask {
        HasTask {
            callback,
            rkv,
            store,
            key,
        }
    }
}

impl BoolTask for HasTask {
    fn run(&self) -> Result<bool, KeyValueError> {
        let key = str::from_utf8(&self.key)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, key)?;
        Ok(value.is_some())
    }

    fn done(&self, result: Result<bool, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(value) => unsafe { self.callback.HandleResult(value.into_variant().ok_or(KeyValueError::Read)?.take().coerce()) },
            Err(err) => unsafe { self.callback.HandleError(&*nsCString::from(err.to_string())) },
        }.to_result()
    }
}

pub struct GetTask {
    callback: RefPtr<nsIKeyValueVariantCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    default_value: RefPtr<nsIVariant>,
}

impl GetTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVariantCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
        default_value: RefPtr<nsIVariant>,
    ) -> GetTask {
        GetTask {
            callback,
            rkv,
            store,
            key,
            default_value,
        }
    }
}

impl VariantTask for GetTask {
    fn run(&self) -> Result<Option<RefPtr<nsIVariant>>, KeyValueError> {
        let key = str::from_utf8(&self.key)?;
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let value = reader.get(&self.store, key)?;

        match value {
            Some(Value::I64(value)) => Ok(Some(value.into_variant().ok_or(KeyValueError::Read)?.take())),
            Some(Value::F64(value)) => Ok(Some(value.into_variant().ok_or(KeyValueError::Read)?.take())),
            Some(Value::Str(value)) => Ok(Some(nsString::from(value)
                .into_variant()
                .ok_or(KeyValueError::Read)?
                .take())),
            Some(Value::Bool(value)) => Ok(Some(value.into_variant().ok_or(KeyValueError::Read)?.take())),
            Some(_value) => Err(KeyValueError::UnexpectedValue),
            None => Ok(Some(into_variant(&self.default_value)?.take())),
        }
    }

    fn done(&self, result: Result<Option<RefPtr<nsIVariant>>, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(Some(value)) => unsafe { self.callback.HandleResult(value.coerce()) },
            Ok(None) => unsafe { self.callback.HandleResult(ptr::null()) },
            Err(err) => unsafe { self.callback.HandleError(&*nsCString::from(err.to_string())) },
        }.to_result()
    }
}

pub struct DeleteTask {
    callback: RefPtr<nsIKeyValueCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
}

impl DeleteTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
    ) -> DeleteTask {
        DeleteTask {
            callback,
            rkv,
            store,
            key,
        }
    }
}

impl Task for DeleteTask {
    fn run(&self) -> Result<Option<RefPtr<nsISupports>>, KeyValueError> {
        let key = str::from_utf8(&self.key)?;
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

        Ok(None)
    }

    fn done(&self, result: Result<Option<RefPtr<nsISupports>>, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(Some(value)) => unsafe { self.callback.HandleResult(value.coerce()) },
            Ok(None) => unsafe { self.callback.HandleResult(ptr::null()) },
            Err(err) => unsafe { self.callback.HandleError(&*nsCString::from(err.to_string())) },
        }.to_result()
    }
}

pub struct EnumerateTask {
    callback: RefPtr<nsIKeyValueCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    from_key: nsCString,
    to_key: nsCString,
}

impl EnumerateTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        from_key: nsCString,
        to_key: nsCString,
    ) -> EnumerateTask {
        EnumerateTask {
            callback,
            rkv,
            store,
            from_key,
            to_key,
        }
    }
}

impl Task for EnumerateTask {
    fn run(&self) -> Result<Option<RefPtr<nsISupports>>, KeyValueError> {
        let env = self.rkv.read()?;
        let reader = env.read()?;
        let from_key = str::from_utf8(&self.from_key)?;
        let to_key = str::from_utf8(&self.to_key)?;

        let iterator = if from_key.is_empty() {
            reader.iter_start(&self.store)?
        } else {
            reader.iter_from(&self.store, &from_key)?
        };

        // Ideally, we'd enumerate pairs lazily, as the consumer calls
        // nsIKeyValueEnumerator.getNext(), which calls our
        // KeyValueEnumerator.get_next() implementation.  But KeyValueEnumerator
        // can't reference the Iter because Rust "cannot #[derive(xpcom)]
        // on a generic type," and the Iter requires a lifetime parameter,
        // which would make KeyValueEnumerator generic.
        //
        // Our fallback approach is to eagerly collect the iterator
        // into a collection that KeyValueEnumerator owns.  Fixing this so we
        // enumerate pairs lazily is bug 1499252.
        let pairs: Vec<(
            Result<String, KeyValueError>,
            Result<OwnedValue, KeyValueError>,
        )> = iterator
            // Convert the key to a string so we can compare it to the "to" key.
            // For forward compatibility, we don't fail here if we can't convert
            // a key to UTF-8.  Instead, we store the Err in the collection
            // and fail lazily in KeyValueEnumerator.get_next().
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

        let enumerator = KeyValueEnumerator::new(get_current_thread().unwrap(), pairs);

        match enumerator.query_interface::<nsISupports>() {
            Some(supports) => Ok(Some(supports)),
            None => Err(KeyValueError::NoInterface("nsISupports")),
        }
    }

    fn done(&self, result: Result<Option<RefPtr<nsISupports>>, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(Some(value)) => unsafe { self.callback.HandleResult(value.coerce()) },
            Ok(None) => unsafe { self.callback.HandleResult(ptr::null()) },
            Err(err) => unsafe { self.callback.HandleError(&*nsCString::from(err.to_string())) },
        }.to_result()
    }
}

pub struct HasMoreElementsTask {
    callback: RefPtr<nsIKeyValueVariantCallback>,
    iter: Rc<RefCell<IntoIter<(
        Result<String, KeyValueError>,
        Result<OwnedValue, KeyValueError>,
    )>>>,
}

impl HasMoreElementsTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVariantCallback>,
        iter: Rc<RefCell<IntoIter<(
            Result<String, KeyValueError>,
            Result<OwnedValue, KeyValueError>,
        )>>>,
    ) -> HasMoreElementsTask {
        HasMoreElementsTask {
            callback,
            iter,
        }
    }
}

impl BoolTask for HasMoreElementsTask {
    fn run(&self) -> Result<bool, KeyValueError> {
        Ok(!self.iter.borrow().as_slice().is_empty())
    }

    fn done(&self, result: Result<bool, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(value) => unsafe { self.callback.HandleResult(value.into_variant().ok_or(KeyValueError::Read)?.take().coerce()) },
            Err(err) => unsafe { self.callback.HandleError(&*nsCString::from(err.to_string())) },
        }.to_result()
    }
}

pub struct GetNextTask {
    callback: RefPtr<nsIKeyValuePairCallback>,
    iter: Rc<RefCell<IntoIter<(
        Result<String, KeyValueError>,
        Result<OwnedValue, KeyValueError>,
    )>>>,
}

impl GetNextTask {
    pub fn new(
        callback: RefPtr<nsIKeyValuePairCallback>,
        iter: Rc<RefCell<IntoIter<(
            Result<String, KeyValueError>,
            Result<OwnedValue, KeyValueError>,
        )>>>,
    ) -> GetNextTask {
        GetNextTask {
            callback,
            iter,
        }
    }

    fn run(&self) -> Result<(String, OwnedValue), KeyValueError> {
        let mut iter = self.iter.borrow_mut();
        let (key, value) = iter.next().ok_or(KeyValueError::from(NS_ERROR_FAILURE))?;

        // We fail on retrieval of the key/value pair if the key isn't valid
        // UTF-*, if the value is unexpected, or if we encountered a store error
        // while retrieving the pair.
        Ok((key?, value?))
    }

    fn done(&self, result: Result<(String, OwnedValue), KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok((key, value)) => unsafe {
                self.callback.HandleResult(
                    &*nsCString::from(key),
                    value.into_variant().ok_or(KeyValueError::from(NS_ERROR_FAILURE))?.take().coerce()
                )
            },
            Err(err) => unsafe { self.callback.HandleError(&*nsCString::from(err.to_string())) },
        }.to_result()
    }
}

#[derive(xpcom)]
#[xpimplements(nsIRunnable, nsINamed)]
#[refcnt = "atomic"]
pub struct InitGetNextRunnable {
    name: &'static str,
    source: RefPtr<nsIThread>,

    /// Holds the task, and the result of the task.  The task is created
    /// on the current thread, run on a target thread, and handled again
    /// on the original thread; the result is mutated on the target thread
    /// and accessed on the original thread.
    task: Box<GetNextTask>,
    result: Cell<Option<Result<(String, OwnedValue), KeyValueError>>>,
}

impl GetNextRunnable {
    pub fn new(
        name: &'static str,
        source: RefPtr<nsIThread>,
        task: Box<GetNextTask>,
        result: Cell<Option<Result<(String, OwnedValue), KeyValueError>>>,
    ) -> RefPtr<GetNextRunnable> {
        GetNextRunnable::allocate(InitGetNextRunnable {
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

#[derive(xpcom)]
#[xpimplements(nsIRunnable, nsINamed)]
#[refcnt = "atomic"]
pub struct InitPutTaskRunnable {
    name: &'static str,
    source: RefPtr<nsIThread>,

    /// Holds the task, and the result of the task.  The task is created
    /// on the current thread, run on a target thread, and handled again
    /// on the original thread; the result is mutated on the target thread
    /// and accessed on the original thread.
    task: Box<PutTask>,
    result: Cell<Option<Result<(), KeyValueError>>>,
}

impl PutTaskRunnable {
    pub fn new(
        name: &'static str,
        source: RefPtr<nsIThread>,
        task: Box<PutTask>,
        result: Cell<Option<Result<(), KeyValueError>>>,
    ) -> RefPtr<PutTaskRunnable> {
        PutTaskRunnable::allocate(InitPutTaskRunnable {
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

#[derive(xpcom)]
#[xpimplements(nsIRunnable, nsINamed)]
#[refcnt = "atomic"]
pub struct InitDatabaseTaskRunnable {
    name: &'static str,
    source: RefPtr<nsIThread>,

    /// Holds the task, and the result of the task.  The task is created
    /// on the current thread, run on a target thread, and handled again
    /// on the original thread; the result is mutated on the target thread
    /// and accessed on the original thread.
    task: Box<DatabaseTask>,
    result: Cell<Option<Result<Option<RefPtr<nsIKeyValueDatabase>>, KeyValueError>>>,
}

impl DatabaseTaskRunnable {
    pub fn new(
        name: &'static str,
        source: RefPtr<nsIThread>,
        task: Box<DatabaseTask>,
        result: Cell<Option<Result<Option<RefPtr<nsIKeyValueDatabase>>, KeyValueError>>>,
    ) -> RefPtr<DatabaseTaskRunnable> {
        DatabaseTaskRunnable::allocate(InitDatabaseTaskRunnable {
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

#[derive(xpcom)]
#[xpimplements(nsIRunnable, nsINamed)]
#[refcnt = "atomic"]
pub struct InitBoolTaskRunnable {
    name: &'static str,
    source: RefPtr<nsIThread>,

    /// Holds the task, and the result of the task.  The task is created
    /// on the current thread, run on a target thread, and handled again
    /// on the original thread; the result is mutated on the target thread
    /// and accessed on the original thread.
    task: Box<BoolTask>,
    result: Cell<Option<Result<bool, KeyValueError>>>,
}

impl BoolTaskRunnable {
    pub fn new(
        name: &'static str,
        source: RefPtr<nsIThread>,
        task: Box<BoolTask>,
        result: Cell<Option<Result<bool, KeyValueError>>>,
    ) -> RefPtr<BoolTaskRunnable> {
        BoolTaskRunnable::allocate(InitBoolTaskRunnable {
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

#[derive(xpcom)]
#[xpimplements(nsIRunnable, nsINamed)]
#[refcnt = "atomic"]
pub struct InitVariantTaskRunnable {
    name: &'static str,
    source: RefPtr<nsIThread>,

    /// Holds the task, and the result of the task.  The task is created
    /// on the current thread, run on a target thread, and handled again
    /// on the original thread; the result is mutated on the target thread
    /// and accessed on the original thread.
    task: Box<VariantTask>,
    result: Cell<Option<Result<Option<RefPtr<nsIVariant>>, KeyValueError>>>,
}

impl VariantTaskRunnable {
    pub fn new(
        name: &'static str,
        source: RefPtr<nsIThread>,
        task: Box<VariantTask>,
        result: Cell<Option<Result<Option<RefPtr<nsIVariant>>, KeyValueError>>>,
    ) -> RefPtr<VariantTaskRunnable> {
        VariantTaskRunnable::allocate(InitVariantTaskRunnable {
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
