import pino, { Logger } from 'pino';

let _logger: Logger | null = null;
export function getLogger(): Logger {
  if (!_logger) {
    _logger = pino({});
  }
  return _logger;
}
