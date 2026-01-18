//! Strategy optimization and hyperparameter tuning
//!
//! This module provides algorithms for optimizing trading strategies and
//! finding optimal hyperparameters for ML models and trading algorithms.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parameter definition for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub min_value: f64,
    pub max_value: f64,
    pub step_size: Option<f64>,
    pub is_integer: bool,
}

impl Parameter {
    pub fn new(name: impl Into<String>, min_value: f64, max_value: f64) -> Self {
        Self {
            name: name.into(),
            min_value,
            max_value,
            step_size: None,
            is_integer: false,
        }
    }

    pub fn with_step(mut self, step: f64) -> Self {
        self.step_size = Some(step);
        self
    }

    pub fn as_integer(mut self) -> Self {
        self.is_integer = true;
        self
    }

    /// Generate a random value within the parameter range
    pub fn random_value(&self) -> f64 {
        let mut rng = rand::rng();
        let value = rng.random_range(self.min_value..=self.max_value);

        if self.is_integer {
            value.round()
        } else if let Some(step) = self.step_size {
            (value / step).round() * step
        } else {
            value
        }
    }

    /// Get all possible values if discrete
    pub fn discrete_values(&self) -> Option<Vec<f64>> {
        if let Some(step) = self.step_size {
            let mut values = Vec::new();
            let mut current = self.min_value;

            while current <= self.max_value {
                values.push(if self.is_integer {
                    current.round()
                } else {
                    current
                });
                current += step;
            }

            Some(values)
        } else if self.is_integer {
            let mut values = Vec::new();
            let mut current = self.min_value.round();
            let max = self.max_value.round();

            while current <= max {
                values.push(current);
                current += 1.0;
            }

            Some(values)
        } else {
            None
        }
    }
}

/// Parameter set for a strategy configuration
pub type ParameterSet = HashMap<String, f64>;

/// Optimization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub best_parameters: ParameterSet,
    pub best_score: f64,
    pub iterations: usize,
    pub all_results: Vec<(ParameterSet, f64)>,
}

impl OptimizationResult {
    pub fn new(best_parameters: ParameterSet, best_score: f64, iterations: usize) -> Self {
        Self {
            best_parameters,
            best_score,
            iterations,
            all_results: Vec::new(),
        }
    }

    pub fn with_history(mut self, results: Vec<(ParameterSet, f64)>) -> Self {
        self.all_results = results;
        self
    }

    /// Get top N parameter sets by score
    pub fn top_n(&self, n: usize) -> Vec<&(ParameterSet, f64)> {
        let mut sorted = self.all_results.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        sorted.into_iter().take(n).collect()
    }
}

/// Objective function for optimization
pub trait ObjectiveFunction: Send + Sync {
    /// Evaluate the objective function with given parameters
    /// Higher scores are better
    fn evaluate(&self, parameters: &ParameterSet) -> anyhow::Result<f64>;
}

/// Grid search optimizer
pub struct GridSearchOptimizer {
    parameters: Vec<Parameter>,
    max_combinations: Option<usize>,
}

impl GridSearchOptimizer {
    pub fn new(parameters: Vec<Parameter>) -> Self {
        Self {
            parameters,
            max_combinations: None,
        }
    }

    pub fn with_max_combinations(mut self, max: usize) -> Self {
        self.max_combinations = Some(max);
        self
    }

    fn generate_grid(&self) -> Vec<ParameterSet> {
        let mut discrete_params = Vec::new();

        for param in &self.parameters {
            if let Some(values) = param.discrete_values() {
                discrete_params.push((param.name.clone(), values));
            } else {
                // For continuous parameters, use sampling
                let samples = 10;
                let step = (param.max_value - param.min_value) / (samples - 1) as f64;
                let values: Vec<f64> = (0..samples)
                    .map(|i| param.min_value + i as f64 * step)
                    .collect();
                discrete_params.push((param.name.clone(), values));
            }
        }

        // Generate all combinations
        let mut combinations = vec![ParameterSet::new()];

        for (name, values) in discrete_params {
            let mut new_combinations = Vec::new();

            for combo in &combinations {
                for &value in &values {
                    let mut new_combo = combo.clone();
                    new_combo.insert(name.clone(), value);
                    new_combinations.push(new_combo);

                    if let Some(max) = self.max_combinations {
                        if new_combinations.len() >= max {
                            return new_combinations;
                        }
                    }
                }
            }

            combinations = new_combinations;
        }

        combinations
    }

