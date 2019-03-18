/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate lmdb;
#[macro_use]
extern crate log;
extern crate nserror;
extern crate nsstring;
extern crate rkv;
extern crate serde_json;
#[macro_use]
extern crate xpcom;

mod error;
mod ffi;
mod iter;
mod store;

use crate::{
    error::{XULStoreError, XULStoreResult},
    iter::XULStoreIterator,
    store::{make_key, RKV, STORE},
};
use lmdb::Error as LmdbError;
use nsstring::nsAString;
use rkv::{StoreError as RkvStoreError, Value};
use std::str;

struct XULStore {}

impl XULStore {
    fn set_value(
        doc: &nsAString,
        id: &nsAString,
        attr: &nsAString,
        value: &nsAString,
    ) -> XULStoreResult<()> {
        debug!("XULStore set value: {} {} {} {}", doc, id, attr, value);

        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard.as_ref().ok_or(XULStoreError::Unavailable)?.read()?;
        let mut writer = rkv.write()?;
        let key = make_key(doc, id, attr);
        let value = String::from_utf16(value)?;

        let store = STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?.clone();
        store.put(&mut writer, &key, &Value::Str(&value))?;
        writer.commit()?;

        Ok(())
    }

    fn has_value(
        doc: &nsAString,
        id: &nsAString,
        attr: &nsAString
    ) -> XULStoreResult<bool> {
        debug!("XULStore has value: {} {} {}", doc, id, attr);

        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard.as_ref().ok_or(XULStoreError::Unavailable)?.read()?;
        let reader = rkv.read()?;
        let key = make_key(doc, id, attr);
        let store = STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?.clone();

        match store.get(&reader, &key) {
            Ok(Some(_)) => Ok(true),

            Ok(None) => Ok(false),

            // For some reason, the first call to STORE.read()?.get/has()
            // when running GTests triggers an LMDB "other" error with code 22,
            // which is described in LMDB docs as EINVAL (BAD COMMAND).
            //
            // TODO: figure out why this happens and fix the actual issue
            // instead of merely working around it.
            Err(RkvStoreError::LmdbError(LmdbError::Other(22))) => {
                error!("XULStore has value error: EINVAL");
                Ok(false)
            }

            Err(err) => Err(err.into()),
        }
    }

    fn get_value(
        doc: &nsAString,
        id: &nsAString,
        attr: &nsAString
    ) -> XULStoreResult<String> {
        debug!("XULStore get value {} {} {}", doc, id, attr);

        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard.as_ref().ok_or(XULStoreError::Unavailable)?.read()?;
        let reader = rkv.read()?;
        let key = make_key(doc, id, attr);
        let store = STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?.clone();

        match store.get(&reader, &key) {
            Ok(Some(Value::Str(val))) => Ok(val.to_owned()),

            // Per the XULStore API, return an empty string if the value
            // isn't found.
            Ok(None) => Ok("".to_owned()),

            // This should never happen, but it could happen in theory
            // if someone writes a different kind of value into the store
            // using a more general API (kvstore, rkv, LMDB).
            Ok(Some(_)) => return Err(XULStoreError::UnexpectedValue),

            // For some reason, the first call to STORE.read()?.get/has()
            // when running GTests triggers an LMDB "other" error with code 22,
            // which is described in LMDB docs as EINVAL (BAD COMMAND).
            //
            // TODO: figure out why this happens and fix the actual issue
            // instead of merely working around it.
            Err(RkvStoreError::LmdbError(LmdbError::Other(22))) => {
                error!("XULStore get value error: EINVAL");
                Ok("".to_owned())
            }

            Err(err) => Err(err.into()),
        }
    }

