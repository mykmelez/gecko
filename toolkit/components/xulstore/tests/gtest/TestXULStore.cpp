#include <stdint.h>
#include "gtest/gtest.h"
#include "nsCOMPtr.h"
#include "nsString.h"

extern "C" {
  nsresult xulstore_set_value_ns(nsAString* doc, nsAString* id, nsAString* attr, nsAString* value);
  bool xulstore_has_value_ns(nsAString* doc, nsAString* id, nsAString* attr);
  nsresult xulstore_get_value_ns(const nsAString* doc, const nsAString* id, const nsAString* attr, nsAString* value);
  nsresult xulstore_remove_value_ns(const nsAString* doc, const nsAString* id, const nsAString* attr);
  void *xulstore_get_ids_iterator_ns(const nsAString* doc);
  void *xulstore_get_attribute_iterator_ns(const nsAString* doc, const nsAString* id);
  bool xulstore_iter_has_more(void *);
  nsresult xulstore_iter_get_next_ns(void *, nsAString* value);
  void xulstore_iter_destroy(void *);
}

TEST(XULStore, SetGetValue) {
  nsAutoString doc(NS_LITERAL_STRING("SetGetValue"));
  nsAutoString id(NS_LITERAL_STRING("foo"));
  nsAutoString attr(NS_LITERAL_STRING("bar"));
  nsAutoString value;

  EXPECT_EQ(xulstore_get_value_ns(&doc, &id, &attr, &value), NS_OK);
  EXPECT_TRUE(value.EqualsASCII(""));

  {
    nsAutoString value(NS_LITERAL_STRING("baz"));
    EXPECT_EQ(xulstore_set_value_ns(&doc, &id, &attr, &value), NS_OK);
  }

  EXPECT_EQ(xulstore_get_value_ns(&doc, &id, &attr, &value), NS_OK);
  EXPECT_TRUE(value.EqualsASCII("baz"));
}

TEST(XULStore, HasValue) {
  nsAutoString doc(NS_LITERAL_STRING("HasValue"));
  nsAutoString id(NS_LITERAL_STRING("foo"));
  nsAutoString attr(NS_LITERAL_STRING("bar"));
  EXPECT_FALSE(xulstore_has_value_ns(&doc, &id, &attr));
  nsAutoString value(NS_LITERAL_STRING("baz"));
  EXPECT_EQ(xulstore_set_value_ns(&doc, &id, &attr, &value), NS_OK);
  EXPECT_TRUE(xulstore_has_value_ns(&doc, &id, &attr));
}

TEST(XULStore, RemoveValue) {
  nsAutoString doc(NS_LITERAL_STRING("RemoveValue"));
  nsAutoString id(NS_LITERAL_STRING("foo"));
  nsAutoString attr(NS_LITERAL_STRING("bar"));
  nsAutoString value(NS_LITERAL_STRING("baz"));
  EXPECT_EQ(xulstore_set_value_ns(&doc, &id, &attr, &value), NS_OK);
  EXPECT_EQ(xulstore_get_value_ns(&doc, &id, &attr, &value), NS_OK);
  EXPECT_TRUE(value.EqualsASCII("baz"));
  EXPECT_EQ(xulstore_remove_value_ns(&doc, &id, &attr), NS_OK);
  EXPECT_EQ(xulstore_get_value_ns(&doc, &id, &attr, &value), NS_OK);
  EXPECT_TRUE(value.EqualsASCII(""));
}

