mod bindings;
pub mod error;
mod sys;
pub mod urls;

use std::{
    ffi::CString,
    fmt::Display,
    path::{Path, PathBuf},
    ptr::null,
    rc::Rc,
    str::FromStr,
    sync::atomic::Ordering,
};

use bitflags::bitflags;
use num_enum::FromPrimitive;
use serde::{Deserialize, Serialize};

pub use error::OfficeError;
use sys::GLOBAL_OFFICE_LOCK;
use thiserror::Error;
pub use urls::DocUrl;

/// Instance of office.
///
/// The underlying raw logic is NOT thread safe
///
/// You cannot use more than one instance at a time in a single process
/// across threads or it will cause a segmentation fault so instance
/// creation is restricted with a static global lock
#[derive(Clone)]
pub struct Office {
    raw: Rc<sys::OfficeRaw>,
}

impl Office {
    /// Creates a new LOK instance from the provided install path
    pub fn new<P: Into<PathBuf>>(install_path: P) -> Result<Office, OfficeError> {
        // Try lock the global office lock
        if GLOBAL_OFFICE_LOCK.swap(true, Ordering::SeqCst) {
            return Err(OfficeError::InstanceLock);
        }

        let mut install_path: PathBuf = install_path.into();

        // Resolve non absolute paths
        if !install_path.is_absolute() {
            install_path =
                std::fs::canonicalize(install_path).map_err(|_| OfficeError::InvalidPath)?;
        }

        let raw = match unsafe { sys::OfficeRaw::init(&install_path) } {
            Ok(value) => value,
            Err(err) => {
                // Unlock the global office lock on init failure
                GLOBAL_OFFICE_LOCK.store(false, Ordering::SeqCst);
                return Err(err);
            }
        };

        // Check initialization errors
        if let Some(err) = unsafe { raw.get_error() } {
            return Err(OfficeError::OfficeError(err));
        }

        Ok(Office { raw: Rc::new(raw) })
    }

    /// Attempts to find an installation path from one of the common system install
    /// locations
    pub fn find_install_path() -> Option<PathBuf> {
        // Common set of install paths
        const KNOWN_PATHS: &[&str] = &[
            "/usr/lib64/libreoffice/program",
            "/usr/lib/libreoffice/program",
        ];

        // Check environment variables
        if let Ok(env) = std::env::var("LOK_PROGRAM_PATH") {
            let path = Path::new(&env);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }

        // Check common paths
        if let Some(value) = KNOWN_PATHS.iter().find_map(|path| {
            let path = Path::new(path);
            if !path.exists() {
                return None;
            }

            Some(path.to_path_buf())
        }) {
            return Some(value);
        }

        // Search /opt for installs
        if let Ok(Some(latest)) = Self::find_opt_latest() {
            return Some(latest);
        }

        // No install found
        None
    }

    /// Finds all installations of LibreOffice from the `/opt` directory
    /// provides back a list of the paths along with the version extracted
    /// from the directory name
    pub fn find_opt_installs() -> std::io::Result<Vec<(ProductVersion, PathBuf)>> {
        let opt_path = Path::new("/opt");
        if !opt_path.exists() {
            return Ok(Vec::with_capacity(0));
        }

        // Find all libreoffice folders
        let installs: Vec<(ProductVersion, PathBuf)> = std::fs::read_dir(opt_path)?
            .filter_map(|value| value.ok())
            .filter_map(|value| {
                // Get entry file type
                let file_type = value.file_type().ok()?;

                // Ignore non directories
                if !file_type.is_dir() {
                    return None;
                }

                let dir_name = value.file_name();
                let dir_name = dir_name.to_str()?;

                // Only use dirs prefixed with libreoffice
                let version = dir_name.strip_prefix("libreoffice")?;

                // Only use valid product versions
                let product_version: ProductVersion = version.parse().ok()?;

                let path = value.path();
                let path = path.join("program");

                // Not a valid office install s
                if !path.exists() {
                    return None;
                }

                Some((product_version, path))
            })
            .collect();

        Ok(installs)
    }

    /// Finds the latest LibreOffice installation from the `/opt` directory
    pub fn find_opt_latest() -> std::io::Result<Option<PathBuf>> {
        // Find all libreoffice folders
        let mut installs: Vec<(ProductVersion, PathBuf)> = Self::find_opt_installs()?;

        // Sort to find the latest installed version
        installs.sort_by_key(|(key, _)| *key);

        // Last item will be the latest
        let latest = installs
            .pop()
            // Only use the path portion
            .map(|(_, path)| path);

        Ok(latest)
    }

