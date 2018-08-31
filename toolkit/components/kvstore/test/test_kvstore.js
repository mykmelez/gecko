/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

"use strict";

ChromeUtils.import("resource://gre/modules/osfile.jsm");

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

add_task(async function getOrCreateDefault() {
  const databaseDir = await makeDatabaseDir("getOrCreateDefault");
  const defaultDatabase = gKeyValueService.getOrCreateDefault(databaseDir);
  Assert.ok(defaultDatabase);
});

add_task(async function putGetHasDelete() {
  const databaseDir = await makeDatabaseDir("putGetHasDelete");
  const database = gKeyValueService.getOrCreateDefault(databaseDir);

  // Getting key/value pairs that don't exist (yet) returns default values
  // or null, depending on whether you specify a default value.
  Assert.strictEqual(database.get("int-key", 1), 1);
  Assert.strictEqual(database.get("double-key", 1.1), 1.1);
  Assert.strictEqual(database.get("string-key", ""), "");
  Assert.strictEqual(database.get("bool-key", false), false);
  Assert.strictEqual(database.get("int-key"), null);
  Assert.strictEqual(database.get("double-key"), null);
  Assert.strictEqual(database.get("string-key"), null);
  Assert.strictEqual(database.get("bool-key"), null);
  Assert.strictEqual(database.getInt("int-key", 1), 1);
  Assert.strictEqual(database.getDouble("double-key", 1.1), 1.1);
  Assert.strictEqual(database.getString("string-key", ""), "");
  Assert.strictEqual(database.getBool("bool-key", false), false);

  // The put method succeeds without returning a value.
  Assert.strictEqual(database.put("int-key", 1234), undefined);
  Assert.strictEqual(database.put("double-key", 56.78), undefined);
  Assert.strictEqual(database.put("string-key", "Héllo, wőrld!"), undefined);
  Assert.strictEqual(database.put("bool-key", true), undefined);

  // Getting key/value pairs that exist returns the expected values.
  Assert.strictEqual(database.get("int-key", 1), 1234);
  Assert.strictEqual(database.get("double-key", 1.1), 56.78);
  Assert.strictEqual(database.get("string-key", ""), "Héllo, wőrld!");
  Assert.strictEqual(database.get("bool-key", false), true);
  Assert.strictEqual(database.get("int-key"), 1234);
  Assert.strictEqual(database.get("double-key"), 56.78);
  Assert.strictEqual(database.get("string-key"), "Héllo, wőrld!");
  Assert.strictEqual(database.get("bool-key"), true);
  Assert.strictEqual(database.getInt("int-key", 1), 1234);
  Assert.strictEqual(database.getDouble("double-key", 1.1), 56.78);
  Assert.strictEqual(database.getString("string-key", ""), "Héllo, wőrld!");
  Assert.strictEqual(database.getBool("bool-key", false), true);

  // You must specify a default value (per note in nsIKeyValue.idl)
  // for the type-specific getters.
  Assert.throws(() => database.getInt("any-key"), /NS_ERROR_XPC_NOT_ENOUGH_ARGS/);
  Assert.throws(() => database.getDouble("any-key"), /NS_ERROR_XPC_NOT_ENOUGH_ARGS/);
  Assert.throws(() => database.getString("any-key"), /NS_ERROR_XPC_NOT_ENOUGH_ARGS/);
  Assert.throws(() => database.getBool("any-key"), /NS_ERROR_XPC_NOT_ENOUGH_ARGS/);

  // If you specify a default value while retrieving the value of a nonexistent
  // key, then the result is the default value, no matter which getter you call.
  Assert.strictEqual(database.getInt("nonexistent-key", 1), 1);
  Assert.strictEqual(database.getDouble("nonexistent-key", 1.1), 1.1);
  Assert.strictEqual(database.getString("nonexistent-key", "Hi."), "Hi.");
  Assert.strictEqual(database.getBool("nonexistent-key", true), true);

  // Getting key/value pairs that do exist, but using the wrong getter
  // for the value's type, throws an exception.
  // NB: the error should be more accurate than NS_ERROR_NOT_IMPLEMENTED.
  Assert.throws(() => database.getString("int-key", ""), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getDouble("int-key", 1.1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getBool("int-key", false), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getInt("string-key", 1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getDouble("string-key", 1.1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getBool("string-key", false), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getInt("bool-key", 1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getDouble("bool-key", 1.1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getString("bool-key", ""), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getInt("double-key", 1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getBool("double-key", false), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.throws(() => database.getString("double-key", ""), /NS_ERROR_NOT_IMPLEMENTED/);

  // The has() method works as expected for both existing and non-existent keys.
  Assert.strictEqual(database.has("int-key"), true);
  Assert.strictEqual(database.has("double-key"), true);
  Assert.strictEqual(database.has("string-key"), true);
  Assert.strictEqual(database.has("bool-key"), true);
  Assert.strictEqual(database.has("nonexistent-key"), false);

  // The delete() method succeeds without returning a value.
  Assert.strictEqual(database.delete("int-key"), undefined);
  Assert.strictEqual(database.delete("double-key"), undefined);
  Assert.strictEqual(database.delete("string-key"), undefined);
  Assert.strictEqual(database.delete("bool-key"), undefined);

  // The has() method works as expected for a deleted key.
  Assert.strictEqual(database.has("int-key"), false);
  Assert.strictEqual(database.has("double-key"), false);
  Assert.strictEqual(database.has("string-key"), false);
  Assert.strictEqual(database.has("bool-key"), false);

  // Getting key/value pairs that were deleted returns default values.
  Assert.strictEqual(database.get("int-key", 1), 1);
  Assert.strictEqual(database.get("double-key", 1.1), 1.1);
  Assert.strictEqual(database.get("string-key", ""), "");
  Assert.strictEqual(database.get("bool-key", false), false);
  Assert.strictEqual(database.get("int-key"), null);
  Assert.strictEqual(database.get("double-key"), null);
  Assert.strictEqual(database.get("string-key"), null);
  Assert.strictEqual(database.get("bool-key"), null);
  Assert.strictEqual(database.getInt("int-key", 1), 1);
  Assert.strictEqual(database.getDouble("double-key", 1.1), 1.1);
  Assert.strictEqual(database.getString("string-key", ""), "");
  Assert.strictEqual(database.getBool("bool-key", false), false);
});

add_task(async function largeNumbers() {
  const databaseDir = await makeDatabaseDir("largeNumbers");
  const database = gKeyValueService.getOrCreateDefault(databaseDir);

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

  database.put("max-int-variant", MAX_INT_VARIANT);
  database.put("min-double-variant", MIN_DOUBLE_VARIANT);
  database.put("max-safe-integer", Number.MAX_SAFE_INTEGER);
  database.put("min-safe-integer", Number.MIN_SAFE_INTEGER);
  database.put("max-value", Number.MAX_VALUE);
  database.put("min-value", Number.MIN_VALUE);

  Assert.strictEqual(database.get("max-int-variant"), MAX_INT_VARIANT);
  Assert.strictEqual(database.getInt("max-int-variant", 1), MAX_INT_VARIANT);

  Assert.strictEqual(database.get("min-double-variant"), MIN_DOUBLE_VARIANT);
  Assert.throws(() => database.getInt("min-double-variant", 1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.strictEqual(database.getDouble("min-double-variant", 1.1), MIN_DOUBLE_VARIANT);

  Assert.strictEqual(database.get("max-safe-integer"), Number.MAX_SAFE_INTEGER);
  Assert.throws(() => database.getInt("max-safe-integer", 1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.strictEqual(database.getDouble("max-safe-integer", 1.1), Number.MAX_SAFE_INTEGER);

  Assert.strictEqual(database.get("min-safe-integer"), Number.MIN_SAFE_INTEGER);
  Assert.throws(() => database.getInt("min-safe-integer", 1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.strictEqual(database.getDouble("min-safe-integer", 1.1), Number.MIN_SAFE_INTEGER);

  Assert.strictEqual(database.get("max-value"), Number.MAX_VALUE);
  Assert.throws(() => database.getInt("max-value", 1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.strictEqual(database.getDouble("max-value", 1.1), Number.MAX_VALUE);

  Assert.strictEqual(database.get("min-value"), Number.MIN_VALUE);
  Assert.throws(() => database.getInt("min-value", 1), /NS_ERROR_NOT_IMPLEMENTED/);
  Assert.strictEqual(database.getDouble("min-value", 1.1), Number.MIN_VALUE);
});

add_task(async function getOrCreateNamedDatabases() {
  const databaseDir = await makeDatabaseDir("getOrCreateNamedDatabases");

  let fooDB = gKeyValueService.getOrCreate(databaseDir, "foo");
  Assert.ok(fooDB, "retrieval of first named database works");

  let barDB = gKeyValueService.getOrCreate(databaseDir, "bar");
  Assert.ok(barDB, "retrieval of second named database works");

  let defaultDB = gKeyValueService.getOrCreateDefault(databaseDir);
  Assert.ok(defaultDB, "retrieval of default database works");

  // Key/value pairs that are put into a database don't exist in others.
  defaultDB.put("key", 1);
  Assert.ok(!fooDB.has("key"), "the foo DB still doesn't have the key");
  fooDB.put("key", 2);
  Assert.ok(!barDB.has("key"), "the bar DB still doesn't have the key");
  barDB.put("key", 3);
  Assert.strictEqual(defaultDB.getInt("key", 0), 1, "the default DB has its KV pair");
  Assert.strictEqual(fooDB.getInt("key", 0), 2, "the foo DB has its KV pair");
  Assert.strictEqual(barDB.getInt("key", 0), 3, "the bar DB has its KV pair");

  // Key/value pairs that are deleted from a database still exist in other DBs.
  defaultDB.delete("key");
  Assert.strictEqual(fooDB.getInt("key", 0), 2, "the foo DB still has its KV pair");
  fooDB.delete("key");
  Assert.strictEqual(barDB.getInt("key", 0), 3, "the bar DB still has its KV pair");
  barDB.delete("key");

  // LMDB uses the default database to store information about named databases,
  // so it's tricky to use both in the same directory (i.e. LMDB environment).

  // If you try to put a key into the default database with the same name as
  // a named database, then the write will fail because LMDB doesn't let you
  // overwrite the key.
  Assert.throws(() => defaultDB.put("foo", 5), /NS_ERROR_FAILURE/);

  // If you try to get a key from the default database for a named database,
  // then the read will fail because rkv doesn't understand the key's data type.
  Assert.throws(() => defaultDB.get("foo"), /NS_ERROR_FAILURE/);
});

add_task(async function enumeration() {
  const databaseDir = await makeDatabaseDir("enumeration");
  const database = gKeyValueService.getOrCreateDefault(databaseDir);

  database.put("int-key", 1234);
  database.put("string-key", "Héllo, wőrld!");
  database.put("bool-key", true);

  function test(fromKey, pairs) {
    const enumerator = database.enumerate(fromKey);

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

  test(null, [
    ["bool-key", true],
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  // Enumerating from a specified key will return the subset of keys that are
  // equal to or greater than (lexicographically) the specified key (whether or
  // not the specified key itself exists).

  test("aaaaa", [
    ["bool-key", true],
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  test("ccccc", [
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  test("int-key", [
    ["int-key", 1234],
    ["string-key", "Héllo, wőrld!"],
  ]);

  test("zzzzz", []);

  // Enumerators don't implement implicit iteration, because they're implemented
  // in Rust, which doesn't support jscontext.
  //
  // This should throw an exception, but instead it crashes the application
  // TODO: file a bug about this crash.
  // Assert.throws(() => { for (let pair of database.enumerate()) {} },
  //               /NS_ERROR_NOT_IMPLEMENTED/);

  // But it's trivial to wrap them in a JavaScript iterable using a generator.
  function* KeyValueIterator(enumerator) {
    while (enumerator.hasMoreElements()) {
      yield enumerator.getNext().QueryInterface(Ci.nsIKeyValuePair);
    }
  }
  let actual = {};
  for (let { key, value } of KeyValueIterator(database.enumerate())) {
    actual[key] = value;
  }
  Assert.deepEqual(actual, {
    "bool-key": true,
    "int-key": 1234,
    "string-key": "Héllo, wőrld!",
  });

  database.delete("int-key");
  database.delete("string-key");
  database.delete("bool-key");
});
