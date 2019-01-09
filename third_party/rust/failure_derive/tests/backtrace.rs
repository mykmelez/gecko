extern crate failure;
<<<<<<< HEAD
#[macro_use] extern crate failure_derive;

use failure::{Fail, Backtrace};
=======
#[macro_use]
extern crate failure_derive;

use failure::{Backtrace, Fail};
>>>>>>> central

#[derive(Fail, Debug)]
#[fail(display = "Error code: {}", code)]
struct BacktraceError {
    backtrace: Backtrace,
    code: u32,
}

#[test]
fn backtrace_error() {
<<<<<<< HEAD
    let err = BacktraceError { backtrace: Backtrace::new(), code: 7 };
=======
    let err = BacktraceError {
        backtrace: Backtrace::new(),
        code: 7,
    };
>>>>>>> central
    let s = format!("{}", err);
    assert_eq!(&s[..], "Error code: 7");
    assert!(err.backtrace().is_some());
}

#[derive(Fail, Debug)]
#[fail(display = "An error has occurred.")]
struct BacktraceTupleError(Backtrace);

#[test]
fn backtrace_tuple_error() {
    let err = BacktraceTupleError(Backtrace::new());
    let s = format!("{}", err);
    assert_eq!(&s[..], "An error has occurred.");
    assert!(err.backtrace().is_some());
}

#[derive(Fail, Debug)]
enum BacktraceEnumError {
    #[fail(display = "Error code: {}", code)]
<<<<<<< HEAD
    StructVariant {
        code: i32,
        backtrace: Backtrace,
    },
=======
    StructVariant { code: i32, backtrace: Backtrace },
>>>>>>> central
    #[fail(display = "Error: {}", _0)]
    TupleVariant(&'static str, Backtrace),
    #[fail(display = "An error has occurred.")]
    UnitVariant,
}

#[test]
fn backtrace_enum_error() {
<<<<<<< HEAD
    let err = BacktraceEnumError::StructVariant { code: 2, backtrace: Backtrace::new() };
=======
    let err = BacktraceEnumError::StructVariant {
        code: 2,
        backtrace: Backtrace::new(),
    };
>>>>>>> central
    let s = format!("{}", err);
    assert_eq!(&s[..], "Error code: 2");
    assert!(err.backtrace().is_some());
    let err = BacktraceEnumError::TupleVariant("foobar", Backtrace::new());
    let s = format!("{}", err);
    assert_eq!(&s[..], "Error: foobar");
    assert!(err.backtrace().is_some());
    let err = BacktraceEnumError::UnitVariant;
    let s = format!("{}", err);
    assert_eq!(&s[..], "An error has occurred.");
    assert!(err.backtrace().is_none());
}
