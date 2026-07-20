use evokups_core::{
    Evaluation, EvaluationError, Individual, Population, PopulationEvaluator, RelaxationConfig,
    RelaxedIndividual,
};

use crate::PolymerTopology;
use crate::mutation::{center_of_geometry, cross, rotate_by_vector, subtract};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolymerPotential {
    pub cutoff: f64,
    pub sigma: f64,
    pub overlap_epsilon: f64,
    pub bond_length: f64,
    pub bond_stiffness: f64,
    pub density_weight: f64,
    pub density_modes: usize,
    pub angle_stiffness: f64,
    pub target_angle_cosine: f64,
    pub dihedral_stiffness: f64,
    pub target_dihedral_cosine: f64,
    pub void_weight: f64,
    pub void_probe: f64,
    pub target_void_fraction: f64,
    pub pressure_weight: f64,
    pub target_reduced_pressure: f64,
    pub reduced_temperature: f64,
}

impl PolymerPotential {
    #[must_use]
    pub const fn prepacking() -> Self {
        Self {
            cutoff: 1.2,
            sigma: 1.0,
            overlap_epsilon: 10.0,
            bond_length: 1.0,
            bond_stiffness: 50.0,
            density_weight: 1.0,
            density_modes: 1,
            angle_stiffness: 4.0,
            target_angle_cosine: -0.30,
            dihedral_stiffness: 0.75,
            target_dihedral_cosine: -0.5,
            void_weight: 2.0,
            void_probe: 0.72,
            target_void_fraction: 0.18,
            pressure_weight: 0.35,
            target_reduced_pressure: 0.0,
            reduced_temperature: 0.35,
        }
    }
}

