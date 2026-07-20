use std::cell::RefCell;

use evokups_core::{
    CartesianGaussianMutation, Evaluation, EvolutionEngine, EvolutionMode, GeneticOperation,
    MutationOperator, Population, PopulationEvaluator, PopulationInitializer, RandomSource,
    RelaxationConfig, SearchConfig, SearchError,
};
use evokups_evaluators::LennardJonesEvaluator;
use evokups_polymer::{
    PolymerArchitecture, PolymerFitness, PolymerMorphology, PolymerMutation,
    PolymerPackingEvaluator, PolymerPackingInitializer, PolymerPotential, PolymerTopology,
};

struct AppState {
    engine: EvolutionEngine,
    population: Population,
    evaluator: Box<dyn PopulationEvaluator>,
    mutation: Box<dyn MutationOperator>,
    polymer_topology: Option<PolymerTopology>,
    polymer_potential: Option<PolymerPotential>,
    best_fitness: Option<(u64, PolymerFitness)>,
    fitness_history: Vec<f64>,
    reconnection_probability: f64,
    monte_carlo_temperature: f64,
    cell_response_rate: f64,
    remote: bool,
    pending_evaluations: Option<Vec<Evaluation>>,
}

type DemoSetup = (
    Population,
    Box<dyn PopulationEvaluator>,
    Box<dyn MutationOperator>,
);

fn create_demo_setup(
    engine: &mut EvolutionEngine,
    polymer_topology: Option<&PolymerTopology>,
    polymer_potential: Option<PolymerPotential>,
    reconnection_probability: f64,
    monte_carlo_temperature: f64,
    forward_cosine: f64,
    ring_puckering: f64,
) -> Result<DemoSetup, i32> {
    if let Some(topology) = polymer_topology {
        let initializer = PolymerPackingInitializer::new(topology.clone(), 1.0)
            .with_chain_geometry(forward_cosine, 6)
            .with_ring_puckering(ring_puckering);
        let population = engine.initialize_with(&initializer).map_err(|_| 6)?;
        let evaluator = PolymerPackingEvaluator::new(
            topology.clone(),
            polymer_potential.unwrap_or_else(PolymerPotential::prepacking),
        )
        .map_err(|_| 7)?;
        Ok((
            population,
            Box::new(evaluator),
            Box::new(
                PolymerMutation::new(topology.clone())
                    .with_reconnection(reconnection_probability, monte_carlo_temperature),
            ),
        ))
    } else {
        let population = engine.initialize_with(&LatticeInitializer).map_err(|_| 6)?;
        Ok((
            population,
            Box::new(LennardJonesEvaluator::default()),
            Box::new(CartesianGaussianMutation {
                standard_deviation: 0.08,
            }),
        ))
    }
}

#[derive(Debug, Clone, Copy)]
struct LatticeInitializer;

impl PopulationInitializer for LatticeInitializer {
    fn generate(
        &self,
        atom_count: usize,
        rng: &mut dyn RandomSource,
    ) -> Result<Vec<[f64; 3]>, SearchError> {
        let side = ceil_cuberoot(atom_count);
        let spacing = 1.18;
        let offset = 0.5 * spacing * side.saturating_sub(1) as f64;
        Ok((0..atom_count)
            .map(|atom| {
                let grid = [atom / side.pow(2), (atom / side) % side, atom % side];
                std::array::from_fn(|axis| {
                    spacing * grid[axis] as f64 - offset + 0.035 * rng.normal()
                })
            })
            .collect())
    }
}

