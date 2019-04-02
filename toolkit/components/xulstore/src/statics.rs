/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::{
    error::XULStoreError, error::XULStoreResult, ffi::ProfileChangeObserver, make_key, SEPARATOR,
};
use moz_task::create_thread;
use nsstring::nsString;
use rkv::{Manager, Rkv, SingleStore, StoreOptions, Value};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{create_dir_all, remove_file, File},
    ops::DerefMut,
    path::PathBuf,
    str,
    sync::{Arc, RwLock},
};
use xpcom::{interfaces::{nsIFile, nsIThread}, RefPtr, ThreadBoundRefPtr, XpCom};

type XULStoreData = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;

lazy_static! {
    pub(crate) static ref PROFILE_DIR: RwLock<Option<PathBuf>> = {
        observe_profile_change();
        RwLock::new(get_profile_dir().ok())
    };

    #[derive(Debug)]
    pub(crate) static ref DATABASE: RwLock<Option<Database>> = {
        RwLock::new(get_database().ok())
    };

    pub(crate) static ref CACHE: RwLock<Option<XULStoreData>> = {
        RwLock::new(get_data().ok())
    };

    pub(crate) static ref THREAD: Option<ThreadBoundRefPtr<nsIThread>> = {
        let thread: RefPtr<nsIThread> = match create_thread("XULStore") {
            Ok(thread) => thread,
            Err(err) => {
                error!("error creating XULStore thread: {}", err);
                return None;
            }
        };

        Some(ThreadBoundRefPtr::new(thread))
    };
}

// Memoized to the PROFILE_DIR lazy static. Prefer that accessor to calling
// this function, to avoid extra trips across the XPCOM FFI.
//
// NB: this code must be kept in sync with the code that updates the store's
// location in toolkit/components/xulstore/XULStore.jsm.
pub(crate) fn get_profile_dir() -> XULStoreResult<PathBuf> {
    let dir_svc = xpcom::services::get_DirectoryService().ok_or(XULStoreError::Unavailable)?;
    let mut profile_dir = xpcom::GetterAddrefs::<nsIFile>::new();
    let property = CString::new("ProfD")?;
    unsafe {
        dir_svc.Get(property.as_ptr(), &nsIFile::IID, profile_dir.void_ptr());
    }
    let profile_dir = profile_dir.refptr().ok_or(XULStoreError::Unavailable)?;

    let mut profile_path = nsString::new();
    unsafe {
        profile_dir.GetPath(profile_path.deref_mut());
    }

    let path = String::from_utf16(&profile_path[..])?;
    Ok(PathBuf::from(&path))
}

fn get_xulstore_dir() -> XULStoreResult<PathBuf> {
    let mut xulstore_dir = PROFILE_DIR
        .read()?
        .as_ref()
        .ok_or(XULStoreError::Unavailable)?
        .clone();
    xulstore_dir.push("xulstore");
    info!("get XULStore dir: {:?}", &xulstore_dir);

    create_dir_all(xulstore_dir.clone())?;

    Ok(xulstore_dir)
}

pub(crate) struct Database {
    pub env: Arc<RwLock<Rkv>>,
    pub store: SingleStore,
}

impl Database {
    fn new(env: Arc<RwLock<Rkv>>, store: SingleStore) -> Database {
        Database { env, store }
    }
}

pub(crate) fn get_database() -> XULStoreResult<Database> {
    let env = get_rkv()?;
    let store = get_store(env.clone())?;
    Ok(Database::new(env, store))
}

pub(crate) fn get_rkv() -> XULStoreResult<Arc<RwLock<Rkv>>> {
    let mut manager = Manager::singleton().write()?;
    let xulstore_dir = get_xulstore_dir()?;
    manager
        .get_or_create(xulstore_dir.as_path(), Rkv::new)
        .map_err(|err| err.into())
}

pub(crate) fn get_store(env: Arc<RwLock<Rkv>>) -> XULStoreResult<SingleStore> {
    match env.read()?.open_single("db", StoreOptions::create()) {
        Ok(store) => {
            maybe_migrate_data(env.clone(), store);
            Ok(store)
        },
        Err(err) => Err(err.into()),
    }
}

