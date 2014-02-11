# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

# Combined with build/autoconf/config.status.m4, ConfigStatus is an almost
# drop-in replacement for autoconf 2.13's config.status, with features
# borrowed from autoconf > 2.5, and additional features.

from __future__ import print_function

import logging
import os
import sys

from optparse import OptionParser

from mach.logging import LoggingManager
from mozbuild.backend.configenvironment import ConfigEnvironment
from mozbuild.backend.recursivemake import RecursiveMakeBackend
from mozbuild.frontend.emitter import TreeMetadataEmitter
from mozbuild.frontend.reader import BuildReader
from mozbuild.mozinfo import write_mozinfo


log_manager = LoggingManager()


def config_status(topobjdir='.', topsrcdir='.',
        defines=[], non_global_defines=[], substs=[], source=None):
    '''Main function, providing config.status functionality.

    Contrary to config.status, it doesn't use CONFIG_FILES or CONFIG_HEADERS
    variables.

    Without the -n option, this program acts as config.status and considers
    the current directory as the top object directory, even when config.status
    is in a different directory. It will, however, treat the directory
    containing config.status as the top object directory with the -n option.

    The --recheck option, like with the original config.status, runs configure
    again, with the options given in the "ac_configure_args" subst.

    The options to this function are passed when creating the
    ConfigEnvironment. These lists, as well as the actual wrapper script
    around this function, are meant to be generated by configure.
    See build/autoconf/config.status.m4.
    '''

    if 'CONFIG_FILES' in os.environ:
        raise Exception('Using the CONFIG_FILES environment variable is not '
            'supported.')
    if 'CONFIG_HEADERS' in os.environ:
        raise Exception('Using the CONFIG_HEADERS environment variable is not '
            'supported.')

    if not os.path.isabs(topsrcdir):
        raise Exception('topsrcdir must be defined as an absolute directory: '
            '%s' % topsrcdir)

    parser = OptionParser()
    parser.add_option('--recheck', dest='recheck', action='store_true',
                      help='update config.status by reconfiguring in the same conditions')
    parser.add_option('-v', '--verbose', dest='verbose', action='store_true',
                      help='display verbose output')
    parser.add_option('-n', dest='not_topobjdir', action='store_true',
                      help='do not consider current directory as top object directory')
    parser.add_option('-d', '--diff', action='store_true',
                      help='print diffs of changed files.')
    options, args = parser.parse_args()

    # Without -n, the current directory is meant to be the top object directory
    if not options.not_topobjdir:
        topobjdir = os.path.abspath('.')

    env = ConfigEnvironment(topsrcdir, topobjdir, defines=defines,
            non_global_defines=non_global_defines, substs=substs, source=source)

    # mozinfo.json only needs written if configure changes and configure always
    # passes this environment variable.
    if 'WRITE_MOZINFO' in os.environ:
        write_mozinfo(os.path.join(topobjdir, 'mozinfo.json'), env, os.environ)

    reader = BuildReader(env)
    emitter = TreeMetadataEmitter(env)
    backend = RecursiveMakeBackend(env)
    # This won't actually do anything because of the magic of generators.
    definitions = emitter.emit(reader.read_topsrcdir())

    if options.recheck:
        # Execute configure from the top object directory
        os.chdir(topobjdir)
        os.execlp('sh', 'sh', '-c', ' '.join([os.path.join(topsrcdir, 'configure'), env.substs['ac_configure_args'], '--no-create', '--no-recursion']))

    log_level = logging.DEBUG if options.verbose else logging.INFO
    log_manager.add_terminal_logging(level=log_level)
    log_manager.enable_unstructured()

    print('Reticulating splines...', file=sys.stderr)
    summary = backend.consume(definitions)

    for line in summary.summaries():
        print(line, file=sys.stderr)

    if options.diff:
        for path, diff in sorted(summary.file_diffs.items()):
            print(diff)
