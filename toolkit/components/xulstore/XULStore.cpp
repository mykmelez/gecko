/* -*- Mode: C++; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 2 -*- */
/* vim: set ts=8 sts=2 et sw=2 tw=80: */
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include "XULStore.h"

extern "C" {
  bool xulstore_has_value_2(const char* doc, const char* id, const char* attr);
}

// Define a C function that is marked used and visible so the linker doesn't
// strip it and js-ctypes can call it.  We only need this because Rust doesn't
// enable us to mark a Rust-implemented C-compatible function used
// <https://github.com/rust-lang/rfcs/issues/1002>.
extern "C" bool __attribute__((__used__)) __attribute__((visibility("default")))
xulstore_has_value_c(const char* doc, const char* id, const char* attr) {
  return xulstore_has_value_2(doc, id, attr);
}
