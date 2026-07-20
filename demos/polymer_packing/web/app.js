import { minimumImageDistance, wrapPoint } from "./periodic-geometry.js?v=20260720d";

const status = document.querySelector("#status");
const advanceButton = document.querySelector("#advance");
const runButton = document.querySelector("#run-simulation");
const structureCanvas = document.querySelector("#structure-view");
const fitnessCanvas = document.querySelector("#fitness-view");
const missingId = 18446744073709551615n;
const operationNames = [
  "initialized",
  "elite clone",
  "Cartesian mutation",
  "rigid-chain transform",
  "crankshaft rotation",
  "double-bridge reconnect",
];
const architectureNames = ["linear", "ring", "dendritic", "bottlebrush"];
const architectureLegends = [
  [["#3ea6ff", "chain head"], ["#5fffd7", "chain tail"]],
  [["#ff5cb8", "cyclic contour"], ["#6ee7ff", "opposite contour"]],
  [["#ffe08a", "core"], ["#ff7a6e", "branch point"], ["#70c7ff", "spacer"], ["#72f2a4", "terminal"]],
  [["#58d5ff", "backbone"], ["#ffa347", "graft point"], ["#d988ff", "side chain"], ["#ff77b7", "terminal"]],
];

let api;
let structureViewer;
let remoteSessionId = null;
let remoteEnabled = false;
let searchReady = false;
let running = false;
let busy = false;
let lastGenerationMs = Number.NaN;
let viewState = {
  coordinates: [],
  chains: [],
  beadIndices: [],
  chainLengths: [],
  beadTypes: [],
  degrees: [],
  terminal: [],
  bonds: [],
  architecture: -1,
  targetGene: -1,
  selectedAtom: -1,
};

function fitCanvas(canvas) {
  const ratio = window.devicePixelRatio || 1;
  const width = Math.max(1, canvas.clientWidth);
  const height = Math.max(1, canvas.clientHeight);
  const pixelWidth = Math.round(width * ratio);
  const pixelHeight = Math.round(height * ratio);
  if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
    canvas.width = pixelWidth;
    canvas.height = pixelHeight;
  }
  const context = canvas.getContext("2d");
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  return { context, width, height };
}

function displayCoordinates() {
  const box = api ? api.evokups_box_length() : Number.NaN;
  const source = viewState.coordinates.map((coordinate) => [...coordinate]);
  if (Number.isFinite(box)) {
    return { points: source.map((point) => wrapPoint(point, box)), extent: box };
  }
  const center = [0, 1, 2].map(
    (axis) => source.reduce((sum, coordinate) => sum + coordinate[axis], 0) / source.length,
  );
  for (const coordinate of source) {
    for (let axis = 0; axis < 3; axis += 1) coordinate[axis] -= center[axis];
  }
  const extent = Math.max(2, ...source.flatMap((point) => point.map((value) => Math.abs(value) * 2)));
  return { points: source, extent };
}

function renderStructure() {
  if (!structureViewer || viewState.coordinates.length === 0) return;
  const { points, extent } = displayCoordinates();
  structureViewer.update({
    points,
    extent,
    chains: viewState.chains,
    beadIndices: viewState.beadIndices,
    chainLengths: viewState.chainLengths,
    beadTypes: viewState.beadTypes,
    degrees: viewState.degrees,
    architecture: viewState.architecture,
    bonds: viewState.bonds,
    targetGene: viewState.targetGene,
    showBonds: document.querySelector("#show-bonds").checked,
    showCell: document.querySelector("#show-cell").checked,
    showVoid: document.querySelector("#show-void").checked,
  });
}

