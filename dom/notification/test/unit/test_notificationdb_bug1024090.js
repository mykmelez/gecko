"use strict";

function run_test() {
  do_get_profile();
  run_next_test();
}

// For bug 1024090: test edge case of notificationstore.json
add_task(async function test_bug1024090_purge() {
  ChromeUtils.import("resource://gre/modules/osfile.jsm");
  const NOTIFICATION_STORE_PATH =
    OS.Path.join(OS.Constants.Path.profileDir, "notificationstore");
    try {
      await OS.File.removeDir(NOTIFICATION_STORE_PATH);
      ok(true, "Notification database cleaned.");
    } catch (reason) {
      ok(false, "Notification database error when cleaning: " + reason);
    }
    info("Cleanup steps completed: " + NOTIFICATION_STORE_PATH);
    startNotificationDB();
});

// Store one notification
add_test(function test_bug1024090_send_one() {
  let requestID = 1;
  let msgReply = "Notification:Save:Return:OK";
  let msgHandler = function(message) {
    equal(requestID, message.data.requestID, "Checking requestID");
  };

  addAndSend("Notification:Save", msgReply, msgHandler, {
    origin: systemNotification.origin,
    notification: systemNotification,
    requestID,
  });
});

// Get one notification, one exists
add_test(function test_bug1024090_get_one() {
  let requestID = 2;
  let msgReply = "Notification:GetAll:Return:OK";
  let msgHandler = function(message) {
    equal(requestID, message.data.requestID, "Checking requestID");
    equal(1, message.data.notifications.length, "One notification stored");
  };

  addAndSend("Notification:GetAll", msgReply, msgHandler, {
    origin: systemNotification.origin,
    requestID,
  });
});
