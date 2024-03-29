"use strict";

const { AddonTestUtils } = ChromeUtils.import("resource://testing-common/AddonTestUtils.jsm");
const { Utils } = ChromeUtils.import("resource://services-settings/Utils.jsm");
const { RemoteSettings } = ChromeUtils.import("resource://services-settings/remote-settings.js");
const { RemoteSecuritySettings } = ChromeUtils.import("resource://gre/modules/psm/RemoteSecuritySettings.jsm");
const { OneCRLBlocklistClient } = RemoteSecuritySettings.init();

const global = this;

function run_test() {
  // Initialize app, user profile etc.
  AddonTestUtils.init(global);
  AddonTestUtils.createAppInfo("XPCShell", "xpcshell@tests.mozilla.org", "1", "");
  AddonTestUtils.promiseStartupManager().then(run_next_test);
}

add_task(async function test_uses_a_custom_signer() {
  Assert.notEqual(OneCRLBlocklistClient.signerName, RemoteSettings("not-specified").signerName);
});

add_task(async function test_has_initial_dump() {
  Assert.ok(await Utils.hasLocalDump(OneCRLBlocklistClient.bucketName, OneCRLBlocklistClient.collectionName));
});

add_task(async function test_default_jexl_filter_is_used() {
  Assert.deepEqual(OneCRLBlocklistClient.filterFunc, RemoteSettings("not-specified").filterFunc);
});

add_task({
skip_if: () => !AppConstants.MOZ_NEW_CERT_STORAGE,
}, async function test_revocations_are_updated_on_sync_with_cert_storage() {
  const certList = Cc["@mozilla.org/security/certstorage;1"]
    .getService(Ci.nsICertStorage);
  const has_revocations = () => new Promise((resolve) => {
    certList.hasPriorData(Ci.nsICertStorage.DATA_TYPE_REVOCATION, (rv, hasPriorData) => {
      if (rv == Cr.NS_OK) {
        return resolve(hasPriorData);
      }
      return resolve(false);
    });
  });

  Assert.ok(!(await has_revocations()));

  await OneCRLBlocklistClient.emit("sync", { data: {
    current: [],
    created: [{
      issuerName: "MBIxEDAOBgNVBAMMB1Rlc3QgQ0E=",
      serialNumber: "a0X7/7DlTaedpgrIJg25iBPOkIM=",
    }],
    updated: [],
    deleted: [],
  }});

  Assert.ok(await has_revocations());
});

add_task({
  skip_if: () => AppConstants.MOZ_NEW_CERT_STORAGE,
}, async function test_revocations_are_updated_on_sync() {
  const profile = do_get_profile();
  const revocations = profile.clone();
  revocations.append("revocations.txt");
  const before = revocations.exists() ? revocations.lastModifiedTime : null;

  await OneCRLBlocklistClient.emit("sync", { data: {
    current: [{
      issuerName: "MBIxEDAOBgNVBAMMB1Rlc3QgQ0E=",
      serialNumber: "a0X7/7DlTaedpgrIJg25iBPOkIM=",
    }],
    deleted: [],
    updated: [],
    created: [{
      issuerName: "MBIxEDAOBgNVBAMMB1Rlc3QgQ0E=",
      serialNumber: "a0X7/7DlTaedpgrIJg25iBPOkIM=",
    }],
  }});

  const after = revocations.lastModifiedTime;
  Assert.notEqual(before, after, "revocation file was modified.");
});