impl Default for PolymerPotential {
    fn default() -> Self {
        Self::prepacking()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PolymerFitness {
    pub bond: f64,
    pub overlap: f64,
    pub angle: f64,
    pub dihedral: f64,
    pub density: f64,
    pub void: f64,
    pub pressure: f64,
    pub reduced_pressure: f64,
    pub void_fraction: f64,
    pub probe_void_fraction: f64,
    pub largest_void_radius: f64,
}

impl PolymerFitness {
    #[must_use]
    pub fn total(self) -> f64 {
        self.bond
            + self.overlap
            + self.angle
            + self.dihedral
            + self.density
            + self.void
            + self.pressure
    }
}

#[derive(Debug, Clone)]
pub struct PolymerPackingEvaluator {
    topology: PolymerTopology,
    potential: PolymerPotential,
}

impl PolymerPackingEvaluator {
    pub fn new(
        topology: PolymerTopology,
        potential: PolymerPotential,
    ) -> Result<Self, EvaluationError> {
        if potential.cutoff < potential.sigma
            || potential.sigma <= 0.0
            || potential.overlap_epsilon <= 0.0
            || potential.bond_length <= 0.0
            || potential.bond_stiffness <= 0.0
            || !potential.density_weight.is_finite()
            || potential.density_weight < 0.0
            || potential.density_modes == 0
            || !potential.angle_stiffness.is_finite()
            || potential.angle_stiffness < 0.0
            || !(-1.0..=1.0).contains(&potential.target_angle_cosine)
            || !potential.dihedral_stiffness.is_finite()
            || potential.dihedral_stiffness < 0.0
            || !(-1.0..=1.0).contains(&potential.target_dihedral_cosine)
            || !potential.void_weight.is_finite()
            || potential.void_weight < 0.0
            || !potential.void_probe.is_finite()
            || potential.void_probe <= 0.0
            || !(0.0..=1.0).contains(&potential.target_void_fraction)
            || !potential.pressure_weight.is_finite()
            || potential.pressure_weight < 0.0
            || !potential.target_reduced_pressure.is_finite()
            || !potential.reduced_temperature.is_finite()
            || potential.reduced_temperature < 0.0
        {
            return Err(EvaluationError::InvalidIndividual(
                "invalid polymer potential parameters".into(),
            ));
        }
        Ok(Self {
            topology,
            potential,
        })
    }

    #[must_use]
    pub fn topology(&self) -> &PolymerTopology {
        &self.topology
    }

    fn evaluate_coordinates(
        &self,
        coordinates: &[[f64; 3]],
    ) -> Result<Evaluation, EvaluationError> {
        if coordinates.len() != self.topology.atom_count() {
            return Err(EvaluationError::InvalidIndividual(format!(
                "expected {} beads, found {}",
                self.topology.atom_count(),
                coordinates.len()
            )));
        }
        let mut fitness = PolymerFitness::default();
        let mut forces = vec![[0.0; 3]; coordinates.len()];
        let mut configurational_virial = 0.0;

        for &(first, second) in self.topology.bonds() {
            let displacement = minimum_image(
                subtract(coordinates[first], coordinates[second]),
                self.topology.box_length(),
            );
            let distance = norm(displacement).max(1.0e-12);
            let extension = distance - self.potential.bond_length;
            fitness.bond += 0.5 * self.potential.bond_stiffness * extension * extension;
            let scale = -self.potential.bond_stiffness * extension / distance;
            configurational_virial += scale * distance * distance;
            add_pair_force(
                &mut forces,
                first,
                second,
                displacement.map(|value| scale * value),
            );
        }
        for &(first, center, third) in self.topology.angles() {
            fitness.angle += add_angle_force(
                coordinates,
                &mut forces,
                first,
                center,
                third,
                self.potential.angle_stiffness,
                self.potential.target_angle_cosine,
            );
        }
        for &(first, second, third, fourth) in self.topology.dihedrals() {
            fitness.dihedral += dihedral_energy(
                coordinates[first],
                coordinates[second],
                coordinates[third],
                coordinates[fourth],
                self.potential.dihedral_stiffness,
                self.potential.target_dihedral_cosine,
            );
        }

        let cells = CellList::build(
            coordinates,
            self.topology.box_length(),
            self.potential.cutoff,
        );
        for (first, second) in cells.pairs(coordinates) {
            if self.topology.are_directly_bonded(first, second) {
                continue;
            }
            let displacement = minimum_image(
                subtract(coordinates[first], coordinates[second]),
                self.topology.box_length(),
            );
            let distance = norm(displacement).max(1.0e-12);
            if distance >= self.potential.sigma {
                continue;
            }
            let overlap = self.potential.sigma - distance;
            fitness.overlap += 0.5 * self.potential.overlap_epsilon * overlap * overlap;
            let scale = self.potential.overlap_epsilon * overlap / distance;
            configurational_virial += scale * distance * distance;
            add_pair_force(
                &mut forces,
                first,
                second,
                displacement.map(|value| scale * value),
            );
        }

        fitness.density = add_density_uniformity_forces(
            coordinates,
            &mut forces,
            self.topology.box_length(),
            self.potential.density_weight,
            self.potential.density_modes,
        );
        fitness.void_fraction = cells.empty_fraction();
        let excess_void = (fitness.void_fraction - self.potential.target_void_fraction).max(0.0);
        fitness.void = self.potential.void_weight * coordinates.len() as f64 * excess_void.powi(2);
        let volume = self.topology.box_length().powi(3);
        let number_density = coordinates.len() as f64 / volume;
        fitness.reduced_pressure = number_density * self.potential.reduced_temperature
            + configurational_virial / (3.0 * volume);
        fitness.pressure = self.potential.pressure_weight
            * coordinates.len() as f64
            * (fitness.reduced_pressure - self.potential.target_reduced_pressure).powi(2);

        Ok(Evaluation {
            energy: fitness.total(),
            forces,
        })
    }

    pub fn fitness(&self, coordinates: &[[f64; 3]]) -> Result<PolymerFitness, EvaluationError> {
        if coordinates.len() != self.topology.atom_count() {
            return Err(EvaluationError::InvalidIndividual(
                "fitness coordinates do not match the polymer topology".into(),
            ));
        }
        let mut forces = vec![[0.0; 3]; coordinates.len()];
        let evaluation = self.evaluate_coordinates(coordinates)?;
        let mut fitness = PolymerFitness::default();
        let mut configurational_virial = 0.0;
        for &(first, second) in self.topology.bonds() {
            let displacement = minimum_image(
                subtract(coordinates[first], coordinates[second]),
                self.topology.box_length(),
            );
            let distance = norm(displacement);
            let extension = distance - self.potential.bond_length;
            fitness.bond += 0.5 * self.potential.bond_stiffness * extension.powi(2);
            configurational_virial += -self.potential.bond_stiffness * extension * distance;
        }
        for &(first, center, third) in self.topology.angles() {
            fitness.angle += add_angle_force(
                coordinates,
                &mut forces,
                first,
                center,
                third,
                self.potential.angle_stiffness,
                self.potential.target_angle_cosine,
            );
        }
        for &(first, second, third, fourth) in self.topology.dihedrals() {
            fitness.dihedral += dihedral_energy(
                coordinates[first],
                coordinates[second],
                coordinates[third],
                coordinates[fourth],
                self.potential.dihedral_stiffness,
                self.potential.target_dihedral_cosine,
            );
        }
        let cells = CellList::build(
            coordinates,
            self.topology.box_length(),
            self.potential.cutoff,
        );
        for (first, second) in cells.pairs(coordinates) {
            if self.topology.are_directly_bonded(first, second) {
                continue;
            }
            let distance = norm(minimum_image(
                subtract(coordinates[first], coordinates[second]),
                self.topology.box_length(),
            ));
            if distance < self.potential.sigma {
                let overlap = self.potential.sigma - distance;
                fitness.overlap += 0.5 * self.potential.overlap_epsilon * overlap.powi(2);
                configurational_virial += self.potential.overlap_epsilon * overlap * distance;
            }
        }
        fitness.density = add_density_uniformity_forces(
            coordinates,
            &mut forces,
            self.topology.box_length(),
            self.potential.density_weight,
            self.potential.density_modes,
        );
        fitness.void_fraction = cells.empty_fraction();
        (fitness.probe_void_fraction, fitness.largest_void_radius) = void_diagnostics(
            coordinates,
            self.topology.box_length(),
            self.potential.void_probe,
        );
        let excess = (fitness.void_fraction - self.potential.target_void_fraction).max(0.0);
        fitness.void = self.potential.void_weight * coordinates.len() as f64 * excess.powi(2);
        let volume = self.topology.box_length().powi(3);
        fitness.reduced_pressure = coordinates.len() as f64 / volume
            * self.potential.reduced_temperature
            + configurational_virial / (3.0 * volume);
        fitness.pressure = self.potential.pressure_weight
            * coordinates.len() as f64
            * (fitness.reduced_pressure - self.potential.target_reduced_pressure).powi(2);
        debug_assert!((fitness.total() - evaluation.energy).abs() < 1.0e-8);
        Ok(fitness)
    }

    pub fn reduced_pressure(&self, coordinates: &[[f64; 3]]) -> Result<f64, EvaluationError> {
        if coordinates.len() != self.topology.atom_count() {
            return Err(EvaluationError::InvalidIndividual(
                "pressure coordinates do not match the polymer topology".into(),
            ));
        }
        let mut configurational_virial = 0.0;
        for &(first, second) in self.topology.bonds() {
            let distance = norm(minimum_image(
                subtract(coordinates[first], coordinates[second]),
                self.topology.box_length(),
            ));
            let extension = distance - self.potential.bond_length;
            configurational_virial += -self.potential.bond_stiffness * extension * distance;
        }
        let cells = CellList::build(
            coordinates,
            self.topology.box_length(),
            self.potential.cutoff,
        );
        for (first, second) in cells.pairs(coordinates) {
            if self.topology.are_directly_bonded(first, second) {
                continue;
            }
            let distance = norm(minimum_image(
                subtract(coordinates[first], coordinates[second]),
                self.topology.box_length(),
            ));
            if distance < self.potential.sigma {
                configurational_virial +=
                    self.potential.overlap_epsilon * (self.potential.sigma - distance) * distance;
            }
        }
        let volume = self.topology.box_length().powi(3);
        Ok(
            coordinates.len() as f64 / volume * self.potential.reduced_temperature
                + configurational_virial / (3.0 * volume),
        )
    }
}

fn add_density_uniformity_forces(
    coordinates: &[[f64; 3]],
    forces: &mut [[f64; 3]],
    box_length: f64,
    weight: f64,
    maximum_mode: usize,
) -> f64 {
    if weight == 0.0 {
        return 0.0;
    }
    let count = coordinates.len() as f64;
    let base_wave_number = std::f64::consts::TAU / box_length;
    let mut energy = 0.0;
    let modes = reciprocal_modes(maximum_mode);
    for mode in &modes {
        let cosine_sum = coordinates
            .iter()
            .map(|coordinate| phase(*coordinate, *mode, base_wave_number).cos())
            .sum::<f64>();
        let sine_sum = coordinates
            .iter()
            .map(|coordinate| phase(*coordinate, *mode, base_wave_number).sin())
            .sum::<f64>();
        let normalization = count * modes.len() as f64;
        energy += weight * (cosine_sum * cosine_sum + sine_sum * sine_sum) / normalization;
        for (coordinate, force) in coordinates.iter().zip(forces.iter_mut()) {
            let atom_phase = phase(*coordinate, *mode, base_wave_number);
            for axis in 0..3 {
                force[axis] += 2.0 * weight * base_wave_number * f64::from(mode[axis])
                    / normalization
                    * (cosine_sum * atom_phase.sin() - sine_sum * atom_phase.cos());
            }
        }
    }
    energy
}

fn reciprocal_modes(maximum: usize) -> Vec<[i32; 3]> {
    let mut modes = Vec::with_capacity(3 * maximum);
    for magnitude in 1..=maximum {
        let magnitude = i32::try_from(magnitude).unwrap_or(i32::MAX);
        for axis in 0..3 {
            let mut mode = [0; 3];
            mode[axis] = magnitude;
            modes.push(mode);
        }
    }
    modes
}

fn phase(coordinate: [f64; 3], mode: [i32; 3], wave_number: f64) -> f64 {
    wave_number
        * (0..3)
            .map(|axis| coordinate[axis] * f64::from(mode[axis]))
            .sum::<f64>()
}

fn add_angle_force(
    coordinates: &[[f64; 3]],
    forces: &mut [[f64; 3]],
    first: usize,
    center: usize,
    third: usize,
    stiffness: f64,
    target_cosine: f64,
) -> f64 {
    if stiffness == 0.0 {
        return 0.0;
    }
    let left = subtract(coordinates[first], coordinates[center]);
    let right = subtract(coordinates[third], coordinates[center]);
    let left_norm = norm(left).max(1.0e-12);
    let right_norm = norm(right).max(1.0e-12);
    let cosine = (dot(left, right) / (left_norm * right_norm)).clamp(-1.0, 1.0);
    let delta = cosine - target_cosine;
    let derivative = stiffness * delta;
    let left_gradient: [f64; 3] = std::array::from_fn(|axis| {
        right[axis] / (left_norm * right_norm) - cosine * left[axis] / left_norm.powi(2)
    });
    let right_gradient: [f64; 3] = std::array::from_fn(|axis| {
        left[axis] / (left_norm * right_norm) - cosine * right[axis] / right_norm.powi(2)
    });
    for axis in 0..3 {
        let first_force = -derivative * left_gradient[axis];
        let third_force = -derivative * right_gradient[axis];
        forces[first][axis] += first_force;
        forces[third][axis] += third_force;
        forces[center][axis] -= first_force + third_force;
    }
    0.5 * stiffness * delta * delta
}

fn dihedral_energy(
    first: [f64; 3],
    second: [f64; 3],
    third: [f64; 3],
    fourth: [f64; 3],
    stiffness: f64,
    target_cosine: f64,
) -> f64 {
    if stiffness == 0.0 {
        return 0.0;
    }
    let first_bond = subtract(second, first);
    let second_bond = subtract(third, second);
    let third_bond = subtract(fourth, third);
    let first_normal = cross(first_bond, second_bond);
    let second_normal = cross(second_bond, third_bond);
    let cosine = (dot(first_normal, second_normal)
        / (norm(first_normal) * norm(second_normal)).max(1.0e-12))
    .clamp(-1.0, 1.0);
    0.5 * stiffness * (cosine - target_cosine).powi(2)
}

fn void_diagnostics(coordinates: &[[f64; 3]], box_length: f64, probe_radius: f64) -> (f64, f64) {
    let grid = (ceil_cuberoot(coordinates.len()) + 3).clamp(6, 12);
    let cells = CellList::build(coordinates, box_length, box_length / grid as f64);
    let mut void_samples = 0_usize;
    let mut largest = 0.0_f64;
    for x in 0..grid {
        for y in 0..grid {
            for z in 0..grid {
                let sample = [x, y, z].map(|index| box_length * (index as f64 + 0.5) / grid as f64);
                let nearest = cells.nearest_distance(sample, coordinates);
                largest = largest.max(nearest);
                void_samples += usize::from(nearest > probe_radius);
            }
        }
    }
    (void_samples as f64 / grid.pow(3) as f64, largest)
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left.into_iter().zip(right).map(|(a, b)| a * b).sum()
}

fn ceil_cuberoot(value: usize) -> usize {
    let mut root = 1_usize;
    while root.saturating_pow(3) < value {
        root += 1;
    }
    root
}

impl PopulationEvaluator for PolymerPackingEvaluator {
    fn evaluate(&self, individual: &Individual) -> Result<Evaluation, EvaluationError> {
        self.evaluate_coordinates(&individual.coordinates)
    }

    fn evaluate_population(
        &self,
        population: &Population,
    ) -> Result<Vec<Evaluation>, EvaluationError> {
        population
            .individuals
            .iter()
            .map(|individual| self.evaluate_coordinates(&individual.coordinates))
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
                    let evaluation = self.evaluate_coordinates(&relaxed.coordinates)?;
                    for range in self.topology.chain_ranges() {
                        let center = center_of_geometry(&relaxed.coordinates[range.clone()]);
                        let mut force = [0.0; 3];
                        let mut torque = [0.0; 3];
                        let mut moment = 0.0;
                        for atom in range.clone() {
                            let relative = subtract(relaxed.coordinates[atom], center);
                            for (axis, component) in force.iter_mut().enumerate() {
                                *component += evaluation.forces[atom][axis];
                            }
                            let atom_torque = cross(relative, evaluation.forces[atom]);
                            for axis in 0..3 {
                                torque[axis] += atom_torque[axis];
                            }
                            moment += relative.iter().map(|value| value * value).sum::<f64>();
                        }
                        let scale = config.step_size / range.len() as f64;
                        let translation = force.map(|value| {
                            scale
                                * value
                                    .clamp(-config.max_force_component, config.max_force_component)
                        });
                        let rotation = torque.map(|value| {
                            (config.step_size * value / moment.max(1.0e-12)).clamp(-0.1, 0.1)
                        });
                        for atom in range.clone() {
                            let relative = subtract(relaxed.coordinates[atom], center);
                            let rotated = rotate_by_vector(relative, rotation);
                            for axis in 0..3 {
                                relaxed.coordinates[atom][axis] =
                                    center[axis] + rotated[axis] + translation[axis];
                            }
                        }
                    }
                }
                let evaluation = self.evaluate_coordinates(&relaxed.coordinates)?;
                Ok(RelaxedIndividual {
                    individual: relaxed,
                    evaluation,
                })
            })
            .collect()
    }
}

