# Simple OpenOffice conversion Docker Image

Simple docker image to have a REST api to convert docs from doc to docx using openoffice.

## Build

```
docker build -t libreoffice-rest:latest .
```

## Run

```
docker run -p 8080:8080 -e TARGET_DIR=/tmp libreoffice-rest:latest
```
