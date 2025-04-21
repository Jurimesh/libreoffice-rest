import process from "node:process";
import pathUtils from "node:path";
import { ChildProcess, spawn } from "node:child_process";

import { spawnPromise } from "../utils/spawn";

const UNOSERVER_PORT = "2003";

class LibreOfficeService {
  private static instance: LibreOfficeService;
  private serverProcess: ChildProcess | null = null;
  private isStarting: boolean = false;
  private startPromise: Promise<void> | null = null;

  private constructor() {}

  public static getInstance(): LibreOfficeService {
    if (!LibreOfficeService.instance) {
      LibreOfficeService.instance = new LibreOfficeService();
    }
    return LibreOfficeService.instance;
  }

  private async startService(): Promise<void> {
    if (this.startPromise) {
      return this.startPromise;
    }

    this.isStarting = true;
    this.startPromise = new Promise<void>((resolve, reject) => {
      try {
        // Start unoserver instead of soffice directly
        this.serverProcess = spawn("unoserver", ["--port", UNOSERVER_PORT]);

        this.serverProcess.on("error", (error) => {
          console.error("unoserver process error:", error);
          this.serverProcess = null;
        });

        this.serverProcess.on("exit", (code) => {
          console.log(`unoserver process exited with code ${code}`);
          this.serverProcess = null;
        });

        // Wait a moment for the service to start
        setTimeout(() => {
          this.isStarting = false;
          resolve();
        }, 2000);
      } catch (error) {
        this.isStarting = false;
        reject(error);
      }
    });

    return this.startPromise;
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

    // Use unoconvert instead of soffice
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

  public async shutdown(): Promise<void> {
    if (this.serverProcess) {
      this.serverProcess.kill();
      this.serverProcess = null;
    }
  }
}

// Export a singleton instance
export const libreOfficeService = LibreOfficeService.getInstance();

// Export the conversion function as before
export async function docToDocx(filename: string): Promise<string> {
  return libreOfficeService.docToDocx(filename);
}

export async function docxToPdf(filename: string): Promise<string> {
  return libreOfficeService.docxToPdf(filename);
}