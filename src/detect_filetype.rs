#[derive(Debug, PartialEq)]
pub enum FileType {
    Word,
    PowerPoint,
    Excel,
    Pdf,
    RichText,
    PlainText,
    OpenDocument, // ODT, ODS, ODP files
    Unknown,      // For unsupported formats
}

pub fn detect_openoffice_file_type(content: &[u8]) -> FileType {
    if content.len() == 0 {
        return FileType::Unknown;
    }

    // PDF signature
    if content.starts_with(b"%PDF-") {
        return FileType::Pdf;
    }

    // RTF signature
    if content.starts_with(b"{\\rtf1") {
        return FileType::RichText;
    }

    let content_slice = content.get(..1024).unwrap_or(content);

    // ZIP-based formats (docx, pptx, xlsx, odt, ods, odp)
    if content.starts_with(b"PK\x03\x04") || content.starts_with(b"PK\x05\x06") {
        return detect_zip_based_format(content_slice);
    }

    // OLE2/Compound Document formats (doc, ppt, xls)
    if content.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1") {
        return detect_ole2_format(content_slice);
    }

    // Plain text detection (basic heuristic)
    if is_likely_text(content_slice) {
        return FileType::PlainText;
    }

    FileType::Unknown
}

fn detect_zip_based_format(content: &[u8]) -> FileType {
    // Look for specific content type strings in ZIP central directory
    let content_str = String::from_utf8_lossy(content);

    // Office Open XML formats
    if content_str.contains("word/")
        || content_str.contains("application/vnd.openxmlformats-officedocument.wordprocessingml")
    {
        return FileType::Word;
    }

    if content_str.contains("ppt/")
        || content_str.contains("application/vnd.openxmlformats-officedocument.presentationml")
    {
        return FileType::PowerPoint;
    }

    if content_str.contains("xl/")
        || content_str.contains("application/vnd.openxmlformats-officedocument.spreadsheetml")
    {
        return FileType::Excel;
    }

    // OpenDocument formats
    if content_str.contains("application/vnd.oasis.opendocument") {
        return FileType::OpenDocument;
    }

    // Check for [Content_Types].xml which is present in Office Open XML files
    if content_str.contains("[Content_Types].xml") {
        // This is likely an Office document, but we couldn't determine the specific type
        // Default to Word as it's most common
        return FileType::Word;
    }

    FileType::Unknown
}

fn detect_ole2_format(content: &[u8]) -> FileType {
    // For OLE2 documents, we need to look deeper into the structure
    // This is a simplified detection - in practice, you'd parse the OLE2 structure
    let content_str = String::from_utf8_lossy(content);

    // Look for application-specific signatures
    if content_str.contains("Microsoft Office Word") || content_str.contains("Word.Document") {
        return FileType::Word;
    }

    if content_str.contains("Microsoft Office PowerPoint")
        || content_str.contains("PowerPoint Document")
    {
        return FileType::PowerPoint;
    }

    if content_str.contains("Microsoft Excel") || content_str.contains("Workbook") {
        return FileType::Excel;
    }

    // Generic OLE2 document - could be any Office format
    FileType::Word // Default assumption
}

fn is_likely_text(content: &[u8]) -> bool {
    // Simple heuristic: check if most bytes are printable ASCII or common UTF-8
    let printable_count = content
        .iter()
        .take(256) // Check first 256 bytes
        .filter(|&&b| {
            b.is_ascii_graphic() || b.is_ascii_whitespace() || b >= 0x80 // Allow UTF-8
        })
        .count();

    let total_checked = content.len().min(256);
    if total_checked == 0 {
        return false;
    }

    // If more than 90% of characters are printable, consider it text
    (printable_count as f32 / total_checked as f32) > 0.9
}

/// Convenience function to detect file type from byte slice
pub fn detect_file_type_from_bytes(bytes: &[u8]) -> FileType {
    detect_openoffice_file_type(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_detection() {
        let pdf_header = b"%PDF-1.4\n1 0 obj\n<<\n/Type /Catalog";
        assert_eq!(detect_file_type_from_bytes(pdf_header), FileType::Pdf);
    }

    #[test]
    fn test_rtf_detection() {
        let rtf_content = b"{\\rtf1\\ansi\\deff0 Hello World}";
        assert_eq!(detect_file_type_from_bytes(rtf_content), FileType::RichText);
    }

    #[test]
    fn test_zip_signature() {
        let zip_header = b"PK\x03\x04\x14\x00\x00\x00\x08\x00";
        // This will be None because it's just a ZIP header without Office-specific content
        assert_eq!(detect_file_type_from_bytes(zip_header), FileType::Unknown);
    }

    #[test]
    fn test_ole2_signature() {
        let ole2_header = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1Microsoft Office Word Document";
        assert_eq!(detect_file_type_from_bytes(ole2_header), FileType::Word);
    }

    #[test]
    fn test_text_detection() {
        let text_content =
            b"This is a plain text file with normal content.\nIt has multiple lines.\n";
        assert_eq!(
            detect_file_type_from_bytes(text_content),
            FileType::PlainText
        );
    }

    #[test]
    fn test_binary_rejection() {
        let binary_content = b"\x00\x01\x02\x03\xFF\xFE\xFD\xFC";
        assert_eq!(
            detect_file_type_from_bytes(binary_content),
            FileType::Unknown
        );
    }
}