function renderLegend() {
  const legend = document.querySelector("#chain-legend");
  const typeLegend = document.querySelector("#type-legend");
  const architecture = architectureNames[viewState.architecture] ?? "Lennard-Jones";
  const legendItems = (architectureLegends[viewState.architecture] ?? [["#63b3ff", "atom"]]).map(([color, label]) => {
    const item = document.createElement("span");
    const swatch = document.createElement("i");
    swatch.style.background = color;
    item.append(swatch, label);
    return item;
  });
  const summary = document.createElement("span");
  const morphology = viewState.architecture === 1
    ? `A/σ ${Number(document.querySelector("#ring-puckering").value).toFixed(2)}`
    : viewState.architecture === 2
      ? `${document.querySelector("#dendritic-branching").value} outward branches · spacer ${document.querySelector("#dendritic-spacer").value}`
      : viewState.architecture === 3
        ? `N_g ${document.querySelector("#graft-spacing").value} · N_sc ${document.querySelector("#side-chain-length").value}`
        : `cos Δθ ${Number(document.querySelector("#persistence").value).toFixed(2)}`;
  summary.textContent = `${architecture} · ${morphology} · ${new Set(viewState.chains.filter((chain) => chain >= 0)).size} chains`;
  legendItems.unshift(summary);
  legend.replaceChildren(...legendItems);
  const voidKey = document.createElement("span");
  const voidSwatch = document.createElement("i");
  voidSwatch.style.background = "linear-gradient(90deg, #34e7ff, #ff4fd8)";
  voidKey.append(voidSwatch, "void clearance: probe threshold → largest pocket");
  typeLegend.replaceChildren(voidKey);
}

function renderSelectedBead() {
  const output = document.querySelector("#selected-bead");
  const atom = viewState.selectedAtom;
  if (atom < 0 || atom >= viewState.coordinates.length) {
    output.textContent = "Select a particle to inspect its identity.";
    return;
  }
  if (viewState.chains[atom] < 0) {
    output.textContent = `Lennard-Jones atom ${atom} · ${viewState.coordinates[atom].map((value) => value.toFixed(3)).join(", ")}`;
    return;
  }
  const roleNames = ["backbone / contour", "terminal", "branch point", "spacer / side chain", "core"];
  const role = roleNames[viewState.beadTypes[atom]] ?? "coarse-grained site";
  output.textContent = `bead ${atom} · chain ${viewState.chains[atom]} · index ${viewState.beadIndices[atom] + 1}/${viewState.chainLengths[atom]} · degree ${viewState.degrees[atom]} · ${role}`;
}

function bondStatistics() {
  const box = api?.evokups_box_length();
  if (!Number.isFinite(box) || viewState.bonds.length === 0) return null;
  let maximum = 0;
  let stretched = 0;
  for (const [first, second] of viewState.bonds) {
    const length = minimumImageDistance(
      viewState.coordinates[first],
      viewState.coordinates[second],
      box,
    );
    maximum = Math.max(maximum, length);
    if (length > 1.5) stretched += 1;
  }
  return { maximum, stretched };
}

function renderBondDiagnostics() {
  const output = document.querySelector("#bond-diagnostics");
  const stats = bondStatistics();
  if (!stats) {
    output.textContent = "No polymer bonds in this problem.";
    return;
  }
  output.textContent = `minimum-image max bond ${stats.maximum.toFixed(3)}σ · ${stats.stretched} bond${stats.stretched === 1 ? "" : "s"} above 1.5σ${stats.stretched ? " (shown red: genuine strain after PBC correction)" : " (PBC crossings imaged normally)"}`;
}

function renderFitnessBreakdown() {
  const panel = document.querySelector("#fitness-breakdown");
  if (!api || viewState.chains.every((chain) => chain < 0)) {
    panel.textContent = "Lennard-Jones fitness is the total pair potential energy.";
    return;
  }
  const labels = [
    "bond strain, ε/bead",
    "nonbonded overlap, ε/bead",
    "angle strain, ε/bead",
    "dihedral score, ε/bead",
    "low-q S(q), ε/bead",
    "cell vacancy, ε/bead",
    "pressure target, ε/bead",
  ];
  const beadCount = Math.max(1, Number(api.evokups_atom_count()));
  const values = labels.map((_, index) => api.evokups_fitness_component(index) / beadCount);
  const scale = Math.max(1e-12, ...values.map((value) => Math.abs(value)));
  panel.replaceChildren(...labels.map((label, index) => {
    const row = document.createElement("div");
    const name = document.createElement("span");
    const track = document.createElement("i");
    const bar = document.createElement("b");
    const value = document.createElement("strong");
    name.textContent = label;
    bar.style.width = `${Math.max(1, 100 * Math.abs(values[index]) / scale)}%`;
    track.append(bar);
    value.textContent = values[index].toFixed(3);
    row.append(name, track, value);
    return row;
  }));
  const diagnostics = document.querySelector("#void-diagnostics");
  const target = Number(document.querySelector("#target-pressure").value);
  const pressure = api.evokups_fitness_component(7);
  diagnostics.textContent = `P* ${pressure.toFixed(3)} → target ${target.toFixed(3)} · ΔP* ${(pressure - target).toFixed(3)} · linked-cell vacancy ${(100 * api.evokups_fitness_component(8)).toFixed(1)}% · probe-accessible void ${(100 * api.evokups_fitness_component(9)).toFixed(1)}% · largest sampled clearance ${api.evokups_fitness_component(10).toFixed(2)}σ`;
}

