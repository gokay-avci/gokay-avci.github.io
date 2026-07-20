use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationError {
    InvalidIndividual(String),
    NumericalFailure(String),
}

impl Display for EvaluationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIndividual(message) => write!(formatter, "invalid individual: {message}"),
            Self::NumericalFailure(message) => write!(formatter, "numerical failure: {message}"),
        }
    }
}

impl Error for EvaluationError {}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchError {
    InvalidConfig(String),
    InvalidPopulation(String),
    MissingEvaluation { individual_id: u64 },
    Evaluation(EvaluationError),
}

impl Display for SearchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(message) => {
                write!(formatter, "invalid search configuration: {message}")
            }
            Self::InvalidPopulation(message) => write!(formatter, "invalid population: {message}"),
            Self::MissingEvaluation { individual_id } => {
                write!(
                    formatter,
                    "individual {individual_id} has not been evaluated"
                )
            }
            Self::Evaluation(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for SearchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Evaluation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EvaluationError> for SearchError {
    fn from(value: EvaluationError) -> Self {
        Self::Evaluation(value)
    }
}
