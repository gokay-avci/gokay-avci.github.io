use crate::{
    Evaluation, EvaluationError, Individual, Population, RelaxationConfig, RelaxedIndividual,
};

pub trait PopulationEvaluator {
    fn evaluate(&self, individual: &Individual) -> Result<Evaluation, EvaluationError>;

    fn evaluate_population(
        &self,
        population: &Population,
    ) -> Result<Vec<Evaluation>, EvaluationError> {
        population
            .individuals
            .iter()
            .map(|individual| self.evaluate(individual))
            .collect()
    }

    fn relax_population(
        &self,
        population: &Population,
        config: &RelaxationConfig,
    ) -> Result<Vec<RelaxedIndividual>, EvaluationError> {
        population
            .individuals
            .iter()
            .map(|individual| {
                let mut relaxed = individual.clone();
                for _ in 0..config.steps {
                    let evaluation = self.evaluate(&relaxed)?;
                    for (coordinate, force) in relaxed.coordinates.iter_mut().zip(evaluation.forces)
                    {
                        for axis in 0..3 {
                            coordinate[axis] += config.step_size
                                * force[axis]
                                    .clamp(-config.max_force_component, config.max_force_component);
                        }
                    }
                }
                let evaluation = self.evaluate(&relaxed)?;
                Ok(RelaxedIndividual {
                    individual: relaxed,
                    evaluation,
                })
            })
            .collect()
    }
}
