use evokups_core::{Evaluation, EvaluationError, Individual, PopulationEvaluator};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LennardJonesEvaluator {
    epsilon: f64,
    sigma: f64,
    minimum_distance: f64,
}

impl LennardJonesEvaluator {
    pub fn new(epsilon: f64, sigma: f64) -> Result<Self, EvaluationError> {
        if !epsilon.is_finite() || epsilon <= 0.0 {
            return Err(EvaluationError::InvalidIndividual(
                "epsilon must be positive and finite".into(),
            ));
        }
        if !sigma.is_finite() || sigma <= 0.0 {
            return Err(EvaluationError::InvalidIndividual(
                "sigma must be positive and finite".into(),
            ));
        }
        Ok(Self {
            epsilon,
            sigma,
            minimum_distance: 1.0e-9 * sigma,
        })
    }

    #[must_use]
    pub fn reduced_units() -> Self {
        Self {
            epsilon: 1.0,
            sigma: 1.0,
            minimum_distance: 1.0e-9,
        }
    }
}

impl Default for LennardJonesEvaluator {
    fn default() -> Self {
        Self::reduced_units()
    }
}

impl PopulationEvaluator for LennardJonesEvaluator {
    fn evaluate(&self, individual: &Individual) -> Result<Evaluation, EvaluationError> {
        if individual.coordinates.len() < 2 {
            return Err(EvaluationError::InvalidIndividual(
                "a Lennard-Jones system needs at least two atoms".into(),
            ));
        }

        let mut energy = 0.0;
        let mut forces = vec![[0.0; 3]; individual.coordinates.len()];
        for first in 0..individual.coordinates.len() {
            for second in (first + 1)..individual.coordinates.len() {
                let displacement = std::array::from_fn::<_, 3, _>(|axis| {
                    individual.coordinates[first][axis] - individual.coordinates[second][axis]
                });
                let distance_squared: f64 = displacement.iter().map(|value| value * value).sum();
                if distance_squared <= self.minimum_distance * self.minimum_distance {
                    return Err(EvaluationError::NumericalFailure(format!(
                        "atoms {first} and {second} overlap"
                    )));
                }

                let inverse_r2 = 1.0 / distance_squared;
                let reduced_distance_squared = self.sigma * self.sigma * inverse_r2;
                let attractive_term = reduced_distance_squared.powi(3);
                let repulsive_term = attractive_term * attractive_term;
                energy += 4.0 * self.epsilon * (repulsive_term - attractive_term);
                let scale =
                    24.0 * self.epsilon * (2.0 * repulsive_term - attractive_term) * inverse_r2;
                for axis in 0..3 {
                    let force = scale * displacement[axis];
                    forces[first][axis] += force;
                    forces[second][axis] -= force;
                }
            }
        }

        if !energy.is_finite() || forces.iter().flatten().any(|value| !value.is_finite()) {
            return Err(EvaluationError::NumericalFailure(
                "Lennard-Jones evaluation produced a non-finite result".into(),
            ));
        }
        Ok(Evaluation { energy, forces })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_atoms_at_the_minimum_have_expected_energy_and_force() {
        let minimum = 2.0_f64.powf(1.0 / 6.0);
        let individual = Individual::new(0, vec![[0.0, 0.0, 0.0], [minimum, 0.0, 0.0]]);
        let result = LennardJonesEvaluator::default()
            .evaluate(&individual)
            .unwrap();

        assert!((result.energy + 1.0).abs() < 1.0e-12);
        assert!(
            result
                .forces
                .iter()
                .flatten()
                .all(|force| force.abs() < 1.0e-12)
        );
    }
}
