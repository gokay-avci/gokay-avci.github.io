from __future__ import annotations

from typing import Any

from kups_backend.runtime import BatchSignature, StaticTopology


def build_reference_energy(
    topology: StaticTopology,
    _signature: BatchSignature,
):
    """Dense JAX reference used to validate the hot runtime before kUPS wiring."""
    import jax.numpy as jnp

    chain_ids = []
    bead_ids = []
    for chain, length in enumerate(topology.chain_lengths):
        chain_ids.extend([chain] * length)
        bead_ids.extend(range(length))
    chain_index = jnp.asarray(chain_ids)
    bead_index = jnp.asarray(bead_ids)
    upper = jnp.triu(
        jnp.ones((topology.atom_count, topology.atom_count), dtype=bool), 1
    )
    directly_bonded = (chain_index[:, None] == chain_index[None, :]) & (
        jnp.abs(bead_index[:, None] - bead_index[None, :]) == 1
    )
    nonbonded_mask = upper & ~directly_bonded

    def energy(positions: Any):
        displacement = positions[:, None, :] - positions[None, :, :]
        displacement -= topology.box_length * jnp.round(
            displacement / topology.box_length
        )
        distance = jnp.sqrt(jnp.sum(displacement * displacement, axis=-1) + 1.0e-12)
        overlap = jnp.maximum(0.0, 1.0 - distance)
        overlap_energy = 5.0 * jnp.sum(
            jnp.where(nonbonded_mask, overlap * overlap, 0.0)
        )
        bond_vectors = positions[1:] - positions[:-1]
        adjacent = (chain_index[1:] == chain_index[:-1]).astype(positions.dtype)
        bond_distance = jnp.sqrt(
            jnp.sum(bond_vectors * bond_vectors, axis=-1) + 1.0e-12
        )
        bond_energy = 25.0 * jnp.sum(adjacent * (bond_distance - 1.0) ** 2)
        wave_number = 2.0 * jnp.pi / topology.box_length
        phases = wave_number * positions
        cosine_sum = jnp.sum(jnp.cos(phases), axis=0)
        sine_sum = jnp.sum(jnp.sin(phases), axis=0)
        density_energy = jnp.sum(cosine_sum**2 + sine_sum**2) / topology.atom_count
        return overlap_energy + bond_energy + density_energy

    return energy
