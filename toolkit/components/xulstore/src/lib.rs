extern crate rkv;
extern crate tempdir;
extern crate xpcom;

use rkv::{Rkv, Store, Value};

use self::tempdir::TempDir;
use std::ffi::CString;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::str;
use xpcom::{getter_addrefs, RefPtr, XpCom};

extern crate nsstring;
use nsstring::{nsAString, nsString};

extern crate nserror;
use nserror::*;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    #[derive(Debug)]
    static ref RKV: Rkv = {
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
        let profile_dir_path = String::from_utf16_lossy(&profile_dir_path[..]);
        let profile_dir_path = Path::new(&profile_dir_path);

        let xulstore_dir_path = profile_dir_path.join("xulstore");
        fs::create_dir_all(xulstore_dir_path.clone()).expect("dir created");
        println!("xulstore directory: {:?}", &xulstore_dir_path);
        Rkv::new(&xulstore_dir_path).expect("new succeeded")
    };

    #[derive(Debug)]
    static ref STORE: Store<&'static str> = {
        println!("{:?}", RKV);
        RKV.create_or_open_default().expect("created default")
    };
}

#[no_mangle]
pub extern "C" fn xulstore_set_value(doc: &nsAString, id: &nsAString, attr: &nsAString, value: &nsAString) -> nsresult {
    let store_name = String::from_utf16_lossy(doc);
    // TODO: migrate data if store doesn't exist.
    // TODO: cache opened stores.
    let store = RKV.create_or_open(Some(store_name.as_str())).expect("open store");
    let key = String::from_utf16_lossy(id) + "=" + &String::from_utf16_lossy(attr);
    let mut writer = store.write(&RKV).expect("writer");

    // This writer.get() call borrows writer immutably, and the &str value
    // that Value::Str wraps is scoped to the lifetime of writer, which means
    // we need to release it before the writer.put() call below that borrows
    // writer mutably.  So we clone the &str.
    // TODO: figure out how to avoid the allocation.
    // let existing_value = match writer.get(&key).expect("read") {
    //     Some(Value::Str(val)) => val,
    //     _ => "",
    // }.to_string();
    // println!("{:?}", existing_value);

    writer.put(&key, &Value::Str(&String::from_utf16_lossy(value)));
    writer.commit();
    NS_OK
}

#[no_mangle]
pub extern "C" fn xulstore_has_value(doc: &nsAString, id: &nsAString, attr: &nsAString) -> bool {
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
pub extern "C" fn xulstore_get_value(doc: &nsAString, id: &nsAString, attr: &nsAString, value: *mut nsAString) {
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
