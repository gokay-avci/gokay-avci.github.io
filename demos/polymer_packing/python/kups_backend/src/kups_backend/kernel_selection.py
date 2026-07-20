from __future__ import annotations

import os

from kups_backend.mace_stage import mace_capability


def select_energy_builder():
    kernel = os.environ.get("EVOKUPS_KERNEL", "kups")
    if kernel == "kups":
        from kups_backend.kups_energy import build_kups_energy

        return kernel, build_kups_energy
    if kernel == "reference":
        from kups_backend.reference_energy import build_reference_energy

        return kernel, build_reference_energy
    if kernel == "mace":
        from kups_backend.mace_stage import build_mace_energy

        return kernel, build_mace_energy
    raise ValueError(f"unknown EVOKUPS_KERNEL: {kernel}")


def kernel_capabilities(kernel: str) -> dict[str, object]:
    return {
        "kernel": kernel,
        "mace": mace_capability(),
    }
