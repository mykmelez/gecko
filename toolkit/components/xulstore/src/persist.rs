/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::{
    error::{XULStoreError, XULStoreResult},
    statics::{get_database, THREAD},
};
use crossbeam_utils::atomic::AtomicCell;
use lmdb::Error as LmdbError;
use moz_task::{Task, TaskRunnable};
use nserror::nsresult;
use rkv::{StoreError as RkvStoreError, Value};
use std::{collections::BTreeMap, mem::replace, sync::Mutex};

lazy_static! {
    static ref WRITES: Mutex<Option<BTreeMap<String, Option<String>>>> = { Mutex::new(None) };
}

pub(crate) fn persist(key: String, value: Option<String>) -> XULStoreResult<()> {
    let mut writes = WRITES.lock()?;

    if writes.is_none() {
        *writes = Some(BTreeMap::new());

        let task = Box::new(PersistTask::new());
        let thread = THREAD
            .as_ref()
            .ok_or(XULStoreError::Unavailable)?
            .get_ref()
            .ok_or(XULStoreError::Unavailable)?;
        TaskRunnable::new("XULStore::Persist", task)?.dispatch(thread)?;
    }

    writes.as_mut().unwrap().insert(key, value);

    Ok(())
}

pub struct PersistTask {
    result: AtomicCell<Option<Result<(), XULStoreError>>>,
}

impl PersistTask {
    pub fn new() -> PersistTask {
        PersistTask {
            result: AtomicCell::default(),
        }
    }
}

impl Task for PersistTask {
    fn run(&self) {
        self.result.store(Some(|| -> Result<(), XULStoreError> {
            let db = get_database()?;
            let mut writer = db.env.write()?;

            // Get the map of key/value pairs from the mutex, replacing it
            // with None.
            let writes = replace(&mut *WRITES.lock()?, None);

            // The Option should be a Some(BTreeMap) (otherwise the task
            // shouldn't have been scheduled in the first place).  If it's None,
            // then we return an early Err result.
            let writes = writes.ok_or(XULStoreError::Unavailable)?;

            for (key, value) in writes.iter() {
                match value {
                    Some(val) => db.store.put(&mut writer, &key, &Value::Str(val))?,
                    None => {
                        match db.store.delete(&mut writer, &key) {
                            Ok(_) => (),

                            // The XULStore API doesn't care if a consumer tries
                            // to remove a value that doesn't exist in the store,
                            // so we ignore the error (although in this case the key
                            // should exist, since it was in the cache!).
                            Err(RkvStoreError::LmdbError(LmdbError::NotFound)) => {
                                warn!("tried to remove key that isn't in the store");
                                ()
                            }

                            Err(err) => return Err(err.into()),
                        }
                    }
                }
            }

            writer.commit()?;

            Ok(())
        }()));
    }

    fn done(&self) -> Result<(), nsresult> {
        match self.result.swap(None) {
            Some(Ok(())) => (),
            Some(Err(err)) => error!("removeDocument error: {}", err),
            None => error!("removeDocument error: unexpected result"),
        };

        Ok(())
    }
}
