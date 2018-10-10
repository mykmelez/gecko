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

TEST(KeyValueStore, GetService) {
    nsCOMPtr<nsIKeyValueService> service = do_GetService(NS_KEY_VALUE_SERVICE_CONTRACTID);
    EXPECT_TRUE(service);
}

TEST(KeyValueStore, GetOrCreate) {
    nsCOMPtr<nsIFile> profileDir;
    nsresult rv = NS_GetSpecialDirectory(NS_APP_USER_PROFILE_50_DIR,
                                         getter_AddRefs(profileDir));

    nsCOMPtr<nsIFile> databaseDir;
    EXPECT_TRUE(NS_SUCCEEDED(rv));

    rv = profileDir->Clone(getter_AddRefs(databaseDir));
    EXPECT_TRUE(NS_SUCCEEDED(rv));

    rv = databaseDir->Append(NS_LITERAL_STRING("GetOrCreate"));
    EXPECT_TRUE(NS_SUCCEEDED(rv));

    bool exists;
    rv = databaseDir->Exists(&exists);
    EXPECT_TRUE(NS_SUCCEEDED(rv));
    EXPECT_FALSE(exists);

    rv = databaseDir->Create(nsIFile::DIRECTORY_TYPE, 0755);
    EXPECT_TRUE(NS_SUCCEEDED(rv));

    nsAutoString path;
    rv = databaseDir->GetPath(path);
    EXPECT_TRUE(NS_SUCCEEDED(rv));

    NS_ConvertUTF16toUTF8 pathUtf8(path);
    nsAutoCString name;
    // nsAutoCString name("");

    nsCOMPtr<nsIKeyValueService> service = do_GetService(NS_KEY_VALUE_SERVICE_CONTRACTID);
    EXPECT_TRUE(service);

    nsCOMPtr<nsIKeyValueDatabase> database;
    rv = service->GetOrCreate(pathUtf8, name, getter_AddRefs(database));
    EXPECT_TRUE(NS_SUCCEEDED(rv));
    EXPECT_TRUE(database);
}
