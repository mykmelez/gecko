/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#ifndef XULStore_h
#define XULStore_h

// Helper Classes
#include "nsCOMPtr.h"
#include "nsString.h"

namespace mozilla {
class XULStoreIterator;
};  // namespace mozilla

using mozilla::UniquePtr;
using mozilla::XULStoreIterator;

extern "C" {
nsresult xulstore_set_value(const nsAString* doc, const nsAString* id,
                            const nsAString* attr, const nsAString* value);
nsresult xulstore_has_value(const nsAString* doc, const nsAString* id,
                        const nsAString* attr, bool* has_value);
nsresult xulstore_get_value(const nsAString* doc, const nsAString* id,
                            const nsAString* attr, nsAString* value);
nsresult xulstore_remove_value(const nsAString* doc, const nsAString* id,
                               const nsAString* attr);
XULStoreIterator* xulstore_get_ids(const nsAString* doc, nsresult* result);
XULStoreIterator* xulstore_get_attrs(const nsAString* doc, const nsAString* id,
                                     nsresult* result);
bool xulstore_iter_has_more(const XULStoreIterator*);
nsresult xulstore_iter_get_next(XULStoreIterator*, nsAString* value);
void xulstore_iter_free(XULStoreIterator* iterator);
}

namespace mozilla {
class XULStoreIterator final {
 public:
  inline bool HasMore() const { return xulstore_iter_has_more(this); }
  inline nsresult GetNext(nsAString* item) {
    return xulstore_iter_get_next(this, item);
  }

  ~XULStoreIterator() {}
  static void operator delete(void* iterator) {
    xulstore_iter_free(reinterpret_cast<XULStoreIterator*>(iterator));
  }

 private:
  XULStoreIterator() = delete;
  XULStoreIterator(const XULStoreIterator&) = delete;
  XULStoreIterator& operator=(const XULStoreIterator&) = delete;
};

class XULStore final {
 public:
  static inline nsresult SetValue(const nsAString* doc, const nsAString* id,
                                  const nsAString* attr,
                                  const nsAString* value) {
    return xulstore_set_value(doc, id, attr, value);
  }
  static inline nsresult HasValue(const nsAString* doc, const nsAString* id,
                                  const nsAString* attr, bool* has_value) {
    return xulstore_has_value(doc, id, attr, has_value);
  }
  static inline nsresult GetValue(const nsAString* doc, const nsAString* id,
                                  const nsAString* attr, nsAString* value) {
    return xulstore_get_value(doc, id, attr, value);
  }
  static inline nsresult RemoveValue(const nsAString* doc, const nsAString* id,
                                     const nsAString* attr) {
    return xulstore_remove_value(doc, id, attr);
  }
  static inline nsresult GetIDs(const nsAString* doc,
                                UniquePtr<XULStoreIterator>* iter) {
    // We assign the value of iter here in C++ via a return value
    // rather than in the Rust function via an out parameter in order
    // to ensure that any old value is deleted, since the UniquePtr's
    // assignment operator won't delete the old value if the assignment
    // happens in Rust.
    nsresult result = NS_OK;
    iter->reset(xulstore_get_ids(doc, &result));
    return result;
  }
  static inline nsresult GetAttrs(const nsAString* doc, const nsAString* id,
                                  UniquePtr<XULStoreIterator>* iter) {
    // We assign the value of iter here in C++ via a return value
    // rather than in the Rust function via an out parameter in order
    // to ensure that any old value is deleted, since the UniquePtr's
    // assignment operator won't delete the old value if the assignment
    // happens in Rust.
    nsresult result = NS_OK;
    iter->reset(xulstore_get_attrs(doc, id, &result));
    return result;
  }

 private:
  XULStore() = delete;
  XULStore(const XULStore&) = delete;
  XULStore& operator=(const XULStore&) = delete;
  ~XULStore() = delete;
};
};  // namespace mozilla

#endif  // XULStore_h
