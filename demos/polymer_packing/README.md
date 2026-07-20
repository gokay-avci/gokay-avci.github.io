# evokUPS

evokUPS is a local-first demonstrator for batched evolutionary atomistic search. Its primary problem is periodic pre-packing of an indexed polydisperse polymer ensemble using topology-preserving variation, cell-list interactions, and differentiable forces. A Lennard-Jones cluster remains available as a baseline.

## Workspace

- `crates/evokups-core`: domain types, evaluator contract, and ask-evaluate-tell search engine.
- `crates/evokups-evaluators`: concrete evaluators, beginning with Lennard-Jones energy and forces.
- `crates/evokups-polymer`: polymer topology, random-walk initialization, rigid-chain mutation, periodic cell lists, and the differentiable packing objective.
- `crates/evokups-web`: dependency-free WebAssembly bridge used by the browser UI.
- `python/kups_backend`: persistent session runtime that imports JAX once, caches compiled population kernels by static signature, and exposes an optional local service.

## Test

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Run the browser app locally

Install the WebAssembly target once, assemble the static site, and serve it:

```sh
rustup target add wasm32-unknown-unknown
sh scripts/build-static.sh
python3 -m http.server 8080 --directory dist
```

Open `http://localhost:8080`. No JavaScript package manager or bundler is required. The `dist` directory can be served unchanged by GitHub Pages.

The browser uses demand-driven Three.js instancing and orbital controls for a 3D chain view, with selectable particles, architecture-aware bead colors, explicit minimum-image graph bonds, stretched-bond warnings, and optional periodic-cell and clearance-colored residual-void layers. Every bead is wrapped into the centered display cell; a bond crossing a periodic face is split at that face and continued from the opposite face rather than drawing geometry outside the box. It also shows best-fitness history, per-bead objective decomposition, live pressure/density/cell state, lineage, and genetic-operation highlighting. The search can run continuously until paused, and the visual refresh cadence can be reduced independently of the scientific generation rate. Large bond sets switch from cylinders to line segments, picking is disabled above 5,000 beads, and WebGL no longer redraws continuously while idle.

## Run the polymer batch directly

The native runner prints the fixed batch shape, chain-length vector, best energy, candidate identity, and lineage for each generation:

```sh
cargo run -p evokups-polymer --example polymer_batch --release -- lamarckian
```

Replace `lamarckian` with `single-point` or `baldwinian` to compare inheritance semantics.

For a repeatable browser-kernel benchmark without rendering:

```sh
sh scripts/build-static.sh
node scripts/benchmark-wasm.mjs
```

## Run the hot evaluator service

Install the optional Python dependencies once, then keep this process running across the complete evolutionary search:

```sh
python -m pip install -e 'python/kups_backend[service]'
EVOKUPS_KERNEL=kups evokups-backend
```

Creating a session compiles and warms its static population shape. Every subsequent generation reuses the loaded kUPS/JAX process and compiled executable. Set `EVOKUPS_KERNEL=reference` to use the dense JAX validation kernel instead of the kUPS cell-list kernel.

The backend also contains a guarded kUPS ToJAX MACE path. It is deliberately not a drop-in replacement for the demo bead potential: use either an atomistic model after backmapping or a MACE model trained on the declared coarse-grained mapping. For a compatible exported model:

```sh
EVOKUPS_KERNEL=mace \
EVOKUPS_MACE_MODEL=/absolute/path/model.zip \
EVOKUPS_MACE_REPRESENTATION=coarse-grained-trained \
EVOKUPS_MACE_TYPE_NUMBERS=6,8 \
evokups-backend
```

`EVOKUPS_MACE_TYPE_NUMBERS` maps demo type IDs `0,1,...` to the node numbers used during training. Only enable this route with an explicit chemistry-specific mapping.

For this local workspace, the existing `kups-diff-polymer-demo` environment already contains JAX and kUPS. The dependency-free HTTP wrapper uses it directly:

```sh
sh scripts/run-hot-kups.sh
```

Leave that terminal running, serve the static site in a second terminal, select **External kUPS/JAX service**, and keep the backend URL at `http://127.0.0.1:8766`. The static page cannot start this Python process by itself.

## Current scope

- Fixed atom count and composition.
- Deterministic seeded initialization and Gaussian mutation.
- Elitist generational search.
- Single-point genetic, Lamarckian, and Baldwinian evolution modes.
- Evaluator-provided force relaxation with clipped update steps.
- Sequential evaluator interface with a batch-shaped extension point.
- Untruncated Lennard-Jones potential in reduced units.
- Indexed polydisperse chains with fixed batch shape across a population.
- Linear, ring, dendritic, and bottlebrush graph templates with degree-bounded connectivity.
- User-selectable systems up to 50,000 beads and 1,000 chains in the browser controls; practical throughput depends on population size, architecture, browser, and hardware.
- Parent identifiers for evolutionary lineage.
- Periodic cell-list evaluation with bonded-neighbour exclusions.
- Correlated fixed-angle chain initialization with local clearance selection.
- Rigid-chain, crankshaft, and locally Metropolis-style-filtered directed double-bridge operators with an adjustable rearrangement probability and algorithmic acceptance scale.
- Explicit reduced units: `σ`, `ε`, `ρ* = Nσ³/V`, `P* = Pσ³/ε`, and `T* = kBT/ε`.
- Fast per-candidate bond, overlap, angle, dihedral, first-shell low-q density, linked-cell vacancy, and reduced-pressure-target terms; detailed void probes are best-candidate-only diagnostics.
- Optional bounded shared-cell pressure response for local pre-packing while retaining one fixed-shape population batch; set `κ_g = 0` for fixed volume.
- Architecture-resolved morphology controls: contour persistence, non-planar ring puckering with projected closure, dendrimer branching/spacer length, and bottlebrush graft spacing/side-chain length.
- Incremental linked-cell initialization and periodic linked-cell nonbonded evaluation.
- Energy-only hot JAX batches for single-point evolution; force tensors are reserved for modes that actually relax coordinates.
- Rigid-body force relaxation that preserves each chain conformation during the pre-packing stage.

The optional browser bridge registers a hot session, transfers an entire population with evolutionary metadata, and connects Rust's `ask` and `tell` phases to the service.
