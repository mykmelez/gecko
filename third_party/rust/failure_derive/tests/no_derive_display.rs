extern crate failure;
<<<<<<< HEAD
#[macro_use] extern crate failure_derive;

use std::fmt::{self, Display};
use failure::Fail;
=======
#[macro_use]
extern crate failure_derive;

use failure::Fail;
use std::fmt::{self, Display};
>>>>>>> central

#[derive(Debug, Fail)]
struct Foo;

impl Display for Foo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("An error occurred.")
    }
}

#[test]
fn handwritten_display() {
    assert!(Foo.cause().is_none());
    assert_eq!(&format!("{}", Foo)[..], "An error occurred.");
}
