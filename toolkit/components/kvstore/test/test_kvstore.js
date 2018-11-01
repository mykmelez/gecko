/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

"use strict";

ChromeUtils.import("resource://gre/modules/osfile.jsm");

function promisify(fn, ...args) {
  return new Promise((resolve, reject) => {
    fn({ handleResult: resolve, handleError: reject }, ...args);
  });
}

class KeyValueDatabase {
  constructor(handle) {
    this.handle = handle;
  }

  static async new(dir, name) {
    return new KeyValueDatabase(
      await promisify(gKeyValueService.getOrCreateAsync, dir, name)
      .then(database => database.QueryInterface(Ci.nsIKeyValueDatabase))
    );
  }

  put(key, value) {
    return promisify(this.handle.putAsync, key, value);
  }

  has(key) {
    return promisify(this.handle.hasAsync, key);
  }

  get(key, defaultValue) {
    return promisify(this.handle.getAsync, key, defaultValue);
  }

  delete(key) {
    return promisify(this.handle.deleteAsync, key);
  }

  enumerate(from_key, to_key) {
    return promisify(this.handle.enumerateAsync, from_key, to_key)
    .then(enumerator => enumerator.QueryInterface(Ci.nsISimpleEnumerator));
  }
}

function run_test() {
  do_get_profile();
  run_next_test();
}

async function makeDatabaseDir(name) {
  const databaseDir = OS.Path.join(OS.Constants.Path.profileDir, name);
  await OS.File.makeDir(databaseDir, { from: OS.Constants.Path.profileDir });
  return databaseDir;
}

const gKeyValueService =
  Cc["@mozilla.org/key-value-service;1"].getService(Ci.nsIKeyValueService);

add_task(async function getService() {
  Assert.ok(gKeyValueService);
});

add_task(async function getOrCreate() {
  const databaseDir = await makeDatabaseDir("getOrCreate");
  const defaultDatabase = await KeyValueDatabase.new(databaseDir);
  Assert.ok(defaultDatabase);
});

