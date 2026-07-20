use crate::{GeneticEvent, GeneticOperation, RandomSource, SearchError};

pub trait PopulationInitializer {
    fn generate(
        &self,
        atom_count: usize,
        rng: &mut dyn RandomSource,
    ) -> Result<Vec<[f64; 3]>, SearchError>;
}

pub trait MutationOperator {
    fn mutate(
        &self,
        coordinates: &mut [[f64; 3]],
        rng: &mut dyn RandomSource,
    ) -> Result<GeneticEvent, SearchError>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UniformBoxInitializer {
    pub extent: f64,
}

impl PopulationInitializer for UniformBoxInitializer {
    fn generate(
        &self,
        atom_count: usize,
        rng: &mut dyn RandomSource,
    ) -> Result<Vec<[f64; 3]>, SearchError> {
        Ok((0..atom_count)
            .map(|_| std::array::from_fn(|_| (2.0 * rng.uniform() - 1.0) * self.extent))
            .collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianGaussianMutation {
    pub standard_deviation: f64,
}

impl MutationOperator for CartesianGaussianMutation {
    fn mutate(
        &self,
        coordinates: &mut [[f64; 3]],
        rng: &mut dyn RandomSource,
    ) -> Result<GeneticEvent, SearchError> {
        let mut squared_displacement = 0.0;
        for coordinate in coordinates {
            for component in coordinate {
                let displacement = self.standard_deviation * rng.normal();
                *component += displacement;
                squared_displacement += displacement * displacement;
            }
        }
        Ok(GeneticEvent {
            operation: GeneticOperation::CartesianMutation,
            target_gene: None,
            magnitude: squared_displacement.sqrt(),
        })
    }
}
