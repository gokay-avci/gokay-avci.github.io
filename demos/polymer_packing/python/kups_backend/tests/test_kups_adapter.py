from __future__ import annotations

import importlib.util
import math
import unittest

from kups_backend.jax_adapter import JaxKernelFactory
from kups_backend.kups_energy import build_kups_energy
from kups_backend.runtime import (
    BatchSignature,
    HotEvaluatorRuntime,
    PopulationBatch,
    StaticTopology,
)


@unittest.skipUnless(importlib.util.find_spec("kups"), "kUPS is not installed")
class KupsAdapterTests(unittest.TestCase):
    def test_vmapped_kernel_is_compiled_once_and_reused(self) -> None:
        topology = StaticTopology(chain_lengths=(2, 2), box_length=8.0)
        signature = BatchSignature(
            population_size=2,
            atom_count=4,
            neighbour_capacity=256,
        )
        runtime = HotEvaluatorRuntime(JaxKernelFactory(build_kups_energy))
        session = runtime.create_session(signature, topology)
        positions = (
            ((0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (3.0, 0.0, 0.0), (4.0, 0.0, 0.0)),
            ((0.0, 1.0, 0.0), (1.0, 1.0, 0.0), (3.0, 1.0, 0.0), (4.0, 1.0, 0.0)),
        )
        batch = PopulationBatch(
            generation=0,
            candidate_ids=(0, 1),
            parent_ids=(None, None),
            operation_codes=(0, 0),
            target_genes=(None, None),
            operation_magnitudes=(0.0, 0.0),
            positions=positions,
        )

        first = runtime.evaluate(session, batch)
        second = runtime.evaluate(session, batch)

        self.assertEqual(first.compile_count, 1)
        self.assertEqual(second.compile_count, 1)
        self.assertEqual(second.evaluation_count, 2)
        self.assertEqual(len(second.forces), 2)
        self.assertEqual(len(second.forces[0]), 4)
        self.assertTrue(all(math.isfinite(energy) for energy in second.energies))
        self.assertTrue(
            all(
                math.isfinite(component)
                for candidate in second.forces
                for atom in candidate
                for component in atom
            )
        )


if __name__ == "__main__":
    unittest.main()
