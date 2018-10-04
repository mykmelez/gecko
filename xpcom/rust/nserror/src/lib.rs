extern crate nsstring;

use nsstring::{nsCString, nsACString};
use std::fmt;
use std::ops::Deref;

/// The type of errors in gecko.
#[allow(non_camel_case_types)]
#[repr(transparent)]
#[derive(Debug)]
pub struct nsresult(pub u32);

impl Deref for nsresult {
    type Target = u32;

    fn deref(&self) -> &u32 {
        &self.0
    }
}

impl fmt::Display for nsresult {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An extension trait that adds methods to `nsresult` types.
pub trait NsresultExt {
    fn failed(&self) -> bool;
    fn succeeded(&self) -> bool;
    fn to_result(self) -> Result<nsresult, nsresult>;

    /// Get a printable name for the nsresult error code. This function returns
    /// a nsCString<'static>, which implements `Display`.
    fn error_name(&self) -> nsCString;
}

impl NsresultExt for nsresult {
    fn failed(&self) -> bool {
        (self.0 >> 31) != 0
    }

    fn succeeded(&self) -> bool {
        !self.failed()
    }

    fn to_result(self) -> Result<nsresult, nsresult> {
        if self.failed() {
            Err(self)
        } else {
            Ok(self)
        }
    }

    fn error_name(&self) -> nsCString {
        let mut cstr = nsCString::new();
        unsafe {
            Gecko_GetErrorName(self, &mut *cstr);
        }
        cstr
    }
}

extern "C" {
    fn Gecko_GetErrorName(rv: &nsresult, cstr: *mut nsACString);
}

mod error_list {
    include!(concat!(env!("MOZ_TOPOBJDIR"), "/xpcom/base/error_list.rs"));
}

pub use error_list::*;
