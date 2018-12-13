/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate xpcom;

use crossbeam_utils::atomic::AtomicCell;
use error::KeyValueError;
use moz_task::{get_main_thread, is_main_thread};
use nserror::{nsresult, NsresultExt, NS_ERROR_FAILURE, NS_OK};
use nsstring::{nsACString, nsCString, nsString};
use owned_value::{value_to_owned, OwnedValue};
use rkv::{Manager, Rkv, Store, StoreError, Value};
use std::{
    cell::Cell,
    path::Path,
    str,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};
use storage_variant::VariantType;
use threadbound::ThreadBound;
use xpcom::{
    interfaces::{
        nsIEventTarget, nsIKeyValueDatabaseCallback, nsIKeyValueEnumeratorCallback,
        nsIKeyValueVariantCallback, nsIKeyValueVoidCallback, nsIThread, nsIVariant,
    },
    RefPtr,
};
use KeyValueDatabase;
use KeyValueEnumerator;

/// A macro to generate a done() implementation for a Task.
/// Takes one argument that specifies the type of the Task's callback function:
///   value: a callback function that takes a value
///   void: the callback function doesn't take a value
macro_rules! task_done {
    (value) => {
        fn done(&self) -> Result<(), nsresult> {
            // If TaskRunnable.run() calls Task.done() to return a result
            // on the main thread before TaskRunnable.run() returns on the database
            // thread, then the Task will get dropped on the database thread.
            //
            // But the callback is an nsXPCWrappedJS that isn't safe to release
            // on the database thread.  So we move it out of the Task here to ensure
            // it gets released on the main thread.
            let callback = self.callback.get_ref().ok_or(NS_ERROR_FAILURE)?.swap(None).ok_or(NS_ERROR_FAILURE)?;

            match self.result.swap(None) {
                Some(Ok(value)) => unsafe { callback.Resolve(value.coerce()) },
                Some(Err(err)) => unsafe { callback.Reject(&*nsCString::from(err.to_string())) },
                None => unsafe { callback.Reject(&*nsCString::from("unexpected")) },
            }.to_result()
        }
    };

    (void) => {
        fn done(&self) -> Result<(), nsresult> {
            // If TaskRunnable.run() calls Task.done() to return a result
            // on the main thread before TaskRunnable.run() returns on the database
            // thread, then the Task will get dropped on the database thread.
            //
            // But the callback is an nsXPCWrappedJS that isn't safe to release
            // on the database thread.  So we move it out of the Task here to ensure
            // it gets released on the main thread.
            let callback = self.callback.get_ref().ok_or(NS_ERROR_FAILURE)?.swap(None).ok_or(NS_ERROR_FAILURE)?;

            match self.result.swap(None) {
                Some(Ok(())) => unsafe { callback.Resolve() },
                Some(Err(err)) => unsafe { callback.Reject(&*nsCString::from(err.to_string())) },
                None => unsafe { callback.Reject(&*nsCString::from("unexpected")) },
            }.to_result()
        }
    };
}

/// A database operation that is executed asynchronously on a database thread
/// and returns its result to the original thread from which it was dispatched.
pub trait Task {
    fn run(&self);
    fn done(&self) -> Result<(), nsresult>;
}

/// The struct responsible for dispatching a Task by calling its run() method
/// on the target thread and returning its result by calling its done() method
/// on the original thread.
///
/// The struct uses its has_run field to determine whether it should call
/// run() or done().  It could instead check if task.result is Some or None,
/// but if run() failed to set task.result, then it would loop infinitely.
#[derive(xpcom)]
#[xpimplements(nsIRunnable, nsINamed)]
#[refcnt = "atomic"]
pub struct InitTaskRunnable {
    name: &'static str,
    task: Box<Task>,
    has_run: AtomicBool,
}

impl TaskRunnable {
    pub fn new(name: &'static str, task: Box<Task>) -> Result<RefPtr<TaskRunnable>, nsresult> {
        debug_assert!(is_main_thread());
        Ok(TaskRunnable::allocate(InitTaskRunnable {
            name,
            task,
            has_run: AtomicBool::new(false),
        }))
    }
    pub fn dispatch(&self, target_thread: RefPtr<nsIThread>) -> Result<(), nsresult> {
        unsafe {
            target_thread.DispatchFromScript(self.coerce(), nsIEventTarget::DISPATCH_NORMAL as u32)
        }.to_result()
    }

    xpcom_method!(Run, run, {});
    fn run(&self) -> Result<(), nsresult> {
        match self.has_run.load(Ordering::Acquire) {
            false => {
                debug_assert!(!is_main_thread());
                self.has_run.store(true, Ordering::Release);
                self.task.run();
                self.dispatch(get_main_thread()?)
            }
            true => {
                debug_assert!(is_main_thread());
                self.task.done()
            }
        }
    }

    xpcom_method!(GetName, get_name, {}, *mut nsACString);
    fn get_name(&self) -> Result<nsCString, nsresult> {
        Ok(nsCString::from(self.name))
    }
}

pub struct GetOrCreateTask {
    callback: ThreadBound<AtomicCell<Option<RefPtr<nsIKeyValueDatabaseCallback>>>>,
    thread: RefPtr<nsIThread>,
    path: nsCString,
    name: nsCString,
    result: AtomicCell<Option<Result<RefPtr<KeyValueDatabase>, KeyValueError>>>,
}

impl GetOrCreateTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueDatabaseCallback>,
        thread: RefPtr<nsIThread>,
        path: nsCString,
        name: nsCString,
    ) -> GetOrCreateTask {
        GetOrCreateTask {
            callback: ThreadBound::new(AtomicCell::new(Some(callback))),
            thread,
            path,
            name,
            result: AtomicCell::default(),
        }
    }
}

