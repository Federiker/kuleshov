// Stages FFmpeg and FFprobe for the tests in the gitignored tools/ folder: manuzio's `--media-only`
// staging (scripts/tools.ts, `prepareMedia` and `stageMedia`), trimmed to what kuleshov needs.
// FFmpeg is never bundled with the library or committed; callers supply their own (README).
import { existsSync } from "node:fs";
import { chmod, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { ffmpegDownload, installDownload } from "./downloads.ts";
import { available } from "./process.ts";

export function executable(name: string): string {
  return process.platform === "win32" ? `${name}.exe` : name;
}

/** The two executables kuleshov drives. */
const MEDIA = ["ffmpeg", "ffprobe"];

/** Which pinned download the staged pair came from; absent when the pair was supplied by hand. */
const RECORD = "download.json";

/**
 * Stages FFmpeg and FFprobe in `<root>/tools`, both runnable. A pair already there is kept: a
 * supplied one always, a downloaded one while its download is still the pinned one (`download.json`);
 * otherwise the pinned download is installed, checksum verified.
 */
export async function prepareMedia(root: string): Promise<void> {
  await stageMedia(root);
  const out = join(root, "tools");
  for (const name of MEDIA) await chmod(join(out, executable(name)), 0o755);
  console.log(`FFmpeg staged: ${out}`);
}

async function stageMedia(root: string): Promise<void> {
  const out = join(root, "tools");
  const staged = MEDIA.map((name) => join(out, executable(name)));
  if (staged.some(existsSync) && !staged.every(existsSync)) {
    throw new Error(
      "Supply both ffmpeg and ffprobe in tools/, or remove the incomplete pair explicitly.",
    );
  }
  const download = ffmpegDownload();
  const record = await readFile(join(out, RECORD), "utf8").then(
    (text) => JSON.parse(text) as Partial<typeof download>,
    () => undefined,
  );
  // The same archive (its SHA-256), wherever it is served from, as the library compares them.
  const outdated =
    record !== undefined &&
    record.sha256?.toLowerCase() !== download.sha256.toLowerCase();
  if (outdated) {
    console.log(`The pinned FFmpeg changed: replacing ${out}`);
    for (const path of [...staged, join(out, RECORD)])
      await rm(path, { force: true });
  }
  if (!staged.every(existsSync)) {
    await installDownload(root, download, out, MEDIA.map(executable));
    await writeFile(
      join(out, RECORD),
      `${JSON.stringify(download, null, 2)}\n`,
    );
  }
  for (const name of MEDIA) {
    if (!available(join(out, executable(name)), ["-version"]))
      throw new Error(`Staged ${name} is not runnable.`);
  }
}
