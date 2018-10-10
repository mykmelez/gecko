/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include "gtest/gtest.h"
#include "nsAppDirectoryServiceDefs.h"
#include "nsCOMPtr.h"
#include "nsDirectoryServiceUtils.h"
#include "nsIKeyValue.h"
#include "nsServiceManagerUtils.h"
#include "nsString.h"
#include "nsToolkitCompsCID.h"

namespace TestKeyValueStore {

class KeyValueStore : public ::testing::Test {
protected:
    void SetUp() override {
        mKeyValueService = do_GetService(NS_KEY_VALUE_SERVICE_CONTRACTID);
        nsresult rv = NS_GetSpecialDirectory(NS_APP_USER_PROFILE_50_DIR,
                                             getter_AddRefs(mProfileDir));
        EXPECT_TRUE(NS_SUCCEEDED(rv));
    }
public:
    nsAutoCString GetDatabasePath(nsLiteralString name) {
        nsresult rv;

        nsCOMPtr<nsIFile> databaseDir;
        rv = mProfileDir->Clone(getter_AddRefs(databaseDir));
        EXPECT_TRUE(NS_SUCCEEDED(rv));

        rv = databaseDir->Append(name);
        EXPECT_TRUE(NS_SUCCEEDED(rv));

        bool exists;
        rv = databaseDir->Exists(&exists);
        EXPECT_TRUE(NS_SUCCEEDED(rv));

        if (!exists) {
            rv = databaseDir->Create(nsIFile::DIRECTORY_TYPE, 0755);
            EXPECT_TRUE(NS_SUCCEEDED(rv));
        }

        nsAutoString path;
        rv = databaseDir->GetPath(path);
        EXPECT_TRUE(NS_SUCCEEDED(rv));

        return NS_ConvertUTF16toUTF8(path);
    }

    nsCOMPtr<nsIKeyValueService> mKeyValueService;
    nsCOMPtr<nsIFile> mProfileDir;
};

TEST_F(KeyValueStore, GetOrCreate) {
    nsresult rv;

    nsAutoCString path = GetDatabasePath(NS_LITERAL_STRING("GetOrCreate"));
    nsAutoCString name;

    nsCOMPtr<nsIKeyValueDatabase> database;
    rv = mKeyValueService->GetOrCreate(path, name, getter_AddRefs(database));
    EXPECT_TRUE(NS_SUCCEEDED(rv));
}

} // namespace TestKeyValueStore
