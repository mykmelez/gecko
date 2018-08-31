/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use libc;
use nserror::{NsresultExt, NS_OK};
use nsstring::{nsACString, nsAString, nsCString, nsString};
use xpcom::{getter_addrefs, interfaces::nsIVariant, RefPtr};

extern "C" {
    fn NS_NewStorageNullVariant(result: *mut *const nsIVariant);
    fn NS_NewStorageBooleanVariant(value: bool, result: *mut *const nsIVariant);
    fn NS_NewStorageIntegerVariant(value: libc::int64_t, result: *mut *const nsIVariant);
    fn NS_NewStorageFloatVariant(value: libc::c_double, result: *mut *const nsIVariant);
    fn NS_NewStorageTextVariant(value: *const nsAString, result: *mut *const nsIVariant);
    fn NS_NewStorageUTF8TextVariant(value: *const nsACString, result: *mut *const nsIVariant);
}

/// A wrapper around `nsIVariant`.
pub struct Variant(RefPtr<nsIVariant>);

/// A trait to convert a type into a variant. `impl IntoVariant for T` is almost
/// equivalent to `impl TryFrom<T> for Variant`, but the `TryFrom` trait is
/// Nightly-only.
pub trait IntoVariant {
    fn into_variant(self) -> Option<Variant>;
}

impl Variant {
    pub fn wrap(variant: RefPtr<nsIVariant>) -> Variant {
        Variant(variant)
    }

    pub fn take(self) -> RefPtr<nsIVariant> {
        self.0
    }
}

/// Implements traits to convert between variants and their types.
macro_rules! variant {
    ($typ:ident, $constructor:ident, $getter:ident) => {
        impl From<Variant> for $typ {
            fn from(variant: Variant) -> Self {
                let mut result = $typ::default();
                let rv = unsafe { (variant.0).$getter(&mut result) };
                if rv.succeeded() {
                    result
                } else {
                    $typ::default()
                }
            }
        }

        impl IntoVariant for $typ {
            fn into_variant(self) -> Option<Variant> {
                let v: RefPtr<nsIVariant> = getter_addrefs(|p| unsafe {
                    $constructor(self.into(), p);
                    NS_OK
                }).ok()?;
                Some(Variant::wrap(v))
            }
        }
    };
    (* $typ:ident, $constructor:ident, $getter:ident) => {
        impl From<Variant> for $typ {
            fn from(variant: Variant) -> Self {
                let mut result = $typ::new();
                let rv = unsafe { (variant.0).$getter(&mut *result) };
                if rv.succeeded() {
                    result
                } else {
                    $typ::new()
                }
            }
        }

        impl IntoVariant for $typ {
            fn into_variant(self) -> Option<Variant> {
                let v: RefPtr<nsIVariant> = getter_addrefs(|p| unsafe {
                    $constructor(&*self, p);
                    NS_OK
                }).ok()?;
                Some(Variant::wrap(v))
            }
        }
    };
}

// The unit type (()) is a reasonable equivalation of the null variant.
// The macro can't produce its implementations of From<Variant> and IntoVariant,
// however, so we implement them concretely.
impl From<Variant> for () {
    fn from(_variant: Variant) -> Self {
        ()
    }
}
impl IntoVariant for () {
    fn into_variant(self) -> Option<Variant> {
        let v: RefPtr<nsIVariant> = getter_addrefs(|p| unsafe {
            NS_NewStorageNullVariant(p);
            NS_OK
        }).ok()?;
        Some(Variant::wrap(v))
    }
}

variant!(bool, NS_NewStorageBooleanVariant, GetAsBool);
variant!(i32, NS_NewStorageIntegerVariant, GetAsInt32);
variant!(i64, NS_NewStorageIntegerVariant, GetAsInt64);
variant!(f64, NS_NewStorageFloatVariant, GetAsDouble);
variant!(*nsString, NS_NewStorageTextVariant, GetAsAString);
variant!(*nsCString, NS_NewStorageUTF8TextVariant, GetAsACString);
