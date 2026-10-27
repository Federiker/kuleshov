// The pinned FFmpeg downloads and their verified installation, copied from manuzio's
// scripts/downloads.ts (`installArchive`, `ffmpegDownload`). The pins themselves are in
// crates/kuleshov/ffmpeg-builds.json, which the library's `download` feature reads too; the parity
// references belong to those exact builds (docs/parity.md).
import { createHash } from "node:crypto";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { join } from "node:path";
import { run } from "./process.ts";

async function findFile(
  folder: string,
  name: string,
): Promise<string | undefined> {
  for (const entry of await readdir(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) {
      const found = await findFile(path, name);
      if (found) return found;
    } else if (entry.isFile() && entry.name === name) return path;
  }
}

/**
 * Downloads `url`, checks its SHA-256, extracts it in the gitignored `tmp/downloads` and copies the
 * executables `names` into `destination`. Nothing is installed unless every name was found.
 */
export async function installArchive(
  root: string,
  url: string,
  destination: string,
  names: string[],
  checksum: string,
): Promise<void> {
  if (!/^[a-fA-F0-9]{64}$/.test(checksum))
    throw new Error(`Invalid SHA-256 checksum for ${url}`);
  const cache = join(root, "tmp", "downloads");
  await mkdir(cache, { recursive: true });
  const work = await mkdtemp(join(cache, "tools-"));
  try {
    console.log(`Downloading ${url}`);
    const response = await fetch(url, { signal: AbortSignal.timeout(300_000) });
    if (!response.ok)
      throw new Error(`Download failed: HTTP ${response.status} (${url})`);
    const bytes = Buffer.from(await response.arrayBuffer());
    const actual = createHash("sha256").update(bytes).digest("hex");
    if (checksum.toLowerCase() !== actual)
      throw new Error(`SHA-256 mismatch for ${url}`);
    const archive = url.endsWith(".zip") ? "download.zip" : "download.tar";
    await writeFile(join(work, archive), bytes);
    const extracted = join(work, "extracted");
    await mkdir(extracted);
    // GNU tar treats drive letters as remote hosts and cannot extract ZIPs.
    if (url.endsWith(".zip")) {
      if (process.platform === "win32")
        await run(
          "powershell.exe",
          [
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$ErrorActionPreference = 'Stop'; Expand-Archive -LiteralPath 'download.zip' -DestinationPath 'extracted'",
          ],
          { cwd: work },
        );
      else
        await run("unzip", ["-q", archive, "-d", "extracted"], { cwd: work });
    } else await run("tar", ["-xf", archive, "-C", "extracted"], { cwd: work });
    // Find everything before copying, so a malformed archive cannot leave a partial install.
    const files = await Promise.all(
      names.map(async (name) => {
        const source = await findFile(extracted, name);
        if (!source) throw new Error(`${name} missing in ${url}`);
        return { name, source };
      }),
    );
    await mkdir(destination, { recursive: true });
    for (const { name, source } of files) {
      await copyFile(source, join(destination, name));
      await chmod(join(destination, name), 0o755);
    }
  } finally {
    await rm(work, { recursive: true, force: true });
  }
}

export interface ToolDownload {
  url: string;
  sha256: string;
}

/** The pinned builds, shared with the library's `download` feature (one manifest, one place). */
type Builds = Record<string, ToolDownload | string>;
const builds: Builds = JSON.parse(
  await readFile(
    new URL("../crates/kuleshov/ffmpeg-builds.json", import.meta.url),
    "utf8",
  ),
);

/** The manifest's `<os>-<arch>` key, in Rust's names, for Node's platform and architecture. */
function buildKey(platform: string, arch: string): string {
  const os = platform === "win32" ? "windows" : platform;
  const cpu = arch === "x64" ? "x86_64" : arch === "arm64" ? "aarch64" : arch;
  return `${os}-${cpu}`;
}

export function ffmpegDownload(
  platform = process.platform,
  arch = process.arch,
): ToolDownload {
  const build = builds[buildKey(platform, arch)];
  if (typeof build === "object")
    return { url: build.url, sha256: build.sha256 };
  throw new Error(
    `Supply matching ffmpeg and ffprobe in tools/ for ${platform}-${arch}, or point KULESHOV_TOOLS_DIR at them; automatic downloads are not available.`,
  );
}
