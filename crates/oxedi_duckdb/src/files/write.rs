//! Writing one whole file through DuckDB's file system.

use std::ffi::CString;

use libduckdb_sys as ffi;

use super::{FileHandle, FileSystem, OpenOptions, take_error};

/// Bytes handed to DuckDB per write call.
const WRITE_BLOCK: usize = 1 << 20;

/// Why a file could not be written: the step that failed and DuckDB's
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteFailure {
    /// The step, e.g. `opened for writing`.
    pub step: &'static str,
    /// What went wrong.
    pub message: String,
}

impl WriteFailure {
    fn new(step: &'static str, message: impl Into<String>) -> WriteFailure {
        WriteFailure {
            step,
            message: message.into(),
        }
    }
}

const OPEN: &str = "opened for writing";
const WRITE: &str = "written";
const CLOSE: &str = "closed";

impl OpenOptions {
    /// Options to open a file for writing, creating it when it is missing.
    fn write() -> Option<OpenOptions> {
        // SAFETY: creating open options has no precondition.
        let options = OpenOptions(unsafe { ffi::duckdb_create_file_open_options() });
        for flag in [
            ffi::duckdb_file_flag_DUCKDB_FILE_FLAG_WRITE,
            ffi::duckdb_file_flag_DUCKDB_FILE_FLAG_CREATE,
        ] {
            // SAFETY: `options.0` is the live handle created above.
            let state = unsafe { ffi::duckdb_file_open_options_set_flag(options.0, flag, true) };
            if state != ffi::duckdb_state_DuckDBSuccess {
                return None;
            }
        }
        Some(options)
    }
}

impl FileSystem {
    /// Writes `bytes` as the whole content of the file at `path`, created
    /// when it is missing. The C API cannot truncate a file, so writing
    /// starts at its first byte and a file that already holds more bytes
    /// than `bytes` is refused and left as it is.
    pub fn write_all(&self, path: &str, bytes: &[u8]) -> Result<(), WriteFailure> {
        let c_path =
            CString::new(path).map_err(|_| WriteFailure::new(OPEN, "the path holds a NUL byte"))?;
        let options = OpenOptions::write()
            .ok_or_else(|| WriteFailure::new(OPEN, "DuckDB refused the write flags"))?;
        let mut handle: ffi::duckdb_file_handle = std::ptr::null_mut();
        // SAFETY: the file system, path and options are live handles; DuckDB
        // writes a new file handle (or null) into `handle`.
        let state = unsafe {
            ffi::duckdb_file_system_open(self.0, c_path.as_ptr(), options.0, &mut handle)
        };
        if state != ffi::duckdb_state_DuckDBSuccess || handle.is_null() {
            // SAFETY: the file system handle is live; the error data it
            // returns is owned and released by `take_error`.
            let error = unsafe { ffi::duckdb_file_system_error_data(self.0) };
            return Err(WriteFailure::new(OPEN, take_error(error)));
        }
        let file = FileHandle(handle);
        // SAFETY: the file handle is live.
        let size = unsafe { ffi::duckdb_file_handle_size(file.0) };
        if usize::try_from(size).map_or(true, |size| size > bytes.len()) {
            return Err(WriteFailure::new(
                OPEN,
                format!(
                    "it already holds {size} bytes, more than the {} to write, and DuckDB's file \
                     system cannot truncate a file from an extension; remove it first, or leave \
                     USE_TMP_FILE at its default",
                    bytes.len()
                ),
            ));
        }
        for block in bytes.chunks(WRITE_BLOCK) {
            let mut rest = block;
            while !rest.is_empty() {
                let wanted = i64::try_from(rest.len()).unwrap_or(i64::MAX);
                // SAFETY: the file handle is live and `rest` holds `wanted`
                // readable bytes.
                let written =
                    unsafe { ffi::duckdb_file_handle_write(file.0, rest.as_ptr().cast(), wanted) };
                let done = usize::try_from(written).ok().filter(|done| *done > 0);
                let Some(done) = done else {
                    // SAFETY: the file handle is live; the error data it
                    // returns is owned and released by `take_error`.
                    let error = unsafe { ffi::duckdb_file_handle_error_data(file.0) };
                    return Err(WriteFailure::new(WRITE, take_error(error)));
                };
                rest = rest.get(done..).unwrap_or_default();
            }
        }
        // SAFETY: the file handle is live; closing it flushes what was
        // written. The handle is destroyed afterwards by `FileHandle`.
        let state = unsafe { ffi::duckdb_file_handle_close(file.0) };
        if state != ffi::duckdb_state_DuckDBSuccess {
            // SAFETY: as above.
            let error = unsafe { ffi::duckdb_file_handle_error_data(file.0) };
            return Err(WriteFailure::new(CLOSE, take_error(error)));
        }
        Ok(())
    }
}
