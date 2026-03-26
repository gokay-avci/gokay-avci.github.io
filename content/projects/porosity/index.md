+++
title = "PolyMorph Foundry"
date = 2026-03-24
description = "A browser game where evolving crystal shapes teach pore space through play."
template = "page.html"
weight = 1

[taxonomies]
tags = ["Rust", "WASM", "Materials", "AI for Science"]
categories = ["Projects"]

[extra]
cover = "aporosity.jpg"
cover_alt = "Abstract porous-material accessibility prototype"
demo = "/demos/ai_porosity_puzzle/index.html"
fullscreen = "/demos/ai_porosity_puzzle/index.html"
+++

This is a small browser game about porous materials. You breed crystal-like shapes, place the survivors, and watch gas find its way through the structure.

The three short phases show open space, narrow passages, and gas reaching hidden pockets.

[Open fullscreen demo](/demos/ai_porosity_puzzle/index.html)

{{ wasm_demo(
  title="PolyMorph Foundry Demo",
  description="A short browser game about evolving pore-forming crystal shapes.",
  demo_path="demos/ai_porosity_puzzle/index.html",
  height="860",
  button="Launch demo"
) }}