fn maybe_migrate_data(env: Arc<RwLock<Rkv>>, store: SingleStore) {
    // Failure to migrate data isn't fatal, so we don't return a result.
    // But we use a closure returning a result to enable use of the ? operator.
    (|| -> XULStoreResult<()> {
        let mut old_datastore = PROFILE_DIR
            .read()?
            .as_ref()
            .ok_or(XULStoreError::Unavailable)?
            .clone();
        old_datastore.push("xulstore.json");
        if !old_datastore.exists() {
            debug!("old datastore doesn't exist: {:?}", old_datastore);
            return Ok(());
        }

        let file = File::open(old_datastore.clone())?;
        let json: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>> =
            serde_json::from_reader(file)?;

        let env = env.read()?;
        let mut writer = env.write()?;

        for (doc, ids) in json {
            for (id, attrs) in ids {
                for (attr, value) in attrs {
                    let key = make_key(&doc, &id, &attr);
                    store.put(&mut writer, &key, &Value::Str(&value))?;
                }
            }
        }

        writer.commit()?;

        remove_file(old_datastore)?;

        Ok(())
    })()
    .unwrap_or_else(|err| error!("error migrating data: {}", err));
}

fn observe_profile_change() {
    // Failure to observe the change isn't fatal (although it means we won't
    // persist XULStore data for this session), so we don't return a result.
    // But we use a closure returning a result to enable use of the ? operator.
    (|| -> XULStoreResult<()> {
        // Observe profile changes so we can update this directory accordingly.
        let obs_svc = xpcom::services::get_ObserverService().ok_or(XULStoreError::Unavailable)?;
        let observer = ProfileChangeObserver::new();
        let topic = CString::new("profile-after-change")?;
        unsafe {
            obs_svc
                .AddObserver(observer.coerce(), topic.as_ptr(), false)
                .to_result()?
        };
        Ok(())
    })()
    .unwrap_or_else(|err| error!("error observing profile change: {}", err));
}

pub(crate) fn update_profile_dir() {
    // Failure to update the dir isn't fatal (although it means that we won't
    // persist XULStore data for this session), so we don't return a result.
    // But we use a closure returning a result to enable use of the ? operator.
    (|| -> XULStoreResult<()> {
        {
            let mut profile_dir_guard = PROFILE_DIR.write()?;
            *profile_dir_guard = get_profile_dir().ok();
        }

        {
            let mut db_guard = DATABASE.write()?;
            // TODO: ensure we drop the old environment before we create
            // the new one.
            *db_guard = get_database().ok();
        }

        let mut data_guard = CACHE.write()?;
        *data_guard = get_data().ok();

        Ok(())
    })()
    .unwrap_or_else(|err| error!("error updating profile dir: {}", err));
}

fn unwrap_value(value: &Option<Value>) -> XULStoreResult<String> {
    match value {
        Some(Value::Str(val)) => Ok(val.to_string()),

        // Per the XULStore API, return an empty string if the value
        // isn't found.
        None => Ok("".to_owned()),

        // This should never happen, but it could happen in theory
        // if someone writes a different kind of value into the store
        // using a more general API (kvstore, rkv, LMDB).
        Some(_) => Err(XULStoreError::UnexpectedValue),
    }
}

fn get_data() -> XULStoreResult<XULStoreData> {
    let db_guard = DATABASE.read()?;
    let db = db_guard
        .as_ref()
        .ok_or(XULStoreError::Unavailable)?;
    let env = db.env.read()?;
    let reader = env.read()?;
    let mut all = BTreeMap::new();
    let iterator = db.store.iter_start(&reader)?;

    for result in iterator {
        let (key, value): (&str, String) = match result {
            Ok((key, value)) => {
                assert!(value.is_some(), "iterated key has value");
                match (str::from_utf8(&key), unwrap_value(&value)) {
                    (Ok(key), Ok(value)) => (key, value),
                    (Err(err), _) => return Err(err.into()),
                    (_, Err(err)) => return Err(err),
                }
            }
            Err(err) => return Err(err.into()),
        };

        let parts = key.split(SEPARATOR).collect::<Vec<&str>>();
        if parts.len() != 3 {
            return Err(XULStoreError::UnexpectedKey(key.to_owned()));
        }
        let (doc, id, attr) = (
            parts[0].to_owned(),
            parts[1].to_owned(),
            parts[2].to_owned(),
        );

        all.entry(doc)
            .or_insert_with(BTreeMap::new)
            .entry(id)
            .or_insert_with(BTreeMap::new)
            .entry(attr)
            .or_insert(value);
    }

    Ok(all)
}