    fn remove_value(doc: &nsAString, id: &nsAString, attr: &nsAString) -> XULStoreResult<()> {
        debug!("XULStore remove value {} {} {}", doc, id, attr);

        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard.as_ref().ok_or(XULStoreError::Unavailable)?.read()?;
        let mut writer = rkv.write()?;
        let key = make_key(doc, id, attr);
        let store = STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?.clone();

        match store.delete(&mut writer, &key) {
            Ok(_) => {
                writer.commit()?;
                Ok(())
            },

            // The XULStore API doesn't care if a consumer tries to remove
            // a value that doesn't actually exist, so we ignore that error.
            Err(RkvStoreError::LmdbError(LmdbError::NotFound)) => Ok(()),

            Err(err) => Err(err.into()),
        }
    }

    fn get_ids(doc: &nsAString) -> XULStoreResult<XULStoreIterator> {
        debug!("XULStore get IDs for {}", doc);

        let doc_url = String::from_utf16(doc)?;

        let store = STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?.clone();
        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard.as_ref().ok_or(XULStoreError::Unavailable)?.read()?;
        let reader = rkv.read()?;
        let iterator = store.iter_from(&reader, &doc_url)?;

        let mut collection = iterator
            // Convert the key to a string.
            .map(|result| match result {
                Ok((key, _)) => match str::from_utf8(&key) {
                    Ok(key) => Ok(key),
                    Err(err) => Err(err.into()),
                },
                Err(err) => Err(err.into()),
            })
            // Stop iterating once we reach another doc URL or hit an error.
            .take_while(|result| match result {
                Ok(key) => {
                    let parts = key.split('\u{0009}').collect::<Vec<&str>>();
                    parts[0] == doc_url
                }
                Err(_) => true,
            })
            // Extract the element ID from the key.
            .map(|result| match result {
                Ok(key) => {
                    let parts = key.split('\u{0009}').collect::<Vec<&str>>();
                    Ok(parts[1].to_owned())
                }
                Err(err) => Err(err),
            })
            // Collect the results or report an error.
            .collect::<XULStoreResult<Vec<String>>>()?;

        // NB: ideally, we'd dedup while iterating, but IterTools.dedup()
        // requires its Item to be PartialEq, and Err(XULStoreError) isn't.
        collection.dedup();

        Ok(XULStoreIterator::new(collection.into_iter()))
    }

    fn get_attrs(doc: &nsAString, id: &nsAString) -> XULStoreResult<XULStoreIterator> {
        debug!("XULStore get attrs for doc, ID: {} {}", doc, id);

        let doc_url = String::from_utf16(doc)?;
        let element_id = String::from_utf16(id)?;
        let key_prefix = doc_url.to_owned() + "\u{0009}" + &element_id;
        let store = STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?.clone();
        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard.as_ref().ok_or(XULStoreError::Unavailable)?.read()?;
        let reader = rkv.read()?;
        let iterator = store.iter_from(&reader, &key_prefix)?;

        let mut collection: Vec<String> = iterator
            // Convert the key to a string.
            .map(|result| match result {
                Ok((key, _)) => match str::from_utf8(&key) {
                    Ok(key) => Ok(key),
                    Err(err) => Err(err.into()),
                },
                Err(err) => Err(err.into()),
            })
            // Stop iterating once we reach another doc URL or element ID
            // or hit an error.
            .take_while(|result| match result {
                Ok(key) => {
                    let parts = key.split('\u{0009}').collect::<Vec<&str>>();
                    parts[0] == doc_url && parts[1] == element_id
                }
                Err(_) => true,
            })
            // Extract the attribute name from the key.
            .map(|result| match result {
                Ok(key) => {
                    let parts = key.split('\u{0009}').collect::<Vec<&str>>();
                    Ok(parts[2].to_owned())
                }
                Err(err) => Err(err),
            })
            .collect::<XULStoreResult<Vec<String>>>()?;

        // NB: ideally, we'd dedup while iterating, but IterTools.dedup()
        // requires its Item to be PartialEq, and Err(XULStoreError) isn't.
        collection.dedup();

        Ok(XULStoreIterator::new(collection.into_iter()))
    }
}
