import fastifyMultipart from '@fastify/multipart';
import fastify from 'fastify';

import { PORT } from './constants';
import { getCloseHandler } from './clean-exit';
import { getLogger } from './logger';
import fastifyPrometheus from './prometheus/plugin';

const logger = getLogger();

export async function start() {
  const app = fastify({
    trustProxy: true,
    // logger: getLogger(),
    logger: true,
    bodyLimit: 100 * 1024 * 1024,
    maxParamLength: 2048,
    rewriteUrl: (req) => {
      if (req.url?.startsWith('/api')) {
        return req.url.substring(4);
      } else {
        return req.url ?? '/';
      }
    },
  });

  getCloseHandler().add(() => app.close());

  app.register(fastifyPrometheus);

  app.get(
    '/ready',
    {
      config: {
        disableMetrics: true,
      },
    },
    (req, reply) => {
      reply.status(200).send('ready');
    },
  );

  app.get(
    '/health',
    {
      config: {
        disableMetrics: true,
      },
    },
    (req, reply) => {
      reply.status(200).send('healthy');
    },
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

  await app.listen({
    port: PORT,
    host: '0.0.0.0',
  });

  logger.info(`Server listening on port: ${PORT}`);
}
