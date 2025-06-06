use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int},
    path::Path,
    ptr::null_mut,
};

use crate::libreofficekit::bindings::{
    LibreOfficeKit, LibreOfficeKitClass, LibreOfficeKitDocument,
};
use dlopen2::wrapper::{Container, WrapperApi};

use crate::libreofficekit::{error::OfficeError, urls::DocUrl};

// Only keep the library container as static - this is fine since it's just loading the .so
// static LOK_CONTAINER: OnceCell<Container<LibreOfficeApi>> = OnceCell::new();

#[cfg(target_os = "windows")]
const TARGET_LIB: &str = "sofficeapp.dll";
#[cfg(target_os = "windows")]
const TARGET_MERGED_LIB: &str = "mergedlo.dll";

#[cfg(target_os = "linux")]
const TARGET_LIB: &str = "libsofficeapp.so";
#[cfg(target_os = "linux")]
const TARGET_MERGED_LIB: &str = "libmergedlo.so";

#[cfg(target_os = "macos")]
const TARGET_LIB: &str = "libsofficeapp.dylib";
#[cfg(target_os = "macos")]
const TARGET_MERGED_LIB: &str = "libmergedlo.dylib";

#[derive(WrapperApi)]
struct LibreOfficeApi {
    /// Pre initialization hook
    lok_preinit: Option<
        fn(
            install_path: *const std::os::raw::c_char,
            user_profile_url: *const std::os::raw::c_char,
        ) -> std::os::raw::c_int,
    >,

    libreofficekit_hook:
        Option<fn(install_path: *const std::os::raw::c_char) -> *mut LibreOfficeKit>,

    libreofficekit_hook_2: Option<
        fn(
            install_path: *const std::os::raw::c_char,
            user_profile_url: *const std::os::raw::c_char,
        ) -> *mut LibreOfficeKit,
    >,
}

/// Loads the LOK functions from the dynamic link library
fn lok_open(install_path: &Path) -> Result<Container<LibreOfficeApi>, OfficeError> {
    // Append program folder to PATH environment for windows DLL loading
    if let Ok(path) = std::env::var("PATH") {
        let install_path = install_path.to_string_lossy();
        let install_path = install_path.as_ref();

        if !path.contains(install_path) {
            unsafe { std::env::set_var("PATH", format!("{};{}", install_path, path)) };
        }
    }

    let target_lib_path = install_path.join(TARGET_LIB);
    if target_lib_path.exists() {
        // Check target library
        let err = match unsafe { Container::load(&target_lib_path) } {
            Ok(value) => return Ok(value),
            Err(err) => err,
        };

        // If the file can be opened and is likely a real library we fail here
        // instead of trying TARGET_MERGED_LIB same as standard LOK
        if std::fs::File::open(target_lib_path)
            .and_then(|file| file.metadata())
            .is_ok_and(|value| value.len() > 100)
        {
            return Err(OfficeError::LoadLibrary(err));
        }
    }

    let target_merged_lib_path = install_path.join(TARGET_MERGED_LIB);
    if target_merged_lib_path.exists() {
        // Check merged target library
        let err = match unsafe { Container::load_with_flags(target_merged_lib_path, Some(2)) } {
            Ok(value) => return Ok(value),
            Err(err) => err,
        };

        return Err(OfficeError::LoadLibrary(err));
    }

    Err(OfficeError::MissingLibrary)
}

fn lok_init(install_path: &Path) -> Result<*mut LibreOfficeKit, OfficeError> {
    // Try initialize the container (If not already initialized)
    let container = lok_open(install_path)?; // LOK_CONTAINER.get_or_try_init(|| lok_open(install_path))?;

    // Get the hook function
    let lok_hook = container
        .libreofficekit_hook
        .ok_or(OfficeError::MissingLibraryHook)?;

    let install_path = install_path.to_str().ok_or(OfficeError::InvalidPath)?;
    let install_path = CString::new(install_path)?;

    let lok = lok_hook(install_path.as_ptr());

    Ok(lok)
}

/// Raw office pointer access - now completely self-contained
pub struct OfficeRaw {
    /// This pointer for LOK
    this: *mut LibreOfficeKit,
    /// Class pointer for LOK
    class: *mut LibreOfficeKitClass,
}

impl OfficeRaw {
    /// Initializes a new instance of LOK
    pub unsafe fn init(install_path: &Path) -> Result<Self, OfficeError> {
        let lok = lok_init(install_path)?;

        if lok.is_null() {
            return Err(OfficeError::UnknownInit);
        }

        let lok_class = (*lok).pClass;

        let instance = Self {
            this: lok,
            class: lok_class,
        };

        Ok(instance)
    }

    /// Gets a [CString] containing the JSON for the available LibreOffice filter types
    pub unsafe fn get_filter_types(&self) -> Result<CString, OfficeError> {
        let get_filter_types = (*self.class)
            .getFilterTypes
            .ok_or(OfficeError::MissingFunction("getFilterTypes"))?;

        let value = get_filter_types(self.this);

        if let Some(error) = self.get_error() {
            return Err(OfficeError::OfficeError(error));
        }

        Ok(CString::from_raw(value))
    }

