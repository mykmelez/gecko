/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A proof-of-concept XULStore implementation that uses rkv to store data.
//! This PoC translates the XULStore data model into rkv concepts by mapping:
//!
//!   * XUL document -> rkv store
//!   * (element ID, attribute name) -> rkv key
//!   * attribute value -> rkv value
//!
//! The PoC creates an rkv store (LMDB database) for each unique XUL document
//! and specifies rkv keys as tuples of element ID and attribute name (serialized
//! to a string concatenation with an equals sign separator, i.e. ID=name).
//!
//! Note that the maximum number of rkv stores needs to be known during
//! initialization of the rkv singleton (LMDB environment).  Also note that
//! "a moderate number of slots are cheap but a huge number gets expensive"
//! <http://www.lmdb.tech/doc/group__mdb.html#gaa2fc2f1f37cb1115e733b62cab2fcdbc>.
//!
//! Since extensions no longer store data using XULStore, and Firefox itself
//! only stores data for a small number of XUL documents, these shouldn't be
//! issues in practice.  Firefox can set MAX_STORES to the number of XUL docs
//! for which it'll ever store values, and migration from the old JSON store
//! to a new rkv store can drop values stored by legacy extensions.
//!
//! However, a complete implementation might choose the alternative of mapping
//! documents to rkv keys in a single rkv store and store element/attribute/value
//! triples for a given document as a single rkv value, using JSON or the like
//! to structure and serialize the data to a blob that can be stored as a value.
//!
//! The API for this crate comprises two sets of C ABI functions, one of which
//! uses raw pointers to char arrays to receive and return strings, the other
//! of which uses Gecko's nsstring crate, which provides Rust implementations
//! of common Gecko string types.
//!
//! These sets model two possible implementation strategies, the first of which
//! is more generic and can be accessed from code that doesn't support the Gecko
//! string types, such as JS (using js-ctypes) or Java/Kotlin/Swift (on mobile);
//! the second of which is more specific to Gecko consumers on desktop and may be
//! safer, faster, more ergonomic, or more efficient.
//!
//! A complete implementation of XULStore will presumably use the nsstring-based
//! API, with a C++ XPCOM interface abstraction for both C++ and JS consumers.

extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate lmdb;
extern crate nserror;
extern crate nsstring;
extern crate rkv;
extern crate tempdir;
// Will need #[macro_use] if we ever implement XPCOM interfaces.
extern crate xpcom;

use itertools::Itertools;
use nserror::{nsresult, NS_OK};
use nsstring::{nsAString, nsString};
use rkv::{Manager, Rkv, StoreError, Value};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::{c_char, c_uint};
use std::path::Path;
use std::str;
use std::sync::{Arc, RwLock};
use xpcom::{interfaces, XpCom};

// NB: this should be set to the maximum number of documents for which Firefox
// persists (element, attribute) values using XULStore.
static MAX_STORES: c_uint = 10;

lazy_static! {
    #[derive(Debug)]
    static ref RKV: Arc<RwLock<Rkv>> = {
        // Get the profile directory path.
        let dir_svc = xpcom::services::get_DirectoryService().unwrap();
        let property = CString::new("ProfD").unwrap();
        let mut profile_dir = xpcom::GetterAddrefs::<interfaces::nsIFile>::new();
        unsafe {
            dir_svc.Get(property.as_ptr(), &interfaces::nsIFile::IID, profile_dir.void_ptr());
        }

        // Convert the profile directory path to a Path.
        //
        // This might be easier if we could access nsIFile::NativePath,
        // but the generated file nsIFile.rs declares:
        // "Unable to generate binding because `nostdcall is unsupported`"
        //
        let mut profile_dir_path = nsString::new();
        unsafe {
            // TODO: ensure the directory service returns a valid pointer
            // to an nsIFile instance.
            profile_dir.refptr().unwrap().GetPath(&mut profile_dir_path);
        }
        let profile_dir_path = String::from_utf16_lossy(&profile_dir_path[..]);
        let profile_dir_path = Path::new(&profile_dir_path);

        let xulstore_dir_path = profile_dir_path.join("xulstore");
        fs::create_dir_all(xulstore_dir_path.clone()).expect("dir created");
        println!("xulstore directory: {:?}", &xulstore_dir_path);

        // NB: this singleton is tied to the profile directory it retrieves
        // during initialization, which can change if the application changes
        // profiles without restarting.
        //
        // It isn't clear that there's any way to initiate such a profile
        // change anymore, but in any case the nsIXULStore implementation
        // ignores writes after receiving a profile-after-change notification,
        // so it isn't necessary to support that here, assuming consumers
        // always access this store via that interface.
        //
        // If they don't, or if we want to support writes (and reads from,
        // the correct profile dir, for that matter), then we could listen
        // for profile-after-change ourselves, as demonstrated in
        // xpcom/rust/gtest/xpcom/test.rs, and reinitialize the singleton
        // with the new profile directory.

        // Rkv::with_capacity(&xulstore_dir_path, MAX_STORES).expect("new succeeded")
        let mut manager = Manager::singleton().write().unwrap();
        manager.get_or_create_with_capacity(xulstore_dir_path.as_path(), MAX_STORES, Rkv::with_capacity).expect("Rkv")
    };
}

