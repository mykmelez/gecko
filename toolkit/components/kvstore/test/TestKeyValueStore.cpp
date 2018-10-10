/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include "gtest/gtest.h"
#include "nsCOMPtr.h"
#include "nsIKeyValue.h"
#include "nsServiceManagerUtils.h"
#include "nsString.h"
#include "nsToolkitCompsCID.h"

TEST(KeyValueStore, GetService) {
    nsCOMPtr<nsIKeyValueService> service = do_GetService(NS_KEY_VALUE_SERVICE_CONTRACTID);
    EXPECT_TRUE(service);
}
