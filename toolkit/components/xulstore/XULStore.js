/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// Enables logging and shorter save intervals.
const debugMode = false;

// Delay when a change is made to when the file is saved.
// 30 seconds normally, or 3 seconds for testing
const WRITE_DELAY_MS = (debugMode ? 3 : 30) * 1000;

const XULSTORE_CID = Components.ID("{6f46b6f4-c8b1-4bd4-a4fa-9ebbed0753ea}");
const STOREDB_FILENAME = "xulstore";

ChromeUtils.import("resource://gre/modules/FileUtils.jsm");
ChromeUtils.import("resource://gre/modules/Services.jsm");
ChromeUtils.import("resource://gre/modules/XPCOMUtils.jsm");

ChromeUtils.defineModuleGetter(this, "OS", "resource://gre/modules/osfile.jsm");

const gKeyValueService =
  Cc["@mozilla.org/key-value-service;1"].getService(Ci.nsIKeyValueService);

let gDatabase;

function XULStore() {
  if (!Services.appinfo.inSafeMode)
    this.load();
}

function* KeyValueIterator(enumerator) {
  while (enumerator.hasMoreElements()) {
    yield enumerator.getNext().QueryInterface(Ci.nsIKeyValuePair);
  }
}

XULStore.prototype = {
  classID: XULSTORE_CID,
  QueryInterface: ChromeUtils.generateQI([Ci.nsIObserver, Ci.nsIXULStore,
                                          Ci.nsISupportsWeakReference]),
  _xpcom_factory: XPCOMUtils.generateSingletonFactory(XULStore),

  _storeFile: null,
  _saveAllowed: true,
  _writeTimer: Cc["@mozilla.org/timer;1"].createInstance(Ci.nsITimer),

  async load() {
    Services.obs.addObserver(this, "profile-before-change", true);

    try {
      this._storeFile = Services.dirsvc.get("ProfD", Ci.nsIFile);
    } catch (ex) {
      try {
        this._storeFile = Services.dirsvc.get("ProfDS", Ci.nsIFile);
      } catch (ex) {
        throw new Error("Can't find profile directory.");
      }
    }
    this._storeFile.append(STOREDB_FILENAME);

    this.readFile();
  },

  observe(subject, topic, data) {
    if (topic == "profile-before-change") {
      this._saveAllowed = false;
    }
  },

  /*
   * Internal function for logging debug messages to the Error Console window
   */
  log(message) {
    if (!debugMode)
      return;
    console.log("XULStore: " + message);
  },

  readFile() {
    if (!this._storeFile.exists()) {
      this._storeFile.create(Ci.nsIFile.DIRECTORY_TYPE, FileUtils.PERMS_DIRECTORY);
    }

    gDatabase = gKeyValueService.getOrCreateDefault(this._storeFile.path);
  },

  /* ---------- interface implementation ---------- */

  persist(node, attr) {
    if (!node.id) {
      throw new Error("Node without ID passed into persist()");
    }

    const uri = node.ownerDocument.documentURI;
    const value = node.getAttribute(attr);

    if (node.localName == "window") {
      this.log("Persisting attributes to windows is handled by nsXULWindow.");
      return;
    }

    // See Bug 1476680 - we could drop the `hasValue` check so that
    // any time there's an empty attribute it gets removed from the
    // store. Since this is copying behavior from document.persist,
    // callers would need to be updated with that change.
    if (!value && this.hasValue(uri, node.id, attr)) {
      this.removeValue(uri, node.id, attr);
    } else {
      this.setValue(uri, node.id, attr, value);
    }
  },

  setValue(docURI, id, attr, value) {
    this.log("Saving " + attr + "=" + value + " for id=" + id + ", doc=" + docURI);

    if (!this._saveAllowed) {
      Services.console.logStringMessage("XULStore: Changes after profile-before-change are ignored!");
      return;
    }

    // bug 319846 -- don't save really long attributes or values.
    if (id.length > 512 || attr.length > 512) {
      throw Components.Exception("id or attribute name too long", Cr.NS_ERROR_ILLEGAL_VALUE);
    }

    if (value.length > 4096) {
      Services.console.logStringMessage("XULStore: Warning, truncating long attribute value");
      value = value.substr(0, 4096);
    }

    const key = [docURI, id, attr].join("\t");
    gDatabase.put(key, value);
  },

  hasValue(docURI, id, attr) {
    this.log("has store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);
    const key = [docURI, id, attr].join("\t");
    return gDatabase.has(key);
  },

  getValue(docURI, id, attr) {
    this.log("get store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);
    const key = [docURI, id, attr].join("\t");
    return gDatabase.getString(key, "");
  },

  removeValue(docURI, id, attr) {
    this.log("remove store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);

    if (!this._saveAllowed) {
      Services.console.logStringMessage("XULStore: Changes after profile-before-change are ignored!");
      return;
    }

    const key = [docURI, id, attr].join("\t");
    return gDatabase.delete(key);
  },

  removeDocument(docURI) {
    this.log("remove store values for doc=" + docURI);

    if (!this._saveAllowed) {
      Services.console.logStringMessage("XULStore: Changes after profile-before-change are ignored!");
      return;
    }

    for (let { key, value } of KeyValueIterator(gDatabase.enumerate(docURI))) {
      if (key.startsWith(docURI)) {
        gDatabase.delete(key);
      } else {
        break;
      }
    }
  },

  getIDsEnumerator(docURI) {
    this.log("Getting ID enumerator for doc=" + docURI);

    let result = new Set();
    for (let { key, value } of KeyValueIterator(gDatabase.enumerate(docURI))) {
      if (key.startsWith(docURI)) {
        let [uri, id, attr] = key.split("\t");
        result.add(id);
      } else {
        break;
      }
    }

    return new nsStringEnumerator(Array.from(result));
  },

  getAttributeEnumerator(docURI, id) {
    this.log("Getting attribute enumerator for id=" + id + ", doc=" + docURI);

    let keyPrefix = [docURI, id].join("\t");

    let attrs = new Set();
    for (let { key, value } of KeyValueIterator(gDatabase.enumerate(keyPrefix))) {
      if (key.startsWith(keyPrefix)) {
        let [uri, id, attr] = key.split("\t");
        attrs.add(attr);
      } else {
        break;
      }
    }

    return new nsStringEnumerator(Array.from(attrs));
  },
};

function nsStringEnumerator(items) {
  this._items = items;
}

nsStringEnumerator.prototype = {
  QueryInterface: ChromeUtils.generateQI([Ci.nsIStringEnumerator]),
  _nextIndex: 0,
  [Symbol.iterator]() {
    return this._items.values();
  },
  hasMore() {
    return this._nextIndex < this._items.length;
  },
  getNext() {
    if (!this.hasMore())
      throw Cr.NS_ERROR_NOT_AVAILABLE;
    return this._items[this._nextIndex++];
  },
};

this.NSGetFactory = XPCOMUtils.generateNSGetFactory([XULStore]);