add_task(async function putGetHasDelete() {
  const databaseDir = await makeDatabaseDir("putGetHasDelete");
  const database = await KeyValueDatabase.new(databaseDir);

  // Getting key/value pairs that don't exist (yet) returns default values
  // or null, depending on whether you specify a default value.
  Assert.strictEqual(await database.get("int-key", 1), 1);
  Assert.strictEqual(await database.get("double-key", 1.1), 1.1);
  Assert.strictEqual(await database.get("string-key", ""), "");
  Assert.strictEqual(await database.get("bool-key", false), false);
  Assert.strictEqual(await database.get("int-key"), null);
  Assert.strictEqual(await database.get("double-key"), null);
  Assert.strictEqual(await database.get("string-key"), null);
  Assert.strictEqual(await database.get("bool-key"), null);
  // Assert.strictEqual(database.getInt("int-key", 1), 1);
  // Assert.strictEqual(database.getDouble("double-key", 1.1), 1.1);
  // Assert.strictEqual(database.getString("string-key", ""), "");
  // Assert.strictEqual(database.getBool("bool-key", false), false);

  // The put method succeeds without returning a value.
  Assert.strictEqual(await database.put("int-key", 1234), null);
  Assert.strictEqual(await database.put("double-key", 56.78), null);
  Assert.strictEqual(await database.put("string-key", "Héllo, wőrld!"), null);
  Assert.strictEqual(await database.put("bool-key", true), null);

  // Getting key/value pairs that exist returns the expected values.
  Assert.strictEqual(await database.get("int-key", 1), 1234);
  Assert.strictEqual(await database.get("double-key", 1.1), 56.78);
  Assert.strictEqual(await database.get("string-key", ""), "Héllo, wőrld!");
  Assert.strictEqual(await database.get("bool-key", false), true);
  Assert.strictEqual(await database.get("int-key"), 1234);
  Assert.strictEqual(await database.get("double-key"), 56.78);
  Assert.strictEqual(await database.get("string-key"), "Héllo, wőrld!");
  Assert.strictEqual(await database.get("bool-key"), true);
  // Assert.strictEqual(database.getInt("int-key", 1), 1234);
  // Assert.strictEqual(database.getDouble("double-key", 1.1), 56.78);
  // Assert.strictEqual(database.getString("string-key", ""), "Héllo, wőrld!");
  // Assert.strictEqual(database.getBool("bool-key", false), true);

  // You must specify a default value (per note in nsIKeyValue.idl)
  // for the type-specific getters.
  // Assert.throws(() => database.getInt("any-key"), /NS_ERROR_XPC_NOT_ENOUGH_ARGS/);
  // Assert.throws(() => database.getDouble("any-key"), /NS_ERROR_XPC_NOT_ENOUGH_ARGS/);
  // Assert.throws(() => database.getString("any-key"), /NS_ERROR_XPC_NOT_ENOUGH_ARGS/);
  // Assert.throws(() => database.getBool("any-key"), /NS_ERROR_XPC_NOT_ENOUGH_ARGS/);

  // If you specify a default value while retrieving the value of a nonexistent
  // key, then the result is the default value, no matter which getter you call.
  // Assert.strictEqual(database.getInt("nonexistent-key", 1), 1);
  // Assert.strictEqual(database.getDouble("nonexistent-key", 1.1), 1.1);
  // Assert.strictEqual(database.getString("nonexistent-key", "Hi."), "Hi.");
  // Assert.strictEqual(database.getBool("nonexistent-key", true), true);

  // Getting key/value pairs that do exist, but using the wrong getter
  // for the value's type, throws an exception.
  // Assert.throws(() => database.getString("int-key", ""), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getDouble("int-key", 1.1), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getBool("int-key", false), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getInt("string-key", 1), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getDouble("string-key", 1.1), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getBool("string-key", false), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getInt("bool-key", 1), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getDouble("bool-key", 1.1), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getString("bool-key", ""), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getInt("double-key", 1), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getBool("double-key", false), /NS_ERROR_UNEXPECTED/);
  // Assert.throws(() => database.getString("double-key", ""), /NS_ERROR_UNEXPECTED/);

  // The has() method works as expected for both existing and non-existent keys.
  Assert.strictEqual(await database.has("int-key"), true);
  Assert.strictEqual(await database.has("double-key"), true);
  Assert.strictEqual(await database.has("string-key"), true);
  Assert.strictEqual(await database.has("bool-key"), true);
  Assert.strictEqual(await database.has("nonexistent-key"), false);

  // The delete() method succeeds without returning a value.
  Assert.strictEqual(await database.delete("int-key"), null);
  Assert.strictEqual(await database.delete("double-key"), null);
  Assert.strictEqual(await database.delete("string-key"), null);
  Assert.strictEqual(await database.delete("bool-key"), null);

  // The has() method works as expected for a deleted key.
  Assert.strictEqual(await database.has("int-key"), false);
  Assert.strictEqual(await database.has("double-key"), false);
  Assert.strictEqual(await database.has("string-key"), false);
  Assert.strictEqual(await database.has("bool-key"), false);

  // Getting key/value pairs that were deleted returns default values.
  Assert.strictEqual(await database.get("int-key", 1), 1);
  Assert.strictEqual(await database.get("double-key", 1.1), 1.1);
  Assert.strictEqual(await database.get("string-key", ""), "");
  Assert.strictEqual(await database.get("bool-key", false), false);
  Assert.strictEqual(await database.get("int-key"), null);
  Assert.strictEqual(await database.get("double-key"), null);
  Assert.strictEqual(await database.get("string-key"), null);
  Assert.strictEqual(await database.get("bool-key"), null);
  // Assert.strictEqual(database.getInt("int-key", 1), 1);
  // Assert.strictEqual(database.getDouble("double-key", 1.1), 1.1);
  // Assert.strictEqual(database.getString("string-key", ""), "");
  // Assert.strictEqual(database.getBool("bool-key", false), false);
});

