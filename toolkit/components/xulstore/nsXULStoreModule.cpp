/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include "mozilla/ModuleUtils.h"
#include "nsIClassInfoImpl.h"
#include "nsIXULStore.h"
#include "nsToolkitCompsCID.h"

extern "C" {
// Implemented in Rust.
nsresult nsXULStoreServiceConstructor(nsISupports* aOuter, REFNSIID aIID,
                                      void** aResult);
}  // extern "C"

NS_DEFINE_NAMED_CID(NS_XUL_STORE_SERVICE_CID);

const mozilla::Module::CIDEntry kXULStoreCIDs[] = {
    {&kNS_XUL_STORE_SERVICE_CID, false, nullptr, nsXULStoreServiceConstructor},
    {nullptr}};

const mozilla::Module::ContractIDEntry kXULStoreContracts[] = {
    {NS_XUL_STORE_SERVICE_CONTRACTID, &kNS_XUL_STORE_SERVICE_CID}, {nullptr}};

extern const mozilla::Module kXULStoreModule = {
    mozilla::Module::kVersion, kXULStoreCIDs, kXULStoreContracts, nullptr};
