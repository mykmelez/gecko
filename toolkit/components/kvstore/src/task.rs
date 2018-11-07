/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate xpcom;

use data_type::{
    DATA_TYPE_BOOL, DATA_TYPE_DOUBLE, DATA_TYPE_EMPTY, DATA_TYPE_INT32, DATA_TYPE_VOID,
    DATA_TYPE_WSTRING,
};
use error::KeyValueError;
use libc::{int32_t, uint16_t};
use nserror::{nsresult, NsresultExt, NS_ERROR_FAILURE, NS_OK};
use nsstring::{nsACString, nsCString, nsString};
use owned_value::{value_to_owned, OwnedValue};
use rkv::{Manager, Rkv, Store, StoreError, Value};
use std::{
    cell::Cell,
    cell::RefCell,
    path::Path,
    ptr,
    rc::Rc,
    str,
    sync::{Arc, RwLock},
    vec::IntoIter,
};
use storage_variant::{IntoVariant, Variant};
use xpcom::{
    getter_addrefs,
    interfaces::{
        nsIEventTarget, nsIKeyValueDatabaseCallback,
        nsIKeyValueEnumeratorCallback, nsIKeyValuePairCallback, nsIKeyValueVariantCallback,
        nsIKeyValueVoidCallback, nsIRunnable, nsIThread, nsIVariant,
    },
    RefPtr,
};
use KeyValueDatabase;
use KeyValueEnumerator;
use KeyValuePair;

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

// Perhaps we should convert this to an implementation of the IntoVariant trait
// in storage/variant/src/lib.rs, although currently it only has one consumer.
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

pub trait Task {
    fn run(&self);
    fn done(&self) -> Result<(), nsresult>;
}

#[derive(xpcom)]
#[xpimplements(nsIRunnable, nsINamed)]
#[refcnt = "atomic"]
pub struct InitTaskRunnable {
    name: &'static str,
    origin: RefPtr<nsIThread>,

    /// Holds the task, and the result of the task.  The task is created
    /// on the current thread, run on a target thread, and handled again
    /// on the original thread; the result is mutated on the target thread
    /// and accessed on the original thread.
    task: Box<Task>,

    has_run: Cell<bool>,
}

impl TaskRunnable {
    pub fn new(
        name: &'static str,
        origin: RefPtr<nsIThread>,
        task: Box<Task>,
    ) -> RefPtr<TaskRunnable> {
        TaskRunnable::allocate(InitTaskRunnable {
            name,
            origin,
            task,
            has_run: Cell::new(false),
        })
    }

    xpcom_method!(Run, run, {});
    fn run(&self) -> Result<(), nsresult> {
        match self.has_run.take() {
            false => {
                self.task.run();
                self.has_run.set(true);
                let target = getter_addrefs(|p| unsafe { self.origin.GetEventTarget(p) })?;
                unsafe {
                    target.DispatchFromScript(self.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
                }.to_result()
            }
            true => self.task.done(),
        }
    }

    xpcom_method!(GetName, get_name, {}, *mut nsACString);
    fn get_name(&self) -> Result<nsCString, nsresult> {
        Ok(nsCString::from(self.name))
    }
}

pub struct GetOrCreateTask {
    callback: RefPtr<nsIKeyValueDatabaseCallback>,
    thread: RefPtr<nsIThread>,
    path: nsCString,
    name: nsCString,
    result: Cell<Option<Result<RefPtr<KeyValueDatabase>, KeyValueError>>>,
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
            result: Cell::default(),
        }
    }
}

impl Task for GetOrCreateTask {
    fn run(&self) {
        self.result.set(Some(
            || -> Result<RefPtr<KeyValueDatabase>, KeyValueError> {
                let mut writer = Manager::singleton().write()?;
                let rkv = writer.get_or_create(Path::new(str::from_utf8(&self.path)?), Rkv::new)?;
                let store = if self.name.is_empty() {
                    rkv.write()?.open_or_create_default()
                } else {
                    rkv.write()?
                        .open_or_create(Some(str::from_utf8(&self.name)?))
                }?;
                Ok(KeyValueDatabase::new(rkv, store, self.thread.clone()))
            }(),
        ));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.take() {
            Some(Ok(value)) => unsafe { self.callback.Resolve(value.coerce()) },
            Some(Err(err)) => unsafe {
                self.callback
                    .Reject(&*nsCString::from(err.to_string()))
            },
            None => unsafe { self.callback.Reject(&*nsCString::from("unexpected")) },
        }.to_result()
    }
}

