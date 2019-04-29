/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this file,
 * You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

const EXPORTED_SYMBOLS = [];

const {AppConstants} = ChromeUtils.import("resource://gre/modules/AppConstants.jsm");

if (AppConstants.MOZ_NEW_NOTIFICATION_STORE) {
  ChromeUtils.import("resource://gre/modules/NotificationDBNew.jsm");
} else {
  ChromeUtils.import("resource://gre/modules/NotificationDBOld.jsm");
}
