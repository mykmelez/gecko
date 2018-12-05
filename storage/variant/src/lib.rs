/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate libc;
extern crate nserror;
extern crate nsstring;
extern crate xpcom;

use nserror::{NsresultExt, NS_OK, nsresult};
use nsstring::{nsACString, nsAString, nsCString, nsString};
use xpcom::{GetterAddrefs, interfaces::nsIVariant, RefPtr};

extern "C" {
    fn NS_NewStorageNullVariant(result: *mut *const nsIVariant);
    fn NS_NewStorageBooleanVariant(value: bool, result: *mut *const nsIVariant);
    fn NS_NewStorageIntegerVariant(value: libc::int64_t, result: *mut *const nsIVariant);
    fn NS_NewStorageFloatVariant(value: libc::c_double, result: *mut *const nsIVariant);
    fn NS_NewStorageTextVariant(value: *const nsAString, result: *mut *const nsIVariant);
    fn NS_NewStorageUTF8TextVariant(value: *const nsACString, result: *mut *const nsIVariant);
}

pub trait VariantType {
    fn into_variant(self) -> RefPtr<nsIVariant>;
    fn from_variant(variant: &nsIVariant) -> Result<Self, nsresult>
        where Self: Sized;
}

/// Implements traits to convert between variants and their types.
macro_rules! variant {
    ($typ:ident, $constructor:ident, $getter:ident) => {
        impl VariantType for $typ {
            fn into_variant(self) -> RefPtr<nsIVariant> {
                let mut ga = GetterAddrefs::<nsIVariant>::new();
                unsafe { $constructor(self.into(), ga.ptr()) };

                // GetterAddrefs.refptr() returns an Option<RefPtr>,
                // but the $constructor is infallible, so we can safely unwrap
                // and return the RefPtr.
                ga.refptr().unwrap()
            }
            fn from_variant(variant: &nsIVariant) -> Result<$typ, nsresult> {
                let mut result = $typ::default();
                let rv = unsafe { variant.$getter(&mut result) };
                if rv.succeeded() {
                    Ok(result)
                } else {
                    Err(rv)
                }
            }
        }
    };
    (* $typ:ident, $constructor:ident, $getter:ident) => {
        impl VariantType for $typ {
            fn into_variant(self) -> RefPtr<nsIVariant> {
                let mut ga = GetterAddrefs::<nsIVariant>::new();
                unsafe { $constructor(&*self, ga.ptr()) };

                // GetterAddrefs.refptr() returns an Option<RefPtr>,
                // but the $constructor is infallible, so we can safely unwrap
                // and return the RefPtr.
                ga.refptr().unwrap()
            }
            fn from_variant(variant: &nsIVariant) -> Result<$typ, nsresult> {
                let mut result = $typ::new();
                let rv = unsafe { variant.$getter(&mut *result) };
                if rv.succeeded() {
                    Ok(result)
                } else {
                    Err(rv)
                }
            }
        }
    };
}

// The unit type (()) is a reasonable equivalation of the null variant.
// The macro can't produce its implementations of VariantType, however,
// so we implement them concretely.
impl VariantType for () {
    fn into_variant(self) -> RefPtr<nsIVariant> {
        let mut ga = GetterAddrefs::<nsIVariant>::new();
        unsafe { NS_NewStorageNullVariant(ga.ptr()) };

        // GetterAddrefs.refptr() returns an Option<RefPtr>, but the
        // constructor is infallible, so we can safely unwrap the Option
        // and return the RefPtr.
        ga.refptr().unwrap()
    }
    fn from_variant(_variant: &nsIVariant) -> Result<Self, nsresult> {
        Ok(())
    }
}

variant!(bool, NS_NewStorageBooleanVariant, GetAsBool);
variant!(i32, NS_NewStorageIntegerVariant, GetAsInt32);
variant!(i64, NS_NewStorageIntegerVariant, GetAsInt64);
variant!(f64, NS_NewStorageFloatVariant, GetAsDouble);
variant!(*nsString, NS_NewStorageTextVariant, GetAsAString);
variant!(*nsCString, NS_NewStorageUTF8TextVariant, GetAsACString);
