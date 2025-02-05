/**
 * Route config for metrics
 *
 * @public
 */
export interface IMetricsRouteContextConfig {
  /** Override route definition */
  statsId?: string;

  /** Disables metric collection on this route */
  disableMetrics?: boolean;
}

export interface IMetricsPluginOptions {
  /**
   * Plugin name that will be registered in fastify instance.
   *
   * @defaultValue `metrics`
   */
  name: string;
}
