//! Reading and writing whole files through the file system of the query's client
//! context, so every path DuckDB can open (local, `s3://`, `https://`, ...)
//! works and nothing here touches the operating system directly.
//!
//! - `mod.rs`: [`FileSystem`], opening and reading one file.
//! - `glob.rs`: expanding glob patterns into the files they match.
//! - `write.rs`: writing one whole file.

mod glob;
mod write;

pub use glob::{CallerSettings, resolve};

use std::ffi::{CStr, CString};

use libduckdb_sys as ffi;

use crate::error::ReadError;

/// Bytes asked for per read call.
const READ_BLOCK: usize = 1 << 20;

/// The client context of the query being bound, released on drop.
#[derive(Debug)]
pub struct ClientContext(ffi::duckdb_client_context);

impl ClientContext {
    /// The client context that binds `info`.
    ///
    /// # Safety
    ///
    /// `info` must be the bind info DuckDB passed to the running bind
    /// callback, and the returned value must be dropped before it returns.
    pub unsafe fn of_bind(info: ffi::duckdb_bind_info) -> Option<ClientContext> {
        let mut context: ffi::duckdb_client_context = std::ptr::null_mut();
        // SAFETY: `info` is a live bind info (caller contract); DuckDB writes
        // a new context wrapper into `context`.
        unsafe { ffi::duckdb_table_function_get_client_context(info, &mut context) };
        (!context.is_null()).then_some(ClientContext(context))
    }

    /// Takes ownership of a client context wrapper DuckDB created for the
    /// caller; `None` for a null handle.
    ///
    /// # Safety
    ///
    /// `context` must be null or a wrapper the caller owns and gives up, and
    /// the returned value must be dropped before the callback that received
    /// it returns.
    pub unsafe fn owned(context: ffi::duckdb_client_context) -> Option<ClientContext> {
        (!context.is_null()).then_some(ClientContext(context))
    }

    /// The raw handle, valid while `self` lives.
    pub fn raw(&self) -> ffi::duckdb_client_context {
        self.0
    }

    /// The context's file system. It refers to the client context itself,
    /// not to this wrapper, so it may outlive the wrapper; it must not
    /// outlive the client context, which holds for bind data.
    pub fn file_system(&self) -> Option<FileSystem> {
        // SAFETY: the context wrapper is live.
        let file_system = unsafe { ffi::duckdb_client_context_get_file_system(self.0) };
        (!file_system.is_null()).then_some(FileSystem(file_system))
    }
}

impl Drop for ClientContext {
    fn drop(&mut self) {
        // SAFETY: the wrapper was created by DuckDB and is destroyed once;
        // the client context it wraps lives on.
        unsafe { ffi::duckdb_destroy_client_context(&mut self.0) };
    }
}

/// DuckDB's file system of one client context, released on drop.
#[derive(Debug)]
pub struct FileSystem(ffi::duckdb_file_system);

impl FileSystem {
    /// Every byte of the file at `path`.
    pub fn read_all(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        let file = self.open(path)?;
        file.read_to_end(path)
    }

    /// DuckDB's message when this file system refuses `path` outright (a
    /// permission error, such as a file system the caller disabled), found
    /// by trying to open it; `None` when it does not, whether or not the
    /// path exists. A path that cannot be tried is refused too.
    pub fn refuses(&self, path: &str) -> Option<String> {
        // Both refusals below are defensive: parameter text reaches the
        // extension as a C string, so it cannot hold a NUL byte, and the read
        // flag is a constant DuckDB always accepts.
        let Ok(c_path) = CString::new(path) else {
            return Some("the pattern holds a NUL byte".to_owned());
        };
        let Some(options) = OpenOptions::read() else {
            return Some("DuckDB refused the read flag".to_owned());
        };
        let mut handle: ffi::duckdb_file_handle = std::ptr::null_mut();
        // SAFETY: the file system, path and options are live handles; DuckDB
        // writes a new file handle (or null) into `handle`.
        let state = unsafe {
            ffi::duckdb_file_system_open(self.0, c_path.as_ptr(), options.0, &mut handle)
        };
        if state == ffi::duckdb_state_DuckDBSuccess && !handle.is_null() {
            drop(FileHandle(handle));
            return None;
        }
        // SAFETY: the file system handle is live; the error data it returns
        // is owned here and released by `take_error`.
        let mut error = unsafe { ffi::duckdb_file_system_error_data(self.0) };
        // SAFETY: `error` is null or a live error data handle.
        let permission = !error.is_null()
            && unsafe { ffi::duckdb_error_data_error_type(error) }
                == ffi::duckdb_error_type_DUCKDB_ERROR_PERMISSION;
        if permission {
            Some(take_error(error))
        } else {
            // SAFETY: `error` is null or was created by DuckDB; destroyed once.
            unsafe { ffi::duckdb_destroy_error_data(&mut error) };
            None
        }
    }