add_task(async function largeNumbers() {
  const databaseDir = await makeDatabaseDir("largeNumbers");
  const database = await KeyValueDatabase.new(databaseDir);

  const MAX_INT_VARIANT = Math.pow(2, 31) - 1;
  const MIN_DOUBLE_VARIANT = Math.pow(2, 31);

  // It's tricky to use getInt() in script, as XPConnect (?) translates
  // integers larger than the maximum value of int32 to doubles, even though
  // nsIVariant and the key/value storage engine both support int64.
  //
  // Thus getInt() on such a value will fail (although getDouble() will work).
  // It's probably better to always use get() in script, especially since script
  // doesn't distinguish between these types, representing them all as Number.

  // Perhaps we should even mark all the type-specific getters as [noscript]
  // and only expose them to native code.

  await database.put("max-int-variant", MAX_INT_VARIANT);
  await database.put("min-double-variant", MIN_DOUBLE_VARIANT);
  await database.put("max-safe-integer", Number.MAX_SAFE_INTEGER);
  await database.put("min-safe-integer", Number.MIN_SAFE_INTEGER);
  await database.put("max-value", Number.MAX_VALUE);
  await database.put("min-value", Number.MIN_VALUE);

  Assert.strictEqual(await database.get("max-int-variant"), MAX_INT_VARIANT);
  // Assert.strictEqual(database.getInt("max-int-variant", 1), MAX_INT_VARIANT);

  Assert.strictEqual(await database.get("min-double-variant"), MIN_DOUBLE_VARIANT);
  // Assert.throws(() => database.getInt("min-double-variant", 1), /NS_ERROR_UNEXPECTED/);
  // Assert.strictEqual(database.getDouble("min-double-variant", 1.1), MIN_DOUBLE_VARIANT);

  Assert.strictEqual(await database.get("max-safe-integer"), Number.MAX_SAFE_INTEGER);
  // Assert.throws(() => database.getInt("max-safe-integer", 1), /NS_ERROR_UNEXPECTED/);
  // Assert.strictEqual(database.getDouble("max-safe-integer", 1.1), Number.MAX_SAFE_INTEGER);

  Assert.strictEqual(await database.get("min-safe-integer"), Number.MIN_SAFE_INTEGER);
  // Assert.throws(() => database.getInt("min-safe-integer", 1), /NS_ERROR_UNEXPECTED/);
  // Assert.strictEqual(database.getDouble("min-safe-integer", 1.1), Number.MIN_SAFE_INTEGER);

  Assert.strictEqual(await database.get("max-value"), Number.MAX_VALUE);
  // Assert.throws(() => database.getInt("max-value", 1), /NS_ERROR_UNEXPECTED/);
  // Assert.strictEqual(database.getDouble("max-value", 1.1), Number.MAX_VALUE);

  Assert.strictEqual(await database.get("min-value"), Number.MIN_VALUE);
  // Assert.throws(() => database.getInt("min-value", 1), /NS_ERROR_UNEXPECTED/);
  // Assert.strictEqual(database.getDouble("min-value", 1.1), Number.MIN_VALUE);
});

add_task(async function extendedCharacterKey() {
  const databaseDir = await makeDatabaseDir("extendedCharacterKey");
  const database = await KeyValueDatabase.new(databaseDir);

  // Ensure that we can use extended character (i.e. non-ASCII) strings as keys.

  await database.put("Héllo, wőrld!", 1);
  Assert.strictEqual(await database.has("Héllo, wőrld!"), true);
  Assert.strictEqual(await database.get("Héllo, wőrld!"), 1);

  const enumerator = await database.enumerate();
  const key = enumerator.getNext().QueryInterface(Ci.nsIKeyValuePair).key;
  Assert.strictEqual(key, "Héllo, wőrld!");

  await database.delete("Héllo, wőrld!");
});

