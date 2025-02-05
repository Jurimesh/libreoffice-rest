import { FastifyInstance, FastifyRequest } from 'fastify';
import promClient, { Histogram, LabelValues, Summary } from 'prom-client';

import { IMetricsPluginOptions } from './types';

/**
 * Plugin constructor
 *
 * @public
 */
interface IConstructiorDeps {
  /** Fastify instance */
  fastify: FastifyInstance;

  /** Metric plugin options */
  options: Partial<IMetricsPluginOptions>;
}

interface IReqMetrics<T extends string> {
  hist?: (labels?: LabelValues<T>) => number;
  sum?: (labels?: LabelValues<T>) => void;
}

interface IRouteMetrics {
  routeHist: Histogram<string>;
  routeSum: Summary<string>;
  labelNames: { method: string; status: string; route: string };
}

export const DEFAULT_OPTIONS: IMetricsPluginOptions = {
  name: 'metrics',
};

/**
 * Fastify metrics handler class
 *
 * @public
 */
export class FastifyMetrics {
  private static getRouteSlug(args: { method: string; url: string }): string {
    return `[${args.method}] ${args.url}`;
  }

  private readonly metricStorage = new WeakMap<FastifyRequest, IReqMetrics<string>>();
  private readonly routesWhitelist = new Set<string>();
  private readonly methodBlacklist = new Set<string>();

  private routeMetrics: IRouteMetrics;
  private readonly routeFallback: string = '__unknown__';

  /** Creates metrics collector instance */
  constructor(private readonly deps: IConstructiorDeps) {
    this.setMethodBlacklist();
    this.setRouteWhitelist();

    this.collectDefaultMetrics();
    this.routeMetrics = this.registerRouteMetrics();
    this.collectRouteMetrics();
  }

  private getRouteLabel(request: FastifyRequest): string {
    return request.routeOptions.config.statsId ?? request.routeOptions.url ?? this.routeFallback;
  }

  /** Populates methods blacklist to exclude them from metrics collection */
  private setMethodBlacklist(): void {
    ['HEAD', 'OPTIONS', 'TRACE', 'CONNECT'].map((v) => v.toUpperCase()).forEach((v) => this.methodBlacklist.add(v));
  }

  /** Populates routes whitelist if */
  private setRouteWhitelist(): void {
    this.deps.fastify.addHook('onRoute', (routeOptions) => {
      // routeOptions.method;
      // routeOptions.schema;
      // routeOptions.url; // the complete URL of the route, it will include the prefix if any
      // routeOptions.path; // `url` alias
      // routeOptions.routePath; // the URL of the route without the prefix
      // routeOptions.prefix;

      [routeOptions.method].flat().forEach((method) => {
        if (!this.methodBlacklist.has(method)) {
          this.routesWhitelist.add(
            FastifyMetrics.getRouteSlug({
              method,
              url: routeOptions.url,
            }),
          );
        }
      });
    });
  }

  /** Collect default prom-client metrics */
  private collectDefaultMetrics(): void {
    promClient.collectDefaultMetrics();
  }

  private registerRouteMetrics(): IRouteMetrics {
    const labelNames = {
      method: 'method',
      status: 'status_code',
      route: 'route',
    };

    const customLabelNames: string[] = Object.keys({});

    const routeHist = new promClient.Histogram<string>({
      name: 'http_request_duration_seconds',
      help: 'request duration in seconds',
      labelNames: [labelNames.method, labelNames.route, labelNames.status, ...customLabelNames] as const,
    });
    const routeSum = new promClient.Summary<string>({
      name: 'http_request_summary_seconds',
      help: 'request duration in seconds summary',
      labelNames: [labelNames.method, labelNames.route, labelNames.status, ...customLabelNames] as const,
    });

    return { routeHist, routeSum, labelNames };
  }

  /**
   * Create timers for histogram and summary based on enabled configuration
   * option
   */
  private createTimers(request: FastifyRequest): void {
    this.metricStorage.set(request, {
      hist: this.routeMetrics.routeHist.startTimer(),
      sum: this.routeMetrics.routeSum.startTimer(),
    });
  }

  /** Collect per-route metrics */
  private collectRouteMetrics(): void {
    this.deps.fastify
      .addHook('onRequest', (request, _, done) => {
        if (request.routeOptions.config.disableMetrics === true || !request.raw.url) {
          return done();
        }

        if (
          this.routesWhitelist.has(
            FastifyMetrics.getRouteSlug({
              method: request.method,
              url: request.routeOptions.url!,
            }),
          )
        ) {
          this.createTimers(request);
        }

        return done();
      })
      .addHook('onResponse', (request, reply, done) => {
        const metrics = this.metricStorage.get(request);
        if (!metrics) {
          return done();
        }

        const statusCode = `${Math.floor(reply.statusCode / 100)}xx`;
        const route = this.getRouteLabel(request);
        const method = request.method;

        const labels = {
          [this.routeMetrics.labelNames.method]: method,
          [this.routeMetrics.labelNames.route]: route,
          [this.routeMetrics.labelNames.status]: statusCode,
        };

        if (metrics.hist) metrics.hist(labels);
        if (metrics.sum) metrics.sum(labels);

        done();
      });
  }
}
