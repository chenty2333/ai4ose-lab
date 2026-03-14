#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
extern crate alloc;

#[cfg(feature = "axstd")]
use alloc::string::String;
#[cfg(feature = "axstd")]
use alloc::vec;

#[cfg(feature = "axstd")]
use axstd::println;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    let s = String::from("Hello, axalloc!");
    println!("Alloc String: \"{}\"", s);

    let mut v = vec![0, 1, 2];
    v.push(3);
    println!("Alloc Vec: {:?}", v);
}