    pub fn optimize<F>(&self, objective: &F) -> anyhow::Result<OptimizationResult>
    where
        F: ObjectiveFunction,
    {
        let grid = self.generate_grid();

        if grid.is_empty() {
            anyhow::bail!("No parameter combinations to evaluate");
        }

        let mut best_params = grid[0].clone();
        let mut best_score = f64::NEG_INFINITY;
        let mut all_results = Vec::new();

        for params in grid {
            let score = objective.evaluate(&params)?;
            all_results.push((params.clone(), score));

            if score > best_score {
                best_score = score;
                best_params = params;
            }
        }

        Ok(
            OptimizationResult::new(best_params, best_score, all_results.len())
                .with_history(all_results),
        )
    }
}

/// Random search optimizer
pub struct RandomSearchOptimizer {
    parameters: Vec<Parameter>,
    iterations: usize,
}

impl RandomSearchOptimizer {
    pub fn new(parameters: Vec<Parameter>, iterations: usize) -> Self {
        Self {
            parameters,
            iterations,
        }
    }

    fn random_parameters(&self) -> ParameterSet {
        self.parameters
            .iter()
            .map(|p| (p.name.clone(), p.random_value()))
            .collect()
    }

    pub fn optimize<F>(&self, objective: &F) -> anyhow::Result<OptimizationResult>
    where
        F: ObjectiveFunction,
    {
        let mut best_params = self.random_parameters();
        let mut best_score = f64::NEG_INFINITY;
        let mut all_results = Vec::new();

        for _ in 0..self.iterations {
            let params = self.random_parameters();
            let score = objective.evaluate(&params)?;
            all_results.push((params.clone(), score));

            if score > best_score {
                best_score = score;
                best_params = params;
            }
        }

        Ok(
            OptimizationResult::new(best_params, best_score, self.iterations)
                .with_history(all_results),
        )
    }
}

/// Bayesian-inspired optimizer (simplified)
pub struct BayesianOptimizer {
    parameters: Vec<Parameter>,
    initial_random: usize,
    total_iterations: usize,
}

impl BayesianOptimizer {
    pub fn new(parameters: Vec<Parameter>, total_iterations: usize) -> Self {
        Self {
            parameters,
            initial_random: (total_iterations as f64 * 0.2).ceil() as usize,
            total_iterations,
        }
    }

    pub fn with_initial_random(mut self, n: usize) -> Self {
        self.initial_random = n;
        self
    }

    fn random_parameters(&self) -> ParameterSet {
        self.parameters
            .iter()
            .map(|p| (p.name.clone(), p.random_value()))
            .collect()
    }

    fn explore_around(&self, params: &ParameterSet, exploration_rate: f64) -> ParameterSet {
        let mut rng = rand::rng();
        let mut new_params = params.clone();

        for param in &self.parameters {
            if rng.random_range(0.0..1.0) < exploration_rate {
                let range = (param.max_value - param.min_value) * 0.1;
                let current = *new_params.get(&param.name).unwrap_or(&param.min_value);
                let perturbation = rng.random_range(-range..=range);
                let new_value = (current + perturbation)
                    .max(param.min_value)
                    .min(param.max_value);

                let adjusted_value = if param.is_integer {
                    new_value.round()
                } else if let Some(step) = param.step_size {
                    (new_value / step).round() * step
                } else {
                    new_value
                };

                new_params.insert(param.name.clone(), adjusted_value);
            }
        }

        new_params
    }

