from __future__ import annotations

import dataclasses
from typing import Any

from kups_backend.runtime import BatchSignature, StaticTopology


def build_kups_energy(
    topology: StaticTopology,
    signature: BatchSignature,
):
    """Build one-candidate kUPS energy; the hot factory vmaps and compiles it."""
    import jax.numpy as jnp
    from jax.tree_util import register_dataclass
    from kups.core.capacity import LensCapacity
    from kups.core.cell import OrthogonalFrame, PeriodicCell
    from kups.core.data.index import Index
    from kups.core.data.table import Table
    from kups.core.lens import lens
    from kups.core.neighborlist import CellListNeighborList
    from kups.core.typing import ParticleId, SystemId

    @register_dataclass
    @dataclasses.dataclass(frozen=True)
    class PolymerPoints:
        positions: Any
        system: Any
        inclusion: Any
        exclusion: Any

    @register_dataclass
    @dataclasses.dataclass(frozen=True)
    class PolymerSystems:
        cell: Any

    atom_count = topology.atom_count
    particle_keys = tuple(ParticleId(index) for index in range(atom_count))
    system_keys = (SystemId(0),)
    chain_ids = []
    bead_ids = []
    angle_indices = []
    dihedral_indices = []
    offset = 0
    for chain, length in enumerate(topology.chain_lengths):
        chain_ids.extend([chain] * length)
        bead_ids.extend(range(length))
        angle_indices.extend(
            (offset + index - 1, offset + index, offset + index + 1)
            for index in range(1, length - 1)
        )
        dihedral_indices.extend(
            (offset + index, offset + index + 1, offset + index + 2, offset + index + 3)
            for index in range(length - 3)
        )
        offset += length
    chain_index = jnp.asarray(chain_ids)
    bead_index = jnp.asarray(bead_ids)
    capacity = LensCapacity(
        size=signature.neighbour_capacity,
        size_lens=lens(lambda state: state),
    )
    image_capacity = LensCapacity(size=27, size_lens=lens(lambda state: state))
    cells_per_axis = max(1, int(topology.box_length // 1.2))
    cell_capacity = LensCapacity(
        size=cells_per_axis**3,
        size_lens=lens(lambda state: state),
    )
    selector = CellListNeighborList(
        avg_candidates=capacity,
        avg_edges=capacity,
        cells=cell_capacity,
        avg_image_candidates=image_capacity,
    )
    reciprocal_modes = ((1, 0, 0), (0, 1, 0), (0, 0, 1))
    mode_vectors = jnp.asarray(reciprocal_modes, dtype=jnp.float32)

    def energy(positions: Any):
        particles = Table(
            keys=particle_keys,
            data=PolymerPoints(
                positions=positions,
                system=Index(
                    keys=system_keys,
                    indices=jnp.zeros((atom_count,), dtype=jnp.int32),
                    _cls=SystemId,
                ),
                inclusion=Index(
                    keys=(0,),
                    indices=jnp.zeros((atom_count,), dtype=jnp.int32),
                    max_count=atom_count,
                    _cls=int,
                ),
                exclusion=Index(
                    keys=tuple(range(atom_count)),
                    indices=jnp.arange(atom_count, dtype=jnp.int32),
                    max_count=atom_count,
                    _cls=int,
                ),
            ),
            _cls=ParticleId,
        )
        systems = Table(
            keys=system_keys,
            data=PolymerSystems(
                cell=PeriodicCell(
                    frame=OrthogonalFrame(
                        lengths=jnp.array(
                            [[topology.box_length] * 3], dtype=positions.dtype
                        )
                    )
                )
            ),
            _cls=SystemId,
        )
        cutoffs = Table(
            keys=system_keys,
            data=jnp.array([1.2], dtype=positions.dtype),
            _cls=SystemId,
        )
        edges = selector(
            lh=particles,
            rh=None,
            systems=systems,
            cutoffs=cutoffs,
            rh_index_remap=None,
        )
        edge_indices = edges.indices.indices
        valid = jnp.all((edge_indices >= 0) & (edge_indices < atom_count), axis=1)
        safe = jnp.where(valid[:, None], edge_indices, 0)
        first = safe[:, 0]
        second = safe[:, 1]
        directly_bonded = (chain_index[first] == chain_index[second]) & (
            jnp.abs(bead_index[first] - bead_index[second]) == 1
        )
        shifts = edges.shifts.reshape((edges.shifts.shape[0], 3)).astype(
            positions.dtype
        )
        displacement = (
            positions[second] + shifts * topology.box_length - positions[first]
        )
        # Padded neighbour-list rows are mapped to particle zero. The epsilon
        # keeps the norm derivative finite at those masked zero displacements.
        distance = jnp.sqrt(jnp.sum(displacement * displacement, axis=1) + 1.0e-12)
        overlap = jnp.maximum(0.0, 1.0 - distance)
        interacting = valid & (first != second) & ~directly_bonded
        overlap_energy = 5.0 * jnp.sum(jnp.where(interacting, overlap * overlap, 0.0))

        bond_vectors = positions[1:] - positions[:-1]
        adjacent = (chain_index[1:] == chain_index[:-1]).astype(positions.dtype)
        bond_distance = jnp.sqrt(
            jnp.sum(bond_vectors * bond_vectors, axis=-1) + 1.0e-12
        )
        bond_energy = 25.0 * jnp.sum(adjacent * (bond_distance - 1.0) ** 2)

        angle_energy = jnp.asarray(0.0, dtype=positions.dtype)
        if angle_indices:
            triples = jnp.asarray(angle_indices, dtype=jnp.int32)
            left = positions[triples[:, 0]] - positions[triples[:, 1]]
            right = positions[triples[:, 2]] - positions[triples[:, 1]]
            cosine = jnp.sum(left * right, axis=1) / jnp.sqrt(
                jnp.sum(left * left, axis=1) * jnp.sum(right * right, axis=1) + 1.0e-12
            )
            angle_energy = 2.0 * jnp.sum((cosine + 0.30) ** 2)

        dihedral_energy = jnp.asarray(0.0, dtype=positions.dtype)
        if dihedral_indices:
            quads = jnp.asarray(dihedral_indices, dtype=jnp.int32)
            first_bond = positions[quads[:, 1]] - positions[quads[:, 0]]
            second_bond = positions[quads[:, 2]] - positions[quads[:, 1]]
            third_bond = positions[quads[:, 3]] - positions[quads[:, 2]]
            first_normal = jnp.cross(first_bond, second_bond)
            second_normal = jnp.cross(second_bond, third_bond)
            cosine = jnp.sum(first_normal * second_normal, axis=1) / jnp.sqrt(
                jnp.sum(first_normal * first_normal, axis=1)
                * jnp.sum(second_normal * second_normal, axis=1)
                + 1.0e-12
            )
            dihedral_energy = 0.375 * jnp.sum((cosine + 0.5) ** 2)

        wave_number = 2.0 * jnp.pi / topology.box_length
        phases = wave_number * positions @ mode_vectors.T
        cosine_sum = jnp.sum(jnp.cos(phases), axis=0)
        sine_sum = jnp.sum(jnp.sin(phases), axis=0)
        density_energy = jnp.sum(cosine_sum**2 + sine_sum**2) / (
            atom_count * len(reciprocal_modes)
        )

        wrapped = jnp.mod(positions, topology.box_length)
        cell_coordinates = jnp.minimum(
            jnp.floor(wrapped / topology.box_length * cells_per_axis).astype(jnp.int32),
            cells_per_axis - 1,
        )
        linear_cells = (
            cell_coordinates[:, 0] * cells_per_axis + cell_coordinates[:, 1]
        ) * cells_per_axis + cell_coordinates[:, 2]
        cell_counts = jnp.bincount(linear_cells, length=cells_per_axis**3)
        empty_fraction = jnp.mean(cell_counts == 0)
        vacancy_energy = 2.0 * atom_count * jnp.maximum(0.0, empty_fraction - 0.18) ** 2
        return (
            overlap_energy
            + bond_energy
            + angle_energy
            + dihedral_energy
            + density_energy
            + vacancy_energy
        )

    return energy