function renderFitness() {
  const { context, width, height } = fitCanvas(fitnessCanvas);
  context.clearRect(0, 0, width, height);
  if (!api) return;
  const count = Number(api.evokups_fitness_history_len());
  const values = Array.from({ length: count }, (_, index) => api.evokups_fitness_history_value(index));
  if (values.length === 0) return;
  const padding = { left: 48, right: 18, top: 24, bottom: 36 };
  const plotWidth = width - padding.left - padding.right;
  const plotHeight = height - padding.top - padding.bottom;
  const minimum = Math.min(...values);
  const maximum = Math.max(...values);
  const span = Math.max(1e-9, maximum - minimum);
  const x = (index) => padding.left + (values.length === 1 ? 0 : (index / (values.length - 1)) * plotWidth);
  const y = (value) => padding.top + ((maximum - value) / span) * plotHeight;

  context.strokeStyle = "rgba(174, 187, 229, 0.2)";
  context.fillStyle = "#9dadcf";
  context.font = "12px system-ui";
  context.lineWidth = 1;
  for (let tick = 0; tick <= 4; tick += 1) {
    const value = maximum - (tick / 4) * span;
    const py = padding.top + (tick / 4) * plotHeight;
    context.beginPath();
    context.moveTo(padding.left, py);
    context.lineTo(width - padding.right, py);
    context.stroke();
    context.fillText(value.toFixed(2), 6, py + 4);
  }
  context.beginPath();
  values.forEach((value, index) => {
    if (index === 0) context.moveTo(x(index), y(value));
    else context.lineTo(x(index), y(value));
  });
  context.strokeStyle = "#76a7ff";
  context.lineWidth = 2.5;
  context.stroke();
  const last = values.length - 1;
  context.beginPath();
  context.arc(x(last), y(values[last]), 4, 0, Math.PI * 2);
  context.fillStyle = "#76a7ff";
  context.fill();
  context.fillStyle = "#9dadcf";
  context.fillText("generation", width - 78, height - 10);
}

