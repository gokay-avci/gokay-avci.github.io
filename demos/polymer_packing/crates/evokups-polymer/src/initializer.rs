use evokups_core::{PopulationInitializer, RandomSource, SearchError};

use crate::{PolymerArchitecture, PolymerTopology};

#[derive(Debug, Clone)]
pub struct PolymerPackingInitializer {
    topology: PolymerTopology,
    bond_length: f64,
    forward_cosine: f64,
    trial_directions: usize,
    ring_puckering: f64,
}

impl PolymerPackingInitializer {
    #[must_use]
    pub fn new(topology: PolymerTopology, bond_length: f64) -> Self {
        Self {
            topology,
            bond_length,
            forward_cosine: 0.30,
            trial_directions: 6,
            ring_puckering: 0.20,
        }
    }

    #[must_use]
    pub fn with_chain_geometry(mut self, forward_cosine: f64, trial_directions: usize) -> Self {
        self.forward_cosine = forward_cosine.clamp(-0.95, 0.95);
        self.trial_directions = trial_directions.max(1);
        self
    }

    #[must_use]
    pub fn with_ring_puckering(mut self, ring_puckering: f64) -> Self {
        self.ring_puckering = ring_puckering.clamp(0.0, 0.8);
        self
    }
}

impl PopulationInitializer for PolymerPackingInitializer {
    fn generate(
        &self,
        atom_count: usize,
        rng: &mut dyn RandomSource,
    ) -> Result<Vec<[f64; 3]>, SearchError> {
        if atom_count != self.topology.atom_count() {
            return Err(SearchError::InvalidPopulation(format!(
                "polymer initializer received {atom_count} atoms for a {}-bead topology",
                self.topology.atom_count()
            )));
        }

        let mut coordinates = Vec::with_capacity(atom_count);
        let mut neighbours = IncrementalCellList::new(
            self.topology.box_length(),
            (0.9 * self.bond_length).max(0.1),
        );
        let mut directions = Vec::with_capacity(atom_count);
        for range in self.topology.chain_ranges() {
            if self.topology.architecture() == PolymerArchitecture::Ring {
                let center: [f64; 3] =
                    std::array::from_fn(|_| rng.uniform() * self.topology.box_length());
                let normal = random_unit_vector(rng);
                let reference = if normal[0].abs() < 0.8 {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
                let first = normalize(cross(normal, reference));
                let second = cross(normal, first);
                let phase = std::f64::consts::TAU * rng.uniform();
                let ring_coordinates = puckered_ring(
                    range.len(),
                    center,
                    first,
                    second,
                    normal,
                    phase,
                    self.bond_length,
                    self.ring_puckering,
                );
                for point in ring_coordinates {
                    coordinates.push(point);
                    directions.push(normal);
                    neighbours.insert(point, coordinates.len() - 1);
                }
                continue;
            }
            let root = std::array::from_fn(|_| rng.uniform() * self.topology.box_length());
            coordinates.push(root);
            directions.push(random_unit_vector(rng));
            neighbours.insert(root, coordinates.len() - 1);
            for atom in (range.start + 1)..range.end {
                let parent = self.topology.growth_parent(atom).unwrap_or(atom - 1);
                let point = coordinates[parent];
                let previous_direction = directions[parent];
                let mut best_direction = previous_direction;
                let mut best_clearance = f64::NEG_INFINITY;
                for _ in 0..self.trial_directions {
                    let direction = if atom == range.start + 1 {
                        random_unit_vector(rng)
                    } else {
                        direction_at_fixed_angle(previous_direction, self.forward_cosine, rng)
                    };
                    let candidate = std::array::from_fn(|axis| {
                        point[axis] + self.bond_length * direction[axis]
                    });
                    let clearance = neighbours.clearance(candidate, &coordinates);
                    if clearance > best_clearance {
                        best_clearance = clearance;
                        best_direction = direction;
                    }
                }
                let child = std::array::from_fn(|axis| {
                    point[axis] + self.bond_length * best_direction[axis]
                });
                coordinates.push(child);
                directions.push(best_direction);
                neighbours.insert(child, coordinates.len() - 1);
            }
        }
        Ok(coordinates)
    }
}

#[allow(clippy::too_many_arguments)]
fn puckered_ring(
    length: usize,
    center: [f64; 3],
    first: [f64; 3],
    second: [f64; 3],
    normal: [f64; 3],
    phase: f64,
    bond_length: f64,
    puckering: f64,
) -> Vec<[f64; 3]> {
    let radius = bond_length / (2.0 * (std::f64::consts::PI / length as f64).sin());
    let amplitude = puckering * bond_length;
    let mut points: Vec<[f64; 3]> = (0..length)
        .map(|local| {
            let angle = phase + std::f64::consts::TAU * local as f64 / length as f64;
            let height = amplitude * (2.0 * angle + 0.37 * phase).sin();
            std::array::from_fn(|axis| {
                center[axis]
                    + radius * (angle.cos() * first[axis] + angle.sin() * second[axis])
                    + height * normal[axis]
            })
        })
        .collect();

    for _ in 0..160 {
        for first_atom in 0..length {
            let second_atom = (first_atom + 1) % length;
            let displacement: [f64; 3] =
                std::array::from_fn(|axis| points[second_atom][axis] - points[first_atom][axis]);
            let distance = displacement
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt()
                .max(f64::MIN_POSITIVE);
            let correction = 0.5 * (distance - bond_length) / distance;
            for axis in 0..3 {
                let shift = correction * displacement[axis];
                points[first_atom][axis] += shift;
                points[second_atom][axis] -= shift;
            }
        }
    }
    let centroid: [f64; 3] = std::array::from_fn(|axis| {
        points.iter().map(|point| point[axis]).sum::<f64>() / length as f64
    });
    for point in &mut points {
        for axis in 0..3 {
            point[axis] += center[axis] - centroid[axis];
        }
    }
    points
}

#[derive(Debug)]
struct IncrementalCellList {
    bins: Vec<Vec<usize>>,
    cells_per_axis: usize,
    cell_width: f64,
    box_length: f64,
}

impl IncrementalCellList {
    fn new(box_length: f64, cutoff: f64) -> Self {
        let cells_per_axis = floor_to_usize(box_length / cutoff).max(1);
        Self {
            bins: vec![Vec::new(); cells_per_axis.pow(3)],
            cells_per_axis,
            cell_width: box_length / cells_per_axis as f64,
            box_length,
        }
    }

