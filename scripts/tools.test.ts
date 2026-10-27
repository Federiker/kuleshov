// The FFmpeg staging's tests, copied from manuzio's tests/tooling/downloads.test.ts and
// tools.test.ts (the parts about FFmpeg) and moved from Vitest to `bun test`. No test downloads:
// `fetch` and the extraction are replaced.
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  mock,
  spyOn,
} from "bun:test";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import {
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { SpawnOptions } from "node:child_process";

const run =
  mock<
    (command: string, args: string[], options?: SpawnOptions) => Promise<void>
  >();
const available = mock<(command: string, args?: string[]) => boolean>();
mock.module("./process.ts", () => ({ run, available }));
const { ffmpegDownload, installArchive } = await import("./downloads.ts");
const { executable, prepareMedia } = await import("./tools.ts");

const bytes = Buffer.from("synthetic archive");
const checksum = createHash("sha256").update(bytes).digest("hex");
const platform = process.platform;
const realFetch = globalThis.fetch;
let root: string;
let destination: string;
// `fetch` as the downloads call it (Bun's own `fetch` type also has `preconnect`).
type Fetcher = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;
let fetcher: ReturnType<typeof mock<Fetcher>>;

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), "kuleshov-download's-"));
  destination = join(root, "installed");
  fetcher = mock<Fetcher>(async () => new Response(bytes));
  globalThis.fetch = fetcher as unknown as typeof fetch;
  spyOn(console, "log").mockReturnValue(undefined);
  run.mockReset();
  run.mockImplementation(async (_command, _args, options) => {
    if (typeof options?.cwd !== "string")
      throw new Error("Extraction must specify a working directory");
    const folder = join(options.cwd, "extracted", "nested", "bin");
    await mkdir(folder, { recursive: true });
    for (const name of ["ffmpeg", "ffprobe", "ffmpeg.exe", "ffprobe.exe"])
      await writeFile(join(folder, name), `synthetic ${name}`);
  });
  available.mockReset();
  available.mockReturnValue(true);
});

afterEach(async () => {
  Object.defineProperty(process, "platform", { value: platform });
  globalThis.fetch = realFetch;
  mock.restore();
  await rm(root, { recursive: true, force: true });
});

describe("verified tool archive installation", () => {
  it.each(["win32", "linux", "darwin"] as const)(
    "extracts ZIPs on %s without passing drive-qualified paths to tar",
    async (host) => {
      Object.defineProperty(process, "platform", { value: host });
      await installArchive(
        root,
        "https://example.test/tools.zip",
        destination,
        ["ffmpeg", "ffprobe"],
        checksum.toUpperCase(),
      );
      const command = host === "win32" ? "powershell.exe" : "unzip";
      expect(run).toHaveBeenCalledWith(
        command,
        host === "win32"
          ? [
              "-NoProfile",
              "-NonInteractive",
              "-Command",
              "$ErrorActionPreference = 'Stop'; Expand-Archive -LiteralPath 'download.zip' -DestinationPath 'extracted'",
            ]
          : ["-q", "download.zip", "-d", "extracted"],
        { cwd: expect.stringContaining(join(root, "tmp", "downloads")) },
      );
      expect(await readFile(join(destination, "ffmpeg"), "utf8")).toBe(
        "synthetic ffmpeg",
      );
      expect(await readFile(join(destination, "ffprobe"), "utf8")).toBe(
        "synthetic ffprobe",
      );
      expect(await readdir(join(root, "tmp", "downloads"))).toEqual([]);
    },
  );

  it.each(["tar.gz", "tar.xz"])(
    "uses relative archive paths for %s extraction",
    async (extension) => {
      await installArchive(
        root,
        `https://example.test/tools.${extension}`,
        destination,
        ["ffmpeg"],
        checksum,
      );
      expect(run).toHaveBeenCalledWith(
        "tar",
        ["-xf", "download.tar", "-C", "extracted"],
        { cwd: expect.any(String) },
      );
    },
  );

  it("rejects an invalid checksum before downloading", async () => {
    await expect(
      installArchive(
        root,
        "https://example.test/tools.zip",
        destination,
        ["ffmpeg"],
        "",
      ),
    ).rejects.toThrow("Invalid SHA-256");
    expect(fetcher).not.toHaveBeenCalled();
    expect(run).not.toHaveBeenCalled();
  });

  it("rejects changed bytes before extraction and cleans up", async () => {
    await expect(
      installArchive(
        root,
        "https://example.test/tools.zip",
        destination,
        ["ffmpeg"],
        "0".repeat(64),
      ),
    ).rejects.toThrow("SHA-256 mismatch");
    expect(run).not.toHaveBeenCalled();
    expect(existsSync(destination)).toBe(false);
    expect(await readdir(join(root, "tmp", "downloads"))).toEqual([]);
  });

  it("surfaces HTTP failures and cleans up", async () => {
    fetcher.mockImplementation(async () => new Response(null, { status: 404 }));
    await expect(
      installArchive(
        root,
        "https://example.test/tools.zip",
        destination,
        ["ffmpeg"],
        checksum,
      ),
    ).rejects.toThrow("HTTP 404");
    expect(run).not.toHaveBeenCalled();
    expect(await readdir(join(root, "tmp", "downloads"))).toEqual([]);
  });

  it("surfaces extraction failures and cleans up", async () => {
    run.mockImplementation(async () => {
      throw new Error("extraction failed");
    });
    await expect(
      installArchive(
        root,
        "https://example.test/tools.zip",
        destination,
        ["ffmpeg"],
        checksum,
      ),
    ).rejects.toThrow("extraction failed");
    expect(existsSync(destination)).toBe(false);
    expect(await readdir(join(root, "tmp", "downloads"))).toEqual([]);
  });

  it("does not install a partial pair when an executable is missing", async () => {
    await expect(
      installArchive(
        root,
        "https://example.test/tools.zip",
        destination,
        ["ffmpeg", "missing-ffprobe"],
        checksum,
      ),
    ).rejects.toThrow("missing-ffprobe missing");
    expect(existsSync(destination)).toBe(false);
    expect(await readdir(join(root, "tmp", "downloads"))).toEqual([]);
  });
});