impl Task for GetOrCreateTask {
    fn run(&self) {
        // We do the work within a closure that returns a Result so we can
        // use the ? operator to simplify the implementation.
        self.result.store(Some(
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

    task_done!(value);
}

pub struct PutTask {
    callback: ThreadBound<AtomicCell<Option<RefPtr<nsIKeyValueVoidCallback>>>>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    value: OwnedValue,
    result: AtomicCell<Option<Result<(), KeyValueError>>>,
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
            callback: ThreadBound::new(AtomicCell::new(Some(callback))),
            rkv,
            store,
            key,
            value,
            result: AtomicCell::default(),
        }
    }
}

impl Task for PutTask {
    fn run(&self) {
        // We do the work within a closure that returns a Result so we can
        // use the ? operator to simplify the implementation.
        self.result.store(Some(|| -> Result<(), KeyValueError> {
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

    task_done!(void);
}

pub struct GetTask {
    callback: ThreadBound<AtomicCell<Option<RefPtr<nsIKeyValueVariantCallback>>>>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    default_value: Option<OwnedValue>,
    result: AtomicCell<Option<Result<RefPtr<nsIVariant>, KeyValueError>>>,
}

impl GetTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVariantCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
        default_value: Option<OwnedValue>,
    ) -> GetTask {
        GetTask {
            callback: ThreadBound::new(AtomicCell::new(Some(callback))),
            rkv,
            store,
            key,
            default_value,
            result: AtomicCell::default(),
        }
    }
}

impl Task for GetTask {
    fn run(&self) {
        // We do the work within a closure that returns a Result so we can
        // use the ? operator to simplify the implementation.
        self.result
            .store(Some(|| -> Result<RefPtr<nsIVariant>, KeyValueError> {
                let key = str::from_utf8(&self.key)?;
                let env = self.rkv.read()?;
                let reader = env.read()?;
                let value = reader.get(&self.store, key)?;

                Ok(if let Some(value) = value {
                    match value {
                        Value::I64(value) => value.into_variant(),
                        Value::F64(value) => value.into_variant(),
                        Value::Str(value) => nsString::from(value).into_variant(),
                        Value::Bool(value) => value.into_variant(),
                        _ => return Err(KeyValueError::UnexpectedValue),
                    }
                } else {
                    match self.default_value {
                        Some(OwnedValue::Bool(value)) => value.into_variant(),
                        Some(OwnedValue::I64(value)) => value.into_variant(),
                        Some(OwnedValue::F64(value)) => value.into_variant(),
                        Some(OwnedValue::Str(ref value)) => nsString::from(value).into_variant(),
                        None => ().into_variant(),
                    }
                })
            }()));
    }

    task_done!(value);
}

pub struct HasTask {
    callback: ThreadBound<AtomicCell<Option<RefPtr<nsIKeyValueVariantCallback>>>>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    result: AtomicCell<Option<Result<RefPtr<nsIVariant>, KeyValueError>>>,
}

impl HasTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVariantCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
    ) -> HasTask {
        HasTask {
            callback: ThreadBound::new(AtomicCell::new(Some(callback))),
            rkv,
            store,
            key,
            result: AtomicCell::default(),
        }
    }
}

impl Task for HasTask {
    fn run(&self) {
        // We do the work within a closure that returns a Result so we can
        // use the ? operator to simplify the implementation.
        self.result
            .store(Some(|| -> Result<RefPtr<nsIVariant>, KeyValueError> {
                let key = str::from_utf8(&self.key)?;
                let env = self.rkv.read()?;
                let reader = env.read()?;
                let value = reader.get(&self.store, key)?;
                Ok(value.is_some().into_variant())
            }()));
    }

    task_done!(value);
}

pub struct DeleteTask {
    callback: ThreadBound<AtomicCell<Option<RefPtr<nsIKeyValueVoidCallback>>>>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    key: nsCString,
    result: AtomicCell<Option<Result<(), KeyValueError>>>,
}

impl DeleteTask {
    pub fn new(
        callback: RefPtr<nsIKeyValueVoidCallback>,
        rkv: Arc<RwLock<Rkv>>,
        store: Store,
        key: nsCString,
    ) -> DeleteTask {
        DeleteTask {
            callback: ThreadBound::new(AtomicCell::new(Some(callback))),
            rkv,
            store,
            key,
            result: AtomicCell::default(),
        }
    }
}

impl Task for DeleteTask {
    fn run(&self) {
        // We do the work within a closure that returns a Result so we can
        // use the ? operator to simplify the implementation.
        self.result.store(Some(|| -> Result<(), KeyValueError> {
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

    task_done!(void);
}

pub struct EnumerateTask {
    callback: ThreadBound<AtomicCell<Option<RefPtr<nsIKeyValueEnumeratorCallback>>>>,
    rkv: Arc<RwLock<Rkv>>,
    store: Store,
    from_key: nsCString,
    to_key: nsCString,
    result: AtomicCell<Option<Result<RefPtr<KeyValueEnumerator>, KeyValueError>>>,
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
            callback: ThreadBound::new(AtomicCell::new(Some(callback))),
            rkv,
            store,
            from_key,
            to_key,
            result: AtomicCell::default(),
        }
    }
}

impl Task for EnumerateTask {
    fn run(&self) {
        // We do the work within a closure that returns a Result so we can
        // use the ? operator to simplify the implementation.
        self.result.store(Some(
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

                Ok(KeyValueEnumerator::new(pairs))
            }(),
        ));
    }

    task_done!(value);
}
