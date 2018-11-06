/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

const gKeyValueService =
  Cc["@mozilla.org/key-value-service;1"].getService(Ci.nsIKeyValueService);

const EXPORTED_SYMBOLS = [
  "KeyValueDatabase",
  "KeyValueEnumerator",
  "KeyValueIterator",
];

function promisify(fn, ...args) {
  return new Promise((resolve, reject) => {
    fn({ handleResult: resolve, handleError: reject }, ...args);
  });
}

class KeyValueEnumerator {
  constructor(enumerator) {
    this.enumerator = enumerator;
  }

  hasMoreElements() {
    return promisify(this.enumerator.hasMoreElementsAsync);
  }

  getNext() {
    return new Promise((resolve, reject) => {
      this.enumerator.getNextAsync({
        handleResult(key, value) { resolve({ key, value }) },
        handleError(error) { reject(error) },
      });
    });
  }
}

async function* KeyValueIterator(enumerator) {
  enumerator = await enumerator;
  while (await enumerator.hasMoreElements()) {
    yield (await enumerator.getNext());
  }
}

class KeyValueDatabase {
  constructor(database) {
    this.database = database;
  }

  static async new(dir, name) {
    return new KeyValueDatabase(
      await promisify(gKeyValueService.getOrCreateAsync, dir, name)
    );
  }

  put(key, value) {
    return promisify(this.database.putAsync, key, value);
  }

  has(key) {
    return promisify(this.database.hasAsync, key);
  }

  get(key, defaultValue) {
    return promisify(this.database.getAsync, key, defaultValue);
  }

  delete(key) {
    return promisify(this.database.deleteAsync, key);
  }

  async enumerate(from_key, to_key) {
    return new KeyValueEnumerator(
      await promisify(this.database.enumerateAsync, from_key, to_key)
    );
  }
}
