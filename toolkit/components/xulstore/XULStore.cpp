/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

extern "C" {
	void xulstore_function_marked_used();
}

// Use a XULStore function to prevent the linker from stripping functions
// that are only called via js-ctypes.
//
// We need this because Rust doesn't enable us to mark functions as "used"
// <https://github.com/rust-lang/rfcs/issues/1002>, and the linker will strip
// XULStore functions that are only called via js-ctypes.
//
// Strangely, however, this doesn't need to call the functions that are only
// called via js-ctypes.  It just needs to call some function in the same Rust
// library.  So we call a stub.
//
// TODO: figure out a better way to tell the linker these functions are used.
//
void __attribute__((__used__)) mark_xulstore_function_used() {
	xulstore_function_marked_used();
}
