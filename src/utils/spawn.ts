import { spawn, SpawnOptions } from "node:child_process";

interface SpawnResult {
  stdout: string;
  stderr: string;
}

export function spawnPromise(
  command: string,
  args: string[] = [],
  options: SpawnOptions = {}
): Promise<SpawnResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, options);

    let stdout = "";
    let stderr = "";

    child.stdout!.on("data", (data) => {
      stdout += data.toString();
    });

    child.stderr!.on("data", (data) => {
      stderr += data.toString();
    });

    child.on("close", (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        console.error(stderr);
        reject(new Error(`Command failed with exit code ${code}`));
      }
    });

    child.on("error", (err) => {
      reject(err);
    });
  });
}
