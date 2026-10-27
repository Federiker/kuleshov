// The pinned FFmpeg downloads and their verified installation, copied from manuzio's
// scripts/downloads.ts (`installArchive`, `ffmpegDownload`). The parity references belong to these
// exact builds (docs/parity.md): change a URL or a checksum only with new references.
import { createHash } from "node:crypto";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readdir,
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

export function ffmpegDownload(
  platform = process.platform,
  arch = process.arch,
): ToolDownload {
  if (platform === "win32" && arch === "x64")
    return {
      url: "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip",
      sha256:
        "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba",
    };
  if (platform === "linux" && (arch === "x64" || arch === "arm64")) {
    return {
      url: `https://johnvansickle.com/ffmpeg/releases/ffmpeg-7.0.2-${arch === "x64" ? "amd64" : "arm64"}-static.tar.xz`,
      sha256:
        arch === "x64"
          ? "abda8d77ce8309141f83ab8edf0596834087c52467f6badf376a6a2a4c87cf67"
          : "f4149bb2b0784e30e99bdda85471c9b5930d3402014e934a5098b41d0f7201b1",
    };
  }

  throw new Error(
    `Supply matching ffmpeg and ffprobe in tools/ for ${platform}-${arch}, or point KULESHOV_TOOLS_DIR at them; automatic downloads are not available.`,
  );
}