function render() {
  const generation = api.evokups_generation();
  const energy = api.evokups_best_energy();
  document.querySelector("#generation").textContent = generation.toString();
  document.querySelector("#energy").textContent = energy.toFixed(8);
  document.querySelector("#batch-time").textContent = Number.isFinite(lastGenerationMs)
    ? `${lastGenerationMs.toFixed(1)} ms`
    : "—";
  document.querySelector("#pressure").textContent = Number.isFinite(api.evokups_fitness_component(7))
    ? api.evokups_fitness_component(7).toFixed(4)
    : "—";
  const density = api.evokups_reduced_density();
  const boxLength = api.evokups_box_length();
  document.querySelector("#density-cell").textContent = Number.isFinite(density)
    ? `${density.toFixed(3)} ρ* · ${boxLength.toFixed(3)}σ`
    : "—";
  const atomCount = Number(api.evokups_atom_count());
  viewState.coordinates = Array.from({ length: atomCount }, (_, atom) =>
    [0, 1, 2].map((axis) => api.evokups_best_coordinate(atom, axis)),
  );
  viewState.chains = Array.from({ length: atomCount }, (_, atom) => api.evokups_chain_index(atom));
  viewState.beadIndices = Array.from({ length: atomCount }, (_, atom) => api.evokups_bead_index(atom));
  viewState.chainLengths = Array.from({ length: atomCount }, (_, atom) => api.evokups_chain_length(atom));
  viewState.beadTypes = Array.from({ length: atomCount }, (_, atom) => api.evokups_bead_type(atom));
  viewState.degrees = Array.from({ length: atomCount }, (_, atom) => api.evokups_bead_degree(atom));
  viewState.terminal = Array.from({ length: atomCount }, (_, atom) => api.evokups_is_terminal_bead(atom) === 1);
  viewState.architecture = api.evokups_architecture();
  viewState.bonds = Array.from({ length: Number(api.evokups_bond_count()) }, (_, bond) => [
    api.evokups_bond_atom(bond, 0),
    api.evokups_bond_atom(bond, 1),
  ]);
  viewState.targetGene = api.evokups_best_target_gene();
  if (viewState.selectedAtom >= atomCount) viewState.selectedAtom = -1;

  const coordinateLimit = Math.min(viewState.coordinates.length, 256);
  const coordinates = viewState.coordinates.slice(0, coordinateLimit).map((coordinate, atom) => {
    const label = viewState.chains[atom] >= 0 ? `c${viewState.chains[atom].toString().padStart(2, "0")}:b${viewState.beadIndices[atom].toString().padStart(2, "0")}` : "atom";
    return `${String(atom).padStart(2, "0")}  ${label}  t${viewState.beadTypes[atom]}  ${coordinate.map((value) => value.toFixed(6)).join("  ")}`;
  });
  const coordinateFooter = viewState.coordinates.length > coordinateLimit
    ? [`… ${viewState.coordinates.length - coordinateLimit} additional beads omitted from this live preview`]
    : [];
  document.querySelector("#coordinates").textContent = [
    "bead chain:site type      x/σ       y/σ       z/σ",
    ...coordinates,
    ...coordinateFooter,
  ].join("\n");

  const bestId = api.evokups_best_id();
  const parentId = api.evokups_best_parent_id();
  const operation = operationNames[api.evokups_best_operation()] ?? "unknown operation";
  const gene = viewState.targetGene >= 0 ? `chain ${viewState.targetGene}` : "whole candidate";
  const magnitude = api.evokups_best_operation_magnitude();
  document.querySelector("#genetic-event").textContent =
    `candidate ${bestId} · parent ${parentId === missingId ? "none" : parentId} · ${operation} · ${gene} · magnitude ${magnitude.toFixed(3)}`;
  renderStructure();
  renderLegend();
  renderSelectedBead();
  renderBondDiagnostics();
  renderFitness();
  renderFitnessBreakdown();
}

async function load() {
  try {
    const { StructureViewer } = await import("./structure-view.js?v=20260720d");
    structureViewer = new StructureViewer(structureCanvas, {
      onSelect: (atom) => {
        viewState.selectedAtom = atom;
        renderSelectedBead();
      },
    });
    const paths = ["web/evokups_web.wasm?v=20260720d", "target/wasm32-unknown-unknown/release/evokups_web.wasm?v=20260720d"];
    let response;
    for (const path of paths) {
      response = await fetch(path);
      if (response.ok) break;
    }
    if (!response?.ok) throw new Error(`WASM module returned ${response?.status ?? "no response"}`);
    const bytes = await response.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    api = instance.exports;
    status.textContent = "Ready.";
  } catch (error) {
    status.textContent = `Could not load the WASM module: ${error.message}`;
  }
}

function backendUrl() {
  return document.querySelector("#backend-url").value.trim().replace(/\/$/, "");
}

async function requestJson(path, options = {}) {
  const response = await fetch(`${backendUrl()}${path}`, {
    ...options,
    headers: { "Content-Type": "application/json", ...(options.headers ?? {}) },
  });
  if (!response.ok) {
    let detail = `${response.status} ${response.statusText}`;
    try {
      const body = await response.json();
      detail = body.detail ?? detail;
    } catch (_) {
      // The status text is sufficient when a proxy returns a non-JSON error page.
    }
    throw new Error(detail);
  }
  return response.json();
}

