/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::{
    error::XULStoreNsResult,
    iter::XULStoreIterator,
    store::{update_profile_dir, STORE},
    XULStore,
};
use libc::{c_char, c_void};
use nserror::{nsresult, NS_ERROR_NO_AGGREGATION, NS_OK};
use nsstring::nsAString;
use std::ptr;
use xpcom::{interfaces::nsISupports, nsIID, RefPtr};

// XULStore no longer expresses an XPCOM API.  Instead, JS consumers import
// xultore.jsm, while C++ consumers include XULStore.h.  But we still construct
// an nsIXULStore instance in order to support migration from the old store
// (xulstore.json) to the new one.
#[no_mangle]
pub unsafe extern "C" fn nsXULStoreServiceConstructor(
    outer: *const nsISupports,
    iid: &nsIID,
    result: *mut *mut c_void,
) -> nsresult {
    *result = ptr::null_mut();

    if !outer.is_null() {
        return NS_ERROR_NO_AGGREGATION;
    }

    let service: RefPtr<XULStoreService> = XULStoreService::new();
    service.QueryInterface(iid, result)
}

#[derive(xpcom)]
#[xpimplements(nsIXULStore)]
#[refcnt = "atomic"]
pub struct InitXULStoreService {}

impl XULStoreService {
    fn new() -> RefPtr<XULStoreService> {
        // Trigger migration of data from the old store to the new one.
        let _ = STORE.read();

        XULStoreService::allocate(InitXULStoreService {})
    }
}

#[derive(xpcom)]
#[xpimplements(nsIObserver)]
#[refcnt = "nonatomic"]
pub(crate) struct InitProfileChangeObserver {}
impl ProfileChangeObserver {
    #[allow(non_snake_case)]
    unsafe fn Observe(
        &self,
        _subject: *const nsISupports,
        _topic: *const c_char,
        _data: *const i16,
    ) -> nsresult {
        update_profile_dir();
        NS_OK
    }

    pub(crate) fn new() -> RefPtr<ProfileChangeObserver> {
        ProfileChangeObserver::allocate(InitProfileChangeObserver {})
    }
}

#[no_mangle]
pub extern "C" fn xulstore_set_value(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
    value: &nsAString,
) -> XULStoreNsResult {
    XULStore::set_value(doc, id, attr, value).into()
}

#[no_mangle]
pub extern "C" fn xulstore_has_value(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
    has_value: *mut bool,
) -> XULStoreNsResult {
    XULStore::has_value(doc, id, attr, has_value).into()
}

#[no_mangle]
pub extern "C" fn xulstore_get_value(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
    value: *mut nsAString,
) -> XULStoreNsResult {
    XULStore::get_value(doc, id, attr, value).into()
}

#[no_mangle]
pub extern "C" fn xulstore_remove_value(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
) -> XULStoreNsResult {
    XULStore::remove_value(doc, id, attr).into()
}

#[no_mangle]
pub extern "C" fn xulstore_get_ids(
    doc: &nsAString,
    result: &mut nsresult,
) -> *mut XULStoreIterator {
    match XULStore::get_ids(doc) {
        Ok(iter) => {
            *result = NS_OK;
            iter
        }
        Err(err) => {
            *result = err.into();
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn xulstore_get_attrs(
    doc: &nsAString,
    id: &nsAString,
    result: &mut nsresult,
) -> *mut XULStoreIterator {
    match XULStore::get_attrs(doc, id) {
        Ok(iter) => {
            *result = NS_OK;
            iter
        }
        Err(err) => {
            *result = err.into();
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_has_more(iter: *const XULStoreIterator) -> bool {
    assert!(!iter.is_null());
    (&*iter).has_more()
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_get_next(
    iter: *mut XULStoreIterator,
    value: *mut nsAString,
) -> XULStoreNsResult {
    assert!(!iter.is_null());
    (&mut *iter).get_next(value).into()
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_free(iter: *mut XULStoreIterator) {
    if !iter.is_null() {
        drop(Box::from_raw(iter));
    }
}
