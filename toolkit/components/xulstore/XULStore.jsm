/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

const EXPORTED_SYMBOLS = ["XULStore"];

const xulStore = Cc["@mozilla.org/xul/xulstore;1"].getService(Ci.nsIXULStore);

const {OS} = ChromeUtils.import("resource://gre/modules/osfile.jsm");
const {Services} = ChromeUtils.import("resource://gre/modules/Services.jsm");
const {XPCOMUtils} = ChromeUtils.import("resource://gre/modules/XPCOMUtils.jsm");

// Enumeration is inclusive of the lower bound and exclusive of the upper
// bound, i.e. [lower,upper).  In order to ensure that we enumerate all keys
// for a given document URI, and only those for the document URI, we enumerate
// from the document URI + the separator char (to ensure we match the URI
// exactly in the URI part of the key) until the document URI + the next char
// after the separator (to stop iteration at the first key greater than
// the document URI + the separator char).
const SEPARATOR = "\t";
const SEPARATOR_NEXT_CHAR = String.fromCharCode(SEPARATOR.charCodeAt(0) + 1);

// Enables logging.
const debugMode = false;

// A cache of XULStore data, indexed by document URL.  This exists to support
// consumers that can't use the async API, and it shouldn't be used except by
// those consumers (namely: Places).  Whenever possible, access XULStore using
// its standard async API.
//
// If you must access it synchronously, however, then (asynchronously) retrieve
// a cache of XULStore values for a given URI via XULStore.cache(), after which
// you can (synchronously) use the cache in place of XULStore's standard API.
let cache = {};

// Internal function for logging debug messages to the Error Console window
function log(message) {
  if (!debugMode)
    return;
  console.log("XULStore: " + message);
}

function makeKey(docURI, id, attr) {
  return docURI.concat(SEPARATOR, id).concat(SEPARATOR, attr);
}

const XULStore = {
  setValue: xulStore.setValue,
  hasValue: xulStore.hasValue,
  getValue: xulStore.getValue,
  removeValue: xulStore.removeValue,

  /**
   * Sets a value for a specified node's attribute, except in
   * the case below (following the original XULDocument::persist):
   * If the value is empty and if calling `hasValue` with the node's
   * document and ID and `attr` would return true, then the
   * value instead gets removed from the store (see Bug 1476680).
   *
   * @param node - DOM node
   * @param attr - attribute to store
   */
  persist(node, attr) {
    if (!node.id) {
      throw new Error("Node without ID passed into persist()");
    }

    const uri = node.ownerDocument.documentURI;
    const value = node.getAttribute(attr);

    if (node.localName == "window") {
      log("Persisting attributes to windows is handled by nsXULWindow.");
      return;
    }

    // See Bug 1476680 - we could drop the `hasValue` check so that
    // any time there's an empty attribute it gets removed from the
    // store. Since this is copying behavior from document.persist,
    // callers would need to be updated with that change.
    if (!value && xulStore.hasValue(uri, node.id, attr)) {
      xulStore.removeValue(uri, node.id, attr);
    } else {
      xulStore.setValue(uri, node.id, attr, value);
    }
  },

  getIDs(docURI) {
    return new XULStoreEnumerator(xulStore.getIDsEnumerator(docURI));
  },

  getAttributes(docURI, id) {
    return new XULStoreEnumerator(xulStore.getAttributeEnumerator(docURI, id));
  },

  removeDocument: xulStore.removeDocument,
};

class XULStoreEnumerator {
  constructor(enumerator) {
    this.enumerator = enumerator;
  }

  hasMore() {
    return this.enumerator.hasMore();
  }

  getNext() {
    return this.enumerator.getNext();
  }

  * [Symbol.iterator]() {
    while (this.enumerator.hasMore()) {
      yield (this.enumerator.getNext());
    }
  }
}
