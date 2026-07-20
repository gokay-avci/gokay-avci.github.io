use evokups_core::{GeneticEvent, GeneticOperation, MutationOperator, RandomSource, SearchError};

use crate::PolymerTopology;

#[derive(Debug, Clone)]
pub struct PolymerMutation {
    rigid: RigidChainMutation,
    topology: PolymerTopology,
    crankshaft_stddev: f64,
    reconnection_probability: f64,
    monte_carlo_temperature: f64,
}

impl PolymerMutation {
    #[must_use]
    pub fn new(topology: PolymerTopology) -> Self {
        Self {
            rigid: RigidChainMutation::new(topology.clone(), 0.45, 0.30),
            topology,
            crankshaft_stddev: 0.65,
            reconnection_probability: 0.15,
            monte_carlo_temperature: 0.35,
        }
    }

    #[must_use]
    pub fn with_reconnection(mut self, probability: f64, temperature: f64) -> Self {
        self.reconnection_probability = probability.clamp(0.0, 0.99);
        self.monte_carlo_temperature = temperature.max(1.0e-8);
        self
    }

    fn crankshaft(
        &self,
        coordinates: &mut [[f64; 3]],
        rng: &mut dyn RandomSource,
    ) -> Result<GeneticEvent, SearchError> {
        let eligible: Vec<usize> = self
            .topology
            .chain_lengths()
            .iter()
            .enumerate()
            .filter_map(|(chain, length)| (*length >= 4).then_some(chain))
            .collect();
        if eligible.is_empty() {
            return self.rigid.mutate(coordinates, rng);
        }
        let chain = eligible[rng.index(eligible.len())];
        let range = self.topology.chain_ranges()[chain].clone();
        let span = range.len();
        let left_offset = rng.index(span - 2);
        let right_offset = left_offset + 2 + rng.index(span - left_offset - 2);
        let left = range.start + left_offset;
        let right = range.start + right_offset;
        let axis_vector = subtract(coordinates[right], coordinates[left]);
        let axis_norm = axis_vector
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        if axis_norm < 1.0e-12 {
            return self.rigid.mutate(coordinates, rng);
        }
        let angle = self.crankshaft_stddev * rng.normal();
        let rotation = axis_vector.map(|value| value * angle / axis_norm);
        for atom in (left + 1)..right {
            let relative = subtract(coordinates[atom], coordinates[left]);
            let rotated = rotate_by_vector(relative, rotation);
            coordinates[atom] = std::array::from_fn(|axis| coordinates[left][axis] + rotated[axis]);
        }
        Ok(GeneticEvent {
            operation: GeneticOperation::CrankshaftRotation,
            target_gene: Some(chain),
            magnitude: angle.abs(),
        })
    }

    fn double_bridge(
        &self,
        coordinates: &mut [[f64; 3]],
        rng: &mut dyn RandomSource,
    ) -> Result<GeneticEvent, SearchError> {
        if self.topology.chain_count() < 2 {
            return self.crankshaft(coordinates, rng);
        }
        let first_chain = rng.index(self.topology.chain_count());
        let mut second_chain = rng.index(self.topology.chain_count() - 1);
        if second_chain >= first_chain {
            second_chain += 1;
        }
        let first = self.topology.chain_ranges()[first_chain].clone();
        let second = self.topology.chain_ranges()[second_chain].clone();
        let tail_length = first.len().min(second.len()).saturating_sub(1);
        if tail_length == 0 {
            return self.rigid.mutate(coordinates, rng);
        }
        let swapped = 1 + rng.index(tail_length);
        let first_cut = first.end - swapped;
        let second_cut = second.end - swapped;
        let affected_bonds = self
            .topology
            .chain_bonds(first_chain)
            .into_iter()
            .flatten()
            .chain(
                self.topology
                    .chain_bonds(second_chain)
                    .into_iter()
                    .flatten(),
            );
        let (old_energy, new_energy) = affected_bonds.fold((0.0, 0.0), |energies, &bond| {
            let mapped =
                |atom| mapped_coordinate(coordinates, atom, first_cut, second_cut, swapped);
            (
                energies.0
                    + bond_deviation_energy(
                        coordinates[bond.0],
                        coordinates[bond.1],
                        self.topology.box_length(),
                    ),
                energies.1
                    + bond_deviation_energy(
                        mapped(bond.0),
                        mapped(bond.1),
                        self.topology.box_length(),
                    ),
            )
        });
        let delta = new_energy - old_energy;
        let acceptance = (-delta / self.monte_carlo_temperature).exp().min(1.0);
        let accepted = delta <= 0.0 || rng.uniform() < acceptance;
        if accepted {
            for offset in 0..swapped {
                coordinates.swap(first_cut + offset, second_cut + offset);
            }
        }
        Ok(GeneticEvent {
            operation: GeneticOperation::DoubleBridge,
            target_gene: Some(first_chain),
            magnitude: if accepted {
                swapped as f64
            } else {
                -(swapped as f64)
            },
        })
    }
}

