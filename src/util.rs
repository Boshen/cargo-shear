use std::{fs, io, path::Path};

/// Read a file to a string using `simdutf8` for faster UTF-8 validation.
///
/// This function is faster than `fs::read_to_string` which uses `std::str::from_utf8`
/// internally. It validates UTF-8 using SIMD instructions when available.
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - The file contents are not valid UTF-8
#[expect(unsafe_code, reason = "Required for performance optimization with simdutf8")]
pub fn read_to_string(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;

    // `simdutf8` is faster than `std::str::from_utf8` which `fs::read_to_string` uses internally
    if simdutf8::basic::from_utf8(&bytes).is_err() {
        // Same error as `fs::read_to_string` produces (`io::Error::INVALID_UTF8`)
        #[cold]
        fn invalid_utf8_error() -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, "stream did not contain valid UTF-8")
        }
        return Err(invalid_utf8_error());
    }

    // SAFETY: `simdutf8` has ensured it's a valid UTF-8 string
    Ok(unsafe { String::from_utf8_unchecked(bytes) })
}
