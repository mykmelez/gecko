extern crate rkv;
extern crate tempdir;
extern crate xpcom;

use rkv::{
    Rkv,
    Store,
    Value,
};

use self::tempdir::TempDir;
use std::fs;

extern crate nsstring;
use nsstring::{nsAString, nsString};

extern crate nserror;
use nserror::*;

#[macro_use]
extern crate lazy_static;

lazy_static! {
  static ref XUL_STORE: XULStore = {
    XULStore {}
  };
}

impl Drop for XULStore {
  fn drop(&mut self) {
    // unsafe { /* TODO: close store */ }
  }
}

#[repr(C)]
pub struct XULStore {
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
pub extern "C" fn xulstore_get_value(doc: &nsAString, id: &nsAString, attr: &nsAString) -> nsString {
    let mut str = nsString::new();
    str
}

#[no_mangle]
pub extern fn test_xul_store() -> *const u8 {
    let dir_svc = xpcom::services::get_DirectoryService().unwrap();

    let root = TempDir::new("use_store").expect("tempdir");
    fs::create_dir_all(root.path()).expect("dir created");
    let k = Rkv::new(root.path()).expect("new succeeded");
    let mut s: Store<&str> = k.create_or_open("s").expect("opened");

    // Add one field.
    {
        let mut writer = s.write(&k).expect("writer");
        writer.put("foo", &Value::I64(1234)).expect("wrote");
        writer.commit().expect("committed");
    }

    // Both ways of reading see the value.
    {
        let reader = &k.read().unwrap();
        assert_eq!(s.get(reader, "foo").expect("read"), Some(Value::I64(1234)));
    }
    {
        let reader = s.read(&k).unwrap();
        assert_eq!(reader.get("foo").expect("read"), Some(Value::I64(1234)));
    }

    // Establish a long-lived reader that outlasts a writer.
    let reader = s.read(&k).expect("reader");
    assert_eq!(reader.get("foo").expect("read"), Some(Value::I64(1234)));

    // Start a write transaction.
    let mut writer = s.write(&k).expect("writer");
    writer.put("foo", &Value::I64(999)).expect("wrote");

    // The reader and writer are isolated.
    assert_eq!(reader.get("foo").expect("read"), Some(Value::I64(1234)));
    assert_eq!(writer.get("foo").expect("read"), Some(Value::I64(999)));

    // If we commit the writer, we still have isolation.
    writer.commit().expect("committed");
    assert_eq!(reader.get("foo").expect("read"), Some(Value::I64(1234)));

    // A new reader sees the committed value. Note that LMDB doesn't allow two
    // read transactions to exist in the same thread, so we abort the previous one.
    reader.abort();
    let reader = s.read(&k).expect("reader");
    assert_eq!(reader.get("foo").expect("read"), Some(Value::I64(999)));

    // NB: rust &str aren't null terminated.
    let greeting = "hello from XUL store.\0";
    greeting.as_ptr()
}