    pub fn optimize<F>(&self, objective: &F) -> anyhow::Result<OptimizationResult>
    where
        F: ObjectiveFunction,
    {
        let mut best_params = self.random_parameters();
        let mut best_score = f64::NEG_INFINITY;
        let mut all_results = Vec::new();

        // Initial random exploration
        for _ in 0..self.initial_random {
            let params = self.random_parameters();
            let score = objective.evaluate(&params)?;
            all_results.push((params.clone(), score));

            if score > best_score {
                best_score = score;
                best_params = params;
            }
        }

        // Exploitation with exploration
        for i in self.initial_random..self.total_iterations {
            let exploration_rate = 0.3 * (1.0 - i as f64 / self.total_iterations as f64);
            let params = self.explore_around(&best_params, exploration_rate);
            let score = objective.evaluate(&params)?;
            all_results.push((params.clone(), score));

            if score > best_score {
                best_score = score;
                best_params = params;
            }
        }

        Ok(
            OptimizationResult::new(best_params, best_score, self.total_iterations)
                .with_history(all_results),
        )
    }
}

/// Genetic algorithm optimizer
pub struct GeneticOptimizer {
    parameters: Vec<Parameter>,
    population_size: usize,
    generations: usize,
    mutation_rate: f64,
    crossover_rate: f64,
}

impl GeneticOptimizer {
    pub fn new(parameters: Vec<Parameter>, generations: usize) -> Self {
        Self {
            parameters,
            population_size: 50,
            generations,
            mutation_rate: 0.1,
            crossover_rate: 0.7,
        }
    }

    pub fn with_population_size(mut self, size: usize) -> Self {
        self.population_size = size;
        self
    }

    pub fn with_mutation_rate(mut self, rate: f64) -> Self {
        self.mutation_rate = rate;
        self
    }

    fn random_individual(&self) -> ParameterSet {
        self.parameters
            .iter()
            .map(|p| (p.name.clone(), p.random_value()))
            .collect()
    }

    fn mutate(&self, individual: &mut ParameterSet) {
        let mut rng = rand::rng();

        for param in &self.parameters {
            if rng.random_range(0.0..1.0) < self.mutation_rate {
                individual.insert(param.name.clone(), param.random_value());
            }
        }
    }

    fn crossover(&self, parent1: &ParameterSet, parent2: &ParameterSet) -> ParameterSet {
        let mut rng = rand::rng();
        let mut child = ParameterSet::new();

        for param in &self.parameters {
            let value = if rng.random_range(0.0..1.0) < 0.5 {
                *parent1.get(&param.name).unwrap()
            } else {
                *parent2.get(&param.name).unwrap()
            };
            child.insert(param.name.clone(), value);
        }

        child
    }

