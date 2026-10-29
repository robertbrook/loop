import { readFile } from "node:fs/promises";

export const indexURL = new URL("../dist/index.html", import.meta.url);
export const html = await readFile(indexURL, "utf8");
const stylesheet = html.match(/href="(\.\/assets\/[a-f0-9]{16}\/style\.css)"/);
if (!stylesheet) throw new Error("Build the versioned site first: bash scripts/build.sh");
export const assetDirectory = new URL("./", new URL(stylesheet[1], indexURL));
export const builtAsset = (name) => new URL(name, assetDirectory);
