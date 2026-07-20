use crate::rng::DeterministicRng;
use crate::{
    CartesianGaussianMutation, Evaluation, EvolutionMode, GeneticEvent, Individual,
    MutationOperator, Population, PopulationEvaluator, PopulationInitializer, SearchConfig,
    SearchError, UniformBoxInitializer,
};

#[derive(Debug, Clone)]
pub struct EvolutionEngine {
    config: SearchConfig,
    rng: DeterministicRng,
    next_id: u64,
}

impl EvolutionEngine {
    pub fn new(config: SearchConfig) -> Result<Self, SearchError> {
        config.validate()?;
        Ok(Self {
            rng: DeterministicRng::new(config.seed),
            config,
            next_id: 0,
        })
    }

    #[must_use]
    pub fn config(&self) -> &SearchConfig {
        &self.config
    }

    pub fn initialize(&mut self) -> Result<Population, SearchError> {
        let initializer = UniformBoxInitializer {
            extent: self.config.initial_extent,
        };
        self.initialize_with(&initializer)
    }

    pub fn initialize_with<I: PopulationInitializer + ?Sized>(
        &mut self,
        initializer: &I,
    ) -> Result<Population, SearchError> {
        let mut individuals = Vec::with_capacity(self.config.population_size);
        for _ in 0..self.config.population_size {
            let coordinates = initializer.generate(self.config.atom_count, &mut self.rng)?;
            if coordinates.len() != self.config.atom_count {
                return Err(SearchError::InvalidPopulation(format!(
                    "initializer produced {} atoms; expected {}",
                    coordinates.len(),
                    self.config.atom_count
                )));
            }
            individuals.push(self.new_individual(coordinates));
        }
        Ok(Population {
            generation: 0,
            individuals,
        })
    }

    pub fn ask(&mut self, current: &Population) -> Result<Population, SearchError> {
        let mutation = CartesianGaussianMutation {
            standard_deviation: self.config.mutation_stddev,
        };
        self.ask_with(current, &mutation)
    }

    pub fn ask_with<M: MutationOperator + ?Sized>(
        &mut self,
        current: &Population,
        mutation: &M,
    ) -> Result<Population, SearchError> {
        current.validate(self.config.population_size, self.config.atom_count)?;
        let mut ranked: Vec<&Individual> = current.individuals.iter().collect();
        for individual in &ranked {
            if !individual.energy.is_some_and(f64::is_finite) {
                return Err(SearchError::MissingEvaluation {
                    individual_id: individual.id,
                });
            }
        }
        ranked.sort_by(|left, right| {
            left.energy
                .unwrap_or(f64::INFINITY)
                .total_cmp(&right.energy.unwrap_or(f64::INFINITY))
        });

        let elites: Vec<(u64, Vec<[f64; 3]>)> = ranked
            .iter()
            .take(self.config.elite_count)
            .map(|individual| (individual.id, individual.coordinates.clone()))
            .collect();
        let mut individuals = Vec::with_capacity(self.config.population_size);
        for (parent_id, coordinates) in &elites {
            individuals.push(self.new_offspring(
                *parent_id,
                GeneticEvent::elite_clone(),
                coordinates.clone(),
            ));
        }
        while individuals.len() < self.config.population_size {
            let parent_index = individuals.len() % elites.len();
            let (parent_id, parent_coordinates) = &elites[parent_index];
            let mut coordinates = parent_coordinates.clone();
            let event = mutation.mutate(&mut coordinates, &mut self.rng)?;
            individuals.push(self.new_offspring(*parent_id, event, coordinates));
        }

        Ok(Population {
            generation: current.generation + 1,
            individuals,
        })
    }

    pub fn tell(
        &self,
        mut population: Population,
        evaluations: Vec<Evaluation>,
    ) -> Result<Population, SearchError> {
        population.validate(self.config.population_size, self.config.atom_count)?;
        if evaluations.len() != population.individuals.len() {
            return Err(SearchError::InvalidPopulation(format!(
                "received {} evaluations for {} individuals",
                evaluations.len(),
                population.individuals.len()
            )));
        }
        for (individual, evaluation) in population.individuals.iter_mut().zip(evaluations) {
            if !evaluation.energy.is_finite() {
                return Err(SearchError::InvalidPopulation(format!(
                    "individual {} received a non-finite energy",
                    individual.id
                )));
            }
            if evaluation.forces.len() != self.config.atom_count {
                return Err(SearchError::InvalidPopulation(format!(
                    "individual {} received {} force vectors; expected {}",
                    individual.id,
                    evaluation.forces.len(),
                    self.config.atom_count
                )));
            }
            individual.energy = Some(evaluation.energy);
        }
        Ok(population)
    }

    pub fn evaluate<E: PopulationEvaluator + ?Sized>(
        &self,
        mut population: Population,
        evaluator: &E,
    ) -> Result<Population, SearchError> {
        let evaluations = match self.config.mode {
            EvolutionMode::SinglePoint => evaluator.evaluate_population(&population)?,
            EvolutionMode::Lamarckian => {
                let relaxed = evaluator.relax_population(&population, &self.config.relaxation)?;
                let mut evaluations = Vec::with_capacity(relaxed.len());
                for (individual, result) in population.individuals.iter_mut().zip(relaxed) {
                    *individual = result.individual;
                    evaluations.push(result.evaluation);
                }
                evaluations
            }
            EvolutionMode::Baldwinian => evaluator
                .relax_population(&population, &self.config.relaxation)?
                .into_iter()
                .map(|result| result.evaluation)
                .collect(),
        };
        self.tell(population, evaluations)
    }