TEST(XULStore, GetIDsIterator) {
  nsAutoString doc(NS_LITERAL_STRING("idIterDoc"));
  nsAutoString id1(NS_LITERAL_STRING("id1"));
  nsAutoString id2(NS_LITERAL_STRING("id2"));
  nsAutoString id3(NS_LITERAL_STRING("id3"));
  nsAutoString attr(NS_LITERAL_STRING("attr"));
  nsAutoString value(NS_LITERAL_STRING("value"));

  // Confirm that the store doesn't have any IDs yet.
  void *raw = xulstore_get_ids_iterator_ns(&doc);
  EXPECT_FALSE(xulstore_iter_has_more(raw));
  xulstore_iter_destroy(raw);

  // Insert with IDs in non-alphanumeric order to confirm
  // that store will order them when iterating them.
  EXPECT_EQ(xulstore_set_value_ns(&doc, &id3, &attr, &value), NS_OK);
  EXPECT_EQ(xulstore_set_value_ns(&doc, &id1, &attr, &value), NS_OK);
  EXPECT_EQ(xulstore_set_value_ns(&doc, &id2, &attr, &value), NS_OK);

  // Insert different ID for another doc to confirm that store
  // won't return it when iterating IDs for our doc.
  nsAutoString otherDoc(NS_LITERAL_STRING("otherDoc"));
  nsAutoString otherID(NS_LITERAL_STRING("otherID"));
  EXPECT_EQ(xulstore_set_value_ns(&otherDoc, &otherID, &attr, &value), NS_OK);

  raw = xulstore_get_ids_iterator_ns(&doc);
  EXPECT_TRUE(xulstore_iter_has_more(raw));
  nsAutoString id;
  xulstore_iter_get_next_ns(raw, &id);
  EXPECT_TRUE(id.EqualsASCII("id1"));
  xulstore_iter_get_next_ns(raw, &id);
  EXPECT_TRUE(id.EqualsASCII("id2"));
  xulstore_iter_get_next_ns(raw, &id);
  EXPECT_TRUE(id.EqualsASCII("id3"));
  EXPECT_FALSE(xulstore_iter_has_more(raw));
  xulstore_iter_destroy(raw);
}

TEST(XULStore, GetAttributeIterator) {
  nsAutoString doc(NS_LITERAL_STRING("attrIterDoc"));
  nsAutoString id(NS_LITERAL_STRING("id"));
  nsAutoString attr1(NS_LITERAL_STRING("attr1"));
  nsAutoString attr2(NS_LITERAL_STRING("attr2"));
  nsAutoString attr3(NS_LITERAL_STRING("attr3"));
  nsAutoString value(NS_LITERAL_STRING("value"));

  void *raw = xulstore_get_attribute_iterator_ns(&doc, &id);
  EXPECT_FALSE(xulstore_iter_has_more(raw));
  xulstore_iter_destroy(raw);

  // Insert with attributes in non-alphanumeric order to confirm
  // that store will order them when iterating them.
  EXPECT_EQ(xulstore_set_value_ns(&doc, &id, &attr3, &value), NS_OK);
  EXPECT_EQ(xulstore_set_value_ns(&doc, &id, &attr1, &value), NS_OK);
  EXPECT_EQ(xulstore_set_value_ns(&doc, &id, &attr2, &value), NS_OK);

  // Insert different attribute for another ID to confirm that store
  // won't return it when iterating attributes for our ID.
  nsAutoString otherID(NS_LITERAL_STRING("otherID"));
  nsAutoString otherAttr(NS_LITERAL_STRING("otherAttr"));
  EXPECT_EQ(xulstore_set_value_ns(&doc, &otherID, &otherAttr, &value), NS_OK);

  raw = xulstore_get_attribute_iterator_ns(&doc, &id);
  EXPECT_TRUE(xulstore_iter_has_more(raw));
  nsAutoString attr;
  xulstore_iter_get_next_ns(raw, &attr);
  EXPECT_TRUE(attr.EqualsASCII("attr1"));
  xulstore_iter_get_next_ns(raw, &attr);
  EXPECT_TRUE(attr.EqualsASCII("attr2"));
  xulstore_iter_get_next_ns(raw, &attr);
  EXPECT_TRUE(attr.EqualsASCII("attr3"));
  EXPECT_FALSE(xulstore_iter_has_more(raw));
  xulstore_iter_destroy(raw);
}