function pendingPopulationPayload() {
  const populationSize = Number(api.evokups_population_size());
  const atomCount = Number(api.evokups_atom_count());
  const candidateIds = [];
  const parentIds = [];
  const operationCodes = [];
  const targetGenes = [];
  const operationMagnitudes = [];
  const positions = [];
  for (let candidate = 0; candidate < populationSize; candidate += 1) {
    const parent = api.evokups_candidate_parent_id(candidate);
    candidateIds.push(Number(api.evokups_candidate_id(candidate)));
    parentIds.push(parent === missingId ? null : Number(parent));
    operationCodes.push(api.evokups_candidate_operation(candidate));
    const targetGene = api.evokups_candidate_target_gene(candidate);
    targetGenes.push(targetGene < 0 ? null : targetGene);
    operationMagnitudes.push(api.evokups_candidate_operation_magnitude(candidate));
    positions.push(Array.from({ length: atomCount }, (_, atom) =>
      [0, 1, 2].map((axis) => api.evokups_candidate_coordinate(candidate, atom, axis)),
    ));
  }
  return {
    generation: Number(api.evokups_generation()),
    candidate_ids: candidateIds,
    parent_ids: parentIds,
    operation_codes: operationCodes,
    target_genes: targetGenes,
    operation_magnitudes: operationMagnitudes,
    positions,
  };
}

async function createRemoteSession(populationSize) {
  const chainCount = Number(api.evokups_chain_count());
  const payload = {
    chain_lengths: Array.from({ length: chainCount }, (_, chain) => Number(api.evokups_chain_size(chain))),
    box_length: api.evokups_box_length(),
    bead_types: Array.from(
      { length: Number(api.evokups_atom_count()) },
      (_, atom) => api.evokups_bead_type(atom),
    ),
    signature: {
      population_size: populationSize,
      atom_count: Number(api.evokups_atom_count()),
      dtype: "float32",
      neighbour_capacity: Math.max(4096, 24 * Number(api.evokups_atom_count())),
      relaxation_steps: 0,
      compute_forces: false,
    },
  };
  const result = await requestJson("/v1/sessions", {
    method: "POST",
    body: JSON.stringify(payload),
  });
  remoteSessionId = result.session_id;
  return result;
}

async function evaluatePendingPopulation() {
  if (!remoteSessionId) throw new Error("no hot evaluator session exists");
  const result = await requestJson(`/v1/sessions/${remoteSessionId}/evaluate`, {
    method: "POST",
    body: JSON.stringify(pendingPopulationPayload()),
  });
  for (let candidate = 0; candidate < result.energies.length; candidate += 1) {
    if (api.evokups_remote_set_energy(candidate, result.energies[candidate]) !== 0) {
      throw new Error(`could not attach energy for candidate ${candidate}`);
    }
    for (let atom = 0; atom < (result.forces?.[candidate]?.length ?? 0); atom += 1) {
      for (let axis = 0; axis < 3; axis += 1) {
        if (api.evokups_remote_set_force(candidate, atom, axis, result.forces[candidate][atom][axis]) !== 0) {
          throw new Error(`could not attach force for candidate ${candidate}, atom ${atom}`);
        }
      }
    }
  }
  const code = api.evokups_remote_commit();
  if (code !== 0) throw new Error(`Rust tell phase failed with code ${code}`);
  document.querySelector("#active-evaluator").textContent =
    `hot kUPS CG · compile ${result.compile_count} · batch ${result.evaluation_count}`;
}

function setBusy(value) {
  busy = value;
  updateActionButtons();
}

function updateActionButtons() {
  document.querySelector("#initialize").disabled = busy || running;
  advanceButton.disabled = busy || running || !searchReady;
  runButton.disabled = busy || !searchReady;
  runButton.textContent = running ? "Pause simulation" : "Start simulation";
  runButton.setAttribute("aria-pressed", running ? "true" : "false");
  const runState = document.querySelector("#run-state");
  if (busy) runState.textContent = "Working…";
  else if (running) runState.textContent = `Running · generation ${api?.evokups_generation?.() ?? 0}`;
  else if (searchReady) runState.textContent = `Paused · generation ${api?.evokups_generation?.() ?? 0}`;
  else runState.textContent = "Not initialized";
}

function pauseSimulation() {
  running = false;
  updateActionButtons();
}

