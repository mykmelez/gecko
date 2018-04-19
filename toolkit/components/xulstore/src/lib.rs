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
use std::path::Path;
use xpcom::{getter_addrefs, RefPtr, XpCom};

extern crate nsstring;
use nsstring::{nsAString, nsString};

extern crate nserror;
use nserror::*;

#[macro_use]
extern crate lazy_static;

struct XULStore {
    rkv: Rkv,
}

lazy_static! {
  #[derive(Debug)]
  static ref XUL_STORE: XULStore = {
    // Get the profile directory path.
    let dir_svc = xpcom::services::get_DirectoryService().unwrap();
    let property = CString::new("ProfD").unwrap();
    let mut profile_dir = xpcom::GetterAddrefs::<xpcom::interfaces::nsIFile>::new();
    unsafe {
        dir_svc.Get(property.as_ptr(), &xpcom::interfaces::nsIFile::IID, profile_dir.void_ptr());
    }

    // Convert the profile directory path to a Path.
    // 
    // This might be easier if we could access nsIFile::NativePath,
    // but the generated file nsIFile.rs declares:
    // "Unable to generate binding because `nostdcall is unsupported`"
    //
    let mut profile_dir_path = nsString::new();
    unsafe {
        profile_dir.refptr().unwrap().GetPath(&mut profile_dir_path);
    }
    println!("profile directory: {:?}", &profile_dir_path);
    let profile_dir_path = String::from_utf16_lossy(&profile_dir_path[..]);
    let profile_dir_path = Path::new(&profile_dir_path);

    let xulstore_dir_path = profile_dir_path.join("xulstore");
    fs::create_dir_all(&xulstore_dir_path).expect("dir created");
    let rkv = Rkv::new(&xulstore_dir_path).expect("new succeeded");

    XULStore {
        rkv: rkv,
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
    println!("{:?}", XUL_STORE.rkv);
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
