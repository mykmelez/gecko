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
#include "mozilla/storage/Variant.h"

// using namespace mozilla;
using namespace mozilla::storage;

namespace TestKeyValueStore {

class KeyValueStore : public ::testing::Test {
protected:
    void SetUp() override {
        mKeyValueService = do_GetService(NS_KEY_VALUE_SERVICE_CONTRACTID);
        nsresult rv = NS_GetSpecialDirectory(NS_APP_USER_PROFILE_50_DIR,
                                             getter_AddRefs(mProfileDir));
        ASSERT_TRUE(NS_SUCCEEDED(rv));
    }
public:
    nsCOMPtr<nsIKeyValueService> mKeyValueService;
    nsCOMPtr<nsIFile> mProfileDir;

    void GetProfileSubdir(const nsAString& name, nsACString& path) {
        nsresult rv;

        nsCOMPtr<nsIFile> databaseDir;
        rv = mProfileDir->Clone(getter_AddRefs(databaseDir));
        ASSERT_TRUE(NS_SUCCEEDED(rv));

        rv = databaseDir->Append(name);
        ASSERT_TRUE(NS_SUCCEEDED(rv));

        bool exists;
        rv = databaseDir->Exists(&exists);
        ASSERT_TRUE(NS_SUCCEEDED(rv));

        if (!exists) {
            rv = databaseDir->Create(nsIFile::DIRECTORY_TYPE, 0755);
            ASSERT_TRUE(NS_SUCCEEDED(rv));
        }

        nsAutoString utf16Path;
        rv = databaseDir->GetPath(utf16Path);
        ASSERT_TRUE(NS_SUCCEEDED(rv));

        CopyUTF16toUTF8(utf16Path, path);
    }
};

TEST_F(KeyValueStore, GetOrCreate) {
    nsresult rv;

    nsAutoCString path;
    GetProfileSubdir(NS_LITERAL_STRING("GetOrCreate"), path);

    nsAutoCString name;

    nsCOMPtr<nsIKeyValueDatabase> database;
    rv = mKeyValueService->GetOrCreate(path, name, getter_AddRefs(database));
    ASSERT_TRUE(NS_SUCCEEDED(rv));
}

TEST_F(KeyValueStore, PutGetHasDelete) {
    nsresult rv;

    nsAutoCString path;
    GetProfileSubdir(NS_LITERAL_STRING("PutGetHasDelete"), path);

    nsAutoCString name;

    nsCOMPtr<nsIKeyValueDatabase> database;
    rv = mKeyValueService->GetOrCreate(path, name, getter_AddRefs(database));
    ASSERT_TRUE(NS_SUCCEEDED(rv));

    int64_t defaultInt = 1;
    nsCOMPtr<nsIVariant> value;
    rv = database->Get(NS_LITERAL_CSTRING("int-key"), new IntegerVariant(defaultInt), getter_AddRefs(value));
    ASSERT_TRUE(NS_SUCCEEDED(rv));
    int64_t intValue;
    rv = value->GetAsInt64(&intValue);
    ASSERT_TRUE(NS_SUCCEEDED(rv));
    EXPECT_EQ(intValue, defaultInt);
}

} // namespace TestKeyValueStore
