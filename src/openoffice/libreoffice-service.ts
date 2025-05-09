import process from "node:process";
import pathUtils from "node:path";
import { ChildProcess, spawn } from "node:child_process";
import net from "node:net";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { spawnPromise } from "../utils/spawn";
import { getLogger } from "../logger";

const UNOSERVER_PORT = "2003";
const MAX_UNREADY_MINUTES = 5;
const POLLING_INTERVAL_MS = 60000;

class LibreOfficeService {
  private static instance: LibreOfficeService;
  private serverProcess: ChildProcess | null = null;
  private isStarting: boolean = false;
  private startPromise: Promise<void> | null = null;
  private pollingTimer: NodeJS.Timeout | null = null;
  private unreadyMinutes: number = 0;
  private logger = getLogger();

  private constructor() { }

  public static getInstance(): LibreOfficeService {
    if (!LibreOfficeService.instance) {
      LibreOfficeService.instance = new LibreOfficeService();
      const logger = LibreOfficeService.instance.logger;
      logger.info("LibreOfficeService singleton instance created");
    }
    return LibreOfficeService.instance;
  }

  private async startService(): Promise<void> {
    if (this.startPromise) {
      this.logger.info("LibreOffice service already starting, reusing existing promise");
      return this.startPromise;
    }

    this.logger.info("Starting LibreOffice service with unoserver");
    this.isStarting = true;
    this.startPromise = new Promise<void>((resolve, reject) => {
      try {
        this.logger.info({ port: UNOSERVER_PORT }, "Spawning unoserver process");
        this.serverProcess = spawn("unoserver", ["--port", UNOSERVER_PORT]);

        this.serverProcess.on("error", (error) => {
          this.logger.error({ error }, "unoserver process error");
          this.serverProcess = null;
        });

        this.serverProcess.on("exit", (code) => {
          this.logger.info({ code }, "unoserver process exited");
          this.serverProcess = null;
        });

        setTimeout(() => {
          this.isStarting = false;
          this.startServerPolling();
          resolve();
        }, 2000);
      } catch (error) {
        this.isStarting = false;
        this.logger.error({ error }, "Error starting unoserver");
        reject(error);
      }
    });

    return this.startPromise;
  }

  private startServerPolling(): void {
    if (this.pollingTimer) {
      clearInterval(this.pollingTimer);
    }

    this.unreadyMinutes = 0;
    this.logger.info({ interval: POLLING_INTERVAL_MS }, "Starting unoserver health polling");

    this.pollingTimer = setInterval(async () => {
      const isReady = await this.checkServerReady();

      if (!isReady) {
        this.unreadyMinutes++;
        this.logger.info({ minutes: this.unreadyMinutes }, `unoserver not ready for ${this.unreadyMinutes} minute(s)`);

        if (this.unreadyMinutes >= MAX_UNREADY_MINUTES) {
          this.logger.warn({ minutes: MAX_UNREADY_MINUTES }, `unoserver not ready for ${MAX_UNREADY_MINUTES} minutes, restarting...`);
          await this.restartServer();
        }
      } else {
        if (this.unreadyMinutes > 0) {
          this.logger.info({ previouslyDownMinutes: this.unreadyMinutes }, "unoserver is now ready, resetting unready counter");
        } else {
          this.logger.info("unoserver is healthy");
        }
        this.unreadyMinutes = 0;
      }
    }, POLLING_INTERVAL_MS);
  }

  private async checkServerReady(): Promise<boolean> {
    return await this.checkServerConnectable() && await this.performFunctionalTest();
  }

  private async checkServerConnectable(): Promise<boolean> {
    return new Promise<boolean>(resolve => {
      const socket = new net.Socket();
      let resolved = false;

      const resolveOnce = (value: boolean) => {
        if (!resolved) {
          resolved = true;
          socket.destroy();
          resolve(value);
        }
      };

      socket.setTimeout(3000);

      socket.on('connect', () => resolveOnce(true));
      socket.on('error', () => resolveOnce(false));
      socket.on('timeout', () => resolveOnce(false));

      try {
        socket.connect(parseInt(UNOSERVER_PORT), '127.0.0.1');
      } catch (error) {
        resolveOnce(false);
      }
    });
  }