    pub fn optimize<F>(&self, objective: &F) -> anyhow::Result<OptimizationResult>
    where
        F: ObjectiveFunction,
    {
        let mut rng = rand::rng();

        // Initialize population
        let mut population: Vec<ParameterSet> = (0..self.population_size)
            .map(|_| self.random_individual())
            .collect();

        let mut best_params = population[0].clone();
        let mut best_score = f64::NEG_INFINITY;
        let mut all_results = Vec::new();

        for _generation in 0..self.generations {
            // Evaluate fitness
            let mut fitness_scores: Vec<(ParameterSet, f64)> = population
                .iter()
                .map(|params| {
                    let score = objective.evaluate(params).unwrap_or(f64::NEG_INFINITY);
                    all_results.push((params.clone(), score));

                    if score > best_score {
                        best_score = score;
                        best_params = params.clone();
                    }

                    (params.clone(), score)
                })
                .collect();

            // Sort by fitness
            fitness_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // Selection: keep top 50%
            let survivors = fitness_scores.len() / 2;
            let selected: Vec<ParameterSet> = fitness_scores
                .into_iter()
                .take(survivors)
                .map(|(params, _)| params)
                .collect();

            // Create new population
            let mut new_population = selected.clone();

            while new_population.len() < self.population_size {
                let idx1 = rng.random_range(0..selected.len());
                let idx2 = rng.random_range(0..selected.len());

                let mut offspring = if rng.random_range(0.0..1.0) < self.crossover_rate {
                    self.crossover(&selected[idx1], &selected[idx2])
                } else {
                    selected[idx1].clone()
                };

                self.mutate(&mut offspring);
                new_population.push(offspring);
            }

            population = new_population;
        }

        let total_evals = all_results.len();
        Ok(OptimizationResult::new(best_params, best_score, total_evals).with_history(all_results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct QuadraticObjective;

    impl ObjectiveFunction for QuadraticObjective {
        fn evaluate(&self, parameters: &ParameterSet) -> anyhow::Result<f64> {
            let x = parameters.get("x").unwrap_or(&0.0);
            let y = parameters.get("y").unwrap_or(&0.0);

            // Maximize: -(x-5)^2 - (y-3)^2
            // Optimal: x=5, y=3, score=0
            let score = -((x - 5.0).powi(2) + (y - 3.0).powi(2));
            Ok(score)
        }
    }

    #[test]
    fn test_parameter_random_value() {
        let param = Parameter::new("test", 0.0, 10.0);
        let value = param.random_value();
        assert!((0.0..=10.0).contains(&value));
    }

    #[test]
    fn test_parameter_discrete_values() {
        let param = Parameter::new("test", 0.0, 10.0).with_step(2.0);

        let values = param.discrete_values().unwrap();
        assert_eq!(values, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
    }

    #[test]
    fn test_grid_search() {
        let params = vec![
            Parameter::new("x", 0.0, 10.0).with_step(1.0),
            Parameter::new("y", 0.0, 10.0).with_step(1.0),
        ];

        let optimizer = GridSearchOptimizer::new(params);
        let objective = QuadraticObjective;

        let result = optimizer.optimize(&objective).unwrap();
        assert!(result.best_score > -5.0);
        assert!(!result.all_results.is_empty());
    }

    #[test]
    fn test_random_search() {
        let params = vec![
            Parameter::new("x", 0.0, 10.0),
            Parameter::new("y", 0.0, 10.0),
        ];

        let optimizer = RandomSearchOptimizer::new(params, 100);
        let objective = QuadraticObjective;

        let result = optimizer.optimize(&objective).unwrap();
        assert_eq!(result.iterations, 100);
        assert!(result.best_score > -50.0);
    }

    #[test]
    fn test_bayesian_optimizer() {
        let params = vec![
            Parameter::new("x", 0.0, 10.0),
            Parameter::new("y", 0.0, 10.0),
        ];

        let optimizer = BayesianOptimizer::new(params, 50);
        let objective = QuadraticObjective;

        let result = optimizer.optimize(&objective).unwrap();
        assert!(result.best_score > -50.0);
    }

    #[test]
    fn test_genetic_optimizer() {
        let params = vec![
            Parameter::new("x", 0.0, 10.0),
            Parameter::new("y", 0.0, 10.0),
        ];

        let optimizer = GeneticOptimizer::new(params, 10).with_population_size(20);
        let objective = QuadraticObjective;

        let result = optimizer.optimize(&objective).unwrap();
        assert!(result.best_score > -50.0);
    }

    #[test]
    fn test_optimization_result_top_n() {
        let mut result =
            OptimizationResult::new([("x".to_string(), 5.0)].iter().cloned().collect(), 10.0, 3);

        result.all_results = vec![
            ([("x".to_string(), 1.0)].iter().cloned().collect(), 5.0),
            ([("x".to_string(), 2.0)].iter().cloned().collect(), 10.0),
            ([("x".to_string(), 3.0)].iter().cloned().collect(), 8.0),
        ];

        let top = result.top_n(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].1, 10.0);
        assert_eq!(top[1].1, 8.0);
    }
}