thread_local! {
    static STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_initialize(
    seed: u64,
    population_size: usize,
    atom_count: usize,
    mode: u32,
    problem: u32,
) -> i32 {
    initialize_app(
        seed,
        population_size,
        atom_count,
        mode,
        problem,
        false,
        4,
        0.65,
        0,
        0.15,
        0.35,
        0.30,
        0.0,
        0.35,
        0.0,
        0.30,
        0.20,
        2,
        2,
        2,
        3,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_initialize_remote(
    seed: u64,
    population_size: usize,
    atom_count: usize,
    mode: u32,
    problem: u32,
) -> i32 {
    initialize_app(
        seed,
        population_size,
        atom_count,
        mode,
        problem,
        true,
        4,
        0.65,
        0,
        0.15,
        0.35,
        0.30,
        0.0,
        0.35,
        0.0,
        0.30,
        0.20,
        2,
        2,
        2,
        3,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_initialize_configured(
    seed: u64,
    population_size: usize,
    atom_count: usize,
    mode: u32,
    problem: u32,
    chain_count: usize,
    dispersity: f64,
) -> i32 {
    initialize_app(
        seed,
        population_size,
        atom_count,
        mode,
        problem,
        false,
        chain_count,
        dispersity,
        0,
        0.15,
        0.35,
        0.30,
        0.0,
        0.35,
        0.0,
        0.30,
        0.20,
        2,
        2,
        2,
        3,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_initialize_remote_configured(
    seed: u64,
    population_size: usize,
    atom_count: usize,
    mode: u32,
    problem: u32,
    chain_count: usize,
    dispersity: f64,
) -> i32 {
    initialize_app(
        seed,
        population_size,
        atom_count,
        mode,
        problem,
        true,
        chain_count,
        dispersity,
        0,
        0.15,
        0.35,
        0.30,
        0.0,
        0.35,
        0.0,
        0.30,
        0.20,
        2,
        2,
        2,
        3,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_initialize_advanced(
    seed: u64,
    population_size: usize,
    atom_count: usize,
    mode: u32,
    problem: u32,
    chain_count: usize,
    dispersity: f64,
    architecture: u32,
    reconnection_probability: f64,
    monte_carlo_temperature: f64,
    target_density: f64,
    target_reduced_pressure: f64,
    reduced_temperature: f64,
    cell_response_rate: f64,
    forward_cosine: f64,
    ring_puckering: f64,
    dendritic_branching: usize,
    dendritic_spacer: usize,
    bottlebrush_graft_spacing: usize,
    bottlebrush_side_chain: usize,
) -> i32 {
    initialize_app(
        seed,
        population_size,
        atom_count,
        mode,
        problem,
        false,
        chain_count,
        dispersity,
        architecture,
        reconnection_probability,
        monte_carlo_temperature,
        target_density,
        target_reduced_pressure,
        reduced_temperature,
        cell_response_rate,
        forward_cosine,
        ring_puckering,
        dendritic_branching,
        dendritic_spacer,
        bottlebrush_graft_spacing,
        bottlebrush_side_chain,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn initialize_app(
    seed: u64,
    population_size: usize,
    atom_count: usize,
    mode: u32,
    problem: u32,
    remote: bool,
    chain_count: usize,
    dispersity: f64,
    architecture: u32,
    reconnection_probability: f64,
    monte_carlo_temperature: f64,
    target_density: f64,
    target_reduced_pressure: f64,
    reduced_temperature: f64,
    cell_response_rate: f64,
    forward_cosine: f64,
    ring_puckering: f64,
    dendritic_branching: usize,
    dendritic_spacer: usize,
    bottlebrush_graft_spacing: usize,
    bottlebrush_side_chain: usize,
) -> i32 {
    let mode = match mode {
        0 => EvolutionMode::SinglePoint,
        1 => EvolutionMode::Lamarckian,
        2 => EvolutionMode::Baldwinian,
        _ => return 3,
    };
    if remote && mode != EvolutionMode::SinglePoint {
        return 9;
    }
    if remote && problem != 1 {
        return 10;
    }
    let polymer_topology = match problem {
        0 => None,
        1 => match build_polymer_topology(
            atom_count,
            chain_count,
            dispersity,
            architecture,
            target_density,
            PolymerMorphology {
                dendritic_branching,
                dendritic_spacer,
                bottlebrush_graft_spacing,
                bottlebrush_side_chain,
            },
        ) {
            Ok(topology) => Some(topology),
            Err(_) => return 4,
        },
        _ => return 5,
    };
    let atom_count = polymer_topology
        .as_ref()
        .map_or(atom_count, PolymerTopology::atom_count);
    let config = SearchConfig {
        population_size,
        atom_count,
        elite_count: (population_size / 8).max(1),
        mutation_stddev: 0.08,
        initial_extent: 1.5,
        seed,
        mode,
        relaxation: RelaxationConfig {
            steps: 8,
            step_size: 0.002,
            max_force_component: 10.0,
        },
    };
    let Ok(mut engine) = EvolutionEngine::new(config) else {
        return 1;
    };

    let polymer_potential = polymer_topology.as_ref().map(|_| PolymerPotential {
        target_reduced_pressure,
        reduced_temperature: reduced_temperature.max(0.0),
        ..PolymerPotential::prepacking()
    });
    let setup = create_demo_setup(
        &mut engine,
        polymer_topology.as_ref(),
        polymer_potential,
        reconnection_probability,
        monte_carlo_temperature,
        forward_cosine,
        ring_puckering,
    );
    let Ok((initial, evaluator, mutation)) = setup else {
        return setup.err().unwrap_or(8);
    };
    let (population, fitness_history, pending_evaluations) = if remote {
        let pending = blank_evaluations(population_size, atom_count);
        (initial, Vec::new(), Some(pending))
    } else {
        let Ok(population) = engine.evaluate(initial, evaluator.as_ref()) else {
            return 2;
        };
        let best = population
            .best()
            .and_then(|individual| individual.energy)
            .unwrap_or(f64::NAN);
        (population, vec![best], None)
    };
    STATE.with_borrow_mut(|state| {
        *state = Some(AppState {
            engine,
            population,
            evaluator,
            mutation,
            polymer_topology,
            polymer_potential,
            best_fitness: None,
            fitness_history,
            reconnection_probability,
            monte_carlo_temperature,
            cell_response_rate: if remote {
                0.0
            } else {
                cell_response_rate.clamp(0.0, 0.12)
            },
            remote,
            pending_evaluations,
        });
    });
    0
}

fn build_polymer_topology(
    atom_count: usize,
    chain_count: usize,
    dispersity: f64,
    architecture: u32,
    target_density: f64,
    morphology: PolymerMorphology,
) -> Result<PolymerTopology, evokups_core::EvaluationError> {
    if chain_count == 0
        || atom_count < 4 * chain_count
        || !dispersity.is_finite()
        || !target_density.is_finite()
        || target_density <= 0.0
    {
        return Err(evokups_core::EvaluationError::InvalidIndividual(
            "polymer systems require at least four beads per chain".into(),
        ));
    }
    let spread = dispersity.clamp(0.0, 1.5);
    let weights: Vec<f64> = (0..chain_count)
        .map(|chain| {
            let coordinate = if chain_count == 1 {
                0.0
            } else {
                2.0 * chain as f64 / (chain_count - 1) as f64 - 1.0
            };
            (spread * coordinate).exp()
        })
        .collect();
    let remaining = atom_count - 4 * chain_count;
    let weight_sum = weights.iter().sum::<f64>();
    let shares: Vec<f64> = weights
        .iter()
        .map(|weight| remaining as f64 * weight / weight_sum)
        .collect();
    let mut lengths: Vec<usize> = shares
        .iter()
        .map(|share| 4 + floor_to_usize(*share))
        .collect();
    let assigned = lengths.iter().sum::<usize>();
    let mut order: Vec<usize> = (0..chain_count).collect();
    order.sort_by(|left, right| shares[*right].fract().total_cmp(&shares[*left].fract()));
    for chain in order.into_iter().take(atom_count - assigned) {
        lengths[chain] += 1;
    }
    let box_length = (atom_count as f64 / target_density).cbrt().max(4.0);
    let architecture = PolymerArchitecture::from_code(architecture).ok_or_else(|| {
        evokups_core::EvaluationError::InvalidIndividual("unknown polymer architecture".into())
    })?;
    PolymerTopology::with_morphology(lengths, box_length, architecture, morphology)
        .and_then(with_demo_bead_types)
}

fn ceil_cuberoot(value: usize) -> usize {
    let mut root = 1_usize;
    while root.saturating_pow(3) < value {
        root += 1;
    }
    root
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn floor_to_usize(value: f64) -> usize {
    value.floor() as usize
}

fn with_demo_bead_types(
    topology: PolymerTopology,
) -> Result<PolymerTopology, evokups_core::EvaluationError> {
    let types = (0..topology.atom_count())
        .map(|atom| topology.morphology_role(atom).unwrap_or(0))
        .collect();
    topology.with_bead_types(types)
}

fn adapt_shared_cell(app: &mut AppState) -> Result<(), ()> {
    if app.cell_response_rate == 0.0 {
        return Ok(());
    }
    let topology = app.polymer_topology.clone().ok_or(())?;
    let potential = app.polymer_potential.ok_or(())?;
    let best = app.population.best().ok_or(())?;
    let pressure = PolymerPackingEvaluator::new(topology.clone(), potential)
        .map_err(|_| ())?
        .reduced_pressure(&best.coordinates)
        .map_err(|_| ())?;
    let pressure_error = pressure - potential.target_reduced_pressure;
    let delta_log_length = (app.cell_response_rate * pressure_error / 3.0).clamp(-0.0125, 0.0125);
    if delta_log_length.abs() < 1.0e-10 {
        return Ok(());
    }
    let scale = delta_log_length.exp();
    for individual in &mut app.population.individuals {
        for coordinate in &mut individual.coordinates {
            for component in coordinate {
                *component *= scale;
            }
        }
    }
    let new_topology = topology
        .with_box_length(topology.box_length() * scale)
        .map_err(|_| ())?;
    app.evaluator =
        Box::new(PolymerPackingEvaluator::new(new_topology.clone(), potential).map_err(|_| ())?);
    app.mutation = Box::new(
        PolymerMutation::new(new_topology.clone())
            .with_reconnection(app.reconnection_probability, app.monte_carlo_temperature),
    );
    app.polymer_topology = Some(new_topology);
    app.best_fitness = None;
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_advance() -> i32 {
    STATE.with_borrow_mut(|state| {
        let Some(app) = state.as_mut() else {
            return 1;
        };
        if app.remote {
            return 3;
        }
        if adapt_shared_cell(app).is_err() {
            return 4;
        }
        match app.engine.evolve_one_with(
            app.population.clone(),
            app.evaluator.as_ref(),
            app.mutation.as_ref(),
        ) {
            Ok(population) => {
                app.population = population;
                app.best_fitness = None;
                app.fitness_history.push(
                    app.population
                        .best()
                        .and_then(|individual| individual.energy)
                        .unwrap_or(f64::NAN),
                );
                0
            }
            Err(_) => 2,
        }
    })
}

fn blank_evaluations(population_size: usize, atom_count: usize) -> Vec<Evaluation> {
    (0..population_size)
        .map(|_| Evaluation {
            energy: f64::NAN,
            forces: vec![[0.0; 3]; atom_count],
        })
        .collect()
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_remote_ask() -> i32 {
    STATE.with_borrow_mut(|state| {
        let Some(app) = state.as_mut() else {
            return 1;
        };
        if !app.remote || app.pending_evaluations.is_some() {
            return 2;
        }
        let Ok(candidates) = app.engine.ask_with(&app.population, app.mutation.as_ref()) else {
            return 3;
        };
        app.pending_evaluations = Some(blank_evaluations(
            candidates.individuals.len(),
            app.engine.config().atom_count,
        ));
        app.population = candidates;
        app.best_fitness = None;
        0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_remote_set_energy(candidate: usize, energy: f64) -> i32 {
    STATE.with_borrow_mut(|state| {
        let Some(evaluation) = state
            .as_mut()
            .filter(|app| app.remote)
            .and_then(|app| app.pending_evaluations.as_mut())
            .and_then(|evaluations| evaluations.get_mut(candidate))
        else {
            return 1;
        };
        if !energy.is_finite() {
            return 2;
        }
        evaluation.energy = energy;
        0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_remote_set_force(
    candidate: usize,
    atom: usize,
    axis: usize,
    force: f64,
) -> i32 {
    STATE.with_borrow_mut(|state| {
        let Some(component) = state
            .as_mut()
            .filter(|app| app.remote)
            .and_then(|app| app.pending_evaluations.as_mut())
            .and_then(|evaluations| evaluations.get_mut(candidate))
            .and_then(|evaluation| evaluation.forces.get_mut(atom))
            .and_then(|vector| vector.get_mut(axis))
        else {
            return 1;
        };
        if !force.is_finite() {
            return 2;
        }
        *component = force;
        0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_remote_commit() -> i32 {
    STATE.with_borrow_mut(|state| {
        let Some(app) = state.as_mut() else {
            return 1;
        };
        if !app.remote {
            return 2;
        }
        let Some(evaluations) = app.pending_evaluations.take() else {
            return 3;
        };
        if evaluations
            .iter()
            .any(|evaluation| !evaluation.energy.is_finite())
        {
            app.pending_evaluations = Some(evaluations);
            return 4;
        }
        let Ok(population) = app.engine.tell(app.population.clone(), evaluations) else {
            return 5;
        };
        app.population = population;
        app.best_fitness = None;
        app.fitness_history.push(
            app.population
                .best()
                .and_then(|individual| individual.energy)
                .unwrap_or(f64::NAN),
        );
        0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_population_size() -> usize {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .map_or(0, |app| app.population.individuals.len())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_candidate_id(candidate: usize) -> u64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.individuals.get(candidate))
            .map_or(u64::MAX, |individual| individual.id)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_candidate_parent_id(candidate: usize) -> u64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.individuals.get(candidate))
            .and_then(|individual| individual.parent_id)
            .unwrap_or(u64::MAX)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_candidate_operation(candidate: usize) -> u32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.individuals.get(candidate))
            .map_or(0, |individual| {
                operation_code(individual.genetic_event.operation)
            })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_candidate_target_gene(candidate: usize) -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.individuals.get(candidate))
            .and_then(|individual| individual.genetic_event.target_gene)
            .and_then(|gene| i32::try_from(gene).ok())
            .unwrap_or(-1)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_candidate_operation_magnitude(candidate: usize) -> f64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.individuals.get(candidate))
            .map_or(f64::NAN, |individual| individual.genetic_event.magnitude)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_candidate_coordinate(candidate: usize, atom: usize, axis: usize) -> f64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.individuals.get(candidate))
            .and_then(|individual| individual.coordinates.get(atom))
            .and_then(|coordinate| coordinate.get(axis))
            .copied()
            .unwrap_or(f64::NAN)
    })
}

const fn operation_code(operation: GeneticOperation) -> u32 {
    match operation {
        GeneticOperation::Initialized => 0,
        GeneticOperation::EliteClone => 1,
        GeneticOperation::CartesianMutation => 2,
        GeneticOperation::RigidChainTransform => 3,
        GeneticOperation::CrankshaftRotation => 4,
        GeneticOperation::DoubleBridge => 5,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_best_id() -> u64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.best())
            .map_or(u64::MAX, |individual| individual.id)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_best_parent_id() -> u64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.best())
            .and_then(|individual| individual.parent_id)
            .unwrap_or(u64::MAX)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_best_operation() -> u32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.best())
            .map_or(0, |individual| {
                operation_code(individual.genetic_event.operation)
            })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_best_target_gene() -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.best())
            .and_then(|individual| individual.genetic_event.target_gene)
            .and_then(|gene| i32::try_from(gene).ok())
            .unwrap_or(-1)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_best_operation_magnitude() -> f64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.best())
            .map_or(f64::NAN, |individual| individual.genetic_event.magnitude)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_fitness_history_len() -> usize {
    STATE.with_borrow(|state| state.as_ref().map_or(0, |app| app.fitness_history.len()))
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_fitness_history_value(index: usize) -> f64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.fitness_history.get(index))
            .copied()
            .unwrap_or(f64::NAN)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_box_length() -> f64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .map_or(f64::NAN, PolymerTopology::box_length)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_reduced_density() -> f64 {
    STATE.with_borrow(|state| {
        let Some(app) = state.as_ref() else {
            return f64::NAN;
        };
        app.polymer_topology.as_ref().map_or(f64::NAN, |topology| {
            app.engine.config().atom_count as f64 / topology.box_length().powi(3)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_generation() -> u64 {
    STATE.with_borrow(|state| state.as_ref().map_or(0, |app| app.population.generation))
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_best_energy() -> f64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.best())
            .and_then(|individual| individual.energy)
            .unwrap_or(f64::NAN)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_atom_count() -> usize {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .map_or(0, |app| app.engine.config().atom_count)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_chain_count() -> usize {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .map_or(0, PolymerTopology::chain_count)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_chain_size(chain: usize) -> usize {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .and_then(|topology| topology.chain_lengths().get(chain))
            .copied()
            .unwrap_or(0)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_bond_count() -> usize {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .map_or(0, |topology| topology.bonds().len())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_bond_atom(bond: usize, endpoint: usize) -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .and_then(|topology| topology.bonds().get(bond))
            .and_then(|&(first, second)| [first, second].get(endpoint).copied())
            .and_then(|atom| i32::try_from(atom).ok())
            .unwrap_or(-1)
    })
}

fn cached_polymer_fitness(app: &mut AppState) -> Option<PolymerFitness> {
    let (best_id, coordinates) = {
        let best = app.population.best()?;
        (best.id, best.coordinates.clone())
    };
    if let Some((id, fitness)) = app.best_fitness
        && id == best_id
    {
        return Some(fitness);
    }
    let evaluator =
        PolymerPackingEvaluator::new(app.polymer_topology.clone()?, app.polymer_potential?).ok()?;
    let fitness = evaluator.fitness(&coordinates).ok()?;
    app.best_fitness = Some((best_id, fitness));
    Some(fitness)
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_fitness_component(component: u32) -> f64 {
    STATE.with_borrow_mut(|state| {
        let Some(fitness) = state.as_mut().and_then(cached_polymer_fitness) else {
            return f64::NAN;
        };
        match component {
            0 => fitness.bond,
            1 => fitness.overlap,
            2 => fitness.angle,
            3 => fitness.dihedral,
            4 => fitness.density,
            5 => fitness.void,
            6 => fitness.pressure,
            7 => fitness.reduced_pressure,
            8 => fitness.void_fraction,
            9 => fitness.probe_void_fraction,
            10 => fitness.largest_void_radius,
            _ => f64::NAN,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_chain_index(atom: usize) -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .and_then(|topology| topology.chain_of(atom))
            .and_then(|chain| i32::try_from(chain).ok())
            .unwrap_or(-1)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_bead_index(atom: usize) -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .and_then(|topology| topology.bead_within_chain(atom))
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_chain_length(atom: usize) -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .and_then(|topology| topology.chain_length_of(atom))
            .and_then(|length| i32::try_from(length).ok())
            .unwrap_or(-1)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_bead_type(atom: usize) -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .and_then(|topology| topology.bead_type(atom))
            .map_or(-1, i32::from)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_bead_degree(atom: usize) -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .and_then(|topology| topology.degree(atom))
            .and_then(|degree| i32::try_from(degree).ok())
            .unwrap_or(-1)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_architecture() -> i32 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.polymer_topology.as_ref())
            .map_or(-1, |topology| match topology.architecture() {
                PolymerArchitecture::Linear => 0,
                PolymerArchitecture::Ring => 1,
                PolymerArchitecture::Dendritic => 2,
                PolymerArchitecture::Bottlebrush => 3,
            })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_is_terminal_bead(atom: usize) -> u32 {
    STATE.with_borrow(|state| {
        u32::from(
            state
                .as_ref()
                .and_then(|app| app.polymer_topology.as_ref())
                .is_some_and(|topology| topology.is_terminal(atom)),
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn evokups_best_coordinate(atom: usize, axis: usize) -> f64 {
    STATE.with_borrow(|state| {
        state
            .as_ref()
            .and_then(|app| app.population.best())
            .and_then(|individual| individual.coordinates.get(atom))
            .and_then(|coordinate| coordinate.get(axis))
            .copied()
            .unwrap_or(f64::NAN)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_pressure_error_expands_the_shared_cell() {
        assert_eq!(
            evokups_initialize_advanced(
                42, 8, 32, 0, 1, 4, 0.4, 0, 0.0, 0.35, 0.30, -100.0, 0.35, 0.12, 0.30, 0.20, 2, 2,
                2, 3,
            ),
            0
        );
        let initial = evokups_box_length();
        assert_eq!(evokups_advance(), 0);
        assert!(evokups_box_length() > initial);
        assert!(evokups_reduced_density() < 0.30);
    }

    #[test]
    fn negative_pressure_error_contracts_the_shared_cell() {
        assert_eq!(
            evokups_initialize_advanced(
                42, 8, 32, 0, 1, 4, 0.4, 0, 0.0, 0.35, 0.30, 100.0, 0.35, 0.12, 0.30, 0.20, 2, 2,
                2, 3,
            ),
            0
        );
        let initial = evokups_box_length();
        assert_eq!(evokups_advance(), 0);
        assert!(evokups_box_length() < initial);
        assert!(evokups_reduced_density() > 0.30);
    }
}
