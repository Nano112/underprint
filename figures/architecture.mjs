import { figure } from "@kineglyph/core";

export const architectureFigure = figure(
  "underprint-architecture",
  {
    title: "One native engine, every surface",
    description:
      "PHP, the command line, and Rust share the same bounded policy and native TrustMark inference path.",
  },
  (f) => {
    const surfaces = f.card({
      eyebrow: "PUBLIC SURFACES",
      title: "PHP · CLI · Rust",
      body: "Binary FFI · stable JSON\nTyped native API",
      motif: "code",
    });
    const core = f.card({
      eyebrow: "ONE POLICY",
      title: "Underprint core",
      body: "Bounded media · immutable profiles\nAdaptive 0.1 strength steps",
      motif: "shield",
      tone: "info",
    });
    const engine = f.card({
      eyebrow: "NATIVE INFERENCE",
      title: "TrustMark Q / BCH-5",
      body: "Rust + ONNX Runtime\nPinned models · no Python",
      motif: "wave",
      tone: "warning",
    });
    const verification = f.card({
      eyebrow: "FAIL CLOSED",
      title: "Serialize · decode",
      body: "Read the final PNG again\nAccept only the exact 61 bits",
      motif: "check",
      tone: "info",
    });
    const result = f.card({
      eyebrow: "VERSIONED RESULT",
      title: "Image + report",
      body: "Payload · hashes · strength\nProfile · model digests",
      motif: "spark",
      tone: "success",
    });

    f.connect(surfaces, core, { label: "one policy", head: "arrow" });
    f.connect(core, engine, { label: "validated RGB", head: "arrow" });
    f.connect(engine, verification, { label: "candidate PNG", head: "arrow" });
    f.connect(verification, result, { label: "exact match", head: "arrow" });

    f.root(
      f.graph([surfaces, core, engine, verification, result], {
        style: "flow",
        direction: {
          wide: "horizontal",
          compact: "horizontal",
          narrow: "vertical",
        },
        layerGap: 38,
        nodeGap: 24,
        padding: 28,
      }),
    );
  },
);
