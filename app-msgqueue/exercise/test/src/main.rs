#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;
extern crate alloc;

use alloc::vec::Vec;

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    println!("Running bump tests...");

    const N: usize = 3_000_000;
    let mut v = Vec::with_capacity(N);
    for i in 0..N {
        v.push(i);
    }
    v.sort();
    for i in 0..N - 1 {
        assert!(v[i] <= v[i + 1]);
    }

    println!("Bump tests run OK!");
}