fn norm(vector: [f64; 3]) -> f64 {
    vector.iter().map(|value| value * value).sum::<f64>().sqrt()
}

fn minimum_image(mut displacement: [f64; 3], box_length: f64) -> [f64; 3] {
    for component in &mut displacement {
        *component -= box_length * (*component / box_length).round();
    }
    displacement
}

fn add_pair_force(forces: &mut [[f64; 3]], first: usize, second: usize, force: [f64; 3]) {
    for axis in 0..3 {
        forces[first][axis] += force[axis];
        forces[second][axis] -= force[axis];
    }
}

#[derive(Debug)]
struct CellList {
    bins: Vec<Vec<usize>>,
    cells_per_axis: usize,
    cell_width: f64,
    box_length: f64,
}

impl CellList {
    fn build(coordinates: &[[f64; 3]], box_length: f64, cutoff: f64) -> Self {
        let cells_per_axis = floor_to_usize(box_length / cutoff).max(1);
        let cell_width = box_length / cells_per_axis as f64;
        let mut bins = vec![Vec::new(); cells_per_axis.pow(3)];
        for (atom, coordinate) in coordinates.iter().enumerate() {
            let cell: [usize; 3] = std::array::from_fn(|axis| {
                floor_to_usize(coordinate[axis].rem_euclid(box_length) / cell_width)
                    .min(cells_per_axis - 1)
            });
            bins[Self::linear_index(cell, cells_per_axis)].push(atom);
        }
        Self {
            bins,
            cells_per_axis,
            cell_width,
            box_length,
        }
    }

