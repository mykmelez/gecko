/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// Enables logging and shorter save intervals.
const debugMode = false;

// Delay when a change is made to when the file is saved.
// 30 seconds normally, or 3 seconds for testing
const WRITE_DELAY_MS = (debugMode ? 3 : 30) * 1000;

const XULSTORE_CONTRACTID = "@mozilla.org/xul/xulstore;1";
const XULSTORE_CID = Components.ID("{6f46b6f4-c8b1-4bd4-a4fa-9ebbed0753ea}");
const STOREDB_FILENAME = "xulstore.json";

ChromeUtils.import("resource://gre/modules/Services.jsm");
ChromeUtils.import("resource://gre/modules/XPCOMUtils.jsm");
ChromeUtils.import("resource://gre/modules/XULStore.jsm");

ChromeUtils.defineModuleGetter(this, "OS", "resource://gre/modules/osfile.jsm");

function XULStore() {
  if (!Services.appinfo.inSafeMode)
    this.load();
}

XULStore.prototype = {
  classID: XULSTORE_CID,
  classInfo: XPCOMUtils.generateCI({classID: XULSTORE_CID,
                                    contractID: XULSTORE_CONTRACTID,
                                    classDescription: "XULStore",
                                    interfaces: [Ci.nsIXULStore]}),
  QueryInterface: ChromeUtils.generateQI([Ci.nsIObserver, Ci.nsIXULStore,
                                          Ci.nsISupportsWeakReference]),
  _xpcom_factory: XPCOMUtils.generateSingletonFactory(XULStore),

  /* ---------- private members ---------- */

  /*
   * The format of _data is _data[docuri][elementid][attribute]. For example:
   *  {
   *      "chrome://blah/foo.xul" : {
   *                                    "main-window" : { aaa : 1, bbb : "c" },
   *                                    "barColumn"   : { ddd : 9, eee : "f" },
   *                                },
   *
   *      "chrome://foopy/b.xul" :  { ... },
   *      ...
   *  }
   */
  _data: {},
  _storeFile: null,
  _needsSaving: false,
  _saveAllowed: true,
  _writeTimer: Cc["@mozilla.org/timer;1"].createInstance(Ci.nsITimer),

  load() {
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
    dump("XULStore: " + message + "\n");
    Services.console.logStringMessage("XULStore: " + message);
  },

  /* ---------- interface implementation ---------- */

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

    XULStoreStore.setValue(docURI, id, attr, value);
  },

  hasValue(docURI, id, attr) {
    this.log("has store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);

    return XULStoreStore.hasValue(docURI, id, attr);
  },

  getValue(docURI, id, attr) {
    this.log("get store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);
    const valuePtr = XULStoreStore.getValue(docURI, id, attr);
    const value = valuePtr.readString();
    XULStoreStore.freeValue(valuePtr);
    return value;
  },

  removeValue(docURI, id, attr) {
    this.log("remove store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);

    if (!this._saveAllowed) {
      Services.console.logStringMessage("XULStore: Changes after profile-before-change are ignored!");
      return;
    }

    XULStoreStore.removeValue(docURI, id, attr);
  },

  removeDocument(docURI) {
    this.log("remove store values for doc=" + docURI);

    if (!this._saveAllowed) {
      Services.console.logStringMessage("XULStore: Changes after profile-before-change are ignored!");
      return;
    }

    // Not implemented because never used.
    // TODO: remove method from nsIXULStore.
    throw new Error(NS_ERROR_NOT_AVAILABLE);
  },

  getIDsEnumerator(docURI) {
    this.log("Getting ID enumerator for doc=" + docURI);
    return new nsStringEnumerator(XULStoreStore.getIDsIterator(docURI));
  },

  getAttributeEnumerator(docURI, id) {
    this.log("Getting attribute enumerator for id=" + id + ", doc=" + docURI);
    return new nsStringEnumerator(XULStoreStore.getAttributeIterator(docURI, id));
  }
};

// TODO: free the iterPtr when the enumerator is destroyed.
function nsStringEnumerator(iterPtr) {
  this._iterPtr = iterPtr;
}

nsStringEnumerator.prototype = {
  QueryInterface: ChromeUtils.generateQI([Ci.nsIStringEnumerator]),
  hasMore() {
    return XULStoreStore.iterHasMore(this._iterPtr);
  },
  getNext() {
    if (!this.hasMore())
      throw Cr.NS_ERROR_NOT_AVAILABLE;
    const valuePtr = XULStoreStore.iterGetNext(this._iterPtr);
    const value = valuePtr.readString();
    XULStoreStore.freeValue(valuePtr);
    return value;
  },
};

this.NSGetFactory = XPCOMUtils.generateNSGetFactory([XULStore]);
