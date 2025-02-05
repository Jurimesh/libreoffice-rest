import process from 'node:process';

type ExitTask = () => Promise<void> | void;

class CleanExitHandler {
  tasks: ExitTask[] = [];

  constructor() {
    process.on('SIGINT', () => this.onExit('SIGINT'));
    process.on('SIGTERM', () => this.onExit('SIGTERM'));
  }

  public add(task: ExitTask) {
    this.tasks.push(task);
  }

  private async onExit(signal: string) {
    console.log(`Received ${signal}, closing server...`);

    const promises = [];
    for (const task of this.tasks) {
      const res = task();
      if (res && typeof res.then === 'function') {
        promises.push(res);
      }
    }

    await Promise.allSettled(promises);

    // clean up resources here
    process.exit(0);
  }
}

let _instance: CleanExitHandler | null = null;
export function getCloseHandler() {
  if (!_instance) {
    _instance = new CleanExitHandler();
  }
  return _instance;
}
