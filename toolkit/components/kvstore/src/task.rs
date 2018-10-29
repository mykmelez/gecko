/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#![allow(non_snake_case)]

extern crate xpcom;

use error::KeyValueError;
use nserror::{nsresult, NsresultExt, NS_OK};
use nsstring::{nsACString, nsCString};
use rkv::{Manager, Rkv};
use std::{cell::Cell, path::Path, ptr, str};
use xpcom::{
    getter_addrefs,
    interfaces::{nsIEventTarget, nsIKeyValueCallback, nsIRunnable, nsISupports, nsIThread},
    RefPtr,
};
use KeyValueDatabase;

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
    fn run(&self) -> Result<RefPtr<nsISupports>, KeyValueError>;
    fn done(&self, result: Result<RefPtr<nsISupports>, KeyValueError>) -> Result<(), nsresult>;
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
    fn run(&self) -> Result<RefPtr<nsISupports>, KeyValueError> {
        let mut writer = Manager::singleton().write()?;
        let rkv = writer.get_or_create(Path::new(str::from_utf8(&self.path)?), Rkv::new)?;
        let store = if self.name.is_empty() {
            rkv.write()?.open_or_create_default()
        } else {
            rkv.write()?
                .open_or_create(Some(str::from_utf8(&self.name)?))
        }?;
        let key_value_db = KeyValueDatabase::new(rkv, store, Some(self.thread.clone()));

        key_value_db
            .query_interface::<nsISupports>()
            .ok_or(KeyValueError::NoInterface("nsISupports"))
    }

    fn done(&self, result: Result<RefPtr<nsISupports>, KeyValueError>) -> Result<(), nsresult> {
        match result {
            Ok(value) => unsafe { self.callback.HandleResult(value.coerce()) },
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
    result: Cell<Option<Result<RefPtr<nsISupports>, KeyValueError>>>,
}

impl TaskRunnable {
    pub fn new(
        name: &'static str,
        source: RefPtr<nsIThread>,
        task: Box<Task>,
        result: Cell<Option<Result<RefPtr<nsISupports>, KeyValueError>>>,
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