pub struct PutTask {
    callback: RefPtr<nsIKeyValueVoidCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    value: OwnedValue,
    result: Cell<Option<Result<(), KeyValueError>>>,
}

impl PutTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVoidCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
        value: OwnedValue,
    ) -> PutTask {
        PutTask {
            callback,
            rkv,
            store,
            key,
            value,
            result: Cell::default(),
        }
    }
}

impl Task for PutTask {
    fn run(&self) {
        self.result.set(Some(|| -> Result<(), KeyValueError> {
            let key = str::from_utf8(&self.key)?;
            let env = self.rkv.read()?;
            let mut writer = env.write()?;

            let value = match self.value {
                OwnedValue::Bool(val) => Value::Bool(val),
                OwnedValue::I64(val) => Value::I64(val),
                OwnedValue::F64(val) => Value::F64(val),
                OwnedValue::Str(ref val) => Value::Str(&val),
            };

            writer.put(&self.store, key, &value)?;
            writer.commit()?;

            Ok(())
        }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.take() {
            Some(Ok(())) => unsafe { self.callback.Resolve() },
            Some(Err(err)) => unsafe {
                self.callback
                    .Reject(&*nsCString::from(err.to_string()))
            },
            None => unsafe { self.callback.Reject(&*nsCString::from("unexpected")) },
        }.to_result()
    }
}

pub struct HasTask {
    callback: RefPtr<nsIKeyValueVariantCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    result: Cell<Option<Result<bool, KeyValueError>>>,
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
            result: Cell::default(),
        }
    }
}

impl Task for HasTask {
    fn run(&self) {
        self.result.set(Some(|| -> Result<bool, KeyValueError> {
            let key = str::from_utf8(&self.key)?;
            let env = self.rkv.read()?;
            let reader = env.read()?;
            let value = reader.get(&self.store, key)?;
            Ok(value.is_some())
        }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.take() {
            Some(Ok(value)) => unsafe {
                self.callback.Resolve(
                    value
                        .into_variant()
                        .ok_or(KeyValueError::Read)?
                        .take()
                        .coerce(),
                )
            },
            Some(Err(err)) => unsafe {
                self.callback
                    .Reject(&*nsCString::from(err.to_string()))
            },
            None => unsafe { self.callback.Reject(&*nsCString::from("unexpected")) },
        }.to_result()
    }
}

pub struct GetTask {
    callback: RefPtr<nsIKeyValueVariantCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    default_value: RefPtr<nsIVariant>,
    result: Cell<Option<Result<Variant, KeyValueError>>>,
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
            result: Cell::default(),
        }
    }
}

impl Task for GetTask {
    fn run(&self) {
        self.result
            .set(Some(|| -> Result<Variant, KeyValueError> {
                let key = str::from_utf8(&self.key)?;
                let env = self.rkv.read()?;
                let reader = env.read()?;
                let value = reader.get(&self.store, key)?;

                match value {
                    Some(Value::I64(value)) => {
                        Ok(value.into_variant().ok_or(KeyValueError::Read)?)
                    }
                    Some(Value::F64(value)) => {
                        Ok(value.into_variant().ok_or(KeyValueError::Read)?)
                    }
                    Some(Value::Str(value)) => Ok(nsString::from(value)
                        .into_variant()
                        .ok_or(KeyValueError::Read)?),
                    Some(Value::Bool(value)) => {
                        Ok(value.into_variant().ok_or(KeyValueError::Read)?)
                    }
                    Some(_value) => Err(KeyValueError::UnexpectedValue),
                    None => Ok(into_variant(&self.default_value)?),
                }
            }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.take() {
            Some(Ok(value)) => unsafe { self.callback.Resolve(value.take().coerce()) },
            Some(Err(err)) => unsafe {
                self.callback
                    .Reject(&*nsCString::from(err.to_string()))
            },
            None => unsafe { self.callback.Reject(&*nsCString::from("unexpected")) },
        }.to_result()
    }
}

pub struct EnumerateTask {
    callback: RefPtr<nsIKeyValueEnumeratorCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    from_key: nsCString,
    to_key: nsCString,
    result: Cell<Option<Result<RefPtr<KeyValueEnumerator>, KeyValueError>>>,
}

impl EnumerateTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueEnumeratorCallback>,
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
            result: Cell::default(),
        }
    }
}