async function initializeSearch() {
  if (!api) return;
  pauseSimulation();
  const seed = BigInt(document.querySelector("#seed").value);
  const population = Number(document.querySelector("#population").value);
  const atoms = Number(document.querySelector("#atoms").value);
  const chainCount = Number(document.querySelector("#chains").value);
  const dispersity = Number(document.querySelector("#dispersity").value);
  const architecture = Number(document.querySelector("#architecture").value);
  const persistence = Number(document.querySelector("#persistence").value);
  const ringPuckering = Number(document.querySelector("#ring-puckering").value);
  const dendriticBranching = Number(document.querySelector("#dendritic-branching").value);
  const dendriticSpacer = Number(document.querySelector("#dendritic-spacer").value);
  const graftSpacing = Number(document.querySelector("#graft-spacing").value);
  const sideChainLength = Number(document.querySelector("#side-chain-length").value);
  const reconnection = Number(document.querySelector("#reconnection").value);
  const mcTemperature = Number(document.querySelector("#mc-temperature").value);
  const targetDensity = Number(document.querySelector("#density").value);
  const targetPressure = Number(document.querySelector("#target-pressure").value);
  const systemTemperature = Number(document.querySelector("#system-temperature").value);
  const cellResponse = Number(document.querySelector("#cell-response").value);
  const modeElement = document.querySelector("#mode");
  const problemElement = document.querySelector("#problem");
  const mode = Number(modeElement.value);
  const problem = Number(problemElement.value);
  remoteEnabled = document.querySelector("#evaluator-source").value === "remote";
  searchReady = false;
  lastGenerationMs = Number.NaN;
  if (remoteEnabled && (mode !== 0 || problem !== 1 || architecture !== 0)) {
    status.textContent = "The hot kUPS coarse-grained kernel currently supports linear polymers in single-point mode. Use the built-in evaluator for graph architectures and all three evolutionary modes.";
    return;
  }
  if (problem === 1 && atoms < 4 * chainCount) {
    status.textContent = `Use at least ${4 * chainCount} beads for ${chainCount} chains.`;
    return;
  }
  setBusy(true);
  status.textContent = remoteEnabled
    ? `Connecting to the separately running kUPS process at ${backendUrl()}…`
    : "Initializing inside this browser…";
  const code = remoteEnabled
    ? api.evokups_initialize_remote_configured(seed, population, atoms, mode, problem, chainCount, dispersity)
    : api.evokups_initialize_advanced(
      seed,
      population,
      atoms,
      mode,
      problem,
      chainCount,
      dispersity,
      architecture,
      reconnection,
      mcTemperature,
      targetDensity,
      targetPressure,
      systemTemperature,
      cellResponse,
      persistence,
      ringPuckering,
      dendriticBranching,
      dendriticSpacer,
      graftSpacing,
      sideChainLength,
    );
  if (code !== 0) {
    status.textContent = `Initialization failed with code ${code}.`;
    setBusy(false);
    return;
  }
  try {
    if (remoteEnabled) {
      const session = await createRemoteSession(population);
      status.textContent = `Session ${session.session_id.slice(0, 8)} is hot; evaluating generation 0 as one batch…`;
      await evaluatePendingPopulation();
    } else {
      remoteSessionId = null;
      document.querySelector("#active-evaluator").textContent = "local Rust/WASM";
    }
    document.querySelector("#active-problem").textContent = problemElement.options[problemElement.selectedIndex].text;
    document.querySelector("#active-mode").textContent = modeElement.options[modeElement.selectedIndex].text;
    document.querySelector("#batch-shape").textContent = `${population} × ${api.evokups_atom_count()} × 3`;
    status.textContent = remoteEnabled ? "Population evaluated by the persistent batch service." : "Population initialized and evaluated locally.";
    searchReady = true;
    render();
    updateActionButtons();
  } catch (error) {
    searchReady = false;
    status.textContent = remoteEnabled
      ? `External kUPS is unavailable: ${error.message}. Start it with scripts/run-hot-kups.sh, then initialize again.`
      : `Initialization failed: ${error.message}`;
  } finally {
    setBusy(false);
  }
}

async function advanceOneGeneration(renderAfter = true) {
  const started = performance.now();
  if (remoteEnabled) {
    const askCode = api.evokups_remote_ask();
    if (askCode !== 0) throw new Error(`Rust ask phase failed with code ${askCode}`);
    await evaluatePendingPopulation();
  } else {
    const code = api.evokups_advance();
    if (code !== 0) throw new Error(`local evolution failed with code ${code}`);
  }
  lastGenerationMs = performance.now() - started;
  if (renderAfter) render();
}

document.querySelector("#initialize").addEventListener("click", initializeSearch);

