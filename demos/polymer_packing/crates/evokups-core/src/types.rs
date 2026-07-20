use crate::SearchError;

#[derive(Debug, Clone, PartialEq)]
pub struct Individual {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub genetic_event: GeneticEvent,
    pub coordinates: Vec<[f64; 3]>,
    pub energy: Option<f64>,
}

impl Individual {
    #[must_use]
    pub fn new(id: u64, coordinates: Vec<[f64; 3]>) -> Self {
        Self {
            id,
            parent_id: None,
            genetic_event: GeneticEvent::initialized(),
            coordinates,
            energy: None,
        }
    }

    #[must_use]
    pub fn offspring(
        id: u64,
        parent_id: u64,
        genetic_event: GeneticEvent,
        coordinates: Vec<[f64; 3]>,
    ) -> Self {
        Self {
            id,
            parent_id: Some(parent_id),
            genetic_event,
            coordinates,
            energy: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneticOperation {
    Initialized,
    EliteClone,
    CartesianMutation,
    RigidChainTransform,
    CrankshaftRotation,
    DoubleBridge,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneticEvent {
    pub operation: GeneticOperation,
    pub target_gene: Option<usize>,
    pub magnitude: f64,
}

impl GeneticEvent {
    #[must_use]
    pub const fn initialized() -> Self {
        Self {
            operation: GeneticOperation::Initialized,
            target_gene: None,
            magnitude: 0.0,
        }
    }

    #[must_use]
    pub const fn elite_clone() -> Self {
        Self {
            operation: GeneticOperation::EliteClone,
            target_gene: None,
            magnitude: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Population {
    pub generation: u64,
    pub individuals: Vec<Individual>,
}

impl Population {
    pub fn validate(&self, expected_size: usize, atom_count: usize) -> Result<(), SearchError> {
        if self.individuals.len() != expected_size {
            return Err(SearchError::InvalidPopulation(format!(
                "expected {expected_size} individuals, found {}",
                self.individuals.len()
            )));
        }
        if let Some(individual) = self
            .individuals
            .iter()
            .find(|individual| individual.coordinates.len() != atom_count)
        {
            return Err(SearchError::InvalidPopulation(format!(
                "individual {} has {} atoms; expected {atom_count}",
                individual.id,
                individual.coordinates.len()
            )));
        }
        Ok(())
    }

    #[must_use]
    pub fn best(&self) -> Option<&Individual> {
        self.individuals
            .iter()
            .filter(|individual| individual.energy.is_some_and(f64::is_finite))
            .min_by(|left, right| {
                left.energy
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&right.energy.unwrap_or(f64::INFINITY))
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub energy: f64,
    pub forces: Vec<[f64; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionMode {
    SinglePoint,
    Lamarckian,
    Baldwinian,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelaxationConfig {
    pub steps: usize,
    pub step_size: f64,
    pub max_force_component: f64,
}

impl RelaxationConfig {
    pub fn validate(&self) -> Result<(), SearchError> {
        if self.steps == 0 {
            return Err(SearchError::InvalidConfig(
                "relaxation steps must be greater than zero".into(),
            ));
        }
        if !self.step_size.is_finite() || self.step_size <= 0.0 {
            return Err(SearchError::InvalidConfig(
                "relaxation step_size must be positive and finite".into(),
            ));
        }
        if !self.max_force_component.is_finite() || self.max_force_component <= 0.0 {
            return Err(SearchError::InvalidConfig(
                "max_force_component must be positive and finite".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelaxedIndividual {
    pub individual: Individual,
    pub evaluation: Evaluation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchConfig {
    pub population_size: usize,
    pub atom_count: usize,
    pub elite_count: usize,
    pub mutation_stddev: f64,
    pub initial_extent: f64,
    pub seed: u64,
    pub mode: EvolutionMode,
    pub relaxation: RelaxationConfig,
}

impl SearchConfig {
    pub fn validate(&self) -> Result<(), SearchError> {
        if self.population_size == 0 {
            return Err(SearchError::InvalidConfig(
                "population_size must be greater than zero".into(),
            ));
        }
        if self.atom_count < 2 {
            return Err(SearchError::InvalidConfig(
                "atom_count must be at least two".into(),
            ));
        }
        if self.elite_count == 0 || self.elite_count > self.population_size {
            return Err(SearchError::InvalidConfig(
                "elite_count must be between one and population_size".into(),
            ));
        }
        if !self.mutation_stddev.is_finite() || self.mutation_stddev <= 0.0 {
            return Err(SearchError::InvalidConfig(
                "mutation_stddev must be positive and finite".into(),
            ));
        }
        if !self.initial_extent.is_finite() || self.initial_extent <= 0.0 {
            return Err(SearchError::InvalidConfig(
                "initial_extent must be positive and finite".into(),
            ));
        }
        self.relaxation.validate()?;
        Ok(())
    }
}
