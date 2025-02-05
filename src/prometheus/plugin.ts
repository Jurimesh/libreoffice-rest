/**
 * Prometheus metrics exporter for Fastify. Based on
 * {@link https://github.com/siimon/prom-client | prom-client}. Also by default
 * it adds fastify route response time metrics (histogram and summary).
 *
 * @packageDocumentation
 */

import fastifyPlugin from 'fastify-plugin';

import { FastifyMetrics } from './fastify-metrics';
import { IMetricsPluginOptions, IMetricsRouteContextConfig } from './types';

declare module 'fastify' {
  interface FastifyContextConfig extends IMetricsRouteContextConfig {
    /** Override route definition */
    statsId?: string;

    /** Disables metric collection on this route */
    disableMetrics?: boolean;
  }
}

export * from './types';

export default fastifyPlugin<Partial<IMetricsPluginOptions>>(
  async (fastify, options) => {
    const { name = 'metrics' } = options;

    const fm = new FastifyMetrics({ fastify, options });
    fastify.decorate(name, fm);
  },
  {
    fastify: '>=4.0.0',
    name: 'fastify-metrics',
  },
);
