/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use libc::uint16_t;

// These are the relevant parts of the nsXPTTypeTag enum in xptinfo.h,
// which nsIVariant.idl reflects into the nsIDataType struct class and uses
// to constrain the values of nsIVariant::dataType.
#[repr(u16)]
pub enum DataType {
    INT32 = 2,
    DOUBLE = 9,
    BOOL = 10,
    VOID = 13,
    WSTRING = 21,
    EMPTY = 255,
}

// Per https://github.com/rust-lang/rust/issues/44266, casts aren't allowed
// in match arms, so it isn't possible to cast DataType variants to u16
// in order to match them against the value of nsIVariant::dataType.
// Instead we have to reflect each variant into a constant and then match
// against the values of the constants.
//
// (Alternatively, we could use the enum_primitive crate to convert primitive
// values of nsIVariant::dataType to their enum equivalents.  Or perhaps
// bindgen would convert the nsXPTTypeTag enum in xptinfo.h into something else
// we could use.  Since we currently only accept a small subset of values,
// and since that enum is unlikely to change frequently, this workaround
// seems sufficient.)
//
pub const DATA_TYPE_INT32: uint16_t = DataType::INT32 as u16;
pub const DATA_TYPE_DOUBLE: uint16_t = DataType::DOUBLE as u16;
pub const DATA_TYPE_BOOL: uint16_t = DataType::BOOL as u16;
pub const DATA_TYPE_VOID: uint16_t = DataType::VOID as u16;
pub const DATA_TYPE_WSTRING: uint16_t = DataType::WSTRING as u16;
pub const DATA_TYPE_EMPTY: uint16_t = DataType::EMPTY as u16;