add_task(async function getOrCreateNamedDatabases() {
  const databaseDir = await makeDatabaseDir("getOrCreateNamedDatabases");

  let fooDB = await KeyValueDatabase.new(databaseDir, "foo");
  Assert.ok(fooDB, "retrieval of first named database works");

  let barDB = await KeyValueDatabase.new(databaseDir, "bar");
  Assert.ok(barDB, "retrieval of second named database works");

  let defaultDB = await KeyValueDatabase.new(databaseDir);
  Assert.ok(defaultDB, "retrieval of default database works");

  // Key/value pairs that are put into a database don't exist in others.
  await defaultDB.put("key", 1);
  Assert.ok(!(await fooDB.has("key")), "the foo DB still doesn't have the key");
  await fooDB.put("key", 2);
  Assert.ok(!(await barDB.has("key")), "the bar DB still doesn't have the key");
  await barDB.put("key", 3);
  Assert.strictEqual(await defaultDB.get("key", 0), 1, "the default DB has its KV pair");
  Assert.strictEqual(await fooDB.get("key", 0), 2, "the foo DB has its KV pair");
  Assert.strictEqual(await barDB.get("key", 0), 3, "the bar DB has its KV pair");

  // Key/value pairs that are deleted from a database still exist in other DBs.
  await defaultDB.delete("key");
  Assert.strictEqual(await fooDB.get("key", 0), 2, "the foo DB still has its KV pair");
  await fooDB.delete("key");
  Assert.strictEqual(await barDB.get("key", 0), 3, "the bar DB still has its KV pair");
  await barDB.delete("key");

  // LMDB uses the default database to store information about named databases,
  // so it's tricky to use both in the same directory (i.e. LMDB environment).

  // If you try to put a key into the default database with the same name as
  // a named database, then the write will fail because LMDB doesn't let you
  // overwrite the key.
  await Assert.rejects(defaultDB.put("foo", 5), /LmdbError\(Incompatible\)/);

  // If you try to get a key from the default database for a named database,
  // then the read will fail because rkv doesn't understand the key's data type.
  await Assert.rejects(defaultDB.get("foo"), /DataError\(UnknownType\(0\)\)/);
});

