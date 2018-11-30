"use strict";

const fooNotification =
  getNotificationObject("foo", "a4f1d54a-98b7-4231-9120-5afc26545bad");
const barNotification =
  getNotificationObject("bar", "a4f1d54a-98b7-4231-9120-5afc26545bad", "baz");
const msg = "Notification:GetAll";
const msgReply = "Notification:GetAll:Return:OK";

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

  await OS.File.writeAtomic(OLD_NOTIFICATION_STORE_PATH,
    JSON.stringify(notifications));
}

function run_test() {
  do_get_profile();
  // Create the old datastore and populate it with data before we initialize
  // the notification database so it has data to migrate.
  createOldDatastore().then(() => {
    startNotificationDB();
    run_next_test();
  });
}

add_test(function test_get_system_notification() {
  const requestID = nextRequestID++;
  const msgHandler = function(message) {
    Assert.equal(requestID, message.data.requestID);
    Assert.equal(0, message.data.notifications.length);
  };

  addAndSend(msg, msgReply, msgHandler, {
    origin: systemNotification.origin,
    requestID,
  });
});

add_test(function test_get_foo_notification() {
  const requestID = nextRequestID++;
  const msgHandler = function(message) {
    Assert.equal(requestID, message.data.requestID);
    Assert.equal(1, message.data.notifications.length);
    Assert.deepEqual(fooNotification, message.data.notifications[0],
      "Notification data migrated");
  };

  addAndSend(msg, msgReply, msgHandler, {
    origin: fooNotification.origin,
    requestID,
  });
});

add_test(function test_get_bar_notification() {
  const requestID = nextRequestID++;
  const msgHandler = function(message) {
    Assert.equal(requestID, message.data.requestID);
    Assert.equal(1, message.data.notifications.length);
    Assert.deepEqual(barNotification, message.data.notifications[0],
      "Notification data migrated");
  };

  addAndSend(msg, msgReply, msgHandler, {
    origin: barNotification.origin,
    requestID,
  });
});
