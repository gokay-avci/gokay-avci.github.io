+++
title = "Rust as a Third Way in Materials Science: Between Python Prototypes and Fortran Production"
date = 2026-01-12
description = "Why Rust is uniquely well-suited for scientific computing: safety, performance, and modern tooling — with concrete lattice and simulation examples."
[taxonomies]
tags = ["Rust", "HPC", "Simulation", "Reproducibility", "Engineering-Notes"]
categories = ["Methods"]
+++


In materials science, we often prototype simulations in **Python** and ship production codes in **Fortran**. That split is understandable: Python is fast to write and iterate, and Fortran is fast to run.

But it can also be frustrating:

- the prototype logic slowly drifts away from the production logic  
- “one-off” scripts become critical infrastructure  
- performance issues stay hidden until the last minute  
- reproducibility becomes optional (until it suddenly matters)

I’ve started to see **Rust** as a genuinely useful “third way”: expressive enough to explore ideas, and safe/fast enough to become the thing you actually run on real systems (and, increasingly, in the browser).

---

## Why Rust feels different (in practice)

Rust tends to push you towards good habits *without* feeling like a chore:

- **C/Fortran-class performance** when you need it
- **memory safety** without a garbage collector
- a type system that catches a surprising number of “scientific bugs”
- excellent tooling by default: `cargo`, tests, formatting, docs

It’s not that Python or Fortran are “bad” — it’s that Rust helps you keep the *clarity of a prototype* while writing something that can survive contact with production.

---

## Example 1: Iterator pattern — a simple lattice energy

Say we have a spin lattice and we want a toy energy proxy:
count mismatched nearest-neighbour spins and add a unit penalty.

### Rust

```rust
let total_energy: f64 = lattice
    .iter()
    .zip(neighbors.iter())
    .filter(|(atom, neighbor)| atom.spin != neighbor.spin)
    .map(|_| 1.0)
    .sum();
```

I like this because it reads almost like a sentence:

> zip the atoms with their neighbours → keep only mismatches → count them → sum

Also: no manual indices, no intermediate buffers, and (in practice) it compiles to very efficient loops.

### Python equivalent

```python
total_energy = sum(
    1.0
    for atom, nb in zip(lattice, neighbors)
    if atom.spin != nb.spin
)
```

Python is similarly expressive for a prototype. If performance becomes a real constraint, you typically switch to NumPy vectorisation, Numba, or push the hot loop into C/Fortran.

### Fortran-style loop

```fortran
total_energy = 0.0d0
do i = 1, n
  if (spin(i) .ne. spin(nb(i))) then
    total_energy = total_energy + 1.0d0
  end if
end do
```

This is fast and familiar — but the moment the logic grows (boundary conditions, multiple neighbour shells, different interaction terms), the index-heavy style becomes more error-prone.

Rust lets you keep the *“loop efficiency”* without forcing you into *“index soup”*.

---

## Example 2: Monte Carlo acceptance (Metropolis)

This is a tiny piece of logic, but it’s the kind of thing that gets duplicated across many projects.

### Rust: explicit RNG, reproducible by default

```rust
use rand::Rng;

fn accept_move(delta_e: f64, beta: f64, rng: &mut impl Rng) -> bool {
    if delta_e <= 0.0 {
        true
    } else {
        let p = (-beta * delta_e).exp();
        rng.gen::<f64>() < p
    }
}
```

A subtle thing I appreciate here: passing the RNG explicitly makes it easier to keep runs reproducible (seed the RNG once, thread it through your simulation).

### Python

```python
import random
import math

def accept_move(delta_e, beta):
    if delta_e <= 0:
        return True
    p = math.exp(-beta * delta_e)
    return random.random() < p
```

Totally fine for exploration. Reproducibility is still easy, but it’s more of a “remember to do it” habit than something the design encourages.

---

## Example 3: Neighbour lists without footguns

Neighbour lists are where performance and correctness often collide.

### Rust: encode invariants in the data structure

```rust
struct NeighborList {
    // For each atom i, store indices of neighbours
    list: Vec<Vec<usize>>,
}

impl NeighborList {
    fn neighbors_of(&self, i: usize) -> &[usize] {
        &self.list[i]
    }
}
```

Then your hot loop reads cleanly:

```rust
let i = 10;
for &j in nlist.neighbors_of(i) {
    // compute pair interaction i-j
}
```

You can later swap the internal layout (flat arrays, CSR-like storage, etc.) without changing your public API — which is a nice “performance upgrade path”.

---

## Where Rust really shines for scientific work

### 1) One “truth model” across environments
The same simulation kernel can be:

- a CLI tool for HPC production
- a library called from Python (via pyo3) if you want notebooks
- a WASM demo embedded in a website (which is *amazing* for communication)

This changes how you present your work. A paper figure can become a reproducible interactive.

### 2) Defensive programming becomes the default
Rust makes it harder to accidentally do the wrong thing:

- explicit error handling (`Result`)
- no silent null dereferences
- fewer unintended copies
- test tooling built into the workflow

### 3) Performance tuning is incremental
You can start with readable iterator code.
If you need more speed later, you can move to:

- flat arrays
- `chunks_exact` / packed memory
- parallelism (e.g., `rayon`)
- SIMD where it makes sense

…and keep the codebase coherent.

---

## A hybrid workflow I actually like
I don’t think Rust replaces everything. For me, the sweet spot looks like:

- **Python** for rapid exploration + plotting  
- **Rust** for the simulation core + tooling + reproducible workflows  
- **Fortran** where legacy solvers are trusted and already validated  

Rust ends up being the connective tissue that keeps prototypes and production closer together.

---
