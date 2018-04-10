#include <stdint.h>
#include "gtest/gtest.h"

extern "C" uint8_t* test_xul_store();

TEST(XULStore, CallFromCpp) {
  auto greeting = test_xul_store();
  EXPECT_STREQ(reinterpret_cast<char*>(greeting), "hello from XUL store.");
}
