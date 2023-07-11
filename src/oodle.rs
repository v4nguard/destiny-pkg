use lazy_static::lazy_static;
use std::ffi::c_void;
use std::ptr::null_mut;

use libloading::*;

type OodleLzDecompress = unsafe extern "C" fn(
    *const u8,
    i64,
    *mut u8,
    i64,
    i32,
    i32,
    i64,
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *const c_void,
    i32,
) -> i64;

#[cfg(target_os = "macos")]
compile_error!("macOS is not supported for Oodle decompression!");

#[cfg(target_os = "linux")]
const OODLE_PATH: &str = "liblinoodle.so";

#[cfg(target_os = "windows")]
const OODLE_PATH: &str = "oo2core_3_win64.dll";

lazy_static! {
    static ref OODLE_LIB: Library = unsafe { Library::new(OODLE_PATH).unwrap() };
    static ref OODLELZ_DECOMPRESS: Symbol<'static, OodleLzDecompress> =
        unsafe { OODLE_LIB.get(b"OodleLZ_Decompress").unwrap() };
}

pub fn decompress(buffer: &[u8], output_buffer: &mut [u8]) -> i64 {
    unsafe {
        OODLELZ_DECOMPRESS(
            buffer.as_ptr() as *mut u8,
            buffer.len() as i64,
            output_buffer.as_mut_ptr(),
            output_buffer.len() as i64,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            3,
        )
    }
}
