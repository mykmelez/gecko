/* -*- Mode: C++; tab-width: 2; indent-tabs-mode: nil; c-basic-offset: 2 -*- */
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include <string>

#ifdef DEBUG
#  define PINGSENDER_LOG(s, ...) printf(s, ##__VA_ARGS__)
#else
#  define PINGSENDER_LOG(s, ...)
#endif  // DEBUG

namespace PingSender {

// System-specific function to make an HTTP POST operation
bool Post(const std::string& url, const std::string& payload);

}  // namespace PingSender
