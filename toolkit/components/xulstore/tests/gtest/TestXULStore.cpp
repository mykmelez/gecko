#include <stdint.h>
#include "gtest/gtest.h"
#include "nsString.h"

extern "C" uint8_t* test_xul_store();
extern "C" nsresult xulstore_set_value(nsAString& doc, nsAString& id, nsAString& attr, nsAString& value);
class XULStore;
extern "C" XULStore XUL_STORE;

TEST(XULStore, CallFromCpp) {
  auto greeting = test_xul_store();
  EXPECT_STREQ(reinterpret_cast<char*>(greeting), "hello from XUL store.");
}

TEST(XULStore, SetValue) {
  nsAutoString doc(NS_LITERAL_STRING("chrome://browser/content/example.xul"));
  nsAutoString id(NS_LITERAL_STRING("window"));
  nsAutoString attr(NS_LITERAL_STRING("width"));
  nsAutoString value(NS_LITERAL_STRING("600"));
  EXPECT_TRUE(xulstore_set_value(doc, id, attr, value) == NS_OK);
}
