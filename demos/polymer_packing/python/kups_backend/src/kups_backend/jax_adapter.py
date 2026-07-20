from __future__ import annotations

from collections.abc import Callable
import math
from typing import Any

from kups_backend.runtime import (
    BatchSignature,
    CompiledBatchEvaluator,
    KernelFactory,
    StaticTopology,
)

EnergyBuilder = Callable[[StaticTopology, BatchSignature], Callable[[Any], Any]]


class JaxKernelFactory(KernelFactory):
    """Loads JAX once and caches one compiled vmapped kernel per static shape."""

    def __init__(self, energy_builder: EnergyBuilder) -> None:
        import jax
        import jax.numpy as jnp

        self._jax = jax
        self._jnp = jnp
        self._energy_builder = energy_builder

    def compile(
        self,
        signature: BatchSignature,
        topology: StaticTopology,
    ) -> CompiledBatchEvaluator:
        energy_one = self._energy_builder(topology, signature)
        if signature.compute_forces:
            kernel = self._jax.value_and_grad(energy_one)
        else:
            kernel = energy_one
        batched = self._jax.jit(self._jax.vmap(kernel))
        sample = self._jnp.arange(
            signature.population_size * signature.atom_count * 3,
            dtype=signature.dtype,
        ).reshape((signature.population_size, signature.atom_count, 3))
        sample = sample * 0.173
        warmed = batched(sample)
        if signature.compute_forces:
            energies, gradients = warmed
            energies.block_until_ready()
            gradients.block_until_ready()
        else:
            warmed.block_until_ready()
        return _CompiledJaxEvaluator(batched, self._jnp, signature.compute_forces)


class _CompiledJaxEvaluator(CompiledBatchEvaluator):
    def __init__(
        self, batched: Callable[[Any], Any], jnp: Any, compute_forces: bool
    ) -> None:
        self._batched = batched
        self._jnp = jnp
        self._compute_forces = compute_forces

    def evaluate(self, positions):
        device_positions = self._jnp.asarray(positions)
        result = self._batched(device_positions)
        if self._compute_forces:
            energies, gradients = result
            gradients.block_until_ready()
        else:
            energies = result
        energy_values = tuple(float(value) for value in energies.tolist())
        force_values = (
            tuple(
                tuple(
                    tuple(float(-component) for component in atom) for atom in candidate
                )
                for candidate in gradients.tolist()
            )
            if self._compute_forces
            else ()
        )
        if any(not math.isfinite(value) for value in energy_values):
            raise ValueError("kUPS/JAX returned a non-finite population energy")
        if any(
            not math.isfinite(component)
            for candidate in force_values
            for atom in candidate
            for component in atom
        ):
            raise ValueError("kUPS/JAX returned a non-finite force component")
        return energy_values, force_values
