from __future__ import annotations

from dataclasses import asdict
import os

from kups_backend.jax_adapter import JaxKernelFactory
from kups_backend.kernel_selection import kernel_capabilities, select_energy_builder
from kups_backend.runtime import (
    BatchSignature,
    HotEvaluatorRuntime,
    PopulationBatch,
    StaticTopology,
)


def create_app(runtime: HotEvaluatorRuntime | None = None):
    from fastapi import FastAPI, HTTPException
    from fastapi.middleware.cors import CORSMiddleware

    if runtime is None:
        kernel, energy_builder = select_energy_builder()
        runtime = HotEvaluatorRuntime(JaxKernelFactory(energy_builder))
    else:
        kernel = "custom"
    evaluator_runtime = runtime
    app = FastAPI(title="evokUPS hot evaluator")
    allowed_origins = [
        origin.strip()
        for origin in os.environ.get(
            "EVOKUPS_ALLOWED_ORIGINS",
            "http://localhost:8080,http://127.0.0.1:8080",
        ).split(",")
        if origin.strip()
    ]
    app.add_middleware(
        CORSMiddleware,
        allow_origins=allowed_origins,
        allow_credentials=False,
        allow_methods=["GET", "POST", "DELETE"],
        allow_headers=["Content-Type"],
    )

    @app.get("/health")
    def health():
        return {
            "status": "hot",
            "compile_count": evaluator_runtime.compile_count,
            **kernel_capabilities(kernel),
        }

    @app.post("/v1/sessions")
    def create_session(payload: dict):
        try:
            topology = StaticTopology(
                chain_lengths=tuple(payload["chain_lengths"]),
                box_length=float(payload["box_length"]),
                bead_types=tuple(payload.get("bead_types", ())),
            )
            signature = BatchSignature(**payload["signature"])
            session_id = evaluator_runtime.create_session(signature, topology)
            return {
                "session_id": session_id,
                "topology_fingerprint": topology.fingerprint,
                "compile_count": evaluator_runtime.compile_count,
            }
        except (KeyError, TypeError, ValueError) as error:
            raise HTTPException(status_code=422, detail=str(error)) from error

    @app.post("/v1/sessions/{session_id}/evaluate")
    def evaluate(session_id: str, payload: dict):
        try:
            batch = PopulationBatch(
                generation=int(payload["generation"]),
                candidate_ids=tuple(payload["candidate_ids"]),
                parent_ids=tuple(payload["parent_ids"]),
                operation_codes=tuple(payload["operation_codes"]),
                target_genes=tuple(payload["target_genes"]),
                operation_magnitudes=tuple(payload["operation_magnitudes"]),
                positions=tuple(
                    tuple(tuple(float(value) for value in atom) for atom in candidate)
                    for candidate in payload["positions"]
                ),
            )
            return asdict(evaluator_runtime.evaluate(session_id, batch))
        except KeyError as error:
            raise HTTPException(status_code=404, detail=str(error)) from error
        except (TypeError, ValueError) as error:
            raise HTTPException(status_code=422, detail=str(error)) from error

    @app.delete("/v1/sessions/{session_id}")
    def close_session(session_id: str):
        evaluator_runtime.close_session(session_id)
        return {"status": "closed"}

    return app


def main() -> None:
    import uvicorn

    host = os.environ.get("HOST", "127.0.0.1")
    port = int(os.environ.get("PORT", "8766"))
    uvicorn.run(create_app(), host=host, port=port)


if __name__ == "__main__":
    main()
