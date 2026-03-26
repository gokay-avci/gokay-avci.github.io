# AI-Guided Porosity Puzzle

Minimal Rust/WebAssembly prototype for an Encode AI for Science Fellowship application.

The demo runs entirely client-side and presents pore accessibility as a small scientific puzzle:

- a porous material is abstracted as a graph of pores and channel apertures,
- the player edits local aperture radii,
- the app computes whether a target probe can traverse the structure,
- a score panel shows accessibility, bottleneck radius, path continuity, and edit cost,
- a rule-based "AI guidance" layer explains what matters scientifically,
- a candidate triage panel flags near-threshold or high-impact states.

## Simplest robust architecture

`wasm-bindgen + web-sys + Trunk` is the smallest reliable stack here.

- Rust owns the scientific state, path reasoning, scoring, hints, and rendering.
- The HTML/CSS shell is static and lightweight.
- Trunk handles the WebAssembly build and writes deployable assets directly into the site static folder.
- No frontend framework is required because the UI is small and deterministic.

## Project structure

```text
demos/porosity_lab/
├── Cargo.toml
├── Trunk.toml
├── index.html
├── README.md
└── src/
    ├── hint.rs
    ├── lib.rs
    ├── puzzles.rs
    ├── render.rs
    ├── scoring.rs
    ├── state.rs
    └── utils.rs
```

## Scientific model

This is not a full simulator. It is an interpretable threshold model:

- each edge has an aperture radius,
- a route is feasible if there is a continuous inlet-to-outlet path,
- the best route is the one with the largest minimum aperture,
- the active probe radius defines whether the route is open or bottlenecked,
- edit cost penalises excessive changes from the baseline structure.

Class labels:

- `Open pathway`: a route exists and its bottleneck clears the target probe radius
- `Bottlenecked`: a route exists but its narrowest channel is still below target
- `Blocked`: no continuous route survives

## Build

From the site root:

```bash
cd personal_website
trunk build --release --config demos/porosity_lab/Trunk.toml
```

For local development with the portfolio site:

```bash
cd personal_website
./2_dev.sh
```

For a full production build of the site and demos:

```bash
cd personal_website
./1_build.sh
```

## GitHub Pages deployment

This demo is integrated into the existing Zola site.

1. Build the WASM assets with `trunk build --release` via `./1_build.sh`.
2. Build the site with `zola build`.
3. Deploy the generated `personal_website/public/` directory to GitHub Pages.
4. The project page lives at `/projects/porosity/`.
5. The fullscreen demo lives at `/demos/ai_porosity_puzzle/index.html`.

If you deploy with GitHub Actions, publish the `public/` directory after the build step.

## 3-minute demo script

1. "This is a browser-native Rust/WASM prototype for AI-guided reasoning in porous-material design spaces."
2. "Each line is a channel aperture. The target probe radius is the guest molecule I want to admit."
3. "When I click or drag a channel, the model immediately recomputes the best inlet-to-outlet route."
4. "The system classifies the structure as open, bottlenecked, or blocked and exposes the limiting bottleneck."
5. "The guidance panel is rule-based in v1, but it shows the future role of AI: explaining what local feature matters and what intervention is likely to help."
6. "The candidate triage panel then flags states where a small local edit creates a large global accessibility shift, which is exactly where deeper simulation or AI assistance becomes valuable."