fn mapped_coordinate(
    coordinates: &[[f64; 3]],
    atom: usize,
    first_cut: usize,
    second_cut: usize,
    swapped: usize,
) -> [f64; 3] {
    if (first_cut..first_cut + swapped).contains(&atom) {
        coordinates[second_cut + atom - first_cut]
    } else if (second_cut..second_cut + swapped).contains(&atom) {
        coordinates[first_cut + atom - second_cut]
    } else {
        coordinates[atom]
    }
}

fn bond_deviation_energy(left: [f64; 3], right: [f64; 3], box_length: f64) -> f64 {
    let distance = periodic_distance_squared(left, right, box_length).sqrt();
    25.0 * (distance - 1.0).powi(2)
}

fn periodic_distance_squared(left: [f64; 3], right: [f64; 3], box_length: f64) -> f64 {
    (0..3)
        .map(|axis| {
            let mut displacement = left[axis] - right[axis];
            displacement -= box_length * (displacement / box_length).round();
            displacement * displacement
        })
        .sum()
}

impl MutationOperator for PolymerMutation {
    fn mutate(
        &self,
        coordinates: &mut [[f64; 3]],
        rng: &mut dyn RandomSource,
    ) -> Result<GeneticEvent, SearchError> {
        let draw = rng.uniform();
        if draw < self.reconnection_probability {
            self.double_bridge(coordinates, rng)
        } else if draw < self.reconnection_probability + 0.50 {
            self.rigid.mutate(coordinates, rng)
        } else {
            self.crankshaft(coordinates, rng)
        }
    }
}

#[derive(Debug, Clone)]
pub struct RigidChainMutation {
    topology: PolymerTopology,
    translation_stddev: f64,
    rotation_stddev: f64,
}

impl RigidChainMutation {
    #[must_use]
    pub fn new(topology: PolymerTopology, translation_stddev: f64, rotation_stddev: f64) -> Self {
        Self {
            topology,
            translation_stddev,
            rotation_stddev,
        }
    }
}

impl MutationOperator for RigidChainMutation {
    fn mutate(
        &self,
        coordinates: &mut [[f64; 3]],
        rng: &mut dyn RandomSource,
    ) -> Result<GeneticEvent, SearchError> {
        if coordinates.len() != self.topology.atom_count() {
            return Err(SearchError::InvalidPopulation(
                "rigid-chain mutation received coordinates with the wrong size".into(),
            ));
        }
        let chain = rng.index(self.topology.chain_count());
        let range = self.topology.chain_ranges()[chain].clone();
        let center = center_of_geometry(&coordinates[range.clone()]);
        let translation: [f64; 3] = std::array::from_fn(|_| self.translation_stddev * rng.normal());
        let rotation: [f64; 3] = std::array::from_fn(|_| self.rotation_stddev * rng.normal());

        for coordinate in &mut coordinates[range] {
            let relative = subtract(*coordinate, center);
            let rotated = rotate_by_vector(relative, rotation);
            for axis in 0..3 {
                coordinate[axis] = center[axis] + rotated[axis] + translation[axis];
            }
        }
        let translation_norm = translation
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        let rotation_norm = rotation
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        Ok(GeneticEvent {
            operation: GeneticOperation::RigidChainTransform,
            target_gene: Some(chain),
            magnitude: translation_norm + rotation_norm,
        })
    }
}

pub(crate) fn center_of_geometry(coordinates: &[[f64; 3]]) -> [f64; 3] {
    let mut center = [0.0; 3];
    for coordinate in coordinates {
        for axis in 0..3 {
            center[axis] += coordinate[axis];
        }
    }
    center.map(|value| value / coordinates.len() as f64)
}

pub(crate) fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| left[axis] - right[axis])
}

pub(crate) fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

pub(crate) fn rotate_by_vector(point: [f64; 3], rotation: [f64; 3]) -> [f64; 3] {
    let angle = rotation
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if angle < 1.0e-14 {
        return point;
    }
    let axis = rotation.map(|value| value / angle);
    let cosine = angle.cos();
    let sine = angle.sin();
    let projection = axis
        .iter()
        .zip(point)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    let perpendicular = cross(axis, point);
    std::array::from_fn(|index| {
        point[index] * cosine
            + perpendicular[index] * sine
            + axis[index] * projection * (1.0 - cosine)
    })
}
