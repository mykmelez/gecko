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
mod statics;

use crate::{
    error::{XULStoreError, XULStoreResult},
    iter::XULStoreIterator,
    statics::{DATA, RKV, STORE},
};
use lmdb::Error as LmdbError;
use nsstring::nsAString;
use rkv::{StoreError as RkvStoreError, Value};
use std::{
    collections::HashMap,
    str,
};

const SEPARATOR: char = '\u{0009}';

pub(crate) fn make_key<T: std::fmt::Display>(doc: &T, id: &T, attr: &T) -> String {
    format!("{}{}{}{}{}", doc, SEPARATOR, id, SEPARATOR, attr)
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

        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard
            .as_ref()
            .ok_or(XULStoreError::Unavailable)?
            .read()?;
        let mut writer = rkv.write()?;
        let key = make_key(doc, id, attr);
        let value = String::from_utf16(value)?;

        let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;
        store.put(&mut writer, &key, &Value::Str(&value))?;
        writer.commit()?;

        let mut data_guard = DATA.write()?;
        let data = data_guard.as_mut().ok_or(XULStoreError::Unavailable)?;
        data.entry(doc.to_string()).or_insert(HashMap::new())
           .entry(id.to_string()).or_insert(HashMap::new())
           .insert(attr.to_string(), value);

        Ok(())
    }

    fn has_value(doc: &nsAString, id: &nsAString, attr: &nsAString) -> XULStoreResult<bool> {
        debug!("XULStore has value: {} {} {}", doc, id, attr);

        let data_guard = DATA.read()?;
        let data = data_guard.as_ref().ok_or(XULStoreError::Unavailable)?;

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
        let data = data_guard.as_ref().ok_or(XULStoreError::Unavailable)?;
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

        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard
            .as_ref()
            .ok_or(XULStoreError::Unavailable)?
            .read()?;
        let mut writer = rkv.write()?;
        let key = make_key(doc, id, attr);
        let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;

        match store.delete(&mut writer, &key) {
            Ok(_) => {
                writer.commit()?;

                let mut data_guard = DATA.write()?;
                let data = data_guard.as_mut().ok_or(XULStoreError::Unavailable)?;
                match data.get_mut(&doc.to_string()) {
                    Some(ids) => {
                        match ids.get_mut(&id.to_string()) {
                            Some(attrs) => { attrs.remove(&attr.to_string()); },
                            None => (),
                        }
                    }
                    None => (),
                };

                Ok(())
            }

            // The XULStore API doesn't care if a consumer tries to remove
            // a value that doesn't actually exist, so we ignore that error.
            Err(RkvStoreError::LmdbError(LmdbError::NotFound)) => Ok(()),

            Err(err) => Err(err.into()),
        }
    }

    fn get_ids(doc: &nsAString) -> XULStoreResult<XULStoreIterator> {
        debug!("XULStore get IDs for {}", doc);

        let doc_url = String::from_utf16(doc)?;

        let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;
        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard
            .as_ref()
            .ok_or(XULStoreError::Unavailable)?
            .read()?;
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
                    let parts = key.split(SEPARATOR).collect::<Vec<&str>>();
                    parts[0] == doc_url
                }
                Err(_) => true,
            })
            // Extract the element ID from the key.
            .map(|result| match result {
                Ok(key) => {
                    let parts = key.split(SEPARATOR).collect::<Vec<&str>>();
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
        let key_prefix = format!("{}{}{}", doc_url, SEPARATOR, element_id);
        let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;
        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard
            .as_ref()
            .ok_or(XULStoreError::Unavailable)?
            .read()?;
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
                    let parts = key.split(SEPARATOR).collect::<Vec<&str>>();
                    parts[0] == doc_url && parts[1] == element_id
                }
                Err(_) => true,
            })
            // Extract the attribute name from the key.
            .map(|result| match result {
                Ok(key) => {
                    let parts = key.split(SEPARATOR).collect::<Vec<&str>>();
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