    /// Obtains the version information from the LibreOffice install
    pub fn get_version_info(&self) -> Result<OfficeVersionInfo, OfficeError> {
        let value = unsafe { self.raw.get_version_info()? };

        let value = value.to_str().map_err(OfficeError::InvalidUtf8String)?;

        let value: OfficeVersionInfo =
            serde_json::from_str(value).map_err(OfficeError::InvalidVersionInfo)?;

        Ok(value)
    }

    /// Loads a document from the provided `url`
    pub fn document_load(&self, url: &DocUrl) -> Result<Document, OfficeError> {
        let raw = unsafe { self.raw.document_load(url)? };
        Ok(Document { raw })
    }
}

/// Instance of a loaded document
pub struct Document {
    /// Raw inner document
    raw: sys::DocumentRaw,
}

impl Document {
    /// Saves the document as another format
    pub fn save_as(
        &mut self,
        url: &DocUrl,
        format: &str,
        filter: Option<&str>,
    ) -> Result<bool, OfficeError> {
        let format: CString = CString::new(format)?;

        let filter = match filter {
            Some(value) => CString::new(value)?,
            None => {
                let result = unsafe { self.raw.save_as(url, format.as_ptr(), null())? };
                return Ok(result != 0);
            }
        };

        let result = unsafe { self.raw.save_as(url, format.as_ptr(), filter.as_ptr())? };

        Ok(result != 0)
    }

    /// Obtain the document type
    pub fn get_document_type(&mut self) -> Result<DocumentType, OfficeError> {
        let result = unsafe { self.raw.get_document_type()? };
        Ok(DocumentType::from_primitive(result))
    }
}

#[derive(Debug, Deserialize)]
pub struct FilterType {
    /// Mime type of the filter format (i.e application/pdf)
    #[serde(rename = "MediaType")]
    pub media_type: String,
}

#[derive(Debug, Deserialize)]
pub struct OfficeVersionInfo {
    #[serde(rename = "ProductName")]
    pub product_name: String,
    #[serde(rename = "ProductVersion")]
    pub product_version: ProductVersion,
    #[serde(rename = "ProductExtension")]
    pub product_extension: String,
    #[serde(rename = "BuildId")]
    pub build_id: String,
}

bitflags! {
    /// Optional features of LibreOfficeKit, in particular callbacks that block
    /// LibreOfficeKit until the corresponding reply is received, which would
    /// deadlock if the client does not support the feature.
    ///
    /// @see [Office::set_optional_features]
    pub struct OfficeOptionalFeatures: u64 {
        /// Handle `LOK_CALLBACK_DOCUMENT_PASSWORD` by prompting the user for a password.
        ///
        /// @see [Office::set_document_password]
        const DOCUMENT_PASSWORD = 1 << 0;

        /// Handle `LOK_CALLBACK_DOCUMENT_PASSWORD_TO_MODIFY` by prompting the user for a password.
        ///
        /// @see [Office::set_document_password]
        const DOCUMENT_PASSWORD_TO_MODIFY = 1 << 1;

        /// Request to have the part number as a 5th value in the `LOK_CALLBACK_INVALIDATE_TILES` payload.
        const PART_IN_INVALIDATION_CALLBACK = 1 << 2;

        /// Turn off tile rendering for annotations.
        const NO_TILED_ANNOTATIONS = 1 << 3;

        /// Enable range based header data.
        const RANGE_HEADERS = 1 << 4;

        /// Request to have the active view's Id as the 1st value in the `LOK_CALLBACK_INVALIDATE_VISIBLE_CURSOR` payload.
        const VIEWID_IN_VISCURSOR_INVALIDATION_CALLBACK = 1 << 5;
    }
}