    fn open(&self, path: &str) -> Result<FileHandle, ReadError> {
        let open_error = |message: String| ReadError::Open {
            file: path.to_owned(),
            message,
        };
        let c_path =
            CString::new(path).map_err(|_| open_error("the path holds a NUL byte".to_owned()))?;
        let options = OpenOptions::read()
            .ok_or_else(|| open_error("DuckDB refused the read flag".to_owned()))?;
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
            return Err(open_error(take_error(error)));
        }
        Ok(FileHandle(handle))
    }
}

impl Drop for FileSystem {
    fn drop(&mut self) {
        // SAFETY: the handle was created by DuckDB and is destroyed once.
        unsafe { ffi::duckdb_destroy_file_system(&mut self.0) };
    }
}

/// Options to open a file for reading, released on drop.
struct OpenOptions(ffi::duckdb_file_open_options);

impl OpenOptions {
    fn read() -> Option<OpenOptions> {
        // SAFETY: creating open options has no precondition.
        let options = OpenOptions(unsafe { ffi::duckdb_create_file_open_options() });
        // SAFETY: `options.0` is the live handle created above.
        let state = unsafe {
            ffi::duckdb_file_open_options_set_flag(
                options.0,
                ffi::duckdb_file_flag_DUCKDB_FILE_FLAG_READ,
                true,
            )
        };
        (state == ffi::duckdb_state_DuckDBSuccess).then_some(options)
    }
}

impl Drop for OpenOptions {
    fn drop(&mut self) {
        // SAFETY: the handle was created by DuckDB and is destroyed once.
        unsafe { ffi::duckdb_destroy_file_open_options(&mut self.0) };
    }
}

/// One open file, closed on drop.
struct FileHandle(ffi::duckdb_file_handle);

impl FileHandle {
    fn read_to_end(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        // SAFETY: the file handle is live.
        let size = unsafe { ffi::duckdb_file_handle_size(self.0) };
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(usize::try_from(size).unwrap_or(0))
            .map_err(|_| ReadError::Read {
                file: path.to_owned(),
                message: format!("{size} bytes do not fit in memory"),
            })?;
        let mut block = vec![0u8; READ_BLOCK];
        loop {
            let wanted = i64::try_from(block.len()).unwrap_or(i64::MAX);
            // SAFETY: the file handle is live and `block` has room for
            // `wanted` bytes.
            let read =
                unsafe { ffi::duckdb_file_handle_read(self.0, block.as_mut_ptr().cast(), wanted) };
            let Ok(read) = usize::try_from(read) else {
                // SAFETY: the file handle is live; the error data it returns
                // is owned and released by `take_error`.
                let error = unsafe { ffi::duckdb_file_handle_error_data(self.0) };
                return Err(ReadError::Read {
                    file: path.to_owned(),
                    message: take_error(error),
                });
            };
            if read == 0 {
                return Ok(bytes);
            }
            let Some(chunk) = block.get(..read) else {
                return Err(ReadError::Read {
                    file: path.to_owned(),
                    message: format!(
                        "DuckDB reported {read} bytes read into a {wanted}-byte block"
                    ),
                });
            };
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| ReadError::Read {
                    file: path.to_owned(),
                    message: format!("{} bytes do not fit in memory", bytes.len() + chunk.len()),
                })?;
            bytes.extend_from_slice(chunk);
        }
    }
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        // SAFETY: the handle was created by DuckDB and is destroyed (and
        // closed) once.
        unsafe { ffi::duckdb_destroy_file_handle(&mut self.0) };
    }
}

/// The message of an error data handle, which is then destroyed.
fn take_error(mut error: ffi::duckdb_error_data) -> String {
    if error.is_null() {
        return "DuckDB gave no error message".to_owned();
    }
    // SAFETY: `error` is a live error data handle; the message pointer it
    // returns is valid until the handle is destroyed below.
    let message = unsafe {
        let text = ffi::duckdb_error_data_message(error);
        if text.is_null() {
            "DuckDB gave no error message".to_owned()
        } else {
            CStr::from_ptr(text).to_string_lossy().into_owned()
        }
    };
    // SAFETY: `error` was created by DuckDB and is destroyed once.
    unsafe { ffi::duckdb_destroy_error_data(&mut error) };
    message
}

#[cfg(test)]
mod tests;
