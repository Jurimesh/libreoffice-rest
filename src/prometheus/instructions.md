# Setup plugin and usage

Little bit of instructions on how to setup and use this plugin

```ts
import fastify from 'fastify';

import fastifyPrometheus from './plugin';
import { getPrometheusRegister } from './client';

const app = fastify();

app.register(fastifyPrometheus);

// Expose the metrics in an endpoint
// Ideally use a different fastify instance for this, do not expose this publicly
app.get('/metrics', async (req, reply) => {
  const registry = getPrometheusRegister();
  const data = await registry.metrics();
  return reply.type(registry.contentType).send(data);
});

// Don't run metrics for / endpoint
app.get(
  '/',
  {
    config: {
      // Disable metrics per endpoint using this flag
      disableMetrics: true,
    },
  },
  async (req, reply) => {
    return reply.end('ok');
  },
);
```
