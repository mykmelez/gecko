"use strict";

const fooNotification =
  getNotificationObject("foo", "a4f1d54a-98b7-4231-9120-5afc26545bad");
const barNotification =
  getNotificationObject("bar", "a4f1d54a-98b7-4231-9120-5afc26545bad", "baz");

let nextRequestID = 0;

async function createOldDatastore() {
  ChromeUtils.import("resource://gre/modules/osfile.jsm");
  const OLD_NOTIFICATION_STORE_PATH =
    OS.Path.join(OS.Constants.Path.profileDir, "notificationstore.json");

  const notifications = {
    [fooNotification.origin]: {
      [fooNotification.id]: fooNotification,
    },
    [barNotification.origin]: {
      [barNotification.id]: barNotification,
    },
  };

  await OS.File.writeAtomic(OLD_NOTIFICATION_STORE_PATH, JSON.stringify(notifications));
}

function run_test() {
  do_get_profile();
  // Create the old datastore before we start the notification database
  // so it has data to migrate.
  createOldDatastore().then(() => {
    startNotificationDB();
    run_next_test();
  });
}

add_test(function test_get_system_notifications() {
  const requestID = nextRequestID++;
  const msgHandler = function(message) {
    Assert.equal(requestID, message.data.requestID);
    Assert.equal(0, message.data.notifications.length);
  };

  addAndSend("Notification:GetAll", "Notification:GetAll:Return:OK", msgHandler, {
    origin: systemNotification.origin,
    requestID,
  });
});

add_test(function test_get_foo_notifications() {
  const requestID = nextRequestID++;
  const msgHandler = function(message) {
    Assert.equal(requestID, message.data.requestID);
    Assert.equal(1, message.data.notifications.length);
    Assert.deepEqual(fooNotification, message.data.notifications[0],
      "Notification data migrated");
  };

  addAndSend("Notification:GetAll", "Notification:GetAll:Return:OK", msgHandler, {
    origin: fooNotification.origin,
    requestID,
  });
});

add_test(function test_get_bar_notifications() {
  const requestID = nextRequestID++;
  const msgHandler = function(message) {
    Assert.equal(requestID, message.data.requestID);
    Assert.equal(1, message.data.notifications.length);
    Assert.deepEqual(barNotification, message.data.notifications[0],
      "Notification data migrated");
  };

  addAndSend("Notification:GetAll", "Notification:GetAll:Return:OK", msgHandler, {
    origin: barNotification.origin,
    requestID,
  });
});