#[derive(Debug, FromPrimitive, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CallbackType {
    InvalidateTiles = 0,
    InvalidateVisibleCursor = 1,
    TextSelection = 2,
    TextSelectionStart = 3,
    TextSelectionEnd = 4,
    CursorVisible = 5,
    GraphicSelection = 6,
    HyperlinkClicked = 7,
    StateChanged = 8,
    StatusIndicatorStart = 9,
    StatusIndicatorSetValue = 10,
    StatusIndicatorFinish = 11,
    SearchNotFound = 12,
    DocumentSizeChanged = 13,
    SetPart = 14,
    SearchResultSelection = 15,
    UnoCommandResult = 16,
    CellCursor = 17,
    MousePointer = 18,
    CellFormula = 19,
    DocumentPassword = 20,
    DocumentPasswordModify = 21,
    Error = 22,
    ContextMenu = 23,
    InvalidateViewCursor = 24,
    TextViewSelection = 25,
    CellViewCursor = 26,
    GraphicViewSelection = 27,
    ViewCursorVisible = 28,
    ViewLock = 29,
    RedlineTableSizeChanged = 30,
    RedlineTableEntryModified = 31,
    Comment = 32,
    InvalidateHeader = 33,
    CellAddress = 34,
    RulerUpdate = 35,
    Window = 36,
    ValidityListButton = 37,
    ClipboardChanged = 38,
    ContextChanged = 39,
    SignatureStatus = 40,
    ProfileFrame = 41,
    CellSelectionArea = 42,
    CellAutoFillArea = 43,
    TableSelected = 44,
    ReferenceMarks = 45,
    JSDialog = 46,
    CalcFunctionList = 47,
    TabStopList = 48,
    FormFieldButton = 49,
    InvalidateSheetGeometry = 50,
    ValidityInputHelp = 51,
    DocumentBackgroundColor = 52,
    CommandedBlocked = 53,
    CellCursorFollowJump = 54,
    ContentControl = 55,
    PrintRanges = 56,
    FontsMissing = 57,
    MediaShape = 58,
    ExportFile = 59,
    ViewRenderState = 60,
    ApplicationBackgroundColor = 61,
    A11YFocusChanged = 62,
    A11YCaretChanged = 63,
    A11YTextSelectionChanged = 64,
    ColorPalettes = 65,
    DocumentPasswordReset = 66,
    A11YFocusedCellChanged = 67,
    A11YEditingInSelectionState = 68,
    A11YSelectionChanged = 69,
    CoreLog = 70,

    #[num_enum(catch_all)]
    Unknown(i32),
}

#[derive(Debug, FromPrimitive, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum DocumentType {
    Text = 0,
    Spreadsheet = 1,
    Presentation = 2,
    Drawing = 3,
    #[num_enum(catch_all)]
    Other(i32),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ProductVersion {
    pub major: u32,
    pub minor: u32,
}

impl ProductVersion {
    const MIN_SUPPORTED_VERSION: ProductVersion = ProductVersion::new(4, 3);
    const VERSION_6_0: ProductVersion = ProductVersion::new(6, 0);

    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// documentLoad requires libreoffice >=4.3
    pub fn is_document_load_available(&self) -> bool {
        self.ge(&Self::MIN_SUPPORTED_VERSION)
    }

    /// documentLoad requires libreoffice >=5.0
    pub fn is_document_load_options_available(&self) -> bool {
        self.ge(&ProductVersion::new(5, 0))
    }

    /// freeError requires libreoffice >=5.2
    pub fn is_free_error_available(&self) -> bool {
        self.ge(&ProductVersion::new(5, 2))
    }

    /// registerCallback requires libreoffice >=6.0
    pub fn is_register_callback_available(&self) -> bool {
        self.ge(&Self::VERSION_6_0)
    }

    /// getFilterTypes requires libreoffice >=6.0
    pub fn is_filter_types_available(&self) -> bool {
        self.ge(&Self::VERSION_6_0)
    }

    /// setOptionalFeatures requires libreoffice >=6.0
    pub fn is_optional_features_available(&self) -> bool {
        self.ge(&Self::VERSION_6_0)
    }

    /// setDocumentPassword requires libreoffice >=6.0
    pub fn is_set_document_password_available(&self) -> bool {
        self.ge(&Self::VERSION_6_0)
    }

    /// getVersionInfo requires libreoffice >=6.0
    pub fn is_get_version_info_available(&self) -> bool {
        self.ge(&Self::VERSION_6_0)
    }

    /// runMacro requires libreoffice >=6.0
    pub fn is_run_macro_available(&self) -> bool {
        self.ge(&Self::VERSION_6_0)
    }

    /// trimMemory requires libreoffice >=7.6
    pub fn is_trim_memory_available(&self) -> bool {
        self.ge(&ProductVersion::new(7, 6))
    }
}

impl PartialOrd for ProductVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProductVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.major.cmp(&other.major) {
            // Ignore equal major versions
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }

        // Check minor versions
        self.minor.cmp(&other.minor)
    }
}

impl Display for ProductVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Error)]
#[error("product version is invalid or malformed")]
pub struct InvalidProductVersion;

impl FromStr for ProductVersion {
    type Err = InvalidProductVersion;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (major, minor) = s.split_once('.').ok_or(InvalidProductVersion)?;

        let major = major.parse().map_err(|_| InvalidProductVersion)?;
        let minor = minor.parse().map_err(|_| InvalidProductVersion)?;

        Ok(Self { major, minor })
    }
}

impl<'de> Deserialize<'de> for ProductVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value: &str = <&str>::deserialize(deserializer)?;

        value
            .parse::<ProductVersion>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}

impl Serialize for ProductVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}
