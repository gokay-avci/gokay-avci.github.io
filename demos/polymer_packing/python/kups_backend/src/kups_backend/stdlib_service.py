from __future__ import annotations

from dataclasses import asdict
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from urllib.parse import urlparse

from kups_backend.jax_adapter import JaxKernelFactory
from kups_backend.kernel_selection import kernel_capabilities, select_energy_builder
from kups_backend.runtime import (
    BatchSignature,
    HotEvaluatorRuntime,
    PopulationBatch,
    StaticTopology,
)


def create_runtime() -> tuple[str, HotEvaluatorRuntime]:
    kernel, energy_builder = select_energy_builder()
    return kernel, HotEvaluatorRuntime(JaxKernelFactory(energy_builder))


def create_handler(runtime: HotEvaluatorRuntime, kernel: str = "custom"):
    allowed_origins = {
        origin.strip()
        for origin in os.environ.get(
            "EVOKUPS_ALLOWED_ORIGINS",
            "http://localhost:8080,http://127.0.0.1:8080",
        ).split(",")
        if origin.strip()
    }

    class Handler(BaseHTTPRequestHandler):
        server_version = "evokUPS/0.1"

        def _send_json(self, status: int, payload: dict) -> None:
            body = json.dumps(payload, allow_nan=False).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            origin = self.headers.get("Origin")
            if origin in allowed_origins:
                self.send_header("Access-Control-Allow-Origin", origin)
                self.send_header("Vary", "Origin")
            self.end_headers()
            self.wfile.write(body)

        def _read_json(self) -> dict:
            length = int(self.headers.get("Content-Length", "0"))
            return json.loads(self.rfile.read(length))

        def do_OPTIONS(self) -> None:  # noqa: N802
            self.send_response(204)
            origin = self.headers.get("Origin")
            if origin in allowed_origins:
                self.send_header("Access-Control-Allow-Origin", origin)
                self.send_header("Vary", "Origin")
            self.send_header(
                "Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS"
            )
            self.send_header("Access-Control-Allow-Headers", "Content-Type")
            self.end_headers()

        def do_GET(self) -> None:  # noqa: N802
            if urlparse(self.path).path == "/health":
                self._send_json(
                    200,
                    {
                        "status": "hot",
                        "compile_count": runtime.compile_count,
                        **kernel_capabilities(kernel),
                    },
                )
            else:
                self._send_json(404, {"detail": "not found"})

        def do_POST(self) -> None:  # noqa: N802
            path = urlparse(self.path).path
            try:
                payload = self._read_json()
                if path == "/v1/sessions":
                    topology = StaticTopology(
                        chain_lengths=tuple(payload["chain_lengths"]),
                        box_length=float(payload["box_length"]),
                        bead_types=tuple(payload.get("bead_types", ())),
                    )
                    signature = BatchSignature(**payload["signature"])
                    session_id = runtime.create_session(signature, topology)
                    self._send_json(
                        200,
                        {
                            "session_id": session_id,
                            "topology_fingerprint": topology.fingerprint,
                            "compile_count": runtime.compile_count,
                        },
                    )
                    return
                prefix = "/v1/sessions/"
                suffix = "/evaluate"
                if path.startswith(prefix) and path.endswith(suffix):
                    session_id = path[len(prefix) : -len(suffix)]
                    batch = PopulationBatch(
                        generation=int(payload["generation"]),
                        candidate_ids=tuple(payload["candidate_ids"]),
                        parent_ids=tuple(payload["parent_ids"]),
                        operation_codes=tuple(payload["operation_codes"]),
                        target_genes=tuple(payload["target_genes"]),
                        operation_magnitudes=tuple(payload["operation_magnitudes"]),
                        positions=tuple(
                            tuple(
                                tuple(float(value) for value in atom)
                                for atom in candidate
                            )
                            for candidate in payload["positions"]
                        ),
                    )
                    self._send_json(200, asdict(runtime.evaluate(session_id, batch)))
                    return
                self._send_json(404, {"detail": "not found"})
            except KeyError as error:
                self._send_json(404, {"detail": str(error)})
            except (json.JSONDecodeError, TypeError, ValueError) as error:
                self._send_json(422, {"detail": str(error)})

        def do_DELETE(self) -> None:  # noqa: N802
            path = urlparse(self.path).path
            prefix = "/v1/sessions/"
            if path.startswith(prefix):
                runtime.close_session(path[len(prefix) :])
                self._send_json(200, {"status": "closed"})
            else:
                self._send_json(404, {"detail": "not found"})

        def log_message(self, message: str, *args: object) -> None:
            print(f"evokUPS: {message % args}")

    return Handler


def main() -> None:
    host = os.environ.get("HOST", "127.0.0.1")
    port = int(os.environ.get("PORT", "8766"))
    kernel, runtime = create_runtime()
    server = HTTPServer((host, port), create_handler(runtime, kernel))
    print(f"evokUPS hot {kernel} evaluator listening on http://{host}:{port}")
    server.serve_forever()


if __name__ == "__main__":
    main()
