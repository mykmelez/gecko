/* Any copyright is dedicated to the Public Domain.
   http://creativecommons.org/publicdomain/zero/1.0/◦
*/

"use strict";

const {Services} = ChromeUtils.import("resource://gre/modules/Services.jsm");
const {XULStore} = ChromeUtils.import("resource://gre/modules/XULStore.jsm");

var browserURI = "chrome://browser/content/browser.xul";
var aboutURI = "about:config";

function run_test() {
  do_get_profile();
  run_next_test();
}

async function checkValue(uri, id, attr, reference) {
  let value = await XULStore.getValue(uri, id, attr);
  Assert.equal(value, reference);
}

async function checkValueExists(uri, id, attr, exists) {
  Assert.equal(await XULStore.hasValue(uri, id, attr), exists);
}

async function getIDs(uri) {
  return Array.from(await XULStore.getIDs(uri)).sort();
}

async function getAttributes(uri, id) {
  return Array.from(await XULStore.getAttributes(uri, id)).sort();
}

function checkArrays(a, b) {
  a.sort();
  b.sort();
  Assert.equal(a.toString(), b.toString());
}

add_task(async function setup() {
  // Set a value that a future test depends on manually
  await XULStore.setValue(browserURI, "main-window", "width", "994");
});

add_task(async function testTruncation() {
  let dos = Array(8192).join("~");
  // Long id names should trigger an exception
  await Assert.rejects(XULStore.setValue(browserURI, dos, "foo", "foo"), /NS_ERROR_ILLEGAL_VALUE/);

  // Long attr names should trigger an exception
  await Assert.rejects(XULStore.setValue(browserURI, "foo", dos, "foo"), /NS_ERROR_ILLEGAL_VALUE/);

  // Long values should be truncated
  await XULStore.setValue(browserURI, "dos", "dos", dos);
  dos = await XULStore.getValue(browserURI, "dos", "dos");
  Assert.ok(dos.length == 4096);
  await XULStore.removeValue(browserURI, "dos", "dos");
});

add_task(async function testGetValue() {
  // Get non-existing property
  await checkValue(browserURI, "side-window", "height", "");

  // Get existing property
  await checkValue(browserURI, "main-window", "width", "994");
});

add_task(async function testHasValue() {
  // Check non-existing property
  await checkValueExists(browserURI, "side-window", "height", false);

  // Check existing property
  await checkValueExists(browserURI, "main-window", "width", true);
});

add_task(async function testSetValue() {
  // Set new attribute
  await checkValue(browserURI, "side-bar", "width", "");
  await XULStore.setValue(browserURI, "side-bar", "width", "1000");
  await checkValue(browserURI, "side-bar", "width", "1000");
  checkArrays(["main-window", "side-bar"], await getIDs(browserURI));
  checkArrays(["width"], await getAttributes(browserURI, "side-bar"));

  // Modify existing property
  await checkValue(browserURI, "side-bar", "width", "1000");
  await XULStore.setValue(browserURI, "side-bar", "width", "1024");
  await checkValue(browserURI, "side-bar", "width", "1024");
  checkArrays(["main-window", "side-bar"], await getIDs(browserURI));
  checkArrays(["width"], await getAttributes(browserURI, "side-bar"));

  // Add another attribute
  await checkValue(browserURI, "side-bar", "height", "");
  await XULStore.setValue(browserURI, "side-bar", "height", "1000");
  await checkValue(browserURI, "side-bar", "height", "1000");
  checkArrays(["main-window", "side-bar"], await getIDs(browserURI));
  checkArrays(["width", "height"], await getAttributes(browserURI, "side-bar"));
});

add_task(async function testRemoveValue() {
  // Remove first attribute
  await checkValue(browserURI, "side-bar", "width", "1024");
  await XULStore.removeValue(browserURI, "side-bar", "width");
  await checkValue(browserURI, "side-bar", "width", "");
  await checkValueExists(browserURI, "side-bar", "width", false);
  checkArrays(["main-window", "side-bar"], await getIDs(browserURI));
  checkArrays(["height"], await getAttributes(browserURI, "side-bar"));

  // Remove second attribute
  await checkValue(browserURI, "side-bar", "height", "1000");
  await XULStore.removeValue(browserURI, "side-bar", "height");
  await checkValue(browserURI, "side-bar", "height", "");
  checkArrays(["main-window"], await getIDs(browserURI));

  // Removing an attribute that doesn't exists shouldn't fail
  await XULStore.removeValue(browserURI, "main-window", "bar");

  // Removing from an id that doesn't exists shouldn't fail
  await XULStore.removeValue(browserURI, "foo", "bar");

  // Removing from a document that doesn't exists shouldn't fail
  let nonDocURI = "chrome://example/content/other.xul";
  await XULStore.removeValue(nonDocURI, "foo", "bar");

  // Remove all attributes in browserURI
  await XULStore.removeValue(browserURI, "addon-bar", "collapsed");
  checkArrays([], await getAttributes(browserURI, "addon-bar"));
  await XULStore.removeValue(browserURI, "main-window", "width");
  await XULStore.removeValue(browserURI, "main-window", "height");
  await XULStore.removeValue(browserURI, "main-window", "screenX");
  await XULStore.removeValue(browserURI, "main-window", "screenY");
  await XULStore.removeValue(browserURI, "main-window", "sizemode");
  checkArrays([], await getAttributes(browserURI, "main-window"));
  await XULStore.removeValue(browserURI, "sidebar-title", "value");
  checkArrays([], await getAttributes(browserURI, "sidebar-title"));
  checkArrays([], await getIDs(browserURI));

  // Remove all attributes in aboutURI
  await XULStore.removeValue(aboutURI, "prefCol", "ordinal");
  await XULStore.removeValue(aboutURI, "prefCol", "sortDirection");
  checkArrays([], await getAttributes(aboutURI, "prefCol"));
  await XULStore.removeValue(aboutURI, "lockCol", "ordinal");
  checkArrays([], await getAttributes(aboutURI, "lockCol"));
  checkArrays([], await getIDs(aboutURI));
});

add_task(async function testSetManyValues() {
  const uri = "chrome://browser/content/testSetManyValues.xul";
  const start = Date.now();
  for (let i = 0; i < 50; i++) {
    for (let j = 0; j < 50; j++) {
      await XULStore.setValue(uri, `id${i}`, `attribute${j}`, "value");
    }
  }
  const end = Date.now();
  const duration = end - start;
  dump(`${end} - ${start} = ${duration}\n`);
});