// See XULStore.cpp for an explanation of this function.
#[no_mangle]
pub extern "C" fn xulstore_function_marked_used() {}

fn get_store(store_name: &str) -> rkv::Store<&str> {
    // NB: an implementation might cache and reuse open stores.

    // NB: an implementation that migrates data from the legacy JSON store
    // might check for the existence of the store in rkv and migrate data
    // before opening the store if it doesn't exist yet.

    RKV.read().unwrap().create_or_open(Some(store_name)).expect("store")
}

fn get_key(id: &str, attr: &str) -> String {
    id.to_owned() + "=" + attr
}

#[no_mangle]
pub extern "C" fn xulstore_set_value_ns(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
    value: &nsAString,
) -> nsresult {
    let rkv = RKV.read().unwrap();
    let store_name = String::from_utf16_lossy(doc);
    let store = get_store(store_name.as_str());
    let key = get_key(
        &String::from_utf16_lossy(id),
        &String::from_utf16_lossy(attr),
    );
    let mut writer = store.write(&rkv).expect("writer");

    // TODO: store (and retrieve) values as blobs instead of converting them
    // to Value::Str (and back).
    // TODO: handle errors by returning NS_ERROR_FAILURE or another nsresult.
    writer
        .put(&key, &Value::Str(&String::from_utf16_lossy(value)))
        .expect("put");
    writer.commit().expect("commit");

    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_set_value_c(
    doc: *const c_char,
    id: *const c_char,
    attr: *const c_char,
    value: *const c_char,
) -> nsresult {
    assert!(!doc.is_null());
    assert!(!id.is_null());
    assert!(!attr.is_null());
    assert!(!value.is_null());

    let rkv = RKV.read().unwrap();
    let store_name = unsafe { CStr::from_ptr(doc) };
    let store = get_store(store_name.to_str().unwrap());
    let key = get_key(
        unsafe { CStr::from_ptr(id) }.to_str().unwrap(),
        unsafe { CStr::from_ptr(attr) }.to_str().unwrap(),
    );
    let mut writer = store.write(&rkv).expect("writer");
    // TODO: store (and retrieve) values as blobs instead of converting them
    // to Value::Str (and back).
    let val = Value::Str(unsafe { CStr::from_ptr(value) }.to_str().unwrap());

    // TODO: handle errors by returning NS_ERROR_FAILURE or another nsresult.
    writer.put(&key, &val).expect("put");
    writer.commit().expect("commit");

    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_has_value_ns(doc: &nsAString, id: &nsAString, attr: &nsAString) -> bool {
    let rkv = RKV.read().unwrap();
    let store_name = String::from_utf16_lossy(doc);
    let store = get_store(store_name.as_str());
    let key = get_key(
        &String::from_utf16_lossy(id),
        &String::from_utf16_lossy(attr),
    );
    let reader = store.read(&rkv).expect("reader");
    let value = reader.get(&key);

    // TODO: distinguish between a value not found and an error retrieving it.
    match value {
        Ok(None) => false,
        Err(_) => false,
        _ => true,
    }
}

#[no_mangle]
pub extern "C" fn xulstore_has_value_c(
    doc: *const c_char,
    id: *const c_char,
    attr: *const c_char,
) -> bool {
    assert!(!doc.is_null());
    assert!(!id.is_null());
    assert!(!attr.is_null());

    let rkv = RKV.read().unwrap();
    let store_name = unsafe { CStr::from_ptr(doc) };
    let store = get_store(store_name.to_str().unwrap());
    let key = get_key(
        unsafe { CStr::from_ptr(id) }.to_str().unwrap(),
        unsafe { CStr::from_ptr(attr) }.to_str().unwrap(),
    );
    let reader = store.read(&rkv).expect("reader");
    let value = reader.get(&key);

    // TODO: distinguish between a value not found and an error retrieving it.
    match value {
        Ok(None) => false,
        Err(_) => false,
        _ => true,
    }
}

#[no_mangle]
pub extern "C" fn xulstore_get_value_ns(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
    value: *mut nsAString,
) -> nsresult {
    let rkv = RKV.read().unwrap();
    let store_name = String::from_utf16_lossy(doc);
    let store = get_store(store_name.as_str());
    let key = get_key(
        &String::from_utf16_lossy(id),
        &String::from_utf16_lossy(attr),
    );
    let reader = store.read(&rkv).expect("reader");

    let retrieved_value = reader.get(&key);

    // TODO: distinguish between a value not found and an error retrieving it.
    // For the former, continue to return an empty string, per the XULStore API.
    // For the latter, return an NS_ERROR_FAILURE or another nsresult.
    let return_value = match retrieved_value {
        Ok(Some(Value::Str(value))) => value,
        Err(_) => "",
        _ => "",
    };

    unsafe {
        (*value).assign(&nsString::from(return_value));
    };

    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_get_value_c(
    doc: *const c_char,
    id: *const c_char,
    attr: *const c_char,
) -> *const c_char {
    assert!(!doc.is_null());
    assert!(!id.is_null());
    assert!(!attr.is_null());

    let rkv = RKV.read().unwrap();
    let store_name = unsafe { CStr::from_ptr(doc) };
    let store = get_store(store_name.to_str().unwrap());
    let key = get_key(
        unsafe { CStr::from_ptr(id) }.to_str().unwrap(),
        unsafe { CStr::from_ptr(attr) }.to_str().unwrap(),
    );
    let reader = store.read(&rkv).expect("reader");
    let retrieved_value = reader.get(&key);

    let return_value = match retrieved_value {
        Ok(Some(Value::Str(value))) => value,
        // TODO: report error instead of merely swallowing it.
        Err(_) => "",
        _ => "",
    };

    CString::new(return_value).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn xulstore_drop_value_c(str: *mut c_char) {
    if str.is_null() {
        // Implicitly calls drop when the CString goes out of scope.
        unsafe { CString::from_raw(str) };
    }
}

#[no_mangle]
pub extern "C" fn xulstore_remove_value_ns(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
) -> nsresult {
    let rkv = RKV.read().unwrap();
    let store_name = String::from_utf16_lossy(doc);
    let store = get_store(store_name.as_str());
    let key = get_key(
        &String::from_utf16_lossy(id),
        &String::from_utf16_lossy(attr),
    );
    let mut writer = store.write(&rkv).expect("writer");

    // TODO: handle errors by returning NS_ERROR_FAILURE or another nsresult.
    match writer.delete(&key) {
        Ok(ok) => Ok(ok),
        // The XULStore API doesn't care if a consumer tries to remove a value
        // that doesn't actually exist, so we ignore that error.
        Err(StoreError::LmdbError(lmdb::Error::NotFound)) => Ok(()),
        Err(err) => Err(err),
    }.expect("delete");

    // NB: an implementation might want to remove the store if it has removed
    // the last value from it.

    writer.commit().expect("commit");

    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_remove_value_c(
    doc: *const c_char,
    id: *const c_char,
    attr: *const c_char,
) -> nsresult {
    assert!(!doc.is_null());
    assert!(!id.is_null());
    assert!(!attr.is_null());

    let rkv = RKV.read().unwrap();
    let store_name = unsafe { CStr::from_ptr(doc) };
    let store = get_store(store_name.to_str().unwrap());
    let key = get_key(
        unsafe { CStr::from_ptr(id) }.to_str().unwrap(),
        unsafe { CStr::from_ptr(attr) }.to_str().unwrap(),
    );
    let mut writer = store.write(&rkv).expect("writer");

    match writer.delete(&key) {
        // The XULStore API doesn't care if a consumer tries to remove a value
        // that doesn't actually exist, so we ignore that error.
        Err(StoreError::LmdbError(lmdb::Error::NotFound)) => Ok(()),
        Ok(ok) => Ok(ok),
        Err(err) => Err(err),
    }.expect("delete");

    writer.commit().expect("commit");

    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_get_ids_iterator_ns(doc: &nsAString) -> *const StringIterator {
    let rkv = RKV.read().unwrap();
    let store_name = String::from_utf16_lossy(doc);
    let store = get_store(store_name.as_str());
    let reader = store.read(&rkv).expect("reader");
    let iterator = reader.iter_start().expect("iter");

    let collection: Vec<String> = iterator
        .map(|(key, _val)| key)
        // TODO: avoid assuming we control writes and check the conversion.
        .map(|v| unsafe { str::from_utf8_unchecked(&v) })
        .map(|v| v.split_at(v.find('=').unwrap()))
        .map(|(id, _attr)| id.to_owned())
        // TODO: unique() collects values, and collect() does too,
        // so do so only once, by collecting the values into a set.
        .unique()
        .collect();

    Box::into_raw(Box::new(StringIterator::new(collection)))
}

#[no_mangle]
pub extern "C" fn xulstore_get_ids_iterator_c<'a>(doc: *const c_char) -> *const StringIterator {
    assert!(!doc.is_null());

    let rkv = RKV.read().unwrap();
    let store_name = unsafe { CStr::from_ptr(doc) };
    let store = get_store(store_name.to_str().unwrap());
    let reader = store.read(&rkv).expect("reader");
    let iterator = reader.iter_start().expect("iter");

    let collection: Vec<String> = iterator
        .map(|(key, _val)| key)
        // TODO: avoid assuming we control writes and check the conversion.
        .map(|v| unsafe { str::from_utf8_unchecked(&v) })
        .map(|v| v.split_at(v.find('=').unwrap()))
        .map(|(id, _attr)| id.to_owned())
        // TODO: unique() collects values, and collect() does too,
        // so do so only once, by collecting the values into a set.
        .unique()
        .collect();

    Box::into_raw(Box::new(StringIterator::new(collection)))
}

#[no_mangle]
pub extern "C" fn xulstore_get_attribute_iterator_ns<'a>(
    doc: &nsAString,
    id: &nsAString,
) -> *const StringIterator {
    let rkv = RKV.read().unwrap();
    let store_name = String::from_utf16_lossy(doc);
    let element_id = String::from_utf16_lossy(id);
    let store = get_store(store_name.as_str());
    let reader = store.read(&rkv).expect("reader");
    let iterator = reader.iter_start().expect("iter");

    let collection: Vec<String> = iterator
        .map(|(key, _val)| key)
        // TODO: avoid assuming we control writes and check the conversion.
        .map(|v| unsafe { str::from_utf8_unchecked(&v) })
        .map(|v| v.split_at(v.find('=').unwrap()))
        .filter(|&(id, _attr)| id == element_id)
        .map(|(_id, attr)| attr[1..].to_owned()) // slice off the leading equals sign
        .unique()
        .collect();

    Box::into_raw(Box::new(StringIterator::new(collection)))
}

#[no_mangle]
pub extern "C" fn xulstore_get_attribute_iterator_c<'a>(
    doc: *const c_char,
    id: *const c_char,
) -> *const StringIterator {
    assert!(!doc.is_null());
    assert!(!id.is_null());

    let rkv = RKV.read().unwrap();
    let store_name = unsafe { CStr::from_ptr(doc) };
    let element_id = unsafe { CStr::from_ptr(id) }.to_str().unwrap();
    let store = get_store(store_name.to_str().unwrap());
    let reader = store.read(&rkv).expect("reader");
    let iterator = reader.iter_from(element_id).expect("iter");

    let collection: Vec<String> = iterator
        // .map(|(key, val)| {
        //     println!("key {:?} = val {:?}", unsafe { str::from_utf8_unchecked(&key) },
        //                                     unsafe { str::from_utf8_unchecked(&val) });
        //     (key, val)
        // })
        .map(|(key, _val)| key)
        // TODO: avoid assuming we control writes and check the conversion.
        .map(|v| unsafe { str::from_utf8_unchecked(&v) })
        .map(|v| v.split_at(v.find('=').unwrap()))
        .filter(|&(id, _attr)| id == element_id)
        .map(|(_id, attr)| attr[1..].to_owned()) // slice off the leading equals sign
        .unique()
        .collect();

    Box::into_raw(Box::new(StringIterator::new(collection)))
}

// In theory, we should be able to implement iteration as nsIStringEnumerator.
// But we need to specify a lifetime parameter on the RoCursor and Iter,
// which runs afoul of the "Cannot #[derive(xpcom)] on a generic type" error
// in the xpcom procedural macro definition
// <https://searchfox.org/mozilla-central/rev/68fdb6c/xpcom/rust/xpcom/xpcom_macros/src/lib.rs#501-505>.
//
// #[macro_use]
// extern crate xpcom;
//
// #[no_mangle]
// pub extern "C" fn xulstore_get_ids_enumerator(doc: &nsAString, ids: *mut *const interfaces::nsIStringEnumerator) -> nsresult {
//     let rkv = RKV.read().unwrap();
//     let store_name = String::from_utf16_lossy(doc);
//     let store: Store<&'static str> = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
//     let reader = store.read(&rkv).expect("reader");
//     let cursor = reader.open_cursor();
//     let iterator = cursor.iter().peekable();
//     println!("{:?}", cursor);
//     let enumerator = ImplStringEnumerator::allocate(InitImplStringEnumerator {
//         iterator: iterator,
//     });
//     unsafe {
//         enumerator.query_interface::<interfaces::nsIStringEnumerator>().unwrap().forget(&mut *ids);
//     }
//     NS_OK
// }
// #[derive(xpcom)]
// #[xpimplements(nsIStringEnumerator)]
// #[refcnt = "atomic"]
// struct InitImplStringEnumerator<'a> {
//     iterator: std::iter::Peekable<std::result::Iter<'a, lmdb::RoCursor<'a>>>,
// }
// impl ImplStringEnumerator {
//     #![allow(non_snake_case)]
//     pub fn HasMore(&self, has_more: *mut bool) -> nsresult {
//         unsafe {
//             *has_more = false;
//         }
//         NS_OK
//     }
//     pub fn GetNext(&self, next_element: *mut nsAString) -> nsresult {
//         unsafe {
//             (*next_element).assign(&nsString::from(""))
//         }
//         NS_OK
//     }
// }

// Another option would be to define a StringIterator struct that encapsulates
// an LMDB cursor, with functions for iterating it.  Unfortunately, that has
// the issue that Rust doesn't support a Struct with fields that reference
// each other, and in this case the struct would need to reference both
// the cursor, which references its reader, and the reader itself, in order
// in order to keep the reader alive as long as the cursor.
//
// <https://stackoverflow.com/questions/32300132/why-cant-i-store-a-value-and-a-reference-to-that-value-in-the-same-struct>
//
// We might be able to work around that in this case with the rental crate.
//
// pub struct StringIterator<'a> {
//     reader: Box<Reader<'a, &'static str>>,
//     cursor: Box<lmdb::RoCursor<'a>>,
// }
// impl<'a> StringReader<'a> {
//     pub fn new(reader: Box<Reader<'a, &'static str>>) -> StringIterator<'a> {
//         Self {
//             reader: reader,
//         }
//     }
// }
// impl<'a> StringIterator<'a> {
//     pub fn new(reader: Box<Reader<'a, &'static str>>) -> StringIterator<'a> {
//         let cursor = Box::new(reader.open_cursor().expect("cursor"));
//         iter = Self {
//             reader: reader,
//             cursor: cursor,
//         }
//     }
// }

// A third option is to pre-collect the values into a StringIterator struct
// and iterator methods that take a raw pointer to the struct.  This avoids
// the limitations of both XPCOM support for Rust and Rust support for Structs
// with fields that reference each other.  It consumes more memory and is
// less performant, but the difference is likely to be insignificant
// for XULStore, which stores and iterates very small amounts of data.

pub struct StringIterator {
    index: usize,
    values: Vec<String>,
}

impl<'a> StringIterator {
    pub fn new(values: Vec<String>) -> Self {
        Self {
            index: 0,
            values: values,
        }
    }

    pub fn has_more(&self) -> bool {
        self.index < self.values.len()
    }

    pub fn get_next_ns(&mut self, value: *mut nsAString) -> nsresult {
        // TODO: confirm that self.index in range.
        // TODO: consume the value being returned.
        unsafe {
            (*value).assign(&nsString::from(self.values[self.index].as_str()));
        }
        self.index = self.index + 1;
        NS_OK
    }

    pub fn get_next_c(&mut self) -> &String {
        // TODO: confirm that self.index in range.
        // TODO: consume the value being returned.
        let value = &self.values[self.index];
        self.index = self.index + 1;
        value
    }
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_has_more(iter: *mut StringIterator) -> bool {
    assert!(!iter.is_null());
    (&*iter).has_more()
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_get_next_ns(
    iter: *mut StringIterator,
    value: *mut nsAString,
) -> nsresult {
    assert!(!iter.is_null());
    (&mut *iter).get_next_ns(value)
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_get_next_c(iter: *mut StringIterator) -> *const c_char {
    assert!(!iter.is_null());
    CString::new((&mut *iter).get_next_c().as_str()).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_drop(iter: *mut StringIterator) {
    if !iter.is_null() {
        drop(Box::from_raw(iter));
    }
}
