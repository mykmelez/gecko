/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

/*
A proof-of-concept XULStore implementation that uses rkv to store data.
This PoC translates the XULStore data model into rkv concepts via this mapping:

  * XUL document -> RKV store
  * (element ID, attribute name) -> RKV key
  * attribute value -> RKV value

The PoC creates an rkv store (LMDB database) for each unique XUL document
and specifies rkv keys as tuples of element ID and attribute name (serialized
to a string concatenation with an equals sign separator, i.e. ID=name).

Note that the maximum number of rkv stores needs to be known during
initialization of the RKV singleton (LMDB environment).  Also note that
"a moderate number of slots are cheap but a huge number gets expensive"
<http://www.lmdb.tech/doc/group__mdb.html#gaa2fc2f1f37cb1115e733b62cab2fcdbc>.

Since extensions no longer store data using XULStore, and Firefox itself
only stores data for a small number of XUL documents, these shouldn't be
issues in practice.  Firefox can set MAX_STORES to the number of XUL docs
for which it'll ever store values, and migration from the old JSON store
to a new rkv store can drop values stored by legacy extensions.

However, a real implementation might choose the alternative of mapping
documents to rkv keys in a single rkv store and store element/attribute/value
triples for a given document as a single rkv value, using JSON or the like
to structure and serialize the data to a blob that can be stored as a value.

The API for this crate comprises two sets of C ABI functions, one of which
uses raw pointers to char arrays to receive and return strings, the other
of which uses Gecko's nsstring crate, which provides Rust implementations
of common Gecko string types.

These sets model two possible implementation strategies, the first of which
is more generic and can be accessed from code that doesn't support the Gecko
string types, such as JS (using js-ctypes) or Java/Kotlin/Swift (on mobile);
the second of which is more specific to Gecko consumers on desktop and may be
safer, faster, more ergonomic, or more efficient.

A real implementation of XULStore will presumably use the nsstring-based API,
with a C++ XPCOM interface abstraction for both C++ and JS consumers.
*/

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
use lmdb::{Cursor};
use nserror::{NS_OK, nsresult};
use nsstring::{nsAString, nsString};
use rkv::{Rkv, Store, StoreError, Value};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::{c_char, c_uint};
use std::path::{Path};
use std::str;
use xpcom::{interfaces, XpCom};

// TODO: set this to the maximum number of documents for which Firefox persists
// element attribute values using XULStore.
static MAX_STORES: c_uint = 10;

lazy_static! {
    #[derive(Debug)]
    static ref RKV: Rkv = {
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
        Rkv::with_capacity(&xulstore_dir_path, MAX_STORES).expect("new succeeded")
    };
}

// See XULStore.cpp for an explanation of this function.
#[no_mangle]
pub extern "C" fn xulstore_function_marked_used() {}

