/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

// Enables logging.
const debugMode = false;

const EXPORTED_SYMBOLS = ["XULStore"];

ChromeUtils.import("resource://gre/modules/kvstore.jsm");
ChromeUtils.import("resource://gre/modules/osfile.jsm");
ChromeUtils.import("resource://gre/modules/Services.jsm");
ChromeUtils.import("resource://gre/modules/XPCOMUtils.jsm");

function makeKey(docURI, id, attr) {
  return docURI.concat("\t", id).concat("\t", attr);
}

XPCOMUtils.defineLazyGetter(this, "gDatabasePromise", async function() {
  const databaseDir = OS.Path.join(OS.Constants.Path.profileDir, "xulstore");
  await OS.File.makeDir(databaseDir, { from: OS.Constants.Path.profileDir });
  return KeyValueService.getOrCreate(databaseDir);
});

let saveAllowed = true;

const XULStore = {
  /*
   * Internal function for logging debug messages to the Error Console window
   */
  log(message) {
    if (!debugMode)
      return;
    console.log("XULStore: " + message);
  },

  async setValue(docURI, id, attr, value) {
    this.log("Saving " + attr + "=" + value + " for id=" + id + ", doc=" + docURI);

    if (!saveAllowed) {
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

    const gDatabase = await gDatabasePromise;
    await gDatabase.put(makeKey(docURI, id, attr), value);
  },

  async hasValue(docURI, id, attr) {
    this.log("has store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);

    const gDatabase = await gDatabasePromise;
    return await gDatabase.has(makeKey(docURI, id, attr));
  },

  async getValue(docURI, id, attr) {
    this.log("get store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);
    const gDatabase = await gDatabasePromise;
    return await gDatabase.get(makeKey(docURI, id, attr)) || "";
  },

  async removeValue(docURI, id, attr) {
    this.log("remove store value for id=" + id + ", attr=" + attr + ", doc=" + docURI);

    if (!saveAllowed) {
      Services.console.logStringMessage("XULStore: Changes after profile-before-change are ignored!");
      return;
    }

    const gDatabase = await gDatabasePromise;
    await gDatabase.delete(makeKey(docURI, id, attr));
  },

  async getIDsEnumerator(docURI) {
    this.log("Getting ID enumerator for doc=" + docURI);
    const gDatabase = await gDatabasePromise;
    const enumerator = await gDatabase.enumerate(docURI.concat("\t"), docURI.concat("\n"));

    // IDs are the second of the three tab-delimited fields in the key.
    const ids = Array.from(enumerator).map(({key}) => key.split("\t")[1]);

    // Filter the array for uniqueness before returning it.
    return [...new Set(ids)];
  },

  async getAttributeEnumerator(docURI, id) {
    this.log("Getting attribute enumerator for id=" + id + ", doc=" + docURI);
    const gDatabase = await gDatabasePromise;
    const prefix = docURI.concat("\t", id);
    const enumerator = await gDatabase.enumerate(prefix.concat("\t"), prefix.concat("\n"));

    // Attributes are the third of the three tab-delimited fields in the key.
    const attrs = Array.from(enumerator).map(({key}) => key.split("\t")[2]);

    // Filter the array for uniqueness before returning it.
    return [...new Set(attrs)];
  },

  async persist(node, attr) {
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
    if (!value && await this.hasValue(uri, node.id, attr)) {
      await this.removeValue(uri, node.id, attr);
    } else {
      await this.setValue(uri, node.id, attr, value);
    }
  },
};
