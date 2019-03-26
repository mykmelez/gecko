/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::{
    error::XULStoreNsResult,
    iter::XULStoreIterator,
    statics::{update_profile_dir, DATA},
    XULStore,
};
use libc::{c_char, c_void};
use nserror::{nsresult, NS_ERROR_NO_AGGREGATION, NS_OK};
use nsstring::{nsAString, nsString};
use std::ptr;
use xpcom::{interfaces::nsISupports, nsIID, RefPtr};

// XULStore no longer expresses an XPCOM API.  Instead, JS consumers import
// xultore.jsm, while C++ consumers include XULStore.h.  But we still construct
// an nsIXULStore instance in order to support migration from the old store
// (xulstore.json) to the new one.
//
// To ensure migration occurs before the new store is accessed for the first
// time, regardless of whether the first caller is JS or C++, xulstore.jsm
// retrieves this service, which triggers lazy instantiation of the "STORE"
// static (which then migrates data if an old store is present in the profile);
// and all of the methods in XULStore.h that access data in the new store
// similarly trigger instantiation of that static (and thus data migration).
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
        // Trigger initialization of statics (if they haven't already been
        // initialized).  DATA accesses STORE, which accesses RKV,
        // which accesses PROFILE_DIR, so getting the read guard for DATA
        // suffices to initialize all statics.
        //
        // TODO: remove this unnecessary initialization step if we decide
        // to make all XULStore access be synchronous.
        let _ = DATA.read();

        XULStoreService::allocate(InitXULStoreService {})
    }

    xpcom_method!(
        set_value => SetValue(
            doc: *const nsAString,
            id: *const nsAString,
            attr: *const nsAString,
            value: *const nsAString
        )
    );

    fn set_value(
        &self,
        doc: &nsAString,
        id: &nsAString,
        attr: &nsAString,
        value: &nsAString,
    ) -> Result<(), nsresult> {
        XULStore::set_value(doc, id, attr, value).map_err(|err| err.into())
    }

    xpcom_method!(
        has_value => HasValue(
            doc: *const nsAString,
            id: *const nsAString,
            attr: *const nsAString
        ) -> bool
    );

    fn has_value(
        &self,
        doc: &nsAString,
        id: &nsAString,
        attr: &nsAString,
    ) -> Result<bool, nsresult> {
        XULStore::has_value(doc, id, attr).map_err(|err| err.into())
    }

    xpcom_method!(
        get_value => GetValue(
            doc: *const nsAString,
            id: *const nsAString,
            attr: *const nsAString
        ) -> nsAString
    );

    fn get_value(
        &self,
        doc: &nsAString,
        id: &nsAString,
        attr: &nsAString,
    ) -> Result<nsString, nsresult> {
        match XULStore::get_value(doc, id, attr) {
            Ok(val) => Ok(nsString::from(&val)),
            Err(err) => Err(err.into()),
        }
    }

    xpcom_method!(
        remove_value => RemoveValue(
            doc: *const nsAString,
            id: *const nsAString,
            attr: *const nsAString
        )
    );

    fn remove_value(
        &self,
        doc: &nsAString,
        id: &nsAString,
        attr: &nsAString,
    ) -> Result<(), nsresult> {
        XULStore::remove_value(doc, id, attr).map_err(|err| err.into())
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
pub unsafe extern "C" fn xulstore_set_value(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
    value: &nsAString,
) -> XULStoreNsResult {
    XULStore::set_value(doc, id, attr, value).into()
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_has_value(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
    has_value: *mut bool,
) -> XULStoreNsResult {
    match XULStore::has_value(doc, id, attr) {
        Ok(val) => {
            *has_value = val;
            XULStoreNsResult(NS_OK)
        }
        Err(err) => XULStoreNsResult(err.into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_get_value(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
    value: *mut nsAString,
) -> XULStoreNsResult {
    match XULStore::get_value(doc, id, attr) {
        Ok(val) => {
            (*value).assign(&nsString::from(&val));
            XULStoreNsResult(NS_OK)
        }
        Err(err) => XULStoreNsResult(err.into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_remove_value(
    doc: &nsAString,
    id: &nsAString,
    attr: &nsAString,
) -> XULStoreNsResult {
    XULStore::remove_value(doc, id, attr).into()
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_get_ids(
    doc: &nsAString,
    result: *mut nsresult,
) -> *mut XULStoreIterator {
    match XULStore::get_ids(doc) {
        Ok(iter) => {
            *result = NS_OK;
            Box::into_raw(Box::new(iter))
        }
        Err(err) => {
            *result = err.into();
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_get_attrs(
    doc: &nsAString,
    id: &nsAString,
    result: *mut nsresult,
) -> *mut XULStoreIterator {
    match XULStore::get_attrs(doc, id) {
        Ok(iter) => {
            *result = NS_OK;
            Box::into_raw(Box::new(iter))
        }
        Err(err) => {
            *result = err.into();
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_has_more(iter: &XULStoreIterator) -> bool {
    iter.has_more()
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_get_next(
    iter: &mut XULStoreIterator,
    value: *mut nsAString,
) -> XULStoreNsResult {
    match iter.get_next() {
        Ok(val) => {
            (*value).assign(&nsString::from(&val));
            XULStoreNsResult(NS_OK)
        }
        Err(err) => XULStoreNsResult(err.into()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn xulstore_iter_free(iter: *mut XULStoreIterator) {
    drop(Box::from_raw(iter));
}
