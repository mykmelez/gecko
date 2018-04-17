#include <stdint.h>
#include "gtest/gtest.h"
#include "nsString.h"

extern "C" uint8_t* test_xul_store();
class XULStore;
extern "C" XULStore XUL_STORE;

TEST(XULStore, CallFromCpp) {
  auto greeting = test_xul_store();
  EXPECT_STREQ(reinterpret_cast<char*>(greeting), "hello from XUL store.");
}

extern "C" nsresult xulstore_set_value(nsAString* doc, nsAString* id, nsAString* attr, nsAString* value);
TEST(XULStore, SetValue) {
  nsAutoString doc(NS_LITERAL_STRING("chrome://browser/content/example.xul"));
  nsAutoString id(NS_LITERAL_STRING("window"));
  nsAutoString attr(NS_LITERAL_STRING("width"));
  nsAutoString value(NS_LITERAL_STRING("600"));
  EXPECT_TRUE(xulstore_set_value(&doc, &id, &attr, &value) == NS_OK);
}

extern "C" bool xulstore_has_value(nsAString* doc, nsAString* id, nsAString* attr);
TEST(XULStore, HasValue) {
  nsAutoString doc(NS_LITERAL_STRING("chrome://browser/content/example.xul"));
  nsAutoString id(NS_LITERAL_STRING("window"));
  nsAutoString attr(NS_LITERAL_STRING("width"));
  EXPECT_TRUE(xulstore_has_value(&doc, &id, &attr) == true);
}

extern "C" void xulstore_get_value(const nsAString* doc, const nsAString* id, const nsAString* attr, nsAString* value);
TEST(XULStore, GetValue) {
  nsAutoString doc(NS_LITERAL_STRING("chrome://browser/content/example.xul"));
  nsAutoString id(NS_LITERAL_STRING("window"));
  nsAutoString attr(NS_LITERAL_STRING("width"));
  nsAutoString value;
  xulstore_get_value(&doc, &id, &attr, &value);
  EXPECT_TRUE(value.EqualsASCII("Hello, World!"));
}
