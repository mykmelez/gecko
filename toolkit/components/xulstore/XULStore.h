/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#ifndef XULStore_h
#define XULStore_h

// Helper Classes
#include "nsCOMPtr.h"
#include "nsString.h"

extern "C" {
void xulstore_function_marked_used();

nsresult xulstore_set_value_ns(nsAString* doc, nsAString* id, nsAString* attr,
                               nsAString* value);
bool xulstore_has_value_ns(nsAString* doc, nsAString* id, nsAString* attr);
nsresult xulstore_get_value_ns(const nsAString* doc, const nsAString* id,
                               const nsAString* attr, nsAString* value);
nsresult xulstore_remove_value_ns(const nsAString* doc, const nsAString* id,
                                  const nsAString* attr);
void* xulstore_get_ids_iterator_ns(const nsAString* doc);
void* xulstore_get_attribute_iterator_ns(const nsAString* doc,
                                         const nsAString* id);
bool xulstore_iter_has_more(void*);
nsresult xulstore_iter_get_next_ns(void*, nsAString* value);
void xulstore_iter_drop(void*);
}

#endif  // XULStore_h