  private async performFunctionalTest(): Promise<boolean> {
    const tempDir = os.tmpdir();
    const testFile = path.join(tempDir, `unoserver-test-${Date.now()}.txt`);
    const outputFile = path.join(tempDir, `unoserver-test-${Date.now()}.pdf`);

    try {
      fs.writeFileSync(testFile, 'Unoserver health check test');

      await spawnPromise(
        "unoconvert",
        ["--port", UNOSERVER_PORT, "--convert-to", "pdf", testFile, outputFile],
        { timeout: 8000, env: process.env }
      );

      const success = fs.existsSync(outputFile);

      try {
        if (fs.existsSync(testFile)) fs.unlinkSync(testFile);
        if (fs.existsSync(outputFile)) fs.unlinkSync(outputFile);
      } catch (e) {
      }

      if (!success) {
        this.logger.warn("Unoserver failed functional test - conversion failed");
      }

      return success;
    } catch (error) {
      this.logger.warn({ error: error instanceof Error ? error.message : String(error) }, "Unoserver failed functional test");

      try {
        if (fs.existsSync(testFile)) fs.unlinkSync(testFile);
        if (fs.existsSync(outputFile)) fs.unlinkSync(outputFile);
      } catch (e) {
      }

      return false;
    }
  }

  private async restartServer(): Promise<void> {
    await this.shutdown();

    this.startPromise = null;
    this.unreadyMinutes = 0;

    await this.startService();
  }

  public async ensureServiceRunning(): Promise<void> {
    if (!this.serverProcess && !this.isStarting) {
      await this.startService();
    } else if (this.isStarting) {
      await this.startPromise;
    }
  }

  public async docToDocx(filename: string): Promise<string> {
    await this.ensureServiceRunning();

    const cwd = pathUtils.dirname(filename);
    const outputPath = filename + "x";

    await spawnPromise(
      "unoconvert",
      ["--port", UNOSERVER_PORT, "--convert-to", "docx", filename, outputPath],
      {
        cwd,
        timeout: 120_000,
        env: process.env,
      }
    );

    return outputPath;
  }

  public async docxToPdf(filename: string): Promise<string> {
    await this.ensureServiceRunning();

    const cwd = pathUtils.dirname(filename);
    const basename = pathUtils.basename(filename, '.docx');
    const outputPath = pathUtils.join(cwd, `${basename}.pdf`);

    await spawnPromise(
      "unoconvert",
      ["--port", UNOSERVER_PORT, "--convert-to", "pdf", filename, outputPath],
      {
        cwd,
        timeout: 120_000,
        env: process.env,
      }
    );

    return outputPath;
  }

  public async pptxToPdf(filename: string): Promise<string> {
    await this.ensureServiceRunning();

    const cwd = pathUtils.dirname(filename);
    const basename = pathUtils.basename(filename, '.pptx');
    const outputPath = pathUtils.join(cwd, `${basename}.pdf`);

    await spawnPromise(
      "unoconvert",
      ["--port", UNOSERVER_PORT, "--convert-to", "pdf", filename, outputPath],
      {
        cwd,
        timeout: 120_000,
        env: process.env,
      }
    );

    return outputPath;
  }

  public async pptToPptx(filename: string): Promise<string> {
    await this.ensureServiceRunning();

    const cwd = pathUtils.dirname(filename);
    const outputPath = filename + "x";

    await spawnPromise(
      "unoconvert",
      ["--port", UNOSERVER_PORT, "--convert-to", "pptx", filename, outputPath],
      {
        cwd,
        timeout: 120_000,
        env: process.env,
      }
    );

    return outputPath;
  }

  public async xlsToXlsx(filename: string): Promise<string> {
    await this.ensureServiceRunning();

    const cwd = pathUtils.dirname(filename);
    const outputPath = filename + "x";

    await spawnPromise(
      "unoconvert",
      ["--port", UNOSERVER_PORT, "--convert-to", "xlsx", filename, outputPath],
      {
        cwd,
        timeout: 120_000,
        env: process.env,
      }
    );

    return outputPath;
  }

  public async shutdown(): Promise<void> {
    if (this.pollingTimer) {
      clearInterval(this.pollingTimer);
      this.pollingTimer = null;
    }

    if (this.serverProcess) {
      this.serverProcess.kill();
      this.serverProcess = null;
    }
  }
}

export const libreOfficeService = LibreOfficeService.getInstance();

export async function docToDocx(filename: string): Promise<string> {
  return libreOfficeService.docToDocx(filename);
}

export async function docxToPdf(filename: string): Promise<string> {
  return libreOfficeService.docxToPdf(filename);
}

export async function pptxToPdf(filename: string): Promise<string> {
  return libreOfficeService.pptxToPdf(filename);
}

export async function pptToPptx(filename: string): Promise<string> {
  return libreOfficeService.pptToPptx(filename);
}

export async function xlsToXlsx(filename: string): Promise<string> {
  return libreOfficeService.xlsToXlsx(filename);
}