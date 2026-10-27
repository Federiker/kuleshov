// `mise run tools`: stages the pinned FFmpeg and FFprobe in tools/ for the tests (and CI), as
// manuzio's `scripts/prepare-tools.ts --media-only` does in its assets/tools.
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { prepareMedia } from "./tools.ts";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
try {
  await prepareMedia(root);
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
