#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]
#![allow(dead_code, overflowing_literals, unused_imports)]
#![doc(html_root_url = "https://docs.rs/openssl-sys/0.9")]

extern crate libc;

use libc::*;

pub use aes::*;
pub use asn1::*;
pub use bio::*;
pub use bn::*;
pub use cms::*;
pub use conf::*;
pub use crypto::*;
pub use dh::*;
pub use dsa::*;
pub use dtls1::*;
pub use ec::*;
pub use err::*;
pub use evp::*;
pub use hmac::*;
pub use obj_mac::*;
pub use object::*;
pub use ocsp::*;
pub use ossl_typ::*;
pub use pem::*;
pub use pkcs12::*;
pub use pkcs7::*;
pub use rand::*;
pub use rsa::*;
pub use safestack::*;
pub use sha::*;
pub use srtp::*;
pub use ssl::*;
pub use ssl3::*;
pub use stack::*;
pub use tls1::*;
pub use x509::*;
pub use x509_vfy::*;
pub use x509v3::*;

#[macro_use]
mod macros;

mod aes;
mod asn1;
mod bio;
mod bn;
mod cms;
mod conf;
mod crypto;
mod dh;
mod dsa;
mod dtls1;
mod ec;
mod err;
mod evp;
mod hmac;
mod obj_mac;
mod object;
mod ocsp;
mod ossl_typ;
mod pem;
mod pkcs12;
mod pkcs7;
mod rand;
mod rsa;
mod safestack;
mod sha;
mod srtp;
mod ssl;
mod ssl3;
mod stack;
mod tls1;
mod x509;
mod x509_vfy;
mod x509v3;

// FIXME remove
pub type PasswordCallback = unsafe extern "C" fn(
    buf: *mut c_char,
    size: c_int,
    rwflag: c_int,
    user_data: *mut c_void,
) -> c_int;

#[cfg(ossl110)]
pub fn init() {
    use std::ptr;
    use std::sync::{Once, ONCE_INIT};

    // explicitly initialize to work around https://github.com/openssl/openssl/issues/3505
    static INIT: Once = ONCE_INIT;

    INIT.call_once(|| unsafe {
        ENGINE_load_builtin_engines();
        OPENSSL_load_builtin_modules();

        OPENSSL_init_ssl(OPENSSL_INIT_LOAD_SSL_STRINGS, ptr::null_mut());
    })
}

#[cfg(not(ossl110))]
pub fn init() {
    use std::io::{self, Write};
    use std::mem;
    use std::process;
    use std::sync::{Mutex, MutexGuard, Once, ONCE_INIT};

    static mut MUTEXES: *mut Vec<Mutex<()>> = 0 as *mut Vec<Mutex<()>>;
    static mut GUARDS: *mut Vec<Option<MutexGuard<'static, ()>>> =
        0 as *mut Vec<Option<MutexGuard<'static, ()>>>;

    unsafe extern "C" fn locking_function(
        mode: c_int,
        n: c_int,
        _file: *const c_char,
        _line: c_int,
    ) {
        let mutex = &(*MUTEXES)[n as usize];

        if mode & ::CRYPTO_LOCK != 0 {
            (*GUARDS)[n as usize] = Some(mutex.lock().unwrap());
        } else {
            if let None = (*GUARDS)[n as usize].take() {
                let _ = writeln!(
                    io::stderr(),
                    "BUG: rust-openssl lock {} already unlocked, aborting",
                    n
                );
                process::abort();
            }
        }
    }

    cfg_if! {
        if #[cfg(unix)] {
            fn set_id_callback() {
                unsafe extern "C" fn thread_id() -> c_ulong {
                    ::libc::pthread_self() as c_ulong
                }

                unsafe {
                    CRYPTO_set_id_callback(thread_id);
                }
            }
        } else {
            fn set_id_callback() {}
        }
    }

    static INIT: Once = ONCE_INIT;

    INIT.call_once(|| unsafe {
        SSL_library_init();
        SSL_load_error_strings();
        OPENSSL_add_all_algorithms_noconf();

        let num_locks = ::CRYPTO_num_locks();
        let mut mutexes = Box::new(Vec::new());
        for _ in 0..num_locks {
            mutexes.push(Mutex::new(()));
        }
        MUTEXES = mem::transmute(mutexes);
        let guards: Box<Vec<Option<MutexGuard<()>>>> =
            Box::new((0..num_locks).map(|_| None).collect());
        GUARDS = mem::transmute(guards);

        CRYPTO_set_locking_callback(locking_function);
        set_id_callback();
    })
}
