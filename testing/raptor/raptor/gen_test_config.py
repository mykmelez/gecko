# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.
from __future__ import absolute_import

import os

from logger.logger import RaptorLogger


here = os.path.abspath(os.path.dirname(__file__))
webext_dir = os.path.join(os.path.dirname(here), 'webext', 'raptor')
LOG = RaptorLogger(component='raptor-gen-test-config')


def gen_test_config(browser, test, cs_port, post_startup_delay,
                    host='127.0.0.1', b_port=0, debug_mode=0,
                    browser_cycle=1):
    LOG.info("writing test settings into background js, so webext can get it")

    data = """// this file is auto-generated by raptor, do not edit directly
function getTestConfig() {
    return {"browser": "%s",
            "cs_port": "%d",
            "test_name": "%s",
            "test_settings_url": "http://%s:%d/%s.json",
            "post_startup_delay": "%s",
            "benchmark_port": "%d",
            "host": "%s",
            "debug_mode": "%d",
            "browser_cycle": "%d"};
}

""" % (browser, cs_port, test, host, cs_port, test, post_startup_delay, b_port, host, debug_mode,
       browser_cycle)

    webext_background_script = (os.path.join(webext_dir, "auto_gen_test_config.js"))

    file = open(webext_background_script, "w")
    file.write(data)
    file.close()

    LOG.info("finished writing test config to %s" % webext_background_script)
