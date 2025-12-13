import * as fs from "fs/promises";
import * as path from "path";

export const fixtureDir = "fixtures";

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
      result.push(entryPath);
    }
  }
}

export async function getAllFixtureFilenames(): Promise<string[]> {
  const result: string[] = [];
  await readDirRec(fixtureDir, "", result);
  return result.filter((file) => file.endsWith(".scm"));
}
