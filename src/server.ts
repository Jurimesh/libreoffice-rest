import fastifyMultipart from "@fastify/multipart";
import fastify from "fastify";
import fs from "fs";
import fsPromises from "fs/promises";
import * as pathUtils from "node:path";
import { pipeline } from "node:stream/promises";

import { PORT, TARGET_DIR } from "./constants";
import { getCloseHandler } from "./clean-exit";
import { getLogger } from "./logger";
import fastifyPrometheus from "./prometheus/plugin";
import {
  docToDocx,
  docxToPdf,
  pptxToPdf,
  pptToPptx,
  xlsToXlsx,
  libreOfficeService,
} from "./openoffice/libreoffice-service";

const logger = getLogger();

export async function start() {
  if (isNaN(PORT)) {
    throw new Error("PORT must be a number");
  }

  const app = fastify({
    trustProxy: true,
    // logger: getLogger(),
    logger: true,
    bodyLimit: 100 * 1024 * 1024,
    maxParamLength: 2048,
  });

  getCloseHandler().add(() => app.close());

  app.register(fastifyPrometheus);

  app.get(
    "/ready",
    {
      config: {
        disableMetrics: true,
      },
    },
    (req, reply) => {
      reply.status(200).send("ready");
    }
  );

  app.get(
    "/health",
    {
      config: {
        disableMetrics: true,
      },
    },
    (req, reply) => {
      reply.status(200).send("healthy");
    }
  );

  app.register(fastifyMultipart, {
    limits: {
      fieldNameSize: 500,
      fieldSize: 25000,
      fields: 10,
      fileSize: 250000000,
      files: 1,
      headerPairs: 2000,
    },
  });

  app.post("/doc-to-docx", {}, async (req, reply) => {
    if (!req.isMultipart) {
      return reply.status(400).send({
        error: "Not a multipart request",
      });
    }

    const data = await req.file();
    if (!data) {
      return reply.status(400).send({
        error: "File parameter required",
      });
    }

    const targetFilepath = pathUtils.join(TARGET_DIR, `${Date.now()}.doc`);
    await pipeline(data.file, fs.createWriteStream(targetFilepath));

    let filesToRemove = [targetFilepath];
    try {
      const mimetype = data.mimetype;
      if (mimetype !== "application/msword") {
        return reply.status(400).send({
          error: "File must be of type application/msword",
        });
      }

      const convertedFilepath = await docToDocx(targetFilepath);
      filesToRemove.push(convertedFilepath);

      reply
        .status(200)
        .header(
          "Content-Type",
          "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );

      // Pipe converted content of convertedFilepath to reply
      await reply.send(fs.createReadStream(convertedFilepath));
    } catch (err: any) {
      logger.error(err);
      reply.status(500).send({ error: err.message });
    }

    for (const filepath of filesToRemove) {
      fsPromises
        .rm(filepath, {
          force: true,
        })
        .catch((err) => {
          logger.error(err);
        });
    }
  });

  app.post("/to-pdf", {}, async (req, reply) => {
    if (!req.isMultipart) {
      return reply.status(400).send({
        error: "Not a multipart request",
      });
    }
  
    const data = await req.file();
    if (!data) {
      return reply.status(400).send({
        error: "File parameter required",
      });
    }
  
    let fileExt = "";
    let convertFn;
    const mimetype = data.mimetype;
    
    if (mimetype === "application/vnd.openxmlformats-officedocument.wordprocessingml.document") {
      fileExt = ".docx";
      convertFn = docxToPdf;
    } else if (mimetype === "application/vnd.openxmlformats-officedocument.presentationml.presentation") {
      fileExt = ".pptx";
      convertFn = pptxToPdf;
    } else {
      return reply.status(400).send({
        error: "File must be of type docx or pptx",
      });
    }
  
    const targetFilepath = pathUtils.join(TARGET_DIR, `${Date.now()}${fileExt}`);
    await pipeline(data.file, fs.createWriteStream(targetFilepath));
  
    let filesToRemove = [targetFilepath];
    try {
      const convertedFilepath = await convertFn(targetFilepath);
      filesToRemove.push(convertedFilepath);
  
      reply
        .status(200)
        .header("Content-Type", "application/pdf");
  
      // Pipe converted content of convertedFilepath to reply
      await reply.send(fs.createReadStream(convertedFilepath));
    } catch (err: any) {
      logger.error(err);
      reply.status(500).send({ error: err.message });
    }
  
    for (const filepath of filesToRemove) {
      fsPromises
        .rm(filepath, {
          force: true,
        })
        .catch((err) => {
          logger.error(err);
        });
    }
  });

  app.post("/ppt-to-pptx", {}, async (req, reply) => {
    if (!req.isMultipart) {
      return reply.status(400).send({
        error: "Not a multipart request",
      });
    }
  
    const data = await req.file();
    if (!data) {
      return reply.status(400).send({
        error: "File parameter required",
      });
    }
  
    const targetFilepath = pathUtils.join(TARGET_DIR, `${Date.now()}.ppt`);
    await pipeline(data.file, fs.createWriteStream(targetFilepath));
  
    let filesToRemove = [targetFilepath];
    try {
      const mimetype = data.mimetype;
      if (mimetype !== "application/vnd.ms-powerpoint") {
        return reply.status(400).send({
          error: "File must be of type application/vnd.ms-powerpoint",
        });
      }
  
      const convertedFilepath = await pptToPptx(targetFilepath);
      filesToRemove.push(convertedFilepath);
  
      reply
        .status(200)
        .header("Content-Type", "application/vnd.openxmlformats-officedocument.presentationml.presentation");
  
      // Pipe converted content of convertedFilepath to reply
      await reply.send(fs.createReadStream(convertedFilepath));
    } catch (err: any) {
      logger.error(err);
      reply.status(500).send({ error: err.message });
    }
  
    for (const filepath of filesToRemove) {
      fsPromises
        .rm(filepath, {
          force: true,
        })
        .catch((err) => {
          logger.error(err);
        });
    }
  });

  app.post("/xls-to-xlsx", {}, async (req, reply) => {
    if (!req.isMultipart) {
      return reply.status(400).send({
        error: "Not a multipart request",
      });
    }
  
    const data = await req.file();
    if (!data) {
      return reply.status(400).send({
        error: "File parameter required",
      });
    }
  
    const targetFilepath = pathUtils.join(TARGET_DIR, `${Date.now()}.xls`);
    await pipeline(data.file, fs.createWriteStream(targetFilepath));
  
    let filesToRemove = [targetFilepath];
    try {
      const mimetype = data.mimetype;
      if (mimetype !== "application/vnd.ms-excel") {
        return reply.status(400).send({
          error: "File must be of type application/vnd.ms-excel",
        });
      }
  
      const convertedFilepath = await xlsToXlsx(targetFilepath);
      filesToRemove.push(convertedFilepath);
  
      reply
        .status(200)
        .header("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
  
      // Pipe converted content of convertedFilepath to reply
      await reply.send(fs.createReadStream(convertedFilepath));
    } catch (err: any) {
      logger.error(err);
      reply.status(500).send({ error: err.message });
    }
  
    for (const filepath of filesToRemove) {
      fsPromises
        .rm(filepath, {
          force: true,
        })
        .catch((err) => {
          logger.error(err);
        });
    }
  });

  await app.listen({
    port: PORT,
    host: "0.0.0.0",
  });

  logger.info(`Server listening on port: ${PORT}`);

  logger.info("Starting libreoffice service");
  await libreOfficeService.ensureServiceRunning();
  logger.info("Libreoffice service started");
}
