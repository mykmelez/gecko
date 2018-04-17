extern crate rkv;
extern crate tempdir;
extern crate xpcom;

use rkv::{
    Rkv,
    Store,
    Value,
};

use self::tempdir::TempDir;
use std::fmt::Write;
use std::fs;

extern crate nsstring;
use nsstring::{nsAString, nsString};

extern crate nserror;
use nserror::*;

#[macro_use]
extern crate lazy_static;

struct XULStore {}

lazy_static! {
  static ref XUL_STORE: XULStore = {
    // TODO: get the profile directory and open the store within it.
    let _dir_svc = xpcom::services::get_DirectoryService().unwrap();
    XULStore {}
  };
}

impl Drop for XULStore {
  fn drop(&mut self) {
    // unsafe { /* TODO: close store */ }
  }
}

#[no_mangle]
pub extern "C" fn xulstore_set_value(doc: &nsAString, id: &nsAString, attr: &nsAString, value: &nsAString) -> nsresult {
    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_has_value(doc: &nsAString, id: &nsAString, attr: &nsAString) -> bool {
    true
}

#[no_mangle]
pub extern "C" fn xulstore_get_value(doc: &nsAString, id: &nsAString, attr: &nsAString, value: *mut nsAString) {
    unsafe {
        (*value).assign(&nsString::from("Hello, World!"));
    }
}