add_task(async function enumeration() {
  const databaseDir = await makeDatabaseDir("enumeration");
  const database = gKeyValueService.getOrCreate(databaseDir);

  database.put("int-key", 1234);
  database.put("double-key", 56.78);
  database.put("string-key", "Héllo, wőrld!");
  database.put("bool-key", true);

  async function test(fromKey, toKey, pairs) {
    const enumerator = await database.enumerate(fromKey, toKey);

    for (const pair of pairs) {
      Assert.strictEqual(enumerator.hasMoreElements(), true);
      const element = enumerator.getNext().QueryInterface(Ci.nsIKeyValuePair);
      Assert.ok(element);
      Assert.strictEqual(element.key, pair[0]);
      Assert.strictEqual(element.value, pair[1]);
    }

    Assert.strictEqual(enumerator.hasMoreElements(), false);
    Assert.throws(() => enumerator.getNext(), /NS_ERROR_FAILURE/);
  }

  // Test enumeration without specifying "from" and "to" keys, which should
  // enumerate all of the pairs in the database.  This test does so explicitly
  // by passing "null", "undefined" or "" (empty string) arguments
  // for those parameters. The iterator test below also tests this implicitly
  // by not specifying arguments for those parameters.
  await test(null, null, [
    ["bool-key", true],
    ["double-key", 56.78],
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);
  await test(undefined, undefined, [
    ["bool-key", true],
    ["double-key", 56.78],
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  // The implementation doesn't distinguish between a null/undefined value
  // and an empty string, so enumerating pairs from "" to "" has the same effect
  // as enumerating pairs without specifying from/to keys: it enumerates
  // all of the pairs in the database.
  await test("", "", [
    ["bool-key", true],
    ["double-key", 56.78],
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  // Await test enumeration from a key that doesn't exist and is lexicographically
  // less than the least key in the database, which should enumerate
  // all of the pairs in the database.
  await test("aaaaa", null, [
    ["bool-key", true],
    ["double-key", 56.78],
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  // Await test enumeration from a key that doesn't exist and is lexicographically
  // greater than the first key in the database, which should enumerate pairs
  // whose key is greater than or equal to the specified key.
  await test("ccccc", null, [
    ["double-key", 56.78],
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  // Await test enumeration from a key that does exist, which should enumerate pairs
  // whose key is greater than or equal to that key.
  await test("int-key", null, [
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  // Await test enumeration from a key that doesn't exist and is lexicographically
  // greater than the greaawait test key in the database, which should enumerate
  // none of the pairs in the database.
  await test("zzzzz", null, []);

  // Await test enumeration to a key that doesn't exist and is lexicographically
  // greater than the greaawait test key in the database, which should enumerate
  // all of the pairs in the database.
  await test(null, "zzzzz", [
    ["bool-key", true],
    ["double-key", 56.78],
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  // Await test enumeration to a key that doesn't exist and is lexicographically
  // less than the greaawait test key in the database, which should enumerate pairs
  // whose key is less than or equal to the specified key.
  await test(null, "ppppp", [
    ["bool-key", true],
    ["double-key", 56.78],
    ["int-key", 1234],
  ]);

  // Await test enumeration to a key that does exist, which should enumerate pairs
  // whose key is less than or equal to that key.
  await test(null, "int-key", [
    ["bool-key", true],
    ["double-key", 56.78],
    ["int-key", 1234],
  ]);

  // Await test enumeration to a key that doesn't exist and is lexicographically
  // less than the least key in the database, which should enumerate
  // none of the pairs in the database.
  await test(null, "aaaaa", []);

  // Await test enumeration between intermediate keys, which should enumerate
  // the pairs whose keys lie in between them.
  await test("int-key", "int-key", [
    ["int-key", 1234],
  ]);
  await test("ggggg", "ppppp", [
    ["int-key", 1234],
  ]);

  // Await test enumeration from a greater key to a lesser one, which should enumerate
  // none of the pairs in the database, even if the reverse ordering would
  // enumerate some pairs.  Consumers are responsible for ordering the "from"
  // and "to" keys such that "from" is less than or equal to "to".
  await test("ppppp", "ccccc", []);
  await test("int-key", "ccccc", []);
  await test("ppppp", "int-key", []);

  // Enumerators don't implement the JS iteration protocol, but it's trivial
  // to wrap them in an iterable using a generator.
  function* KeyValueIterator(enumerator) {
    while (enumerator.hasMoreElements()) {
      yield enumerator.getNext().QueryInterface(Ci.nsIKeyValuePair);
    }
  }
  let actual = {};
  for (let { key, value } of KeyValueIterator(await database.enumerate())) {
    actual[key] = value;
  }
  Assert.deepEqual(actual, {
    "bool-key": true,
    "double-key": 56.78,
    "int-key": 1234,
    "string-key": "Héllo, wőrld!",
  });

  database.delete("int-key");
  database.delete("double-key");
  database.delete("string-key");
  database.delete("bool-key");
});

add_task(async function getOrCreateAsync() {
  const databaseDir = await makeDatabaseDir("getOrCreateAsync");
  let defaultDatabase = await new Promise((resolve, reject) => {
    gKeyValueService.getOrCreateAsync({
      handleResult(result) {
        resolve(result);
      },
      handleError(error) {
        reject(error);
      },
    }, databaseDir);
  });

  Assert.ok(defaultDatabase);
  Assert.ok(defaultDatabase instanceof Ci.nsISupports);
  Assert.ok(defaultDatabase.QueryInterface(Ci.nsIKeyValueDatabase));
  Assert.ok(defaultDatabase instanceof Ci.nsIKeyValueDatabase);

  let namedDatabase = await new Promise((resolve, reject) => {
    gKeyValueService.getOrCreateAsync({
      handleResult(result) {
        resolve(result);
      },
      handleError(error) {
        reject(error);
      },
    }, databaseDir, "named-database");
  });

  Assert.ok(namedDatabase);
  Assert.ok(namedDatabase instanceof Ci.nsISupports);
  Assert.ok(namedDatabase.QueryInterface(Ci.nsIKeyValueDatabase));
  Assert.ok(namedDatabase instanceof Ci.nsIKeyValueDatabase);
});

add_task(async function putAsync() {
  const databaseDir = await makeDatabaseDir("getOrCreateAsync");
  let defaultDatabase = await KeyValueDatabase.new(databaseDir);

  Assert.strictEqual(await defaultDatabase.has("foo"), false);
  await defaultDatabase.put("foo", "bar");
  Assert.strictEqual(await defaultDatabase.get("foo"), "bar");
  Assert.strictEqual(await defaultDatabase.has("foo"), true);
  await defaultDatabase.delete("foo");
  Assert.strictEqual(await defaultDatabase.has("foo"), false);
});
