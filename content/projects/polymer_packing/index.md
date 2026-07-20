+++
title = "evokUPS Polymer Packing"
date = 2026-07-20
description = "Evolutionary pre-packing of polydisperse polymer chains in the browser."
template = "page.html"
weight = 1

[taxonomies]
tags = ["Rust", "WASM", "Polymers", "Evolutionary Search"]
categories = ["Projects"]

[extra]
cover = "polymer_packing.png"
cover_alt = "Polydisperse bead-spring polymer chains packed inside a periodic simulation cell"
demo = "/demos/polymer_packing/index.html"
fullscreen = "/demos/polymer_packing/index.html"
+++

**Status:** Interactive research prototype | **Stack:** Rust, WebAssembly, JavaScript

evokUPS explores how evolutionary search can generate dense, low-overlap starting configurations for polydisperse polymer systems. Each candidate contains a complete periodic ensemble of chains, while topology-aware mutations translate, rotate, reshape, or reconnect chains without changing the fixed population tensor.

The browser demo compares single-point genetic search with Lamarckian and Baldwinian force relaxation. It reports fitness, reduced pressure, density, residual voids, and lineage while rendering the best packing directly in the periodic cell.

[Open fullscreen demo](/demos/polymer_packing/index.html)

{{ wasm_demo(
  title="evokUPS Polymer Packing",
  description="Evolve and inspect polydisperse polymer packings directly in the browser.",
  demo_path="demos/polymer_packing/index.html",
  height="900",
  button="Launch demo"
) }}
