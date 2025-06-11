# Simple OpenOffice conversion Docker Image

Simple docker image to have a REST api to convert various Office document formats using LibreOffice.

## Build

```
docker build -t libreoffice-rest:latest .
```

## Run

```
docker run -p 1234:1234 -e TMPDIR=/tmp libreoffice-rest:latest
```

### Temp directory

On unix rust temp_dir is using TMPDIR environment variable and has some fallbacks if not set.

## API Usage

POST /convert
Content-Type: multipart/form-data
file=@presentation.ppt
input_format=ppt
output_format=pptx
