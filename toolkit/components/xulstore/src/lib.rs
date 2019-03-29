/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate crossbeam_utils;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate lmdb;
#[macro_use]
extern crate log;
extern crate moz_task;
extern crate nserror;
extern crate nsstring;
extern crate rkv;
extern crate serde_json;
#[macro_use]
extern crate xpcom;

mod error;
mod ffi;
mod iter;
mod statics;

use crate::{
    error::{XULStoreError, XULStoreResult},
    iter::XULStoreIterator,
    statics::{DATA, RKV, STORE},
};
use crossbeam_utils::atomic::AtomicCell;
use lmdb::Error as LmdbError;
use moz_task::{create_thread, Task, TaskRunnable};
use nserror::nsresult;
use nsstring::nsAString;
use rkv::{StoreError as RkvStoreError, Value};
use std::{
    collections::BTreeMap,
};
use xpcom::{interfaces::nsIThread, RefPtr, ThreadBoundRefPtr};

const SEPARATOR: char = '\u{0009}';

pub(crate) fn make_key<T: std::fmt::Display>(doc: &T, id: &T, attr: &T) -> String {
    format!("{}{}{}{}{}", doc, SEPARATOR, id, SEPARATOR, attr)
}

lazy_static! {
    pub static ref THREAD: Option<ThreadBoundRefPtr<nsIThread>> = {
        let thread: RefPtr<nsIThread> = match create_thread("XULStore") {
            Ok(thread) => thread,
            Err(err) => {
                error!("error creating XULStore thread: {}", err);
                return None;
            },
        };

        Some(ThreadBoundRefPtr::new(thread))
    };
}

struct XULStore {}

impl XULStore {
    fn set_value(
        doc: &nsAString,
        id: &nsAString,
        attr: &nsAString,
        value: &nsAString,
    ) -> XULStoreResult<()> {
        debug!("XULStore set value: {} {} {} {}", doc, id, attr, value);

        // bug 319846 -- don't save really long attributes or values.
        if id.len() > 512 || attr.len() > 512 {
          return Err(XULStoreError::IdAttrNameTooLong);
        }

        let key = make_key(doc, id, attr);
        let value = if value.len() > 4096 {
            warn!("XULStore: truncating long attribute value");
            String::from_utf16(&value[0..4096])?
        } else {
            String::from_utf16(value)?
        };

        let mut data_guard = DATA.write()?;
        let data = match data_guard.as_mut() {
            Some(data) => data,
            None => return Ok(()),
        };
        data.entry(doc.to_string()).or_insert(BTreeMap::new())
           .entry(id.to_string()).or_insert(BTreeMap::new())
           .insert(attr.to_string(), value.clone());

        let task = Box::new(SetValueTask::new(key, value));
        let thread = THREAD.as_ref().ok_or(XULStoreError::Unavailable)?.get_ref().ok_or(XULStoreError::Unavailable)?;
        TaskRunnable::new("XULStore::SetValue", task)?.dispatch(thread)?;

        Ok(())
    }

    fn has_value(doc: &nsAString, id: &nsAString, attr: &nsAString) -> XULStoreResult<bool> {
        debug!("XULStore has value: {} {} {}", doc, id, attr);

        let data_guard = DATA.read()?;
        let data = match data_guard.as_ref() {
            Some(data) => data,
            None => return Ok(false),
        };

        match data.get(&doc.to_string()) {
            Some(ids) => {
                match ids.get(&id.to_string()) {
                    Some(attrs) => Ok(attrs.contains_key(&attr.to_string())),
                    None => Ok(false),
                }
            }
            None => Ok(false),
        }
    }

    fn get_value(doc: &nsAString, id: &nsAString, attr: &nsAString) -> XULStoreResult<String> {
        debug!("XULStore get value {} {} {}", doc, id, attr);

        let data_guard = DATA.read()?;
        let data = match data_guard.as_ref() {
            Some(data) => data,
            None => return Ok("".to_owned()),
        };

        match data.get(&doc.to_string()) {
            Some(ids) => {
                match ids.get(&id.to_string()) {
                    Some(attrs) => {
                        match attrs.get(&attr.to_string()) {
                            Some(value) => Ok(value.to_owned()),
                            None => Ok("".to_owned()),
                        }
                    }
                    None => Ok("".to_owned()),
                }
            }
            None => Ok("".to_owned()),
        }
    }

