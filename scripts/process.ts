// Running commands for the scripts: copied from manuzio's scripts/process.ts (`available`, `run`),
// without the logged variant, which staging FFmpeg does not use.
import {
  spawn,
  spawnSync,
  type ChildProcess,
  type SpawnOptions,
} from "node:child_process";

class CommandError extends Error {
  constructor(command: string, code: number | null) {
    super(
      `${command} failed (${code === null ? "terminated" : `exit ${code}`}).`,
    );
  }
}

export function available(
  command: string,
  args: string[] = ["--version"],
  okStatuses: readonly number[] = [0],
): boolean {
  const result = spawnSync(command, args, { stdio: "ignore" });
  if (
    result.error &&
    !("code" in result.error && result.error.code === "ENOENT")
  )
    throw result.error;
  return (
    !result.error &&
    result.status !== null &&
    okStatuses.includes(result.status)
  );
}

function terminate(child: ChildProcess): void {
  if (child.exitCode !== null || child.signalCode !== null || !child.pid)
    return;
  if (process.platform === "win32") {
    const result = spawnSync(
      "taskkill.exe",
      ["/PID", String(child.pid), "/T", "/F"],
      { stdio: "ignore" },
    );
    if (result.error) throw result.error;
    if (result.status !== 0 && child.exitCode === null) {
      console.warn(
        `taskkill failed for PID ${child.pid}; attempting direct termination.`,
      );
      child.kill();
    }
  } else {
    const result = spawnSync("ps", ["-ax", "-o", "pid=,ppid="], {
      encoding: "utf8",
    });
    if (result.error) throw result.error;
    if (result.status !== 0)
      throw new Error("Cannot list descendants for process cleanup.");
    const children = result.stdout
      .split(/\r?\n/)
      .map((line) => line.trim().split(/\s+/).map(Number));
    const descendants: number[] = [];
    const visit = (parent: number) => {
      for (const [pid, ppid] of children) {
        if (pid && ppid === parent) {
          visit(pid);
          descendants.push(pid);
        }
      }
    };
    visit(child.pid);
    for (const pid of [...descendants, child.pid]) {
      try {
        process.kill(pid, "SIGTERM");
      } catch (error) {
        if (
          !(error instanceof Error && "code" in error && error.code === "ESRCH")
        )
          throw error;
      }
    }
  }
}

export async function run(
  command: string,
  args: string[],
  options: SpawnOptions = {},
): Promise<void> {
  const child = spawn(command, args, { stdio: "inherit", ...options });
  const interrupt = () => terminate(child);
  process.once("SIGINT", interrupt);
  process.once("SIGTERM", interrupt);
  try {
    await new Promise<void>((resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (code) =>
        code === 0 ? resolve() : reject(new CommandError(command, code)),
      );
    });
  } finally {
    process.removeListener("SIGINT", interrupt);
    process.removeListener("SIGTERM", interrupt);
  }
}