    fn pairs(&self, coordinates: &[[f64; 3]]) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();
        for first in 0..coordinates.len() {
            let cell: [usize; 3] = std::array::from_fn(|axis| {
                floor_to_usize(
                    coordinates[first][axis].rem_euclid(self.box_length) / self.cell_width,
                )
                .min(self.cells_per_axis - 1)
            });
            let mut neighbour_cells = Vec::with_capacity(27);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let neighbour = [
                            wrap_cell(cell[0], dx, self.cells_per_axis),
                            wrap_cell(cell[1], dy, self.cells_per_axis),
                            wrap_cell(cell[2], dz, self.cells_per_axis),
                        ];
                        neighbour_cells.push(Self::linear_index(neighbour, self.cells_per_axis));
                    }
                }
            }
            neighbour_cells.sort_unstable();
            neighbour_cells.dedup();
            for cell_index in neighbour_cells {
                for &second in &self.bins[cell_index] {
                    if second > first {
                        let displacement = minimum_image(
                            subtract(coordinates[first], coordinates[second]),
                            self.box_length,
                        );
                        if norm(displacement) < self.cell_width {
                            pairs.push((first, second));
                        }
                    }
                }
            }
        }
        pairs
    }

    fn empty_fraction(&self) -> f64 {
        self.bins.iter().filter(|bin| bin.is_empty()).count() as f64 / self.bins.len() as f64
    }

    fn nearest_distance(&self, point: [f64; 3], coordinates: &[[f64; 3]]) -> f64 {
        let origin: [usize; 3] = std::array::from_fn(|axis| {
            floor_to_usize(point[axis].rem_euclid(self.box_length) / self.cell_width)
                .min(self.cells_per_axis - 1)
        });
        let mut nearest = f64::INFINITY;
        let mut visited = Vec::new();
        for shell in 0..=self.cells_per_axis.div_ceil(2) {
            let shell = isize::try_from(shell).unwrap_or(isize::MAX);
            for dx in -shell..=shell {
                for dy in -shell..=shell {
                    for dz in -shell..=shell {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != shell {
                            continue;
                        }
                        let cell = [
                            wrap_cell(origin[0], dx, self.cells_per_axis),
                            wrap_cell(origin[1], dy, self.cells_per_axis),
                            wrap_cell(origin[2], dz, self.cells_per_axis),
                        ];
                        let index = Self::linear_index(cell, self.cells_per_axis);
                        if visited.contains(&index) {
                            continue;
                        }
                        visited.push(index);
                        for &atom in &self.bins[index] {
                            nearest = nearest.min(norm(minimum_image(
                                subtract(point, coordinates[atom]),
                                self.box_length,
                            )));
                        }
                    }
                }
            }
            if nearest.is_finite()
                && shell as f64 * self.cell_width > nearest + 1.75 * self.cell_width
            {
                break;
            }
        }
        nearest
    }

    fn linear_index(cell: [usize; 3], cells_per_axis: usize) -> usize {
        (cell[0] * cells_per_axis + cell[1]) * cells_per_axis + cell[2]
    }
}

