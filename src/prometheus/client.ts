import promClient from 'prom-client';

export function getPrometheusRegister() {
  return promClient.register;
}
