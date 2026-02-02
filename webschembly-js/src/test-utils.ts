import { createHash } from "crypto";
import * as fs from "fs/promises";
import * as path from "path";

const shardPrefix = process.env.WEBSCHEMBLY_SHARD_PREFIX || "";
const fileFilter = process.env.WEBSCHEMBLY_FILE_FILTER || "";
export const fixtureDir = "fixtures";

export function isShaPrefix(str: string, prefix: string): boolean {
  if (!prefix) {
    return true;
  }

  const hash = createHash("sha256").update(str).digest("hex");
  const hashBinary = BigInt("0x" + hash)
    .toString(2)
    .padStart(256, "0");

  return hashBinary.startsWith(prefix);
}

function shouldIncludePath(entryPath: string): boolean {
  return isShaPrefix(entryPath, shardPrefix);
}

function shouldIncludeFile(entryPath: string): boolean {
  if (!fileFilter) {
    return true;
  }

  return entryPath.includes(fileFilter);
}

async function readDirRec(
  basePath: string,
  curDir: string,
  result: string[],
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
      if (shouldIncludePath(entryPath) && shouldIncludeFile(entryPath)) {
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
