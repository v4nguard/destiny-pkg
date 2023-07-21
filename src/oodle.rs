use anyhow::anyhow;
use lazy_static::lazy_static;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::null_mut;

use shared_library::dynamic_library::DynamicLibrary;

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

pub enum OodleVersion {
    V3 = 3,
    V9 = 9,
}

impl OodleVersion {
    pub fn num(self) -> u32 {
        match self {
            OodleVersion::V3 => 3,
            OodleVersion::V9 => 9,
        }
    }
}

pub struct Oodle {
    _lib: DynamicLibrary,
    fn_decompress: *mut OodleLzDecompress,
}

unsafe impl Send for Oodle {}
unsafe impl Sync for Oodle {}

impl Oodle {
    pub fn new(version: OodleVersion) -> anyhow::Result<Oodle> {
        #[cfg(target_os = "windows")]
        let lib_path = format!("oo2core_{}_win64.dll", version.num());
        #[cfg(target_os = "linux")]
        let lib_path = format!("liblinoodle{}.so", version.num());
        #[cfg(target_os = "macos")]
        compile_error!("macOS is not supported for Oodle decompression!");

        // let lib = unsafe { Library::new(lib_path)? };
        // let fn_decompress: Symbol<'static, OodleLzDecompress> =
        //     unsafe { lib.get(b"OodleLZ_Decompress\0")? };
        let lib = DynamicLibrary::open(Some(Path::new(&lib_path))).map_err(|e| anyhow!("{e}"))?;
        let fn_decompress = unsafe {
            lib.symbol("OodleLZ_Decompress")
                .map_err(|e| anyhow!("{e}"))?
        };

        Ok(Oodle {
            _lib: lib,
            fn_decompress,
        })
    }

    pub fn decompress(&self, buffer: &[u8], output_buffer: &mut [u8]) -> i64 {
        unsafe {
            (*self.fn_decompress)(
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
}

lazy_static! {
    static ref OODLE_3: Option<Oodle> = Oodle::new(OodleVersion::V3).ok();
    static ref OODLE_9: Option<Oodle> = Oodle::new(OodleVersion::V9).ok();
}

/// Fails if the library isn't loaded
pub fn decompress_3(buffer: &[u8], output_buffer: &mut [u8]) -> anyhow::Result<i64> {
    OODLE_3
        .as_ref()
        .map(|o| o.decompress(buffer, output_buffer))
        .ok_or_else(|| anyhow::anyhow!("Oodle 3 isn't loaded!"))
}

/// Fails if the library isn't loaded
pub fn decompress_9(buffer: &[u8], output_buffer: &mut [u8]) -> anyhow::Result<i64> {
    OODLE_9
        .as_ref()
        .map(|o| o.decompress(buffer, output_buffer))
        .ok_or_else(|| anyhow::anyhow!("Oodle 9 isn't loaded!"))
}
