import fs from "node:fs";

const bytes = fs.readFileSync("dist/web/evokups_web.wasm");
const { instance } = await WebAssembly.instantiate(bytes, {});
const api = instance.exports;
const cases = [
  { population: 32, beads: 256, chains: 32 },
  { population: 8, beads: 1000, chains: 100 },
  { population: 2, beads: 4000, chains: 1000 },
];
const rows = [];

for (const test of cases) {
  const code = api.evokups_initialize_advanced(
    42n,
    test.population,
    test.beads,
    0,
    1,
    test.chains,
    0.5,
    0,
    0.15,
    0.35,
    0.30,
    0.0,
    0.35,
    0.03,
    0.30,
    0.20,
    2,
    2,
    2,
    3,
  );
  if (code !== 0) throw new Error(`initialization failed with code ${code}`);

  const timings = [];
  for (let generation = 0; generation < 10; generation += 1) {
    const started = performance.now();
    if (api.evokups_advance() !== 0) throw new Error("generation failed");
    timings.push(performance.now() - started);
  }
  const diagnosticsStarted = performance.now();
  api.evokups_fitness_component(9);
  rows.push({
    ...test,
    generation_ms: Number(
      (timings.reduce((sum, value) => sum + value, 0) / timings.length).toFixed(2),
    ),
    best_diagnostics_ms: Number((performance.now() - diagnosticsStarted).toFixed(2)),
  });
}

console.table(rows);
