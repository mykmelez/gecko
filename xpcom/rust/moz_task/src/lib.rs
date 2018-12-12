/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! This module wraps XPCOM threading functions with Rust functions
//! to make it safer and more convenient to call the XPCOM functions.

extern crate nserror;
extern crate nsstring;
extern crate xpcom;

use nserror::nsresult;
use nsstring::{nsACString, nsCString};
use std::ptr;
use xpcom::{
    getter_addrefs,
    interfaces::{nsIRunnable, nsIThread},
    RefPtr,
};

extern "C" {
    fn NS_GetCurrentThreadEventTarget(result: *mut *const nsIThread) -> nsresult;
    fn NS_GetMainThreadEventTarget(result: *mut *const nsIThread) -> nsresult;
    fn NS_IsMainThread() -> bool;
    fn NS_NewNamedThreadWithDefaultStackSize(
        name: *const nsACString,
        result: *mut *const nsIThread,
        event: *const nsIRunnable,
    ) -> nsresult;
}

pub fn get_current_thread() -> Result<RefPtr<nsIThread>, nsresult> {
    getter_addrefs(|p| unsafe { NS_GetCurrentThreadEventTarget(p) })
}

pub fn get_main_thread() -> Result<RefPtr<nsIThread>, nsresult> {
    getter_addrefs(|p| unsafe { NS_GetMainThreadEventTarget(p) })
}

pub fn is_main_thread() -> bool {
    unsafe { NS_IsMainThread() }
}

pub fn create_thread(name: &str) -> Result<RefPtr<nsIThread>, nsresult> {
    getter_addrefs(|p| unsafe {
        NS_NewNamedThreadWithDefaultStackSize(&*nsCString::from(name), p, ptr::null())
    })
}
