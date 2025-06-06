use std::ffi::{CString, c_char};
use std::fmt;
use std::path::{Path, PathBuf};
use url::Url;

use crate::libreofficekit::error::OfficeError;

/// Type-safe URL "container" for LibreOffice documents
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocUrl(CString);

impl DocUrl {
    /// Internal use only, obtains a pointer to the string value
    pub(crate) fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr()
    }

    /// Converts a local absolute path into a [DocUrl] the path MUST be an absolute path
    /// otherwise you'll get an error from LibreOffice
    ///
    /// Path MUST be an absolute path, you'll receive an error if is not
    pub fn from_absolute_path<S: AsRef<str>>(path: S) -> Result<DocUrl, OfficeError> {
        let value = path.as_ref();
        let path = Path::new(value);

        if !path.is_absolute() {
            return Err(OfficeError::OfficeError(format!(
                "The file path {} must be absolute!",
                &value
            )));
        }

        let url_value = Url::from_file_path(value)
            .map_err(|_| OfficeError::OfficeError(format!("failed to parse url {}", value)))?;

        let value_str = CString::new(url_value.as_str())?;
        Ok(DocUrl(value_str))
    }

    /// Converts a path type into a [DocUrl]
    pub fn from_path<P: Into<PathBuf>>(path: P) -> Result<DocUrl, OfficeError> {
        let path: PathBuf = path.into();
        let abs_path = match path.is_absolute() {
            false => std::path::absolute(&path)
                .map_err(|err| OfficeError::OfficeError(err.to_string()))?,
            true => path,
        };

        Self::from_absolute_path(abs_path.display().to_string())
    }
}

impl fmt::Display for DocUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_string_lossy())
    }
}
