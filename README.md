# Simple OpenOffice conversion Docker Image

Simple docker image to have a REST api to convert various Office document formats using LibreOffice.

## Build

```
docker build -t libreoffice-rest:latest .
```

## Run

```
docker run -p 8080:8080 -e TARGET_DIR=/tmp libreoffice-rest:latest
```

## API Usage

### Convert DOC to DOCX
POST /doc-to-docx
Content-Type: multipart/form-data
file=@document.doc

### Convert to PDF (from DOCX or PPTX)
POST /to-pdf
Content-Type: multipart/form-data
file=@document.docx or file=@presentation.pptx

### Convert PPT to PPTX
POST /ppt-to-pptx
Content-Type: multipart/form-data
file=@presentation.ppt

### Convert XLS to XLSX
POST /xls-to-xlsx
Content-Type: multipart/form-data
file=@spreadsheet.xls