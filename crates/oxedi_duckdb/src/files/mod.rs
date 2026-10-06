//! Reading whole files through the file system of the query's client
//! context, so every path DuckDB can open (local, `s3://`, `https://`, ...)
//! works and nothing here touches the operating system directly.

use std::ffi::{CStr, CString};

use duckdb::ffi;

use crate::error::ReadError;

/// Bytes asked for per read call.
const READ_BLOCK: usize = 1 << 20;

/// DuckDB's file system of one client context, released on drop.
#[derive(Debug)]
pub struct FileSystem(ffi::duckdb_file_system);

impl FileSystem {
    /// The file system of the client context that binds `info`.
    ///
    /// # Safety
    ///
    /// `info` must be the bind info DuckDB passed to the running bind
    /// callback. The returned handle must not outlive the client context,
    /// which holds for bind data: DuckDB drops it before the context.
    pub unsafe fn of_bind(info: ffi::duckdb_bind_info) -> Option<FileSystem> {
        let mut context: ffi::duckdb_client_context = std::ptr::null_mut();
        // SAFETY: `info` is a live bind info (caller contract); DuckDB writes
        // a new context wrapper into `context`.
        unsafe { ffi::duckdb_table_function_get_client_context(info, &mut context) };
        if context.is_null() {
            return None;
        }
        // SAFETY: `context` is the live wrapper created above. The file
        // system it returns refers to the client context itself, not to
        // the wrapper, so destroying the wrapper next is sound.
        let file_system = unsafe { ffi::duckdb_client_context_get_file_system(context) };
        // SAFETY: `context` was created by DuckDB above and is destroyed once.
        unsafe { ffi::duckdb_destroy_client_context(&mut context) };
        (!file_system.is_null()).then_some(FileSystem(file_system))
    }

    /// Every byte of the file at `path`.
    pub fn read_all(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        let file = self.open(path)?;
        file.read_to_end(path)
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
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
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
