from __future__ import annotations

from dataclasses import dataclass
from hashlib import sha256
from threading import RLock
from time import monotonic
from typing import Protocol
from uuid import uuid4


@dataclass(frozen=True)
class BatchSignature:
    population_size: int
    atom_count: int
    dtype: str = "float32"
    neighbour_capacity: int = 4096
    relaxation_steps: int = 0
    compute_forces: bool = True


@dataclass(frozen=True)
class StaticTopology:
    chain_lengths: tuple[int, ...]
    box_length: float
    bead_types: tuple[int, ...] = ()

    def __post_init__(self) -> None:
        if self.bead_types and len(self.bead_types) != self.atom_count:
            raise ValueError("bead_types must contain one identity per bead")
        if any(bead_type < 0 for bead_type in self.bead_types):
            raise ValueError("bead types must be non-negative")

    @property
    def atom_count(self) -> int:
        return sum(self.chain_lengths)

    @property
    def fingerprint(self) -> str:
        payload = (
            f"{self.chain_lengths}|{self.box_length:.12g}|{self.bead_types}"
        ).encode()
        return sha256(payload).hexdigest()[:16]


@dataclass(frozen=True)
class PopulationBatch:
    generation: int
    candidate_ids: tuple[int, ...]
    parent_ids: tuple[int | None, ...]
    operation_codes: tuple[int, ...]
    target_genes: tuple[int | None, ...]
    operation_magnitudes: tuple[float, ...]
    positions: tuple[tuple[tuple[float, float, float], ...], ...]


@dataclass(frozen=True)
class EvaluationBatch:
    generation: int
    candidate_ids: tuple[int, ...]
    parent_ids: tuple[int | None, ...]
    operation_codes: tuple[int, ...]
    target_genes: tuple[int | None, ...]
    operation_magnitudes: tuple[float, ...]
    energies: tuple[float, ...]
    forces: tuple[tuple[tuple[float, float, float], ...], ...]
    compile_count: int
    evaluation_count: int


class CompiledBatchEvaluator(Protocol):
    def evaluate(
        self,
        positions: tuple[tuple[tuple[float, float, float], ...], ...],
    ) -> tuple[
        tuple[float, ...],
        tuple[tuple[tuple[float, float, float], ...], ...],
    ]: ...


class KernelFactory(Protocol):
    def compile(
        self,
        signature: BatchSignature,
        topology: StaticTopology,
    ) -> CompiledBatchEvaluator: ...


@dataclass
class _Session:
    signature: BatchSignature
    topology: StaticTopology
    evaluator: CompiledBatchEvaluator
    evaluation_count: int = 0
    last_used: float = 0.0


class HotEvaluatorRuntime:
    """Owns long-lived compiled kernels and never reloads JAX between generations."""

    def __init__(self, factory: KernelFactory) -> None:
        self._factory = factory
        self._compiled: dict[tuple[BatchSignature, str], CompiledBatchEvaluator] = {}
        self._sessions: dict[str, _Session] = {}
        self._compile_count = 0
        self._lock = RLock()

    @property
    def compile_count(self) -> int:
        return self._compile_count

    def create_session(
        self,
        signature: BatchSignature,
        topology: StaticTopology,
    ) -> str:
        self._validate_signature(signature, topology)
        cache_key = (signature, topology.fingerprint)
        with self._lock:
            evaluator = self._compiled.get(cache_key)
            if evaluator is None:
                evaluator = self._factory.compile(signature, topology)
                self._compiled[cache_key] = evaluator
                self._compile_count += 1
            session_id = uuid4().hex
            self._sessions[session_id] = _Session(
                signature=signature,
                topology=topology,
                evaluator=evaluator,
                last_used=monotonic(),
            )
            return session_id

    def evaluate(self, session_id: str, batch: PopulationBatch) -> EvaluationBatch:
        with self._lock:
            session = self._sessions.get(session_id)
            if session is None:
                raise KeyError(f"unknown evaluator session: {session_id}")
        self._validate_batch(session.signature, batch)
        energies, forces = session.evaluator.evaluate(batch.positions)
        with self._lock:
            session.evaluation_count += 1
            session.last_used = monotonic()
            evaluation_count = session.evaluation_count
        return EvaluationBatch(
            generation=batch.generation,
            candidate_ids=batch.candidate_ids,
            parent_ids=batch.parent_ids,
            operation_codes=batch.operation_codes,
            target_genes=batch.target_genes,
            operation_magnitudes=batch.operation_magnitudes,
            energies=energies,
            forces=forces,
            compile_count=self._compile_count,
            evaluation_count=evaluation_count,
        )

    def close_session(self, session_id: str) -> None:
        with self._lock:
            self._sessions.pop(session_id, None)

    @staticmethod
    def _validate_signature(
        signature: BatchSignature,
        topology: StaticTopology,
    ) -> None:
        if signature.population_size < 1:
            raise ValueError("population_size must be positive")
        if signature.atom_count != topology.atom_count:
            raise ValueError("signature atom_count does not match topology")
        if signature.neighbour_capacity < signature.atom_count:
            raise ValueError("neighbour_capacity is too small for the topology")

    @staticmethod
    def _validate_batch(signature: BatchSignature, batch: PopulationBatch) -> None:
        if len(batch.positions) != signature.population_size:
            raise ValueError("population batch has the wrong population size")
        if len(batch.candidate_ids) != signature.population_size:
            raise ValueError("candidate_ids has the wrong length")
        if len(batch.parent_ids) != signature.population_size:
            raise ValueError("parent_ids has the wrong length")
        if len(batch.operation_codes) != signature.population_size:
            raise ValueError("operation_codes has the wrong length")
        if len(batch.target_genes) != signature.population_size:
            raise ValueError("target_genes has the wrong length")
        if len(batch.operation_magnitudes) != signature.population_size:
            raise ValueError("operation_magnitudes has the wrong length")
        if any(len(candidate) != signature.atom_count for candidate in batch.positions):
            raise ValueError("a candidate has the wrong atom count")
