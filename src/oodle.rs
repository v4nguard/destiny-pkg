use lazy_static::lazy_static;
use libloading::Library;
use parking_lot::RwLock;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::null_mut;
use tracing::info;

#[cfg(unix)]
use libloading::os::unix as ll_impl;
#[cfg(windows)]
use libloading::os::windows as ll_impl;

#[repr(u32)]
enum OodleLzFuzzSafe {
    No = 0,
    Yes = 1,
}

#[repr(u32)]
enum OodleLzCheckCRC {
    No = 0,
    Yes = 1,
}

#[repr(u32)]
enum OodleLzVerbosity {
    None = 0,
    Minimal = 1,
    Some = 2,
    Lots = 3,
}

#[repr(u32)]
enum OodleLzThreadPhase {
    ThreadPhase1 = 1,
    ThreadPhase2 = 2,
    ThreadPhaseAll = 3,
}

type OodleLzDecompress = unsafe extern "C" fn(
    compBuf: *const u8,
    compBufSize: i64,
    rawBuf: *mut u8,
    rawLen: i64,
    fuzzSafe: OodleLzFuzzSafe,
    checkCRC: OodleLzCheckCRC,
    verbosity: OodleLzVerbosity,
    decBufBase: *mut c_void,
    decBufSize: *mut c_void,
    fpCallback: *mut c_void,
    callbackUserData: *mut c_void,
    decoderMemory: *mut c_void,
    decoderMemorySize: *const c_void,
    threadPhase: OodleLzThreadPhase,
) -> i64;

#[derive(Clone, Copy)]
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
    _lib: Library,
    fn_decompress: ll_impl::Symbol<OodleLzDecompress>,
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

        let oodle = Self::from_path(lib_path)?;
        info!("Successfully loaded Oodle {}", version.num());

        Ok(oodle)
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Oodle> {
        let path = path.as_ref();
        let lib = unsafe { Library::new(path)? };
        let fn_decompress = unsafe {
            lib.get::<OodleLzDecompress>(b"OodleLZ_Decompress")?
                .into_raw()
        };

        info!(
            "Successfully loaded Oodle from {}",
            path.canonicalize()?.display()
        );

        Ok(Oodle {
            _lib: lib,
            fn_decompress,
        })
    }

    pub fn decompress(&self, buffer: &[u8], output_buffer: &mut [u8]) -> i64 {
        unsafe {
            (self.fn_decompress)(
                buffer.as_ptr() as *mut u8,
                buffer.len() as i64,
                output_buffer.as_mut_ptr(),
                output_buffer.len() as i64,
                OodleLzFuzzSafe::Yes,
                OodleLzCheckCRC::No,
                OodleLzVerbosity::Minimal,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                OodleLzThreadPhase::ThreadPhaseAll,
            )
        }
    }
}

lazy_static! {
    pub static ref OODLE_3: RwLock<Option<Oodle>> = RwLock::new(Oodle::new(OodleVersion::V3).ok());
    pub static ref OODLE_9: RwLock<Option<Oodle>> = RwLock::new(Oodle::new(OodleVersion::V9).ok());
}

/// Fails if the library isn't loaded
pub fn decompress_3(buffer: &[u8], output_buffer: &mut [u8]) -> anyhow::Result<i64> {
    OODLE_3
        .read()
        .as_ref()
        .map(|o| o.decompress(buffer, output_buffer))
        .ok_or_else(|| panic!("Oodle 3 isn't loaded!"))
}

/// Fails if the library isn't loaded
pub fn decompress_9(buffer: &[u8], output_buffer: &mut [u8]) -> anyhow::Result<i64> {
    OODLE_9
        .read()
        .as_ref()
        .map(|o| o.decompress(buffer, output_buffer))
        .ok_or_else(|| panic!("Oodle 9 isn't loaded!"))
}
