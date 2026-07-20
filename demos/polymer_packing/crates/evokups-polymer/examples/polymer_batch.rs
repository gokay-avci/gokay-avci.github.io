use evokups_core::{
    EvolutionEngine, EvolutionMode, GeneticOperation, RelaxationConfig, SearchConfig,
};
use evokups_polymer::{
    PolymerPackingEvaluator, PolymerPackingInitializer, PolymerPotential, PolymerTopology,
    RigidChainMutation,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = match std::env::args().nth(1).as_deref() {
        None | Some("lamarckian") => EvolutionMode::Lamarckian,
        Some("single-point") => EvolutionMode::SinglePoint,
        Some("baldwinian") => EvolutionMode::Baldwinian,
        Some(other) => return Err(format!("unknown mode: {other}").into()),
    };
    let topology = PolymerTopology::new(vec![4, 6, 8, 10], 4.8)?;
    let evaluator = PolymerPackingEvaluator::new(topology.clone(), PolymerPotential::prepacking())?;
    let initializer = PolymerPackingInitializer::new(topology.clone(), 1.0);
    let mutation = RigidChainMutation::new(topology.clone(), 0.4, 0.25);
    let mut engine = EvolutionEngine::new(SearchConfig {
        population_size: 32,
        atom_count: topology.atom_count(),
        elite_count: 4,
        mutation_stddev: 0.08,
        initial_extent: 1.0,
        seed: 42,
        mode,
        relaxation: RelaxationConfig {
            steps: 8,
            step_size: 0.002,
            max_force_component: 10.0,
        },
    })?;
    let initial = engine.initialize_with(&initializer)?;
    let mut population = engine.evaluate(initial, &evaluator)?;

    println!("batch_shape,32,{},3", topology.atom_count());
    println!("chain_lengths,4,6,8,10");
    println!("generation,best_energy,best_id,parent_id,operation,target_gene,magnitude");
    print_best(&population);
    for _ in 0..12 {
        population = engine.evolve_one_with(population, &evaluator, &mutation)?;
        print_best(&population);
    }
    Ok(())
}

fn print_best(population: &evokups_core::Population) {
    let best = population
        .best()
        .expect("evaluated population is non-empty");
    println!(
        "{},{:.8},{},{},{},{},{:.5}",
        population.generation,
        best.energy.unwrap_or(f64::NAN),
        best.id,
        best.parent_id
            .map_or_else(|| "none".into(), |id| id.to_string()),
        match best.genetic_event.operation {
            GeneticOperation::Initialized => "initialized",
            GeneticOperation::EliteClone => "elite-clone",
            GeneticOperation::CartesianMutation => "cartesian-mutation",
            GeneticOperation::RigidChainTransform => "rigid-chain-transform",
            GeneticOperation::CrankshaftRotation => "crankshaft-rotation",
            GeneticOperation::DoubleBridge => "double-bridge",
        },
        best.genetic_event
            .target_gene
            .map_or_else(|| "none".into(), |gene| gene.to_string()),
        best.genetic_event.magnitude,
    );
}