    /// Gets a [CString] containing the JSON for the current LibreOffice version details
    pub unsafe fn get_version_info(&self) -> Result<CString, OfficeError> {
        let get_version_info = (*self.class)
            .getVersionInfo
            .ok_or(OfficeError::MissingFunction("getVersionInfo"))?;

        let value = get_version_info(self.this);

        if let Some(error) = self.get_error() {
            return Err(OfficeError::OfficeError(error));
        }

        Ok(CString::from_raw(value))
    }

    /// Gets a [CString] containing a dump of the current LibreOffice state
    pub unsafe fn dump_state(&self) -> Result<CString, OfficeError> {
        let mut state: *mut c_char = null_mut();
        let dump_state = (*self.class)
            .dumpState
            .ok_or(OfficeError::MissingFunction("dumpState"))?;
        dump_state(self.this, std::ptr::null(), &mut state);

        if let Some(error) = self.get_error() {
            return Err(OfficeError::OfficeError(error));
        }

        Ok(CString::from_raw(state))
    }

    /// Trims memory from LibreOffice
    pub unsafe fn trim_memory(&self, target: c_int) -> Result<(), OfficeError> {
        let trim_memory = (*self.class)
            .trimMemory
            .ok_or(OfficeError::MissingFunction("trimMemory"))?;
        trim_memory(self.this, target);

        // Check for errors
        if let Some(error) = self.get_error() {
            return Err(OfficeError::OfficeError(error));
        }

        Ok(())
    }

    /// Sets an office option
    pub unsafe fn set_option(
        &self,
        option: *const c_char,
        value: *const c_char,
    ) -> Result<(), OfficeError> {
        let set_option = (*self.class)
            .setOption
            .ok_or(OfficeError::MissingFunction("setOption"))?;
        set_option(self.this, option, value);

        // Check for errors
        if let Some(error) = self.get_error() {
            return Err(OfficeError::OfficeError(error));
        }

        Ok(())
    }

    /// Loads a document without any options
    pub unsafe fn document_load(&self, url: &DocUrl) -> Result<DocumentRaw, OfficeError> {
        let document_load = (*self.class)
            .documentLoad
            .ok_or(OfficeError::MissingFunction("documentLoad"))?;
        let this = document_load(self.this, url.as_ptr());

        // Check for errors
        if let Some(error) = self.get_error() {
            return Err(OfficeError::OfficeError(error));
        }

        debug_assert!(!this.is_null());

        Ok(DocumentRaw { this })
    }

    /// Requests the latest error from LOK if one is available
    pub unsafe fn get_error(&self) -> Option<String> {
        let get_error = (*self.class).getError.expect("missing getError function");
        let raw_error = get_error(self.this);

        // Empty error is considered to be no error
        if *raw_error == 0 {
            return None;
        }

        // Create rust copy of the error message
        let value = CStr::from_ptr(raw_error).to_string_lossy().into_owned();

        // Free error memory
        self.free_error(raw_error);

        Some(value)
    }

    /// Frees the memory allocated for an error by LOK
    ///
    /// Used when we've obtained the error as we clone
    /// our own copy of the error
    unsafe fn free_error(&self, error: *mut c_char) {
        // Only available LibreOffice >=5.2
        if let Some(free_error) = (*self.class).freeError {
            free_error(error);
        }
    }

    /// Destroys the LOK instance and frees any other allocated memory
    /// Made this safe to call multiple times
    pub unsafe fn destroy(&mut self) {
        if !self.this.is_null() {
            if let Some(destroy) = (*self.class).destroy {
                destroy(self.this);
            }
            // Mark as destroyed to prevent double-free
            self.this = null_mut();
        }
    }
}

impl Drop for OfficeRaw {
    fn drop(&mut self) {
        // Skip the problematic destroy() call since it hangs
        // Let the OS clean up the LibreOffice processes when the container exits
        // If you want to try destroying (risky):
        // unsafe { self.destroy() }
    }
}

pub struct DocumentRaw {
    /// This pointer for the document
    this: *mut LibreOfficeKitDocument,
}

impl DocumentRaw {
    /// Saves the document as another format
    pub unsafe fn save_as(
        &mut self,
        url: &DocUrl,
        format: *const c_char,
        filter: *const c_char,
    ) -> Result<i32, OfficeError> {
        let class = (*self.this).pClass;
        let save_as = (*class)
            .saveAs
            .ok_or(OfficeError::MissingFunction("saveAs"))?;

        Ok(save_as(self.this, url.as_ptr(), format, filter))
    }

    /// Get the type of document
    pub unsafe fn get_document_type(&mut self) -> Result<i32, OfficeError> {
        let class = (*self.this).pClass;
        let get_document_type = (*class)
            .getDocumentType
            .ok_or(OfficeError::MissingFunction("getDocumentType"))?;

        Ok(get_document_type(self.this))
    }

    pub unsafe fn destroy(&mut self) {
        if !self.this.is_null() {
            let class = (*self.this).pClass;
            if let Some(destroy) = (*class).destroy {
                destroy(self.this);
            }
            self.this = null_mut();
        }
    }
}

impl Drop for DocumentRaw {
    fn drop(&mut self) {
        unsafe { self.destroy() }
    }
}