    pub fn evolve_one<E: PopulationEvaluator + ?Sized>(
        &mut self,
        current: Population,
        evaluator: &E,
    ) -> Result<Population, SearchError> {
        let current = if current
            .individuals
            .iter()
            .all(|individual| individual.energy.is_some_and(f64::is_finite))
        {
            current
        } else {
            self.evaluate(current, evaluator)?
        };
        let candidates = self.ask(&current)?;
        self.evaluate(candidates, evaluator)
    }

    pub fn evolve_one_with<E: PopulationEvaluator + ?Sized, M: MutationOperator + ?Sized>(
        &mut self,
        current: Population,
        evaluator: &E,
        mutation: &M,
    ) -> Result<Population, SearchError> {
        let current = if current
            .individuals
            .iter()
            .all(|individual| individual.energy.is_some_and(f64::is_finite))
        {
            current
        } else {
            self.evaluate(current, evaluator)?
        };
        let candidates = self.ask_with(&current, mutation)?;
        self.evaluate(candidates, evaluator)
    }

    fn new_individual(&mut self, coordinates: Vec<[f64; 3]>) -> Individual {
        let individual = Individual::new(self.next_id, coordinates);
        self.next_id += 1;
        individual
    }

    fn new_offspring(
        &mut self,
        parent_id: u64,
        event: GeneticEvent,
        coordinates: Vec<[f64; 3]>,
    ) -> Individual {
        let individual = Individual::offspring(self.next_id, parent_id, event, coordinates);
        self.next_id += 1;
        individual
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvaluationError, PopulationEvaluator, RelaxationConfig};

    #[derive(Debug)]
    struct QuadraticEvaluator;

    impl PopulationEvaluator for QuadraticEvaluator {
        fn evaluate(&self, individual: &Individual) -> Result<Evaluation, EvaluationError> {
            let energy = individual
                .coordinates
                .iter()
                .flatten()
                .map(|value| value * value)
                .sum();
            Ok(Evaluation {
                energy,
                forces: individual
                    .coordinates
                    .iter()
                    .map(|coordinate| coordinate.map(|value| -2.0 * value))
                    .collect(),
            })
        }
    }

    fn config(seed: u64) -> SearchConfig {
        SearchConfig {
            population_size: 6,
            atom_count: 3,
            elite_count: 2,
            mutation_stddev: 0.1,
            initial_extent: 1.0,
            seed,
            mode: EvolutionMode::SinglePoint,
            relaxation: RelaxationConfig {
                steps: 4,
                step_size: 0.05,
                max_force_component: 10.0,
            },
        }
    }

    #[test]
    fn mutation_is_deterministic_for_a_seed() {
        let mut first = EvolutionEngine::new(config(42)).unwrap();
        let mut second = EvolutionEngine::new(config(42)).unwrap();
        let first_initial = first.initialize().unwrap();
        let second_initial = second.initialize().unwrap();
        let first_population = first.evaluate(first_initial, &QuadraticEvaluator).unwrap();
        let second_population = second
            .evaluate(second_initial, &QuadraticEvaluator)
            .unwrap();

        assert_eq!(first.ask(&first_population), second.ask(&second_population));
    }

    #[test]
    fn evolves_exactly_one_generation() {
        for mode in [
            EvolutionMode::SinglePoint,
            EvolutionMode::Lamarckian,
            EvolutionMode::Baldwinian,
        ] {
            let mut search_config = config(7);
            search_config.mode = mode;
            let mut engine = EvolutionEngine::new(search_config).unwrap();
            let initial = engine.initialize().unwrap();
            let evolved = engine.evolve_one(initial, &QuadraticEvaluator).unwrap();

            assert_eq!(evolved.generation, 1);
            assert_eq!(evolved.individuals.len(), 6);
            assert!(evolved.best().is_some());
            assert!(evolved.individuals.iter().all(|item| item.energy.is_some()));
        }
    }

    #[test]
    fn modes_control_genotype_inheritance() {
        let mut base_config = config(19);
        base_config.population_size = 2;
        base_config.elite_count = 1;

        let mut initial_engine = EvolutionEngine::new(base_config.clone()).unwrap();
        let initial = initial_engine.initialize().unwrap();

        base_config.mode = EvolutionMode::Lamarckian;
        let lamarckian = EvolutionEngine::new(base_config.clone())
            .unwrap()
            .evaluate(initial.clone(), &QuadraticEvaluator)
            .unwrap();

        base_config.mode = EvolutionMode::Baldwinian;
        let baldwinian = EvolutionEngine::new(base_config)
            .unwrap()
            .evaluate(initial.clone(), &QuadraticEvaluator)
            .unwrap();

        assert_ne!(
            lamarckian.individuals[0].coordinates,
            initial.individuals[0].coordinates
        );
        assert_eq!(
            baldwinian.individuals[0].coordinates,
            initial.individuals[0].coordinates
        );
        assert_eq!(
            lamarckian.individuals[0].energy,
            baldwinian.individuals[0].energy
        );
    }
}
