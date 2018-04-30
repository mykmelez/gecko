#include <stdint.h>
#include "gtest/gtest.h"
#include "nsCOMPtr.h"
#include "nsString.h"

extern "C" {
  nsresult xulstore_set_value(nsAString* doc, nsAString* id, nsAString* attr, nsAString* value);
  bool xulstore_has_value(nsAString* doc, nsAString* id, nsAString* attr);
  void xulstore_get_value(const nsAString* doc, const nsAString* id, const nsAString* attr, nsAString* value);
  nsresult xulstore_remove_value(const nsAString* doc, const nsAString* id, const nsAString* attr);
  void *xulstore_get_ids_iterator(const nsAString* doc);
  void *xulstore_get_attribute_iterator(const nsAString* doc);
  bool xulstore_iter_has_more(void *);
  nsresult xulstore_iter_get_next(void *, nsAString* value);
  void xulstore_iter_destroy(void *);
}

TEST(XULStore, SetGetValue) {
  nsAutoString doc(NS_LITERAL_STRING("SetGetValue"));
  nsAutoString id(NS_LITERAL_STRING("foo"));
  nsAutoString attr(NS_LITERAL_STRING("bar"));

  {
    nsAutoString value(NS_LITERAL_STRING("baz"));
    EXPECT_EQ(xulstore_set_value(&doc, &id, &attr, &value), NS_OK);
  }

  {
    nsAutoString value;
    EXPECT_TRUE(value.EqualsASCII(""));
    xulstore_get_value(&doc, &id, &attr, &value);
    EXPECT_TRUE(value.EqualsASCII("baz"));
  }
}

TEST(XULStore, HasValue) {
  nsAutoString doc(NS_LITERAL_STRING("HasValue"));
  nsAutoString id(NS_LITERAL_STRING("foo"));
  nsAutoString attr(NS_LITERAL_STRING("bar"));
  EXPECT_FALSE(xulstore_has_value(&doc, &id, &attr));
  nsAutoString value(NS_LITERAL_STRING("baz"));
  EXPECT_EQ(xulstore_set_value(&doc, &id, &attr, &value), NS_OK);
  EXPECT_TRUE(xulstore_has_value(&doc, &id, &attr));
}

TEST(XULStore, GetMissingValue) {
  nsAutoString doc(NS_LITERAL_STRING("GetMissingValue"));
  nsAutoString id(NS_LITERAL_STRING("foo"));
  nsAutoString attr(NS_LITERAL_STRING("bar"));
  nsAutoString value;
  xulstore_get_value(&doc, &id, &attr, &value);
  EXPECT_TRUE(value.EqualsASCII(""));
}

TEST(XULStore, RemoveValue) {
  nsAutoString doc(NS_LITERAL_STRING("RemoveValue"));
  nsAutoString id(NS_LITERAL_STRING("foo"));
  nsAutoString attr(NS_LITERAL_STRING("bar"));
  nsAutoString value(NS_LITERAL_STRING("baz"));
  EXPECT_EQ(xulstore_set_value(&doc, &id, &attr, &value), NS_OK);
  xulstore_get_value(&doc, &id, &attr, &value);
  EXPECT_TRUE(value.EqualsASCII("baz"));
  EXPECT_EQ(xulstore_remove_value(&doc, &id, &attr), NS_OK);
  xulstore_get_value(&doc, &id, &attr, &value);
  EXPECT_TRUE(value.EqualsASCII(""));
}

TEST(XULStore, GetIDsIterator) {
  nsAutoString doc(NS_LITERAL_STRING("GetIDsIterator"));
  // We insert them out of order and assert that rkv will return them in order.
  nsAutoString id1(NS_LITERAL_STRING("id1"));
  nsAutoString id2(NS_LITERAL_STRING("id3"));
  nsAutoString id3(NS_LITERAL_STRING("id2"));
  nsAutoString attr(NS_LITERAL_STRING("attr"));
  nsAutoString value(NS_LITERAL_STRING("value"));

  void *raw = xulstore_get_ids_iterator(&doc);
  EXPECT_FALSE(xulstore_iter_has_more(raw));
  xulstore_iter_destroy(raw);

  EXPECT_EQ(xulstore_set_value(&doc, &id1, &attr, &value), NS_OK);
  EXPECT_EQ(xulstore_set_value(&doc, &id2, &attr, &value), NS_OK);
  EXPECT_EQ(xulstore_set_value(&doc, &id3, &attr, &value), NS_OK);

  raw = xulstore_get_ids_iterator(&doc);
  EXPECT_TRUE(xulstore_iter_has_more(raw));
  nsAutoString id;
  xulstore_iter_get_next(raw, &id);
  EXPECT_TRUE(id.EqualsASCII("id1"));
  xulstore_iter_get_next(raw, &id);
  EXPECT_TRUE(id.EqualsASCII("id2"));
  xulstore_iter_get_next(raw, &id);
  EXPECT_TRUE(id.EqualsASCII("id3"));
  EXPECT_FALSE(xulstore_iter_has_more(raw));
  xulstore_iter_destroy(raw);
}

TEST(XULStore, GetAttributeIterator) {
  nsAutoString doc(NS_LITERAL_STRING("GetAttributeIterator"));
  nsAutoString id(NS_LITERAL_STRING("id"));
  // We insert them out of order and assert that rkv will return them in order.
  nsAutoString attr1(NS_LITERAL_STRING("attr1"));
  nsAutoString attr2(NS_LITERAL_STRING("attr3"));
  nsAutoString attr3(NS_LITERAL_STRING("attr2"));
  nsAutoString value(NS_LITERAL_STRING("value"));

  void *raw = xulstore_get_attribute_iterator(&doc);
  EXPECT_FALSE(xulstore_iter_has_more(raw));
  xulstore_iter_destroy(raw);

  EXPECT_EQ(xulstore_set_value(&doc, &id, &attr1, &value), NS_OK);
  EXPECT_EQ(xulstore_set_value(&doc, &id, &attr2, &value), NS_OK);
  EXPECT_EQ(xulstore_set_value(&doc, &id, &attr3, &value), NS_OK);

  raw = xulstore_get_attribute_iterator(&doc);
  EXPECT_TRUE(xulstore_iter_has_more(raw));
  nsAutoString attr;
  xulstore_iter_get_next(raw, &attr);
  EXPECT_TRUE(attr.EqualsASCII("attr1"));
  xulstore_iter_get_next(raw, &attr);
  EXPECT_TRUE(attr.EqualsASCII("attr2"));
  xulstore_iter_get_next(raw, &attr);
  EXPECT_TRUE(attr.EqualsASCII("attr3"));
  EXPECT_FALSE(xulstore_iter_has_more(raw));
  xulstore_iter_destroy(raw);
}