    fn remove_value(doc: &nsAString, id: &nsAString, attr: &nsAString) -> XULStoreResult<()> {
        debug!("XULStore remove value {} {} {}", doc, id, attr);

        let mut data_guard = DATA.write()?;
        let data = match data_guard.as_mut() {
            Some(data) => data,
            None => return Ok(()),
        };

        let mut ids_empty = false;
        match data.get_mut(&doc.to_string()) {
            Some(ids) => {
                let mut attrs_empty = false;
                match ids.get_mut(&id.to_string()) {
                    Some(attrs) => {
                        attrs.remove(&attr.to_string());
                        if attrs.is_empty() {
                            attrs_empty = true;
                        }
                    },
                    None => (),
                }
                if attrs_empty {
                    ids.remove(&id.to_string());
                    if ids.is_empty() {
                        ids_empty = true;
                    }
                }
            }
            None => (),
        };
        if ids_empty {
            data.remove(&doc.to_string());
        }

        let key = make_key(doc, id, attr);
        let task = Box::new(RemoveValueTask::new(key));
        let thread = THREAD.as_ref().ok_or(XULStoreError::Unavailable)?.get_ref().ok_or(XULStoreError::Unavailable)?;
        TaskRunnable::new("XULStore::RemoveValue", task)?.dispatch(thread)?;

        Ok(())
    }

    fn remove_document(doc: &nsAString) -> XULStoreResult<()> {
        debug!("XULStore remove document {}", doc);

        let mut data_guard = DATA.write()?;
        let data = match data_guard.as_mut() {
            Some(data) => data,
            None => return Ok(()),
        };

        let mut keys_to_remove: Vec<String> = Vec::new();
        let doc = doc.to_string();

        // Build a list of keys to remove from the store.
        match data.get(&doc) {
            Some(ids) => {
                for (id, attrs) in ids {
                    for (attr, _value) in attrs {
                        keys_to_remove.push(make_key(&doc, id, attr));
                    }
                }
            }
            None => (),
        };

        // We can remove the document from the data cache in one fell swoop.
        data.remove(&doc.to_string());

        let task = Box::new(RemoveDocumentTask::new(keys_to_remove));
        let thread = THREAD.as_ref().ok_or(XULStoreError::Unavailable)?.get_ref().ok_or(XULStoreError::Unavailable)?;
        TaskRunnable::new("XULStore::RemoveDocument", task)?.dispatch(thread)?;

        Ok(())
    }

    fn get_ids(doc: &nsAString) -> XULStoreResult<XULStoreIterator> {
        debug!("XULStore get IDs for {}", doc);

        let data_guard = DATA.read()?;
        let data = match data_guard.as_ref() {
            Some(data) => data,
            None => return Ok(XULStoreIterator::new(vec![].into_iter())),
        };

        match data.get(&doc.to_string()) {
            Some(ids) => {
                let mut ids: Vec<String> = ids.keys()
                .map(|id| id.to_owned())
                .collect();
                // TODO: rather than sorting here, use a pre-sorted
                // data structure, such as a BTreeMap, so the items
                // are already in sorted order.
                ids.sort();
                Ok(XULStoreIterator::new(ids.into_iter()))
            },
            None => Ok(XULStoreIterator::new(vec![].into_iter())),
        }
    }

    fn get_attrs(doc: &nsAString, id: &nsAString) -> XULStoreResult<XULStoreIterator> {
        debug!("XULStore get attrs for doc, ID: {} {}", doc, id);

        let data_guard = DATA.read()?;
        let data = match data_guard.as_ref() {
            Some(data) => data,
            None => return Ok(XULStoreIterator::new(vec![].into_iter())),
        };

        match data.get(&doc.to_string()) {
            Some(ids) => {
                match ids.get(&id.to_string()) {
                    Some(attrs) => {
                        let mut attrs: Vec<String> = attrs.keys().map(|attr| attr.to_owned()).collect();
                        // TODO: rather than sorting here, use a pre-sorted
                        // data structure, such as a BTreeMap, so the items
                        // are already in sorted order.
                        attrs.sort();
                        Ok(XULStoreIterator::new(attrs.into_iter()))
                    },
                    None => Ok(XULStoreIterator::new(vec![].into_iter())),
                }
            },
            None => Ok(XULStoreIterator::new(vec![].into_iter())),
        }
    }
}

