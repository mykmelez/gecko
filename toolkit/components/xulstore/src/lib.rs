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

struct XULStore {
    foo: u32,
}

lazy_static! {
  #[derive(Debug)]
  static ref XUL_STORE: XULStore = {
    let dir_svc = xpcom::services::get_DirectoryService().unwrap();
    let property = CString::new("ProfD").unwrap();
    let mut ga = xpcom::GetterAddrefs::<xpcom::interfaces::nsIFile>::new();
    unsafe {
        dir_svc.Get(property.as_ptr(), &xpcom::interfaces::nsIFile::IID, ga.void_ptr());
    }
    let mut s = nsString::new();
    unsafe {
        ga.refptr().unwrap().GetPath(&mut s);
    }
    println!("profile directory: {:?}", s);

    // TODO: open the store and store a reference to it.

    XULStore {
        foo: 5,
    }
  };
}

impl Drop for XULStore {
  fn drop(&mut self) {
    // unsafe { /* TODO: close store */ }
  }
}

#[no_mangle]
pub extern "C" fn xulstore_set_value(doc: &nsAString, id: &nsAString, attr: &nsAString, value: &nsAString) -> nsresult {
    println!("XUL_STORE.foo: {:?}", XUL_STORE.foo);
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
