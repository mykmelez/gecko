#include <stdint.h>
#include "gtest/gtest.h"
#include "nsString.h"

extern "C" nsresult xulstore_set_value(nsAString* doc, nsAString* id, nsAString* attr, nsAString* value);
extern "C" bool xulstore_has_value(nsAString* doc, nsAString* id, nsAString* attr);
extern "C" void xulstore_get_value(const nsAString* doc, const nsAString* id, const nsAString* attr, nsAString* value);

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
