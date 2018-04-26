#include <stdint.h>
#include "gtest/gtest.h"
#include "nsCOMPtr.h"
#include "nsIStringEnumerator.h"
#include "nsString.h"

extern "C" {
  nsresult xulstore_set_value(nsAString* doc, nsAString* id, nsAString* attr, nsAString* value);
  bool xulstore_has_value(nsAString* doc, nsAString* id, nsAString* attr);
  void xulstore_get_value(const nsAString* doc, const nsAString* id, const nsAString* attr, nsAString* value);
  nsresult xulstore_remove_value(const nsAString* doc, const nsAString* id, const nsAString* attr);
  void *xulstore_get_ids_iterator(const nsAString* doc);
  void xulstore_destroy_iterator(void *);
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

TEST(XULStore, GetIDsEnumerator) {
  nsAutoString doc(NS_LITERAL_STRING("GetIDsEnumerator"));
  nsAutoString id1(NS_LITERAL_STRING("foo"));
  nsAutoString id2(NS_LITERAL_STRING("bar"));
  nsAutoString id3(NS_LITERAL_STRING("baz"));
  nsAutoString attr(NS_LITERAL_STRING("attr"));
  nsAutoString value(NS_LITERAL_STRING("value"));

  void *raw = xulstore_get_ids_iterator(&doc);

  // Temporarily work around unused-variables error.
  (void)raw;

  // bool hasmore = true;
  // ids->HasMore(&hasmore);
  // EXPECT_FALSE(hasmore);

  // EXPECT_EQ(xulstore_set_value(&doc, &id1, &attr, &value), NS_OK);
  // EXPECT_EQ(xulstore_set_value(&doc, &id2, &attr, &value), NS_OK);
  // EXPECT_EQ(xulstore_set_value(&doc, &id3, &attr, &value), NS_OK);

  // rv = xulstore_get_ids_enumerator(&doc, getter_AddRefs(ids));
  // ids->HasMore(&hasmore);
  // EXPECT_TRUE(hasmore);

  // nsAutoString id;
  // ids->GetNext(id);
  // EXPECT_TRUE(id.EqualsASCII("bar"));
  // ids->GetNext(id);
  // EXPECT_TRUE(id.EqualsASCII("baz"));
  // ids->GetNext(id);
  // EXPECT_TRUE(id.EqualsASCII("foo"));

  // ids->HasMore(&hasmore);
  // EXPECT_FALSE(hasmore);
}
