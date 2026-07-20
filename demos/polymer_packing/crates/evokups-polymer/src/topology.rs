use evokups_core::EvaluationError;
use std::ops::Range;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PolymerArchitecture {
    #[default]
    Linear,
    Ring,
    Dendritic,
    Bottlebrush,
}

impl PolymerArchitecture {
    #[must_use]
    pub const fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Linear),
            1 => Some(Self::Ring),
            2 => Some(Self::Dendritic),
            3 => Some(Self::Bottlebrush),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolymerMorphology {
    pub dendritic_branching: usize,
    pub dendritic_spacer: usize,
    pub bottlebrush_graft_spacing: usize,
    pub bottlebrush_side_chain: usize,
}

impl Default for PolymerMorphology {
    fn default() -> Self {
        Self {
            dendritic_branching: 2,
            dendritic_spacer: 2,
            bottlebrush_graft_spacing: 2,
            bottlebrush_side_chain: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolymerTopology {
    chain_lengths: Vec<usize>,
    chain_ranges: Vec<Range<usize>>,
    atom_to_chain: Vec<usize>,
    bead_within_chain: Vec<usize>,
    bead_type_ids: Vec<u16>,
    architecture: PolymerArchitecture,
    morphology: PolymerMorphology,
    bonds: Vec<(usize, usize)>,
    chain_bonds: Vec<Vec<(usize, usize)>>,
    angles: Vec<(usize, usize, usize)>,
    dihedrals: Vec<(usize, usize, usize, usize)>,
    degrees: Vec<usize>,
    growth_parents: Vec<Option<usize>>,
    box_length: f64,
}

impl PolymerTopology {
    pub fn new(chain_lengths: Vec<usize>, box_length: f64) -> Result<Self, EvaluationError> {
        Self::with_architecture(chain_lengths, box_length, PolymerArchitecture::Linear)
    }

    pub fn with_architecture(
        chain_lengths: Vec<usize>,
        box_length: f64,
        architecture: PolymerArchitecture,
    ) -> Result<Self, EvaluationError> {
        Self::with_morphology(
            chain_lengths,
            box_length,
            architecture,
            PolymerMorphology::default(),
        )
    }

    pub fn with_morphology(
        chain_lengths: Vec<usize>,
        box_length: f64,
        architecture: PolymerArchitecture,
        morphology: PolymerMorphology,
    ) -> Result<Self, EvaluationError> {
        if chain_lengths.is_empty() || chain_lengths.iter().any(|length| *length < 2) {
            return Err(EvaluationError::InvalidIndividual(
                "polymer topology requires at least one chain and two beads per chain".into(),
            ));
        }
        if !box_length.is_finite() || box_length <= 0.0 {
            return Err(EvaluationError::InvalidIndividual(
                "box length must be positive and finite".into(),
            ));
        }
        if !(2..=3).contains(&morphology.dendritic_branching)
            || !(1..=8).contains(&morphology.dendritic_spacer)
            || !(1..=16).contains(&morphology.bottlebrush_graft_spacing)
            || !(1..=32).contains(&morphology.bottlebrush_side_chain)
        {
            return Err(EvaluationError::InvalidIndividual(
                "polymer morphology parameters are outside supported bounds".into(),
            ));
        }

        let mut chain_ranges = Vec::with_capacity(chain_lengths.len());
        let mut atom_to_chain = Vec::new();
        let mut bead_within_chain = Vec::new();
        let mut bonds = Vec::new();
        let mut offset = 0;
        let mut chain_bonds = Vec::with_capacity(chain_lengths.len());
        for (chain, length) in chain_lengths.iter().copied().enumerate() {
            let range = offset..offset + length;
            chain_ranges.push(range.clone());
            atom_to_chain.extend(std::iter::repeat_n(chain, length));
            bead_within_chain.extend(0..length);
            let first_bond = bonds.len();
            add_architecture_bonds(&mut bonds, range, architecture, morphology);
            chain_bonds.push(bonds[first_bond..].to_vec());
            offset += length;
        }
        bonds.sort_unstable();
        bonds.dedup();

        let mut adjacency = vec![Vec::new(); offset];
        for &(first, second) in &bonds {
            adjacency[first].push(second);
            adjacency[second].push(first);
        }
        for neighbours in &mut adjacency {
            neighbours.sort_unstable();
        }
        let degrees = adjacency.iter().map(Vec::len).collect();
        let growth_parents = adjacency
            .iter()
            .enumerate()
            .map(|(atom, neighbours)| {
                neighbours
                    .iter()
                    .copied()
                    .filter(|other| *other < atom)
                    .max()
            })
            .collect();
        let angles = build_angles(&adjacency);
        let dihedrals = build_dihedrals(&bonds, &adjacency);

        Ok(Self {
            chain_lengths,
            chain_ranges,
            atom_to_chain,
            bead_within_chain,
            bead_type_ids: vec![0; offset],
            architecture,
            morphology,
            bonds,
            chain_bonds,
            angles,
            dihedrals,
            degrees,
            growth_parents,
            box_length,
        })
    }

    pub fn with_bead_types(mut self, bead_type_ids: Vec<u16>) -> Result<Self, EvaluationError> {
        if bead_type_ids.len() != self.atom_count() {
            return Err(EvaluationError::InvalidIndividual(format!(
                "received {} bead types for {} beads",
                bead_type_ids.len(),
                self.atom_count()
            )));
        }
        self.bead_type_ids = bead_type_ids;
        Ok(self)
    }

    pub fn with_box_length(&self, box_length: f64) -> Result<Self, EvaluationError> {
        Self::with_morphology(
            self.chain_lengths.clone(),
            box_length,
            self.architecture,
            self.morphology,
        )?
        .with_bead_types(self.bead_type_ids.clone())
    }

    #[must_use]
    pub fn chain_lengths(&self) -> &[usize] {
        &self.chain_lengths
    }
    #[must_use]
    pub fn chain_ranges(&self) -> &[Range<usize>] {
        &self.chain_ranges
    }
    #[must_use]
    pub fn atom_count(&self) -> usize {
        self.atom_to_chain.len()
    }
    #[must_use]
    pub fn chain_count(&self) -> usize {
        self.chain_lengths.len()
    }
    #[must_use]
    pub fn box_length(&self) -> f64 {
        self.box_length
    }
    #[must_use]
    pub fn architecture(&self) -> PolymerArchitecture {
        self.architecture
    }
    #[must_use]
    pub fn morphology(&self) -> PolymerMorphology {
        self.morphology
    }
    #[must_use]
    pub fn bonds(&self) -> &[(usize, usize)] {
        &self.bonds
    }
    #[must_use]
    pub fn chain_bonds(&self, chain: usize) -> Option<&[(usize, usize)]> {
        self.chain_bonds.get(chain).map(Vec::as_slice)
    }
    #[must_use]
    pub fn angles(&self) -> &[(usize, usize, usize)] {
        &self.angles
    }
    #[must_use]
    pub fn dihedrals(&self) -> &[(usize, usize, usize, usize)] {
        &self.dihedrals
    }
    #[must_use]
    pub fn degree(&self, atom: usize) -> Option<usize> {
        self.degrees.get(atom).copied()
    }
    #[must_use]
    pub fn growth_parent(&self, atom: usize) -> Option<usize> {
        self.growth_parents.get(atom).copied().flatten()
    }
    #[must_use]
    pub fn chain_of(&self, atom: usize) -> Option<usize> {
        self.atom_to_chain.get(atom).copied()
    }
    #[must_use]
    pub fn bead_within_chain(&self, atom: usize) -> Option<usize> {
        self.bead_within_chain.get(atom).copied()
    }
    #[must_use]
    pub fn chain_length_of(&self, atom: usize) -> Option<usize> {
        self.chain_of(atom)
            .and_then(|chain| self.chain_lengths.get(chain))
            .copied()
    }
    #[must_use]
    pub fn bead_type(&self, atom: usize) -> Option<u16> {
        self.bead_type_ids.get(atom).copied()
    }
    #[must_use]
    pub fn morphology_role(&self, atom: usize) -> Option<u16> {
        let chain = self.chain_of(atom)?;
        let range = self.chain_ranges.get(chain)?;
        let local = atom.checked_sub(range.start)?;
        let degree = self.degree(atom)?;
        Some(match self.architecture {
            PolymerArchitecture::Linear => u16::from(degree == 1),
            PolymerArchitecture::Ring => 0,
            PolymerArchitecture::Dendritic => {
                if local == 0 {
                    4
                } else if degree >= 3 {
                    2
                } else if degree == 1 {
                    1
                } else {
                    3
                }
            }
            PolymerArchitecture::Bottlebrush => {
                let backbone = bottlebrush_backbone_length(range.len(), self.morphology);
                if degree >= 3 {
                    2
                } else if local >= backbone {
                    if degree == 1 { 1 } else { 3 }
                } else {
                    u16::from(degree == 1)
                }
            }
        })
    }
    #[must_use]
    pub fn is_terminal(&self, atom: usize) -> bool {
        self.degree(atom) == Some(1)
    }
    #[must_use]
    pub fn are_directly_bonded(&self, first: usize, second: usize) -> bool {
        let edge = if first < second {
            (first, second)
        } else {
            (second, first)
        };
        self.bonds.binary_search(&edge).is_ok()
    }
}

fn add_architecture_bonds(
    bonds: &mut Vec<(usize, usize)>,
    range: Range<usize>,
    architecture: PolymerArchitecture,
    morphology: PolymerMorphology,
) {
    let length = range.len();
    match architecture {
        PolymerArchitecture::Linear | PolymerArchitecture::Ring => {
            bonds.extend((range.start..range.end - 1).map(|atom| (atom, atom + 1)));
            if architecture == PolymerArchitecture::Ring && length > 2 {
                bonds.push((range.start, range.end - 1));
            }
        }
        PolymerArchitecture::Dendritic => {
            let mut cursor = range.start + 1;
            let mut tips = vec![range.start];
            while cursor < range.end {
                let mut next_tips = Vec::new();
                for parent in tips {
                    for _ in 0..morphology.dendritic_branching {
                        if cursor >= range.end {
                            break;
                        }
                        let mut previous = parent;
                        for _ in 0..morphology.dendritic_spacer {
                            if cursor >= range.end {
                                break;
                            }
                            bonds.push((previous, cursor));
                            previous = cursor;
                            cursor += 1;
                        }
                        next_tips.push(previous);
                    }
                }
                tips = next_tips;
            }
        }
        PolymerArchitecture::Bottlebrush => {
            let backbone = bottlebrush_backbone_length(length, morphology);
            bonds.extend(
                (0..backbone - 1).map(|local| (range.start + local, range.start + local + 1)),
            );
            let mut cursor = range.start + backbone;
            let mut last_tip = range.start + backbone - 1;
            for graft in (0..backbone).step_by(morphology.bottlebrush_graft_spacing) {
                let mut previous = range.start + graft;
                for _ in 0..morphology.bottlebrush_side_chain {
                    if cursor >= range.end {
                        break;
                    }
                    bonds.push((previous, cursor));
                    previous = cursor;
                    last_tip = cursor;
                    cursor += 1;
                }
            }
            while cursor < range.end {
                bonds.push((last_tip, cursor));
                last_tip = cursor;
                cursor += 1;
            }
        }
    }
}

fn bottlebrush_backbone_length(length: usize, morphology: PolymerMorphology) -> usize {
    (2..=length)
        .rev()
        .find(|backbone| {
            let grafts = backbone.div_ceil(morphology.bottlebrush_graft_spacing);
            backbone + grafts * morphology.bottlebrush_side_chain <= length
        })
        .unwrap_or(2.min(length))
}

fn build_angles(adjacency: &[Vec<usize>]) -> Vec<(usize, usize, usize)> {
    let mut angles = Vec::new();
    for (center, neighbours) in adjacency.iter().enumerate() {
        for left in 0..neighbours.len() {
            for right in left + 1..neighbours.len() {
                angles.push((neighbours[left], center, neighbours[right]));
            }
        }
    }
    angles
}

fn build_dihedrals(
    bonds: &[(usize, usize)],
    adjacency: &[Vec<usize>],
) -> Vec<(usize, usize, usize, usize)> {
    let mut dihedrals = Vec::new();
    for &(second, third) in bonds {
        for &first in adjacency[second].iter().filter(|&&atom| atom != third) {
            for &fourth in adjacency[third].iter().filter(|&&atom| atom != second) {
                dihedrals.push((first, second, third, fourth));
            }
        }
    }
    dihedrals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_templates_are_connected_and_degree_bounded() {
        for architecture in [
            PolymerArchitecture::Linear,
            PolymerArchitecture::Ring,
            PolymerArchitecture::Dendritic,
            PolymerArchitecture::Bottlebrush,
        ] {
            let topology = PolymerTopology::with_architecture(vec![9], 8.0, architecture).unwrap();
            assert!(topology.bonds().len() >= 8);
            assert!(topology.degrees.iter().all(|degree| *degree <= 3));
        }
    }

    #[test]
    fn morphology_parameters_change_graph_connectivity() {
        let compact = PolymerTopology::with_morphology(
            vec![48],
            12.0,
            PolymerArchitecture::Dendritic,
            PolymerMorphology {
                dendritic_branching: 3,
                dendritic_spacer: 1,
                ..PolymerMorphology::default()
            },
        )
        .unwrap();
        let open = PolymerTopology::with_morphology(
            vec![48],
            12.0,
            PolymerArchitecture::Dendritic,
            PolymerMorphology {
                dendritic_branching: 2,
                dendritic_spacer: 4,
                ..PolymerMorphology::default()
            },
        )
        .unwrap();
        assert_ne!(compact.bonds(), open.bonds());
        assert!(compact.degrees.contains(&4));
        assert!(open.degrees.iter().all(|degree| *degree <= 3));
    }
}