describe("pinned FFmpeg downloads", () => {
  it.each([
    [
      "win32",
      "x64",
      "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip",
      "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba",
    ],
    [
      "linux",
      "x64",
      "https://johnvansickle.com/ffmpeg/releases/ffmpeg-7.0.2-amd64-static.tar.xz",
      "abda8d77ce8309141f83ab8edf0596834087c52467f6badf376a6a2a4c87cf67",
    ],
    [
      "linux",
      "arm64",
      "https://johnvansickle.com/ffmpeg/releases/ffmpeg-7.0.2-arm64-static.tar.xz",
      "f4149bb2b0784e30e99bdda85471c9b5930d3402014e934a5098b41d0f7201b1",
    ],
  ] as const)("pins FFmpeg for %s-%s", (host, arch, url, sha256) => {
    expect(ffmpegDownload(host, arch)).toEqual({ url, sha256 });
  });

  it("keeps unsupported targets on the explicit supplied-binary path", () => {
    expect(() => ffmpegDownload("darwin", "arm64")).toThrow(
      "Supply matching ffmpeg and ffprobe",
    );
  });
});

describe("staging FFmpeg for the tests", () => {
  const out = () => join(root, "tools");
  const staged = (name: string) => join(out(), executable(name));

  it("keeps a supplied pair, downloads the pinned one otherwise, and checks both run", async () => {
    await mkdir(out(), { recursive: true });
    for (const name of ["ffmpeg", "ffprobe"])
      await writeFile(staged(name), name);
    await prepareMedia(root);
    for (const name of ["ffmpeg", "ffprobe"])
      expect(await readFile(staged(name), "utf8")).toBe(name);
    expect(fetcher).not.toHaveBeenCalled();
    expect(run).not.toHaveBeenCalled();

    // Nothing supplied: the pinned download, recorded; a tool that does not run is an error.
    await rm(root, { recursive: true, force: true });
    const pinned = ffmpegDownload();
    fetcher.mockImplementation(
      async () => new Response(Buffer.from("not the pinned archive")),
    );
    await expect(prepareMedia(root)).rejects.toThrow("SHA-256 mismatch");
    expect(fetcher).toHaveBeenCalledWith(pinned.url, expect.anything());
    await mkdir(out(), { recursive: true });
    await writeFile(
      join(out(), "download.json"),
      JSON.stringify({ url: pinned.url, sha256: pinned.sha256 }),
    );
    for (const name of ["ffmpeg", "ffprobe"])
      await writeFile(staged(name), name);
    available.mockReturnValue(false);
    await expect(prepareMedia(root)).rejects.toThrow("not runnable");
  });

  it("replaces a pair downloaded for an older pin", async () => {
    await mkdir(out(), { recursive: true });
    for (const name of ["ffmpeg", "ffprobe"])
      await writeFile(staged(name), "old");
    await writeFile(
      join(out(), "download.json"),
      JSON.stringify({ url: "https://example.test/old.zip", sha256: "0" }),
    );
    fetcher.mockImplementation(async () => new Response(bytes));
    // The pinned archive's checksum is not this synthetic one: the old pair is gone all the same.
    await expect(prepareMedia(root)).rejects.toThrow("SHA-256 mismatch");
    expect(existsSync(staged("ffmpeg"))).toBe(false);
    expect(existsSync(join(out(), "download.json"))).toBe(false);
  });

  it("refuses an incomplete supplied pair", async () => {
    await mkdir(out(), { recursive: true });
    await writeFile(staged("ffmpeg"), "ffmpeg");
    await expect(prepareMedia(root)).rejects.toThrow(
      "Supply both ffmpeg and ffprobe",
    );
    expect(fetcher).not.toHaveBeenCalled();
  });
});
