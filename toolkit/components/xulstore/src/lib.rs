extern crate rkv;
extern crate tempdir;
extern crate xpcom;

use rkv::{
    Rkv,
    Store,
    Value,
};

use self::tempdir::TempDir;
use std::ffi::CString;
use std::fmt::Write;
use std::fs;
// use std::os::raw::c_void;
use xpcom::{getter_addrefs, RefPtr, XpCom};

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
    let dir_svc = xpcom::services::get_DirectoryService().unwrap();
    let property = CString::new("ProfD").unwrap();

    // Given refptr.rs, it seems like we should be able to do this:
    // let x: Result<RefPtr<T>, nsresult> =
    //     getter_addrefs(|p| dir_svc.Get(property.as_ptr(), &xpcom::interfaces::nsIFile::IID, p));

    // But it doesn't work, and I'm not sure why.  This does, on the other hand,
    // although I worry about it.  More research needed.
    let nsi_file = 0 as *mut *mut xpcom::reexports::libc::c_void;
    unsafe {
        dir_svc.Get(property.as_ptr(), &xpcom::interfaces::nsIFile::IID, nsi_file);
    }

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
