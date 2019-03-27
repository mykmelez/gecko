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
    collections::BTreeMap,
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

        // bug 319846 -- don't save really long attributes or values.
        if id.len() > 512 || attr.len() > 512 {
          return Err(XULStoreError::IdAttrNameTooLong);
        }

        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard
            .as_ref()
            .ok_or(XULStoreError::Unavailable)?
            .read()?;
        let mut writer = rkv.write()?;
        let key = make_key(doc, id, attr);
        let value = if value.len() > 4096 {
            warn!("XULStore: truncating long attribute value");
            String::from_utf16(&value[0..4096])?
        } else {
            String::from_utf16(value)?
        };

        let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;
        store.put(&mut writer, &key, &Value::Str(&value))?;
        writer.commit()?;

        let mut data_guard = DATA.write()?;
        let data = data_guard.as_mut().ok_or(XULStoreError::Unavailable)?;
        data.entry(doc.to_string()).or_insert(BTreeMap::new())
           .entry(id.to_string()).or_insert(BTreeMap::new())
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

                Ok(())
            }


            // The XULStore API doesn't care if a consumer tries to remove
            // a value that doesn't actually exist, so we ignore that error.
            Err(RkvStoreError::LmdbError(LmdbError::NotFound)) => Ok(()),

            Err(err) => Err(err.into()),
        }
    }

    fn remove_document(doc: &nsAString) -> XULStoreResult<()> {
        debug!("XULStore remove document {}", doc);

        let mut data_guard = DATA.write()?;
        let data = data_guard.as_mut().ok_or(XULStoreError::Unavailable)?;
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

        let rkv_guard = RKV.read()?;
        let rkv = rkv_guard
            .as_ref()
            .ok_or(XULStoreError::Unavailable)?
            .read()?;
        let mut writer = rkv.write()?;
        let store = *STORE.read()?.as_ref().ok_or(XULStoreError::Unavailable)?;

        // Removing the document from the store requires iterating the keys
        // to remove.
        keys_to_remove.iter().map(|key|
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
    }

    fn get_ids(doc: &nsAString) -> XULStoreResult<XULStoreIterator> {
        debug!("XULStore get IDs for {}", doc);

        let data_guard = DATA.read()?;
        let data = data_guard.as_ref().ok_or(XULStoreError::Unavailable)?;

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
        let data = data_guard.as_ref().ok_or(XULStoreError::Unavailable)?;

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