advanceButton.addEventListener("click", async () => {
  setBusy(true);
  status.textContent = remoteEnabled ? "Sending one population batch to the hot evaluator…" : "Advancing one generation…";
  try {
    await advanceOneGeneration();
    status.textContent = `Advanced to generation ${api.evokups_generation()}.`;
  } catch (error) {
    status.textContent = `Evolution failed: ${error.message}`;
  } finally {
    setBusy(false);
  }
});

runButton.addEventListener("click", async () => {
  if (running) {
    pauseSimulation();
    status.textContent = `Paused at generation ${api.evokups_generation()}.`;
    return;
  }
  running = true;
  updateActionButtons();
  try {
    while (running) {
      const stride = Number(document.querySelector("#render-stride").value);
      const start = Number(api.evokups_generation()) + 1;
      const end = start + stride - 1;
      status.textContent = `Evaluating generations ${start}–${end}; rendering after the batch sequence…`;
      for (let step = 0; step < stride && running; step += 1) {
        await advanceOneGeneration(false);
        document.querySelector("#run-state").textContent = `Running · generation ${api.evokups_generation()}`;
        await new Promise((resolve) => setTimeout(resolve, 0));
      }
      if (running) render();
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
    status.textContent = `Paused at generation ${api.evokups_generation()}.`;
  } catch (error) {
    status.textContent = `Evolution failed: ${error.message}`;
  } finally {
    running = false;
    updateActionButtons();
  }
});

document.querySelector("#reset-view").addEventListener("click", () => {
  viewState.selectedAtom = -1;
  structureViewer?.reset();
  renderSelectedBead();
});
for (const control of ["show-bonds", "show-cell", "show-void"]) {
  document.querySelector(`#${control}`).addEventListener("change", renderStructure);
}
function updateProblemControls() {
  const polymer = document.querySelector("#problem").value === "1";
  const architecture = Number(document.querySelector("#architecture").value);
  document.querySelector("#chains").disabled = !polymer;
  document.querySelector("#dispersity").disabled = !polymer;
  document.querySelector("#architecture").disabled = !polymer;
  document.querySelector("#persistence").disabled = !polymer;
  document.querySelector("#reconnection").disabled = !polymer;
  document.querySelector("#mc-temperature").disabled = !polymer;
  document.querySelector("#density").disabled = !polymer;
  document.querySelector("#target-pressure").disabled = !polymer;
  document.querySelector("#system-temperature").disabled = !polymer;
  document.querySelector("#cell-response").disabled = !polymer || document.querySelector("#evaluator-source").value === "remote";
  document.querySelector("#show-void").disabled = !polymer;
  for (const control of document.querySelectorAll(".morphology-control")) {
    const architectures = control.dataset.architectures.split(",").map(Number);
    control.hidden = !polymer || !architectures.includes(architecture);
  }
}
document.querySelector("#problem").addEventListener("change", updateProblemControls);
document.querySelector("#evaluator-source").addEventListener("change", updateProblemControls);
document.querySelector("#architecture").addEventListener("change", updateProblemControls);
document.querySelector("#dispersity").addEventListener("input", (event) => {
  document.querySelector("#dispersity-value").value = Number(event.target.value).toFixed(2);
});
for (const [control, output] of [
  ["reconnection", "reconnection-value"],
  ["mc-temperature", "mc-temperature-value"],
  ["density", "density-value"],
  ["system-temperature", "system-temperature-value"],
  ["cell-response", "cell-response-value"],
  ["persistence", "persistence-value"],
  ["ring-puckering", "ring-puckering-value"],
]) {
  document.querySelector(`#${control}`).addEventListener("input", (event) => {
    document.querySelector(`#${output}`).value = Number(event.target.value).toFixed(2);
  });
}
for (const [control, output] of [
  ["dendritic-spacer", "dendritic-spacer-value"],
  ["graft-spacing", "graft-spacing-value"],
  ["side-chain-length", "side-chain-length-value"],
]) {
  document.querySelector(`#${control}`).addEventListener("input", (event) => {
    document.querySelector(`#${output}`).value = Number(event.target.value).toFixed(0);
  });
}
updateProblemControls();
updateActionButtons();
window.addEventListener("resize", () => {
  renderFitness();
});

load();
