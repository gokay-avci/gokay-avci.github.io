+++
title = "Zero-Cost Abstractions in Science"
date = 2025-01-15
description = "Why Rust is replacing Fortran."
[taxonomies]
tags = ["Rust", "Science"]
categories = ["Journal"]
+++

In Materials Science, we often write prototype simulations in Python and production codes in Fortran. Rust offers a third way.


### The Iterator Pattern
Consider calculating the average energy of a lattice:

```rust
let total_energy: f64 = lattice
    .iter()
    .zip(neighbors.iter())
    .filter(|(atom, neighbor)| atom.spin != neighbor.spin)
    .map(|_| 1.0)
    .sum();
```
