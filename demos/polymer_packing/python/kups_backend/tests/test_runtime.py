from __future__ import annotations

import unittest

from kups_backend.runtime import (
    BatchSignature,
    HotEvaluatorRuntime,
    PopulationBatch,
    StaticTopology,
)


class _Compiled:
    def evaluate(self, positions):
        energies = tuple(
            sum(value * value for atom in candidate for value in atom)
            for candidate in positions
        )
        forces = tuple(
            tuple(tuple(-2.0 * value for value in atom) for atom in candidate)
            for candidate in positions
        )
        return energies, forces


class _Factory:
    def __init__(self) -> None:
        self.compile_calls = 0

    def compile(self, signature, topology):
        self.compile_calls += 1
        return _Compiled()


class RuntimeTests(unittest.TestCase):
    def test_compilation_is_reused_across_generations_and_sessions(self) -> None:
        factory = _Factory()
        runtime = HotEvaluatorRuntime(factory)
        topology = StaticTopology(chain_lengths=(2, 3), box_length=8.0)
        signature = BatchSignature(population_size=2, atom_count=5)
        first_session = runtime.create_session(signature, topology)
        second_session = runtime.create_session(signature, topology)
        positions = tuple(
            tuple((float(atom), 0.0, 0.0) for atom in range(5)) for _ in range(2)
        )
        batch = PopulationBatch(
            generation=0,
            candidate_ids=(10, 11),
            parent_ids=(None, None),
            operation_codes=(0, 0),
            target_genes=(None, None),
            operation_magnitudes=(0.0, 0.0),
            positions=positions,
        )

        first = runtime.evaluate(first_session, batch)
        second = runtime.evaluate(
            first_session,
            PopulationBatch(
                generation=1,
                candidate_ids=(12, 13),
                parent_ids=(10, 11),
                operation_codes=(3, 3),
                target_genes=(0, 1),
                operation_magnitudes=(0.4, 0.6),
                positions=positions,
            ),
        )
        runtime.evaluate(second_session, batch)

        self.assertEqual(factory.compile_calls, 1)
        self.assertEqual(runtime.compile_count, 1)
        self.assertEqual(first.evaluation_count, 1)
        self.assertEqual(second.evaluation_count, 2)
        self.assertEqual(second.parent_ids, (10, 11))
        self.assertEqual(second.target_genes, (0, 1))


if __name__ == "__main__":
    unittest.main()