    fn insert(&mut self, coordinate: [f64; 3], atom: usize) {
        let cell = self.cell(coordinate);
        let index = self.linear_index(cell);
        self.bins[index].push(atom);
    }

    fn clearance(&self, candidate: [f64; 3], coordinates: &[[f64; 3]]) -> f64 {
        let cell = self.cell(candidate);
        let mut nearest = f64::INFINITY;
        let mut visited = Vec::with_capacity(27);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let neighbour = [
                        wrap(cell[0], dx, self.cells_per_axis),
                        wrap(cell[1], dy, self.cells_per_axis),
                        wrap(cell[2], dz, self.cells_per_axis),
                    ];
                    visited.push(self.linear_index(neighbour));
                }
            }
        }
        visited.sort_unstable();
        visited.dedup();
        for bin in visited {
            for &atom in &self.bins[bin] {
                nearest = nearest.min(periodic_distance(
                    candidate,
                    coordinates[atom],
                    self.box_length,
                ));
            }
        }
        nearest
    }

    fn cell(&self, coordinate: [f64; 3]) -> [usize; 3] {
        std::array::from_fn(|axis| {
            floor_to_usize(coordinate[axis].rem_euclid(self.box_length) / self.cell_width)
                .min(self.cells_per_axis - 1)
        })
    }

    fn linear_index(&self, cell: [usize; 3]) -> usize {
        (cell[0] * self.cells_per_axis + cell[1]) * self.cells_per_axis + cell[2]
    }
}

fn wrap(cell: usize, offset: isize, cells_per_axis: usize) -> usize {
    (cell.cast_signed() + offset)
        .rem_euclid(cells_per_axis.cast_signed())
        .cast_unsigned()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn floor_to_usize(value: f64) -> usize {
    value.floor() as usize
}

fn random_unit_vector(rng: &mut dyn RandomSource) -> [f64; 3] {
    let vector: [f64; 3] = std::array::from_fn(|_| rng.normal());
    normalize(vector)
}

fn direction_at_fixed_angle(
    previous: [f64; 3],
    cosine: f64,
    rng: &mut dyn RandomSource,
) -> [f64; 3] {
    let reference = if previous[0].abs() < 0.8 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let first = normalize(cross(previous, reference));
    let second = cross(previous, first);
    let azimuth = std::f64::consts::TAU * rng.uniform();
    let sine = (1.0 - cosine * cosine).sqrt();
    normalize(std::array::from_fn(|axis| {
        cosine * previous[axis]
            + sine * (azimuth.cos() * first[axis] + azimuth.sin() * second[axis])
    }))
}

fn normalize(vector: [f64; 3]) -> [f64; 3] {
    let norm = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
    vector.map(|value| value / norm.max(f64::MIN_POSITIVE))
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn periodic_distance(left: [f64; 3], right: [f64; 3], box_length: f64) -> f64 {
    (0..3)
        .map(|axis| {
            let mut displacement = left[axis] - right[axis];
            displacement -= box_length * (displacement / box_length).round();
            displacement * displacement
        })
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedRng {
        state: f64,
    }

    impl RandomSource for FixedRng {
        fn uniform(&mut self) -> f64 {
            self.state = (self.state + 0.271_828).fract();
            self.state
        }

        fn normal(&mut self) -> f64 {
            2.0 * self.uniform() - 1.0
        }
    }

    #[test]
    fn ring_initializer_closes_every_bond_at_the_requested_length() {
        let topology =
            PolymerTopology::with_architecture(vec![12], 12.0, PolymerArchitecture::Ring).unwrap();
        let initializer = PolymerPackingInitializer::new(topology.clone(), 1.0);
        let coordinates = initializer
            .generate(topology.atom_count(), &mut FixedRng { state: 0.1 })
            .unwrap();
        for &(first, second) in topology.bonds() {
            assert!(
                (periodic_distance(
                    coordinates[first],
                    coordinates[second],
                    topology.box_length(),
                ) - 1.0)
                    .abs()
                    < 1.0e-8
            );
        }
    }
}
