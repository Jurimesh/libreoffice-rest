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

POST /convert
Content-Type: multipart/form-data
file=@presentation.ppt
input_format=ppt
output_format=pptx