fn wrap_cell(cell: usize, offset: isize, cells_per_axis: usize) -> usize {
    (cell.cast_signed() + offset)
        .rem_euclid(cells_per_axis.cast_signed())
        .cast_unsigned()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn floor_to_usize(value: f64) -> usize {
    value.floor() as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RigidChainMutation;
    use evokups_core::{
        EvolutionEngine, EvolutionMode, MutationOperator, PopulationEvaluator, RandomSource,
        RelaxationConfig, SearchConfig,
    };

    #[derive(Debug)]
    struct FixedRng;

    impl RandomSource for FixedRng {
        fn uniform(&mut self) -> f64 {
            0.1
        }
        fn normal(&mut self) -> f64 {
            0.2
        }
    }

    #[derive(Debug)]
    struct BridgeRng;

    impl RandomSource for BridgeRng {
        fn uniform(&mut self) -> f64 {
            0.95
        }

        fn normal(&mut self) -> f64 {
            0.2
        }
    }

    fn topology() -> PolymerTopology {
        PolymerTopology::new(vec![3, 4], 8.0).unwrap()
    }

    fn potential() -> PolymerPotential {
        PolymerPotential {
            overlap_epsilon: 2.0,
            ..PolymerPotential::prepacking()
        }
    }

    #[test]
    fn bonded_pairs_are_excluded_from_overlap_energy() {
        let mut overlap_only = potential();
        overlap_only.density_weight = 0.0;
        overlap_only.angle_stiffness = 0.0;
        overlap_only.dihedral_stiffness = 0.0;
        overlap_only.void_weight = 0.0;
        overlap_only.pressure_weight = 0.0;
        let evaluator = PolymerPackingEvaluator::new(topology(), overlap_only).unwrap();
        let coordinates = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            [1.0, 0.5, 0.0],
            [2.0, 0.5, 0.0],
            [3.0, 0.5, 0.0],
        ];
        let result = evaluator
            .evaluate(&Individual::new(0, coordinates))
            .unwrap();
        assert!(result.energy > 0.0);

        let isolated = Individual::new(
            1,
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [4.0, 4.0, 4.0],
                [5.0, 4.0, 4.0],
                [6.0, 4.0, 4.0],
                [7.0, 4.0, 4.0],
            ],
        );
        assert!(evaluator.evaluate(&isolated).unwrap().energy.abs() < 1.0e-12);
    }

    #[test]
    fn density_objective_prefers_uniform_fundamental_modes() {
        let mut density_only = potential();
        density_only.overlap_epsilon = f64::EPSILON;
        density_only.bond_stiffness = f64::EPSILON;
        let evaluator = PolymerPackingEvaluator::new(topology(), density_only).unwrap();

        let clustered = Individual::new(0, vec![[0.0, 0.0, 0.0]; 7]);
        let uniform = Individual::new(
            1,
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 2.0, 3.0],
                [2.0, 4.0, 6.0],
                [3.0, 6.0, 1.0],
                [4.0, 1.0, 4.0],
                [5.0, 3.0, 7.0],
                [6.0, 5.0, 2.0],
            ],
        );

        assert!(
            evaluator.evaluate(&uniform).unwrap().energy
                < evaluator.evaluate(&clustered).unwrap().energy
        );
    }

    #[test]
    fn rigid_chain_mutation_preserves_internal_distances() {
        let topology = topology();
        let mutation = RigidChainMutation::new(topology, 0.5, 0.2);
        let mut coordinates = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [1.0, 3.0, 0.0],
            [2.0, 3.0, 0.0],
            [3.0, 3.0, 0.0],
        ];
        let before = coordinates.clone();
        mutation.mutate(&mut coordinates, &mut FixedRng).unwrap();
        for pair in [(0, 1), (1, 2)] {
            assert!(
                (norm(subtract(coordinates[pair.0], coordinates[pair.1]))
                    - norm(subtract(before[pair.0], before[pair.1])))
                .abs()
                    < 1.0e-12
            );
        }
    }

    #[test]
    fn polymer_mutation_can_reconnect_equal_length_tails() {
        let topology = topology();
        let mutation = crate::PolymerMutation::new(topology).with_reconnection(0.99, 1.0e6);
        let mut coordinates = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [1.0, 2.0, 0.0],
            [2.0, 2.0, 0.0],
            [3.0, 2.0, 0.0],
        ];
        let before = coordinates.len();
        let event = mutation.mutate(&mut coordinates, &mut BridgeRng).unwrap();
        assert_eq!(
            event.operation,
            evokups_core::GeneticOperation::DoubleBridge
        );
        assert_eq!(coordinates.len(), before);
        assert!(coordinates.iter().flatten().all(|value| value.is_finite()));
    }

    #[test]
    fn population_batch_makes_deterministic_evolutionary_progress() {
        let topology = topology();
        let evaluator = PolymerPackingEvaluator::new(topology.clone(), potential()).unwrap();
        let initializer = crate::PolymerPackingInitializer::new(topology.clone(), 1.0);
        let mutation = RigidChainMutation::new(topology.clone(), 0.35, 0.2);
        let mut engine = EvolutionEngine::new(SearchConfig {
            population_size: 12,
            atom_count: topology.atom_count(),
            elite_count: 2,
            mutation_stddev: 0.1,
            initial_extent: 1.0,
            seed: 73,
            mode: EvolutionMode::SinglePoint,
            relaxation: RelaxationConfig {
                steps: 3,
                step_size: 0.002,
                max_force_component: 10.0,
            },
        })
        .unwrap();
        let initial = engine.initialize_with(&initializer).unwrap();
        let mut population = engine.evaluate(initial, &evaluator).unwrap();
        let initial_best = population.best().unwrap().energy.unwrap();

        for _ in 0..5 {
            population = engine
                .evolve_one_with(population, &evaluator, &mutation)
                .unwrap();
        }

        assert_eq!(population.generation, 5);
        assert_eq!(population.individuals.len(), 12);
        assert!(population.best().unwrap().energy.unwrap() <= initial_best);
        assert!(
            population
                .individuals
                .iter()
                .all(|individual| individual.parent_id.is_some())
        );
        assert_eq!(
            evaluator.evaluate_population(&population).unwrap().len(),
            12
        );
    }
}
