from __future__ import annotations

from dataclasses import dataclass
import dataclasses
import os
from pathlib import Path
from typing import Any, Literal

from kups_backend.runtime import BatchSignature, StaticTopology

MaceRepresentation = Literal["atomistic-backmapped", "coarse-grained-trained"]


@dataclass(frozen=True)
class MaceStageConfig:
    """Explicit mapping contract for a kUPS-hosted MACE stage."""

    model_path: Path
    representation: MaceRepresentation
    node_numbers: tuple[int, ...]

    @classmethod
    def from_environment(cls, topology: StaticTopology) -> "MaceStageConfig":
        raw_path = os.environ.get("EVOKUPS_MACE_MODEL")
        if not raw_path:
            raise ValueError("EVOKUPS_MACE_MODEL is required for the MACE kernel")
        representation = os.environ.get(
            "EVOKUPS_MACE_REPRESENTATION", "coarse-grained-trained"
        )
        if representation not in {"atomistic-backmapped", "coarse-grained-trained"}:
            raise ValueError(
                "EVOKUPS_MACE_REPRESENTATION must be atomistic-backmapped or "
                "coarse-grained-trained"
            )

        if representation == "coarse-grained-trained":
            if not topology.bead_types:
                raise ValueError("the coarse-grained MACE stage requires bead_types")
            raw_mapping = os.environ.get("EVOKUPS_MACE_TYPE_NUMBERS")
            if not raw_mapping:
                raise ValueError(
                    "EVOKUPS_MACE_TYPE_NUMBERS must map CG type IDs to the node numbers "
                    "used to train the model"
                )
            mapping = tuple(int(value) for value in raw_mapping.split(","))
            try:
                node_numbers = tuple(
                    mapping[type_id] for type_id in topology.bead_types
                )
            except IndexError as error:
                raise ValueError(
                    "a bead type is absent from EVOKUPS_MACE_TYPE_NUMBERS"
                ) from error
        else:
            raw_numbers = os.environ.get("EVOKUPS_MACE_ATOMIC_NUMBERS")
            if not raw_numbers:
                raise ValueError(
                    "EVOKUPS_MACE_ATOMIC_NUMBERS is required after backmapping"
                )
            node_numbers = tuple(int(value) for value in raw_numbers.split(","))

        if len(node_numbers) != topology.atom_count:
            raise ValueError(
                "the MACE node-number count must match the evaluated coordinates"
            )
        if any(number <= 0 for number in node_numbers):
            raise ValueError("MACE node numbers must be positive")

        model_path = Path(raw_path).expanduser()
        if not model_path.is_file():
            raise ValueError(f"MACE model does not exist: {model_path}")
        if model_path.suffix != ".zip":
            raise ValueError(
                "the hot JAX path expects a kUPS ToJAX .zip export, not a PyTorch .model"
            )
        return cls(model_path, representation, node_numbers)


def mace_capability() -> dict[str, object]:
    model = os.environ.get("EVOKUPS_MACE_MODEL")
    return {
        "configured": bool(model),
        "model": Path(model).name if model else None,
        "representation": os.environ.get("EVOKUPS_MACE_REPRESENTATION"),
    }


def build_mace_energy(topology: StaticTopology, signature: BatchSignature):
    """Load one ToJAX MACE export and build a capacity-bounded kUPS graph."""
    import jax.numpy as jnp
    from jax.tree_util import register_dataclass
    from kups.core.capacity import LensCapacity
    from kups.core.cell import OrthogonalFrame, PeriodicCell
    from kups.core.data.index import Index
    from kups.core.data.table import Table
    from kups.core.lens import lens
    from kups.core.neighborlist import CellListNeighborList
    from kups.core.typing import ParticleId, SystemId
    from kups.potential.mliap.tojax import TojaxedMliap

    @register_dataclass
    @dataclasses.dataclass(frozen=True)
    class MacePoints:
        positions: Any
        atomic_numbers: Any
        system: Any
        inclusion: Any
        exclusion: Any

    @register_dataclass
    @dataclasses.dataclass(frozen=True)
    class MaceSystems:
        cell: Any

    config = MaceStageConfig.from_environment(topology)
    model = TojaxedMliap.from_zip_file(config.model_path)
    atom_count = topology.atom_count
    particle_keys = tuple(ParticleId(index) for index in range(atom_count))
    system_keys = (SystemId(0),)
    capacity = LensCapacity(
        size=signature.neighbour_capacity,
        size_lens=lens(lambda state: state),
    )
    image_capacity = LensCapacity(size=27, size_lens=lens(lambda state: state))
    cutoff = float(model.cutoff.data[0])
    cells_per_axis = max(1, int(topology.box_length // cutoff))
    selector = CellListNeighborList(
        avg_candidates=capacity,
        avg_edges=capacity,
        cells=LensCapacity(
            size=cells_per_axis**3,
            size_lens=lens(lambda state: state),
        ),
        avg_image_candidates=image_capacity,
    )
    atomic_numbers = jnp.asarray((*config.node_numbers, 0), dtype=jnp.int32)
    batch = jnp.asarray((*([0] * atom_count), 1), dtype=jnp.int32)
    cell = jnp.asarray(
        (
            (
                (topology.box_length, 0.0, 0.0),
                (0.0, topology.box_length, 0.0),
                (0.0, 0.0, topology.box_length),
            ),
            ((0.0, 0.0, 0.0),) * 3,
        ),
        dtype=jnp.float32,
    )

    def energy(positions: Any):
        particles = Table(
            keys=particle_keys,
            data=MacePoints(
                positions=positions,
                atomic_numbers=atomic_numbers[:-1],
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
            data=MaceSystems(
                cell=PeriodicCell(
                    frame=OrthogonalFrame(
                        lengths=jnp.asarray(
                            [[topology.box_length] * 3], dtype=positions.dtype
                        )
                    )
                )
            ),
            _cls=SystemId,
        )
        edges = selector(
            lh=particles,
            rh=None,
            systems=systems,
            cutoffs=model.cutoff,
            rh_index_remap=None,
        )
        indices = edges.indices.indices
        valid = jnp.all((indices >= 0) & (indices < atom_count), axis=1)
        valid &= indices[:, 0] != indices[:, 1]
        safe_indices = jnp.where(valid[:, None], indices, atom_count)
        cell_offsets = edges.shifts.reshape((edges.shifts.shape[0], 3))
        cell_offsets = jnp.where(valid[:, None], cell_offsets, 0)
        padded_positions = jnp.pad(positions, ((0, 1), (0, 0)))
        values = model.call(
            {
                "pos": padded_positions,
                "atomic_numbers": atomic_numbers,
                "cell": cell.astype(positions.dtype),
                "pbc": jnp.asarray(((True, True, True), (False, False, False))),
                "edge_index": safe_indices.T,
                "cell_offsets": cell_offsets.astype(positions.dtype),
                "batch": batch,
                "charge": jnp.zeros((2,), dtype=positions.dtype),
                "spin": jnp.zeros((2,), dtype=positions.dtype),
            }
        )
        return jnp.ravel(values)[0]

    return energy
