#[no_mangle]
pub extern fn test_xul_store() -> *const u8 {
    // NB: rust &str aren't null terminated.
    let greeting = "hello from XUL store.\0";
    greeting.as_ptr()
}
