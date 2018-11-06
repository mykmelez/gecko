/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

const gKeyValueService =
  Cc["@mozilla.org/key-value-service;1"].getService(Ci.nsIKeyValueService);

const EXPORTED_SYMBOLS = [
  "KeyValueDatabase",
  "KeyValueIterator",
];

function promisify(fn, ...args) {
  return new Promise((resolve, reject) => {
    fn({ handleResult: resolve, handleError: reject }, ...args);
  });
}

/**
 * A class that wraps an nsIKeyValueDatabase component in a Promise-based API.
 * To use it, call it with the database's path and (optionally) its name:
 * 
 *     let database = await KeyValueDatabase.new(path, name);
 *
 * See the docs for nsIKeyValueDatabase for more information.
 */
class KeyValueDatabase {
  constructor(database) {
    this.database = database;
  }

  static async new(dir, name) {
    return new KeyValueDatabase(
      await promisify(gKeyValueService.getOrCreate, dir, name)
    );
  }

  put(key, value) {
    return promisify(this.database.put, key, value);
  }

  has(key) {
    return promisify(this.database.has, key);
  }

  get(key, defaultValue) {
    return promisify(this.database.get, key, defaultValue);
  }

  delete(key) {
    return promisify(this.database.delete, key);
  }

  async enumerate(from_key, to_key) {
    return new KeyValueEnumerator(
      await promisify(this.database.enumerate, from_key, to_key)
    );
  }
}

/**
 * A class that wraps an nsIKeyValueEnumerator component in a Promise-based API.
 * KeyValueDatabase.enumerate() returns an instance of this class automatically.
 *
 * The easiest way to use it is to wrap it in a KeyValueIterator and iterate it
 * asynchronously using the `for await...of` statement:
 *
 * for await (let { key, value } of KeyValueIterator(database.enumerate())) {
 *     ...
 * }
 */
class KeyValueEnumerator {
  constructor(enumerator) {
    this.enumerator = enumerator;
  }

  hasMoreElements() {
    return promisify(this.enumerator.hasMoreElements);
  }

  getNext() {
    return new Promise((resolve, reject) => {
      this.enumerator.getNext({
        handleResult(key, value) { resolve({ key, value }) },
        handleError(error) { reject(error) },
      });
    });
  }
}

/**
 * A generator function that wraps a KeyValueEnumerator and implements
 * the iterator protocol, so you can iterate key/value pairs using the JS
 * `for await...of` statement.
 */
async function* KeyValueIterator(enumerator) {
  enumerator = await enumerator;
  while (await enumerator.hasMoreElements()) {
    yield (await enumerator.getNext());
  }
}
