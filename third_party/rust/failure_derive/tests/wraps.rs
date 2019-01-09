extern crate failure;
<<<<<<< HEAD
#[macro_use] extern crate failure_derive;

use std::io;
use std::fmt;
=======
#[macro_use]
extern crate failure_derive;

use std::fmt;
use std::io;
>>>>>>> central

use failure::{Backtrace, Fail};

#[derive(Fail, Debug)]
#[fail(display = "An error has occurred: {}", inner)]
struct WrapError {
<<<<<<< HEAD
    #[cause] inner: io::Error,
=======
    #[fail(cause)]
    inner: io::Error,
>>>>>>> central
}

#[test]
fn wrap_error() {
    let inner = io::Error::from_raw_os_error(98);
    let err = WrapError { inner };
<<<<<<< HEAD
    assert!(err.cause().and_then(|err| err.downcast_ref::<io::Error>()).is_some());
=======
    assert!(
        err.cause()
            .and_then(|err| err.downcast_ref::<io::Error>())
            .is_some()
    );
>>>>>>> central
}

#[derive(Fail, Debug)]
#[fail(display = "An error has occurred: {}", _0)]
<<<<<<< HEAD
struct WrapTupleError(#[cause] io::Error);
=======
struct WrapTupleError(#[fail(cause)] io::Error);
>>>>>>> central

#[test]
fn wrap_tuple_error() {
    let io_error = io::Error::from_raw_os_error(98);
    let err: WrapTupleError = WrapTupleError(io_error);
<<<<<<< HEAD
    assert!(err.cause().and_then(|err| err.downcast_ref::<io::Error>()).is_some());
=======
    assert!(
        err.cause()
            .and_then(|err| err.downcast_ref::<io::Error>())
            .is_some()
    );
>>>>>>> central
}

#[derive(Fail, Debug)]
#[fail(display = "An error has occurred: {}", inner)]
struct WrapBacktraceError {
<<<<<<< HEAD
    #[cause] inner: io::Error,
=======
    #[fail(cause)]
    inner: io::Error,
>>>>>>> central
    backtrace: Backtrace,
}

#[test]
fn wrap_backtrace_error() {
    let inner = io::Error::from_raw_os_error(98);
<<<<<<< HEAD
    let err: WrapBacktraceError = WrapBacktraceError { inner, backtrace: Backtrace::new() };
    assert!(err.cause().and_then(|err| err.downcast_ref::<io::Error>()).is_some());
=======
    let err: WrapBacktraceError = WrapBacktraceError {
        inner,
        backtrace: Backtrace::new(),
    };
    assert!(
        err.cause()
            .and_then(|err| err.downcast_ref::<io::Error>())
            .is_some()
    );
>>>>>>> central
    assert!(err.backtrace().is_some());
}

#[derive(Fail, Debug)]
enum WrapEnumError {
    #[fail(display = "An error has occurred: {}", _0)]
<<<<<<< HEAD
    Io(#[cause] io::Error),
    #[fail(display = "An error has occurred: {}", inner)]
    Fmt {
        #[cause] inner: fmt::Error,
=======
    Io(#[fail(cause)] io::Error),
    #[fail(display = "An error has occurred: {}", inner)]
    Fmt {
        #[fail(cause)]
        inner: fmt::Error,
>>>>>>> central
        backtrace: Backtrace,
    },
}

#[test]
fn wrap_enum_error() {
    let io_error = io::Error::from_raw_os_error(98);
    let err: WrapEnumError = WrapEnumError::Io(io_error);
<<<<<<< HEAD
    assert!(err.cause().and_then(|err| err.downcast_ref::<io::Error>()).is_some());
    assert!(err.backtrace().is_none());
    let fmt_error = fmt::Error::default();
    let err: WrapEnumError = WrapEnumError::Fmt { inner: fmt_error, backtrace: Backtrace::new() };
    assert!(err.cause().and_then(|err| err.downcast_ref::<fmt::Error>()).is_some());
=======
    assert!(
        err.cause()
            .and_then(|err| err.downcast_ref::<io::Error>())
            .is_some()
    );
    assert!(err.backtrace().is_none());
    let fmt_error = fmt::Error::default();
    let err: WrapEnumError = WrapEnumError::Fmt {
        inner: fmt_error,
        backtrace: Backtrace::new(),
    };
    assert!(
        err.cause()
            .and_then(|err| err.downcast_ref::<fmt::Error>())
            .is_some()
    );
>>>>>>> central
    assert!(err.backtrace().is_some());
}
