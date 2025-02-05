# Simple OpenOffice conversion Docker Image

Simple docker image to have a REST api to convert docs from doc to docx using openoffice.

## Build

```
docker build -t libreoffice-rest:latest .
```

## Run

```
docker run -d -p 8080:8080 -p 8081:8081 libreoffice-rest:latest
```
