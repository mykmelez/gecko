/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

const EXPORTED_SYMBOLS = ["XULStore"];

// Get the nsIXULStore service to ensure that data is migrated from the old
// store (xulstore.json) to the new one before we access the new store.
const xulStore = Cc["@mozilla.org/xul/xulstore;1"].getService(Ci.nsIXULStore);

const {KeyValueService} = ChromeUtils.import("resource://gre/modules/kvstore.jsm");
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

async function getDatabase() {
  // This module should not generally be loaded before the profile dir
  // is available, but it can happen during profile migration.
  //
  // This code must be kept in sync with the code that updates the store's
  // location in toolkit/components/xulstore/src/ffi.rs.
  const profileDir = OS.Constants.Path.profileDir || OS.Constants.Path.tmpDir;
  const databaseDir = OS.Path.join(profileDir, "xulstore");
  await OS.File.makeDir(databaseDir, { from: OS.Constants.Path.profileDir });
  return KeyValueService.getOrCreate(databaseDir, "db");
}

XPCOMUtils.defineLazyGetter(this, "gDatabasePromise", getDatabase);

Services.obs.addObserver({
  async observe() {
    gDatabasePromise = getDatabase();
    cache = {};
  },
}, "profile-after-change");

const XULStore = {
  async setValue(docURI, id, attr, value) {
    return xulStore.setValue(docURI, id, attr, value);
  },

  async hasValue(docURI, id, attr) {
    return xulStore.hasValue(docURI, id, attr);
  },

  async getValue(docURI, id, attr) {
    return xulStore.getValue(docURI, id, attr);
  },

  async removeValue(docURI, id, attr) {
    return xulStore.removeValue(docURI, id, attr);
  },

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
  async persist(node, attr) {
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

  async removeDocument(docURI) {
    log("remove store values for doc=" + docURI);

    const gDatabase = await gDatabasePromise;
    const from = docURI.concat(SEPARATOR);
    const to = docURI.concat(SEPARATOR_NEXT_CHAR);
    const enumerator = await gDatabase.enumerate(from, to);

    await Promise.all(Array.from(enumerator).map(({key}) => gDatabase.delete(key)));
  },

  async cache(docURI) {
    log("cache store values for doc=" + docURI);

    const gDatabase = await gDatabasePromise;
    const from = docURI.concat(SEPARATOR);
    const to = docURI.concat(SEPARATOR_NEXT_CHAR);
    const enumerator = await gDatabase.enumerate(from, to);
    const cache = {};

    for (const {key, value} of enumerator) {
      const [, id, attr] = key.split(SEPARATOR);
        if (!(id in cache)) {
          cache[id] = {};
        }
        cache[id][attr] = value;
    }

    return new XULStoreCache(docURI, cache);
  },

  decache(docURI) {
    delete cache[docURI];
  },
};

class XULStoreCache {
  constructor(uri, cache) {
    this.uri = uri;
    this.cache = cache;
  }

  getValue(id, attr) {
    if (id in this.cache && attr in this.cache[id]) {
      return this.cache[id][attr];
    }
    return "";
  }

  setValue(id, attr, value) {
    if (!(id in this.cache)) {
      this.cache[id] = {};
    }
    this.cache[id][attr] = value;
    XULStore.setValue(this.uri, id, attr, value).catch(Cu.reportError);
  }

  removeValue(id, attr) {
    if (!(id in this.cache) || !(attr in this.cache[id])) {
      return;
    }
    delete this.cache[id][attr];
    XULStore.removeValue(this.uri, id, attr).catch(Cu.reportError);
  }
}

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