#[no_mangle]
pub extern "C" fn xulstore_set_value_ns(doc: &nsAString, id: &nsAString, attr: &nsAString, value: &nsAString) -> nsresult {
    let store_name = String::from_utf16_lossy(doc);
    // TODO: migrate data if store doesn't exist.
    // TODO: cache opened stores.
    let store = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
    let key = String::from_utf16_lossy(id) + "=" + &String::from_utf16_lossy(attr);
    let mut writer = store.write(&RKV).expect("writer");

    // TODO: store (and retrieve) values as raw bytes rather than converting
    // them to String and back, which is not only potentially lossy but also
    // presumably unnecessary expense.
    writer.put(&key, &Value::Str(&String::from_utf16_lossy(value))).expect("put");
    writer.commit().expect("commit");
    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_set_value_c(doc: *const c_char, id: *const c_char, attr: *const c_char, value: *const c_char) -> nsresult {
    let store_name = unsafe { CStr::from_ptr(doc) }.to_str().unwrap();
    let store = RKV.create_or_open(Some(store_name)).expect("open store");
    let key = unsafe { CStr::from_ptr(id) }.to_str().unwrap().to_owned() + "=" +
              unsafe { CStr::from_ptr(attr) }.to_str().unwrap();
    let mut writer = store.write(&RKV).expect("writer");
    writer.put(&key, &Value::Str(unsafe { CStr::from_ptr(value) }.to_str().unwrap())).expect("put");
    writer.commit().expect("commit");
    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_has_value_ns(doc: &nsAString, id: &nsAString, attr: &nsAString) -> bool {
    let store_name = String::from_utf16_lossy(doc);
    let store = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
    let key = String::from_utf16_lossy(id) + "=" + &String::from_utf16_lossy(attr);
    let reader = store.read(&RKV).expect("reader");

    let value = reader.get(key);
    println!("{:?}", value);
    match value {
        Result::Ok(None) => false,
        // TODO: report error instead of merely swallowing it.
        Result::Err(_) => false,
        _ => true,
    }
}

#[no_mangle]
pub extern "C" fn xulstore_has_value_c(doc: *const c_char, id: *const c_char, attr: *const c_char) -> bool {
    let store_name = unsafe { CStr::from_ptr(doc) }.to_str().unwrap();
    let store = RKV.create_or_open(Some(store_name)).expect("open store");
    let key = unsafe { CStr::from_ptr(id) }.to_str().unwrap().to_owned() + "=" +
              unsafe { CStr::from_ptr(attr) }.to_str().unwrap();
    let reader = store.read(&RKV).expect("reader");

    let value = reader.get(key);
    println!("{:?}", value);
    match value {
        Result::Ok(None) => false,
        // TODO: report error instead of merely swallowing it.
        Result::Err(_) => false,
        _ => true,
    }
}

#[no_mangle]
pub extern "C" fn xulstore_get_value_ns(doc: &nsAString, id: &nsAString, attr: &nsAString, value: *mut nsAString) {
    let store_name = String::from_utf16_lossy(doc);
    let store = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
    let key = String::from_utf16_lossy(id) + "=" + &String::from_utf16_lossy(attr);
    let reader = store.read(&RKV).expect("reader");

    let retrieved_value = reader.get(key);
    println!("retrieved_value: {:?}", retrieved_value);

    let return_value = match retrieved_value {
        Ok(Some(Value::Str(value))) => value,
        // TODO: report error instead of merely swallowing it.
        Err(_) => "",
        _ => "",
    };
    println!("return_value: {:?}", return_value);
    unsafe {
        (*value).assign(&nsString::from(return_value))
    }
}

#[no_mangle]
pub extern "C" fn xulstore_get_value_c(doc: *const c_char, id: *const c_char, attr: *const c_char) -> *const c_char {
    assert!(!doc.is_null());
    assert!(!id.is_null());
    assert!(!attr.is_null());

    let store_name = unsafe { CStr::from_ptr(doc) }.to_str().unwrap();
    let store = RKV.create_or_open(Some(store_name)).expect("open store");
    let key = unsafe { CStr::from_ptr(id) }.to_str().unwrap().to_owned() + "=" +
              unsafe { CStr::from_ptr(attr) }.to_str().unwrap();
    let reader = store.read(&RKV).expect("reader");

    let retrieved_value = reader.get(key);
    println!("{:?}", retrieved_value);
    let return_value = match retrieved_value {
        Ok(Some(Value::Str(value))) => value,
        // TODO: report error instead of merely swallowing it.
        Err(_) => "",
        _ => "",
    };
    println!("return_value: {:?}", return_value);
    CString::new(return_value).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn xulstore_free_value_c(str: *mut c_char) {
    if str.is_null() {
        unsafe { CString::from_raw(str) };
    }
}

#[no_mangle]
pub extern "C" fn xulstore_remove_value_ns(doc: &nsAString, id: &nsAString, attr: &nsAString) -> nsresult {
    let store_name = String::from_utf16_lossy(doc);
    let store = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
    let key = String::from_utf16_lossy(id) + "=" + &String::from_utf16_lossy(attr);
    let mut writer = store.write(&RKV).expect("writer");
    match writer.delete(&key) {
        // The XULStore API doesn't care if a consumer tries to remove a value
        // that doesn't actually exist, so we ignore that error.
        Err(StoreError::LmdbError(lmdb::Error::NotFound)) => Ok(()),
        Ok(ok) => Ok(ok),
        Err(err) => Err(err),
    }.expect("delete");
    // TODO: remove database if we've removed the last key/value pair from it.
    writer.commit().expect("commit");
    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_remove_value_c(doc: *const c_char, id: *const c_char, attr: *const c_char) -> nsresult {
    let store_name = unsafe { CStr::from_ptr(doc) }.to_str().unwrap();
    let store = RKV.create_or_open(Some(store_name)).expect("open store");
    let key = unsafe { CStr::from_ptr(id) }.to_str().unwrap().to_owned() + "=" +
              unsafe { CStr::from_ptr(attr) }.to_str().unwrap();
println!("xulstore_remove_value_C; key: {:?}", key);
    let mut writer = store.write(&RKV).expect("writer");
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
pub extern "C" fn xulstore_get_ids_iterator_ns(doc: &nsAString) -> *mut StringIterator {
    let store_name = String::from_utf16_lossy(doc);
    let store: Store<&'static str> = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
    let reader = store.read(&RKV).expect("reader");
    let mut cursor = reader.open_cursor().expect("cursor");
    println!("cursor: {:?}", cursor);
    let iterator = cursor.iter();
    println!("iterator: {:?}", iterator);
    // let collection: () = iterator.map(|v| println!("item: {:?}", v)).collect();
    let collection: Vec<&str> = iterator
        .map(|(key, _val)| key)

        // Assumes we control all writes into database.
        // TODO: avoid making that assumption and check the conversion.
        .map(|v| unsafe { str::from_utf8_unchecked(&v) })

        .map(|v| v.split_at(v.find('=').unwrap()))
        .map(|(id, _attr)| id)
        // .map(|v| println!("item: {:?}", v))
        // TODO: unique() collects values, and collect() does too,
        // so do so only once, by collecting the values into a set.
        .unique()
        .collect();

    println!("collection: {:?}", collection);

    Box::into_raw(Box::new(StringIterator::new(collection)))
    // ptr::null_mut()
}
// TODO: refactor all duplicate implementations.

#[no_mangle]
pub extern "C" fn xulstore_get_ids_iterator_c<'a>(doc: *const c_char) -> *mut StringIterator<'a> {
    let store_name = unsafe { CStr::from_ptr(doc) }.to_str().unwrap();
    let store: Store<&'static str> = RKV.create_or_open(Some(store_name)).expect("open store");
    let reader = store.read(&RKV).expect("reader");
    let mut cursor = reader.open_cursor().expect("cursor");
    println!("cursor: {:?}", cursor);
    let iterator = cursor.iter();
    println!("iterator: {:?}", iterator);
    // let collection: () = iterator.map(|v| println!("item: {:?}", v)).collect();
    let collection: Vec<&str> = iterator
        .map(|(key, _val)| key)

        // Assumes we control all writes into database.
        // TODO: avoid making that assumption and check the conversion.
        .map(|v| unsafe { str::from_utf8_unchecked(&v) })

        .map(|v| v.split_at(v.find('=').unwrap()))
        .map(|(id, _attr)| id)
        // .map(|v| println!("item: {:?}", v))
        .unique()
        .collect();

    println!("collection: {:?}", collection);

    Box::into_raw(Box::new(StringIterator::new(collection)))
    // ptr::null_mut()
}

// TODO refactor with xulstore_get_ids_iterator.
#[no_mangle]
pub extern "C" fn xulstore_get_attribute_iterator_ns<'a>(doc: &nsAString, id: &nsAString) -> *mut StringIterator<'a> {
    let store_name = String::from_utf16_lossy(doc);
    let element_id = String::from_utf16_lossy(id);
    let store: Store<&'static str> = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
    let reader = store.read(&RKV).expect("reader");
    let mut cursor = reader.open_cursor().expect("cursor");
    println!("cursor: {:?}", cursor);
    let iterator = cursor.iter();
    println!("iterator: {:?}", iterator);
    // let collection: () = iterator.map(|v| println!("item: {:?}", v)).collect();
    let collection: Vec<&str> = iterator
        .map(|(key, _val)| key)

        // Assumes we control all writes into database.
        // TODO: avoid making that assumption and check the conversion.
        .map(|v| unsafe { str::from_utf8_unchecked(&v) })

        .map(|v| v.split_at(v.find('=').unwrap()))
        .filter(|&(id, _attr)| id == element_id)
        // Split-at doesn't remove the character at which the string is split,
        // so we have to slice it off ourselves.
        .map(|(_id, attr)| &attr[1..])
        // .map(|v| println!("item: {:?}", v))
        .unique()
        .collect();

    println!("collection: {:?}", collection);

    Box::into_raw(Box::new(StringIterator::new(collection)))
    // ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn xulstore_get_attribute_iterator_c<'a>(doc: *const c_char, id: *const c_char) -> *mut StringIterator<'a> {
    let store_name = unsafe { CStr::from_ptr(doc) }.to_str().unwrap();
    let element_id = unsafe { CStr::from_ptr(id) }.to_str().unwrap();
    let store: Store<&'static str> = RKV.create_or_open(Some(store_name)).expect("open store");
    let reader = store.read(&RKV).expect("reader");
    let mut cursor = reader.open_cursor().expect("cursor");
    println!("cursor: {:?}", cursor);
    let iterator = cursor.iter();
    println!("iterator: {:?}", iterator);
    // let collection: () = iterator.map(|v| println!("item: {:?}", v)).collect();
    let collection: Vec<&str> = iterator
        .map(|(key, _val)| key)
        .map(|v| { println!("v: {:?}", v); v })

        // Assumes we control all writes into database.
        // TODO: avoid making that assumption and check the conversion.
        .map(|v| unsafe { str::from_utf8_unchecked(&v) })

        .map(|v| v.split_at(v.find('=').unwrap()))
        .filter(|&(id, _attr)| id == element_id)
        // Split-at doesn't remove the character at which the string is split,
        // so we have to slice it off ourselves.
        .map(|(_id, attr)| &attr[1..])
        .unique()
        .collect();

    println!("collection: {:?}", collection);

    Box::into_raw(Box::new(StringIterator::new(collection)))
    // ptr::null_mut()
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
//     let store_name = String::from_utf16_lossy(doc);
//     let store: Store<&'static str> = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
//     let reader = store.read(&RKV).expect("reader");
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

pub struct StringIterator<'a> {
    index: usize,
    values: Vec<&'a str>,
}

impl<'a> StringIterator<'a> {
    pub fn new(values: Vec<&'a str>) -> StringIterator {
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
            (*value).assign(&nsString::from(self.values[self.index]));
        }
        self.index = self.index + 1;
        NS_OK
    }

    pub fn get_next_c(&mut self) -> &'a str {
        // TODO: confirm that self.index in range.
        // TODO: consume the value being returned.
        let value = self.values[self.index];
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
pub unsafe extern "C" fn xulstore_iter_get_next_ns(iter: *mut StringIterator, value: *mut nsAString) -> nsresult {
    assert!(!iter.is_null());
    (&mut *iter).get_next_ns(value)
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_get_next_c(iter: *mut StringIterator) -> *const c_char {
    assert!(!iter.is_null());
    CString::new((&mut *iter).get_next_c()).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_destroy(iter: *mut StringIterator) {
    if !iter.is_null() {
        drop(Box::from_raw(iter));
    }
}
