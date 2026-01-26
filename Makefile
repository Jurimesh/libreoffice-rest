.PHONY: build run stop clean logs shell

IMAGE_NAME := libreoffice-rest
CONTAINER_NAME := libreoffice-rest
PORT := 1234

build:
	docker build -t $(IMAGE_NAME) .

run: build
	docker run -d --name $(CONTAINER_NAME) -p $(PORT):$(PORT) $(IMAGE_NAME)

run-fg: build
	docker run --rm --name $(CONTAINER_NAME) -p $(PORT):$(PORT) $(IMAGE_NAME)

stop:
	docker stop $(CONTAINER_NAME) && docker rm $(CONTAINER_NAME)

logs:
	docker logs -f $(CONTAINER_NAME)

shell:
	docker exec -it $(CONTAINER_NAME) /bin/bash

clean:
	docker rmi $(IMAGE_NAME)
