import { figure } from "@kineglyph/core";

export const performanceFigure = figure(
  "underprint-performance",
  {
    title: "Choose the worker you need",
    description:
      "Measured through PHP FFI on Apple Silicon with a 240 × 240 public reference image.",
  },
  (f) => {
    const original = f.card({
      eyebrow: "ORIGINAL NATIVE",
      title: "442 MiB steady RSS",
      body: "38.52 ms detect\n228.20 ms embed + verify\n8 inference threads",
      badge: "baseline",
      motif: "gauge",
    });
    const bounded = f.card({
      eyebrow: "DEFAULT · DENSITY",
      title: "147 MiB steady RSS",
      body: "44.02 ms detect\n222.14 ms embed + verify\nLazy sessions · bounded arena",
      badge: "−67% memory",
      motif: "leaf",
      tone: "success",
    });
    const throughput = f.card({
      eyebrow: "OPT-IN · LATENCY",
      title: "201.29 ms embed",
      body: "35.04 ms detect\n300 MiB steady RSS\nRetained CPU arena",
      badge: "−12% embed",
      motif: "bolt",
      tone: "info",
    });

    f.connect(original, bounded, {
      label: "more workers per host",
      head: "arrow",
    });
    f.connect(bounded, throughput, {
      label: "trade memory for latency",
      head: "arrow",
    });

    f.root(
      f.graph([original, bounded, throughput], {
        style: "flow",
        direction: {
          wide: "horizontal",
          compact: "horizontal",
          narrow: "vertical",
        },
        layerGap: 52,
        nodeGap: 30,
        padding: 30,
      }),
    );
  },
);
