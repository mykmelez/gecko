/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include "mozilla/ClearOnShutdown.h"
#include "mozilla/ModuleUtils.h"
#include "nsIClassInfoImpl.h"
#include "nsIKeyValue.h"
#include "nsToolkitCompsCID.h"
#include "mozilla/storage/Variant.h"

extern "C" {
// Implemented in Rust.
nsresult KeyValueServiceConstructor(nsISupports* aOuter, REFNSIID aIID, void** aResult);
} // extern "C"

NS_DEFINE_NAMED_CID(NS_KEY_VALUE_SERVICE_CID);

const mozilla::Module::CIDEntry kKeyValueCIDs[] = {
  { &kNS_KEY_VALUE_SERVICE_CID, false, nullptr, KeyValueServiceConstructor },
  { nullptr }
};

const mozilla::Module::ContractIDEntry kKeyValueContracts[] = {
  { NS_KEY_VALUE_SERVICE_CONTRACTID, &kNS_KEY_VALUE_SERVICE_CID },
  { nullptr }
};

const mozilla::Module::CategoryEntry kKeyValueCategories[] = {
  { nullptr }
};

const mozilla::Module kKeyValueModule = {
  mozilla::Module::kVersion,
  kKeyValueCIDs,
  kKeyValueContracts,
  kKeyValueCategories
};

NSMODULE_DEFN(nsKeyValueModule) = &kKeyValueModule;

// Based on a patch in bug 1482608.
// TODO: move these to a more common location, such as storage/.
extern "C" {

using namespace mozilla::storage;

// Convenience functions to create Storage variants from Rust.
void
NS_NewStorageNullVariant(nsIVariant** aVariant)
{
  nsCOMPtr<nsIVariant> variant = new NullVariant();
  variant.forget(aVariant);
}

void
NS_NewStorageBooleanVariant(bool aValue, nsIVariant** aVariant)
{
  nsCOMPtr<nsIVariant> variant = new BooleanVariant(aValue);
  variant.forget(aVariant);
}

void
NS_NewStorageIntegerVariant(int64_t aValue, nsIVariant** aVariant)
{
  nsCOMPtr<nsIVariant> variant = new IntegerVariant(aValue);
  variant.forget(aVariant);
}

void
NS_NewStorageFloatVariant(double aValue, nsIVariant** aVariant)
{
  nsCOMPtr<nsIVariant> variant = new FloatVariant(aValue);
  variant.forget(aVariant);
}

void
NS_NewStorageTextVariant(const nsAString& aValue, nsIVariant** aVariant)
{
  nsCOMPtr<nsIVariant> variant = new TextVariant(aValue);
  variant.forget(aVariant);
}

void
NS_NewStorageUTF8TextVariant(const nsACString& aValue, nsIVariant** aVariant)
{
  nsCOMPtr<nsIVariant> variant = new UTF8TextVariant(aValue);
  variant.forget(aVariant);
}

} // extern "C"
