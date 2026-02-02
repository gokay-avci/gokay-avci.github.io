+++
title = "MC Packing Visualiser"
date = 2026-01-11
description = "Hard-disk Monte Carlo packing with reproducible, shareable URLs."
+++

This interactive method demonstrates a simple Monte Carlo hard-disk packing process.

**Reproducibility:** parameters are encoded in the URL (`seed`, `n`, `r`, `step`, `iters`).
Use the **Copy link** button inside the demo to generate a shareable link.


[extra]
cover = "1.jpg"
cover_alt = "sample"
+++


{{ wasm_demo(
  title="MC Packing Visualiser",
  description="Click to load. Export PNG/CSV/JSON and share via URL parameters.",
  demo_path="demos/mc_packing/index.html",
  height="640"
) }}