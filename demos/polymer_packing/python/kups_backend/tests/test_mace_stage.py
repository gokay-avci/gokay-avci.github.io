from __future__ import annotations

import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from kups_backend.mace_stage import MaceStageConfig
from kups_backend.runtime import StaticTopology


class MaceStageConfigTests(unittest.TestCase):
    def test_maps_typed_beads_for_a_cg_trained_model(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            model = Path(directory, "model.zip")
            model.touch()
            topology = StaticTopology((2, 2), 8.0, (0, 1, 1, 0))
            environment = {
                "EVOKUPS_MACE_MODEL": str(model),
                "EVOKUPS_MACE_REPRESENTATION": "coarse-grained-trained",
                "EVOKUPS_MACE_TYPE_NUMBERS": "6,8",
            }
            with patch.dict(os.environ, environment, clear=False):
                config = MaceStageConfig.from_environment(topology)
            self.assertEqual(config.node_numbers, (6, 8, 8, 6))

    def test_rejects_atomistic_species_for_raw_bead_count(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            model = Path(directory, "model.zip")
            model.touch()
            topology = StaticTopology((2, 2), 8.0, (0, 1, 1, 0))
            environment = {
                "EVOKUPS_MACE_MODEL": str(model),
                "EVOKUPS_MACE_REPRESENTATION": "atomistic-backmapped",
                "EVOKUPS_MACE_ATOMIC_NUMBERS": "6,1",
            }
            with patch.dict(os.environ, environment, clear=False):
                with self.assertRaisesRegex(ValueError, "count must match"):
                    MaceStageConfig.from_environment(topology)


if __name__ == "__main__":
    unittest.main()
