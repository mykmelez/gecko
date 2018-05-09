/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

"use strict";

ChromeUtils.import("resource://gre/modules/XULStore.jsm");

function run_test() {
  do_get_profile();
  run_next_test();
}

add_task(async function testHasValue() {
  Assert.equal(XULStoreStore.hasValue("HasValue", "foo", "bar"), false);
  Assert.equal(XULStoreStore.setValue("HasValue", "foo", "bar", "baz"), Cr.NS_OK);
  Assert.equal(XULStoreStore.hasValue("HasValue", "foo", "bar"), true);
});

add_task(async function testSetGetValue() {
  {
    let value = XULStoreStore.getValue("SetGetValue", "foo", "bar");
    Assert.equal(value.readString(), "");
    XULStoreStore.freeValue(value);
  }
  Assert.equal(XULStoreStore.setValue("SetGetValue", "foo", "bar", "baz"), Cr.NS_OK);
  {
    let value = XULStoreStore.getValue("SetGetValue", "foo", "bar");
    Assert.equal(value.readString(), "baz");
    XULStoreStore.freeValue(value);
  }
});

add_task(async function testRemoveValue() {
  Assert.equal(XULStoreStore.setValue("RemoveValue", "foo", "bar", "baz"), Cr.NS_OK);
  Assert.equal(XULStoreStore.hasValue("RemoveValue", "foo", "bar"), true);
  {
    let value = XULStoreStore.getValue("RemoveValue", "foo", "bar");
    Assert.equal(value.readString(), "baz");
    XULStoreStore.freeValue(value);
  }
  Assert.equal(XULStoreStore.removeValue("RemoveValue", "foo", "bar"), Cr.NS_OK);
  Assert.equal(XULStoreStore.hasValue("RemoveValue", "foo", "bar"), false);
  {
    let value = XULStoreStore.getValue("RemoveValue", "foo", "bar");
    Assert.equal(value.readString(), "");
    XULStoreStore.freeValue(value);
  }
});

add_task(async function testGetIDsIterator() {
  let iterPtr, value;

  iterPtr = XULStoreStore.getIDsIterator("idIterDoc");
  Assert.equal(XULStoreStore.iterHasMore(iterPtr), false);
  XULStoreStore.iterDestroy(iterPtr);

  // Insert with IDs in non-alphanumeric order to confirm
  // that store will order them when iterating them.
  Assert.equal(XULStoreStore.setValue("idIterDoc", "id3", "attr", "value"), Cr.NS_OK);
  Assert.equal(XULStoreStore.setValue("idIterDoc", "id1", "attr", "value"), Cr.NS_OK);
  Assert.equal(XULStoreStore.setValue("idIterDoc", "id2", "attr", "value"), Cr.NS_OK);

  // Insert different ID for another doc to confirm that store
  // won't return it when iterating IDs for our doc.
  Assert.equal(XULStoreStore.setValue("otherDoc", "otherID", "attr", "value"), Cr.NS_OK);

  iterPtr = XULStoreStore.getIDsIterator("idIterDoc");
  Assert.equal(XULStoreStore.iterHasMore(iterPtr), true);
  value = XULStoreStore.iterGetNext(iterPtr);
  Assert.equal(value.readString(), "id1");
  XULStoreStore.freeValue(value);
  value = XULStoreStore.iterGetNext(iterPtr);
  Assert.equal(value.readString(), "id2");
  XULStoreStore.freeValue(value);
  value = XULStoreStore.iterGetNext(iterPtr);
  Assert.equal(value.readString(), "id3");
  XULStoreStore.freeValue(value);
  Assert.equal(XULStoreStore.iterHasMore(iterPtr), false);
  XULStoreStore.iterDestroy(iterPtr);
});

add_task(async function GetAttributeIterator() {
  let iterPtr, value;

  iterPtr = XULStoreStore.getAttributeIterator("attrIterDoc", "id");
  Assert.equal(XULStoreStore.iterHasMore(iterPtr), false);
  XULStoreStore.iterDestroy(iterPtr);

  // Insert with attributes in non-alphanumeric order to confirm
  // that store will order them when iterating them.
  Assert.equal(XULStoreStore.setValue("attrIterDoc", "id", "attr3", "value"), Cr.NS_OK);
  Assert.equal(XULStoreStore.setValue("attrIterDoc", "id", "attr1", "value"), Cr.NS_OK);
  Assert.equal(XULStoreStore.setValue("attrIterDoc", "id", "attr2", "value"), Cr.NS_OK);

  // Insert different attribute for another ID to confirm that store
  // won't return it when iterating attributes for our ID.
  Assert.equal(XULStoreStore.setValue("attrIterDoc", "otherID", "otherAttr", "value"), Cr.NS_OK);

  iterPtr = XULStoreStore.getAttributeIterator("attrIterDoc", "id");
  Assert.equal(XULStoreStore.iterHasMore(iterPtr), true);
  value = XULStoreStore.iterGetNext(iterPtr);
  Assert.equal(value.readString(), "attr1");
  XULStoreStore.freeValue(value);
  value = XULStoreStore.iterGetNext(iterPtr);
  Assert.equal(value.readString(), "attr2");
  XULStoreStore.freeValue(value);
  value = XULStoreStore.iterGetNext(iterPtr);
  Assert.equal(value.readString(), "attr3");
  XULStoreStore.freeValue(value);
  Assert.equal(XULStoreStore.iterHasMore(iterPtr), false);
  XULStoreStore.iterDestroy(iterPtr);
});
