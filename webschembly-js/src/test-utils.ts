import * as fs from "fs/promises";
import * as path from "path";
import { createHash } from "crypto";

const shardPrefix = process.env.WEBSCHEMBLY_SHARD_PREFIX || "";
export const fixtureDir = "fixtures";

function shouldIncludePath(entryPath: string): boolean {
  if (!shardPrefix) {
    return true;
  }

  const hash = createHash("sha256").update(entryPath).digest("hex");
  const hashBinary = BigInt("0x" + hash)
    .toString(2)
    .padStart(256, "0");

  return hashBinary.startsWith(shardPrefix);
}

async function readDirRec(
  basePath: string,
  curDir: string,
  result: string[]
): Promise<void> {
  const dir = path.join(basePath, curDir);
  const entries = await fs.readdir(dir, {
    withFileTypes: true,
  });
  for (const entry of entries) {
    if (entry.name.startsWith(".")) {
      continue;
    }
    const entryPath = path.join(curDir, entry.name);
    if (entry.isDirectory()) {
      await readDirRec(basePath, entryPath, result);
    } else {
      if (shouldIncludePath(entryPath)) {
        result.push(entryPath);
      }
    }
  }
}

export async function getAllFixtureFilenames(): Promise<string[]> {
  const result: string[] = [];
  await readDirRec(fixtureDir, "", result);
  return result.filter((file) => file.endsWith(".scm"));
}
