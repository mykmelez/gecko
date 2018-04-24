#include <stdint.h>
#include "gtest/gtest.h"
#include "nsString.h"

extern "C" nsresult xulstore_set_value(nsAString* doc, nsAString* id, nsAString* attr, nsAString* value);
extern "C" void xulstore_get_value(const nsAString* doc, const nsAString* id, const nsAString* attr, nsAString* value);
TEST(XULStore, SetGetValue) {
  nsAutoString doc(NS_LITERAL_STRING("chrome://browser/content/example.xul"));
  nsAutoString id(NS_LITERAL_STRING("window"));
  nsAutoString attr(NS_LITERAL_STRING("width"));

  {
    nsAutoString value(NS_LITERAL_STRING("800"));
    EXPECT_EQ(xulstore_set_value(&doc, &id, &attr, &value), NS_OK);
  }

  {
    nsAutoString value;
    xulstore_get_value(&doc, &id, &attr, &value);
    EXPECT_TRUE(value.EqualsASCII("800"));
  }
}

extern "C" bool xulstore_has_value(nsAString* doc, nsAString* id, nsAString* attr);
TEST(XULStore, HasValue) {
  nsAutoString doc(NS_LITERAL_STRING("chrome://browser/content/has-value.xul"));
  nsAutoString id(NS_LITERAL_STRING("window"));
  nsAutoString attr(NS_LITERAL_STRING("height"));
  EXPECT_FALSE(xulstore_has_value(&doc, &id, &attr));
  nsAutoString value(NS_LITERAL_STRING("600"));
  EXPECT_EQ(xulstore_set_value(&doc, &id, &attr, &value), NS_OK);
  EXPECT_TRUE(xulstore_has_value(&doc, &id, &attr));
}
