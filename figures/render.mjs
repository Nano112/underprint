import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { kineglyphTheme, resolveScene } from "@kineglyph/core";
import { renderSvg } from "@kineglyph/svg";

import { architectureFigure } from "./architecture.mjs";
import { performanceFigure } from "./performance.mjs";

const project = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = resolve(project, "docs/assets/readme");

await mkdir(outputDirectory, { recursive: true });

const figures = [
  ["architecture.svg", architectureFigure, 1280],
  ["performance.svg", performanceFigure, 1120],
];

for (const [filename, definition, width] of figures) {
  const scene = resolveScene(definition, { width, theme: kineglyphTheme });
  await writeFile(resolve(outputDirectory, filename), renderSvg(scene), "utf8");
  console.log(`Wrote docs/assets/readme/${filename}`);
}