impl Task for EnumerateTask {
    fn run(&self) {
        self.result.set(Some(
            || -> Result<RefPtr<KeyValueEnumerator>, KeyValueError> {
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
                            match val {
                                Ok(val) => value_to_owned(val),
                                Err(err) => Err(KeyValueError::StoreError(err)),
                            },
                        )
                    }).collect();

                Ok(KeyValueEnumerator::new(get_current_thread()?, pairs))
            }(),
        ));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.take() {
            Some(Ok(value)) => unsafe { self.callback.Resolve(value.coerce()) },
            Some(Err(err)) => unsafe {
                self.callback
                    .Reject(&*nsCString::from(err.to_string()))
            },
            None => unsafe { self.callback.Reject(&*nsCString::from("unexpected")) },
        }.to_result()
    }
}

pub struct HasMoreElementsTask {
    callback: RefPtr<nsIKeyValueVariantCallback>,
    iter: Rc<
        RefCell<
            IntoIter<(
                Result<String, KeyValueError>,
                Result<OwnedValue, KeyValueError>,
            )>,
        >,
    >,
    result: Cell<Option<Result<bool, KeyValueError>>>,
}

impl HasMoreElementsTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVariantCallback>,
        iter: Rc<
            RefCell<
                IntoIter<(
                    Result<String, KeyValueError>,
                    Result<OwnedValue, KeyValueError>,
                )>,
            >,
        >,
    ) -> HasMoreElementsTask {
        HasMoreElementsTask {
            callback,
            iter,
            result: Cell::default(),
        }
    }
}

impl Task for HasMoreElementsTask {
    fn run(&self) {
        self.result.set(Some(|| -> Result<bool, KeyValueError> {
            Ok(!self.iter.borrow().as_slice().is_empty())
        }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.take() {
            Some(Ok(value)) => unsafe {
                self.callback.Resolve(
                    value
                        .into_variant()
                        .ok_or(KeyValueError::Read)?
                        .take()
                        .coerce(),
                )
            },
            Some(Err(err)) => unsafe {
                self.callback
                    .Reject(&*nsCString::from(err.to_string()))
            },
            None => unsafe { self.callback.Reject(&*nsCString::from("unexpected")) },
        }.to_result()
    }
}

pub struct GetNextTask {
    callback: RefPtr<nsIKeyValuePairCallback>,
    iter: Rc<
        RefCell<
            IntoIter<(
                Result<String, KeyValueError>,
                Result<OwnedValue, KeyValueError>,
            )>,
        >,
    >,
    result: Cell<Option<Result<RefPtr<KeyValuePair>, KeyValueError>>>,
}

impl GetNextTask {
    pub fn new(
        callback: RefPtr<nsIKeyValuePairCallback>,
        iter: Rc<
            RefCell<
                IntoIter<(
                    Result<String, KeyValueError>,
                    Result<OwnedValue, KeyValueError>,
                )>,
            >,
        >,
    ) -> GetNextTask {
        GetNextTask {
            callback,
            iter,
            result: Cell::default(),
        }
    }
}

impl Task for GetNextTask {
    fn run(&self) {
        self.result
            .set(Some(|| -> Result<RefPtr<KeyValuePair>, KeyValueError> {
                let mut iter = self.iter.borrow_mut();
                let (key, value) = iter.next().ok_or(KeyValueError::from(NS_ERROR_FAILURE))?;

                // We fail on retrieval of the key/value pair if the key isn't valid
                // UTF-*, if the value is unexpected, or if we encountered a store error
                // while retrieving the pair.
                Ok(KeyValuePair::new(key?, value?))
            }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.take() {
            Some(Ok(value)) => unsafe { self.callback.Resolve(value.coerce()) },
            Some(Err(err)) => unsafe {
                self.callback
                    .Reject(&*nsCString::from(err.to_string()))
            },
            None => unsafe { self.callback.Reject(&*nsCString::from("unexpected")) },
        }.to_result()
    }
}

pub struct DeleteTask {
    callback: RefPtr<nsIKeyValueVoidCallback>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    result: Cell<Option<Result<(), KeyValueError>>>,
}

impl DeleteTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVoidCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
    ) -> DeleteTask {
        DeleteTask {
            callback,
            rkv,
            store,
            key,
            result: Cell::default(),
        }
    }
}

impl Task for DeleteTask {
    fn run(&self) {
        self.result.set(Some(|| -> Result<(), KeyValueError> {
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

            Ok(())
        }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.take() {
            Some(Ok(())) => unsafe { self.callback.Resolve() },
            Some(Err(err)) => unsafe {
                self.callback
                    .Reject(&*nsCString::from(err.to_string()))
            },
            None => unsafe { self.callback.Reject(&*nsCString::from("unexpected")) },
        }.to_result()
    }
}