pub struct SetValueTask {
    key: String,
    value: String,
    result: AtomicCell<Option<Result<(), XULStoreError>>>,
}

impl SetValueTask {
    pub fn new(
        key: String,
        value: String,
    ) -> SetValueTask {
        SetValueTask {
            key,
            value,
            result: AtomicCell::default(),
        }
    }
}

impl Task for SetValueTask {
    fn run(&self) {
        self.result.store(Some(|| -> Result<(), XULStoreError> {
            let rkv_guard = RKV.read()?;
            let rkv = rkv_guard
                .as_ref()
                .ok_or(XULStoreError::Unavailable)?
                .read()?;
            let mut writer = rkv.write()?;
            let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;
            store.put(&mut writer, &self.key, &Value::Str(&self.value))?;
            writer.commit()?;

            Ok(())
        }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.swap(None) {
            // TODO: error! -> info!
            Some(Ok(())) => { error!("setValue succeeded")},
            Some(Err(err)) => error!("setValue error: {}", err),
            None => error!("setValue error: unexpected result"),
        };

        Ok(())
    }
}

pub struct RemoveValueTask {
    key: String,
    result: AtomicCell<Option<Result<(), XULStoreError>>>,
}

impl RemoveValueTask {
    pub fn new(
        key: String,
    ) -> RemoveValueTask {
        RemoveValueTask {
            key,
            result: AtomicCell::default(),
        }
    }
}

impl Task for RemoveValueTask {
    fn run(&self) {
        self.result.store(Some(|| -> Result<(), XULStoreError> {
            let rkv_guard = RKV.read()?;
            let rkv = rkv_guard
                .as_ref()
                .ok_or(XULStoreError::Unavailable)?
                .read()?;
            let mut writer = rkv.write()?;
            let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;

            match store.delete(&mut writer, &self.key) {
                Ok(_) => {
                    writer.commit()?;

                    Ok(())
                }

                // The XULStore API doesn't care if a consumer tries to remove
                // a value that doesn't actually exist, so we ignore that error.
                Err(RkvStoreError::LmdbError(LmdbError::NotFound)) => Ok(()),

                Err(err) => Err(err.into()),
            }
        }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.swap(None) {
            // TODO: error! -> info!
            Some(Ok(())) => { error!("removeValue succeeded")},
            Some(Err(err)) => error!("removeValue error: {}", err),
            None => error!("removeValue error: unexpected result"),
        };

        Ok(())
    }
}

pub struct RemoveDocumentTask {
    keys_to_remove: Vec<String>,
    result: AtomicCell<Option<Result<(), XULStoreError>>>,
}

impl RemoveDocumentTask {
    pub fn new(
        keys_to_remove: Vec<String>,
    ) -> RemoveDocumentTask {
        RemoveDocumentTask {
            keys_to_remove,
            result: AtomicCell::default(),
        }
    }
}

impl Task for RemoveDocumentTask {
    fn run(&self) {
        self.result.store(Some(|| -> Result<(), XULStoreError> {
            let rkv_guard = RKV.read()?;
            let rkv = rkv_guard
                .as_ref()
                .ok_or(XULStoreError::Unavailable)?
                .read()?;
            let mut writer = rkv.write()?;
            let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;

            // Removing the document from the store requires iterating the keys
            // to remove.
            self.keys_to_remove.iter().map(|key|
                match store.delete(&mut writer, &key) {
                    Ok(_) => Ok(()),

                    // The XULStore API doesn't care if a consumer tries to remove
                    // a value that doesn't actually exist, so we ignore that error,
                    // although in this case the key should exist since it was in
                    // the cache!
                    // TODO: warn if a key doesn't exist.
                    Err(RkvStoreError::LmdbError(LmdbError::NotFound)) => Ok(()),

                    Err(err) => Err(err.into()),
                }
            ).collect::<Result<Vec<()>, XULStoreError>>()?;

            writer.commit()?;

            Ok(())
        }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.swap(None) {
            // TODO: error! -> info!
            Some(Ok(())) => { error!("removeDocument succeeded")},
            Some(Err(err)) => error!("removeDocument error: {}", err),
            None => error!("removeDocument error: unexpected result"),
        };

        Ok(())
    }
}
