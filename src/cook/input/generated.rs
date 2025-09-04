use super::provider::{InputConfig, InputProvider, ValidationIssue, ValidationSeverity};
use super::types::{ExecutionInput, InputType, VariableDefinition, VariableType, VariableValue};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use rand::Rng;

pub struct GeneratedInputProvider;

#[async_trait]
impl InputProvider for GeneratedInputProvider {
    fn input_type(&self) -> InputType {
        InputType::Generated {
            generator: "default".to_string(),
            config: serde_json::Value::Null,
        }
    }

    async fn validate(&self, config: &InputConfig) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check generator type is specified
        let generator = config
            .get_string("generator")
            .unwrap_or_else(|_| "default".to_string());

        let supported_generators = vec![
            "sequence",
            "random",
            "uuid",
            "timestamp",
            "range",
            "grid",
            "fibonacci",
            "factorial",
            "prime",
        ];

        if !supported_generators.contains(&generator.as_str()) {
            issues.push(ValidationIssue {
                field: "generator".to_string(),
                message: format!(
                    "Unsupported generator type: {}. Supported: {:?}",
                    generator, supported_generators
                ),
                severity: ValidationSeverity::Error,
            });
        }

        // Validate generator-specific configuration
        match generator.as_str() {
            "sequence" | "range" => {
                if config.get_string("start").is_err() && config.get_string("end").is_err() {
                    issues.push(ValidationIssue {
                        field: "config".to_string(),
                        message: "Range generator requires 'start' and/or 'end' parameters"
                            .to_string(),
                        severity: ValidationSeverity::Warning,
                    });
                }
            }
            "random" => {
                if config.get_string("count").is_err() {
                    issues.push(ValidationIssue {
                        field: "count".to_string(),
                        message: "Random generator requires 'count' parameter".to_string(),
                        severity: ValidationSeverity::Warning,
                    });
                }
            }
            _ => {}
        }

        Ok(issues)
    }

    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let generator = config
            .get_string("generator")
            .unwrap_or_else(|_| "sequence".to_string());

        match generator.as_str() {
            "sequence" => self.generate_sequence(config),
            "random" => self.generate_random(config),
            "uuid" => self.generate_uuids(config),
            "timestamp" => self.generate_timestamps(config),
            "range" => self.generate_range(config),
            "grid" => self.generate_grid(config),
            "fibonacci" => self.generate_fibonacci(config),
            "factorial" => self.generate_factorial(config),
            "prime" => self.generate_primes(config),
            _ => Err(anyhow!("Unsupported generator type: {}", generator)),
        }
    }

    fn available_variables(&self, config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        let generator = config
            .get_string("generator")
            .unwrap_or_else(|_| "sequence".to_string());

        let mut vars = vec![
            VariableDefinition {
                name: "generated_type".to_string(),
                var_type: VariableType::String,
                description: "Type of generated data".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "index".to_string(),
                var_type: VariableType::Number,
                description: "Index of the generated item".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
        ];

        match generator.as_str() {
            "sequence" | "range" => {
                vars.push(VariableDefinition {
                    name: "value".to_string(),
                    var_type: VariableType::Number,
                    description: "Generated sequence value".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            "random" => {
                vars.push(VariableDefinition {
                    name: "random_value".to_string(),
                    var_type: VariableType::Number,
                    description: "Random number value".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            "uuid" => {
                vars.push(VariableDefinition {
                    name: "uuid".to_string(),
                    var_type: VariableType::String,
                    description: "Generated UUID".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            "timestamp" => {
                vars.push(VariableDefinition {
                    name: "timestamp".to_string(),
                    var_type: VariableType::Number,
                    description: "Unix timestamp".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
                vars.push(VariableDefinition {
                    name: "datetime".to_string(),
                    var_type: VariableType::String,
                    description: "ISO 8601 datetime string".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            "grid" => {
                vars.push(VariableDefinition {
                    name: "x".to_string(),
                    var_type: VariableType::Number,
                    description: "X coordinate".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
                vars.push(VariableDefinition {
                    name: "y".to_string(),
                    var_type: VariableType::Number,
                    description: "Y coordinate".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
            _ => {
                vars.push(VariableDefinition {
                    name: "value".to_string(),
                    var_type: VariableType::Number,
                    description: "Generated value".to_string(),
                    required: true,
                    default_value: None,
                    validation_rules: vec![],
                });
            }
        }

        Ok(vars)
    }

    fn supports(&self, config: &InputConfig) -> bool {
        config.get_string("generator").is_ok()
            || config
                .get_string("input_type")
                .map(|t| t == "generated")
                .unwrap_or(false)
    }
}

impl GeneratedInputProvider {
    fn generate_sequence(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let start = config
            .get_string("start")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let end = config
            .get_string("end")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(start + 10);
        let step = config
            .get_string("step")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(1);

        let mut inputs = Vec::new();
        let mut current = start;
        let mut index = 0;

        while (step > 0 && current <= end) || (step < 0 && current >= end) {
            let mut input = ExecutionInput::new(
                format!("seq_{}", index),
                InputType::Generated {
                    generator: "sequence".to_string(),
                    config: serde_json::json!({
                        "start": start,
                        "end": end,
                        "step": step
                    }),
                },
            );

            input.add_variable("value".to_string(), VariableValue::Number(current));
            input.add_variable("index".to_string(), VariableValue::Number(index));
            input.add_variable(
                "generated_type".to_string(),
                VariableValue::String("sequence".to_string()),
            );

            inputs.push(input);
            current += step;
            index += 1;
        }

        Ok(inputs)
    }

    fn generate_random(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let count = config
            .get_string("count")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10);
        let min = config
            .get_string("min")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let max = config
            .get_string("max")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(100);

        let mut rng = rand::rng();
        let mut inputs = Vec::new();

        for i in 0..count {
            let value = rng.random_range(min..=max);

            let mut input = ExecutionInput::new(
                format!("random_{}", i),
                InputType::Generated {
                    generator: "random".to_string(),
                    config: serde_json::json!({
                        "min": min,
                        "max": max
                    }),
                },
            );

            input.add_variable("random_value".to_string(), VariableValue::Number(value));
            input.add_variable("index".to_string(), VariableValue::Number(i as i64));
            input.add_variable(
                "generated_type".to_string(),
                VariableValue::String("random".to_string()),
            );

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn generate_uuids(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let count = config
            .get_string("count")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(5);

        let mut inputs = Vec::new();

        for i in 0..count {
            let uuid = uuid::Uuid::new_v4();

            let mut input = ExecutionInput::new(
                format!("uuid_{}", i),
                InputType::Generated {
                    generator: "uuid".to_string(),
                    config: serde_json::json!({"version": 4}),
                },
            );

            input.add_variable("uuid".to_string(), VariableValue::String(uuid.to_string()));
            input.add_variable("index".to_string(), VariableValue::Number(i as i64));
            input.add_variable(
                "generated_type".to_string(),
                VariableValue::String("uuid".to_string()),
            );

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn generate_timestamps(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let count = config
            .get_string("count")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(5);
        let interval_seconds = config
            .get_string("interval")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(3600); // Default 1 hour intervals

        let mut inputs = Vec::new();
        let base_time = Utc::now();

        for i in 0..count {
            let timestamp = base_time + chrono::Duration::seconds(i as i64 * interval_seconds);

            let mut input = ExecutionInput::new(
                format!("timestamp_{}", i),
                InputType::Generated {
                    generator: "timestamp".to_string(),
                    config: serde_json::json!({
                        "interval": interval_seconds
                    }),
                },
            );

            input.add_variable(
                "timestamp".to_string(),
                VariableValue::Number(timestamp.timestamp()),
            );
            input.add_variable(
                "datetime".to_string(),
                VariableValue::String(timestamp.to_rfc3339()),
            );
            input.add_variable("index".to_string(), VariableValue::Number(i as i64));
            input.add_variable(
                "generated_type".to_string(),
                VariableValue::String("timestamp".to_string()),
            );

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn generate_range(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        // Similar to sequence but with float support
        let start = config
            .get_string("start")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let end = config
            .get_string("end")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0);
        let steps = config
            .get_string("steps")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10);

        let mut inputs = Vec::new();
        let step_size = (end - start) / (steps - 1) as f64;

        for i in 0..steps {
            let value = start + (i as f64 * step_size);

            let mut input = ExecutionInput::new(
                format!("range_{}", i),
                InputType::Generated {
                    generator: "range".to_string(),
                    config: serde_json::json!({
                        "start": start,
                        "end": end,
                        "steps": steps
                    }),
                },
            );

            input.add_variable("value".to_string(), VariableValue::Float(value));
            input.add_variable("index".to_string(), VariableValue::Number(i as i64));
            input.add_variable(
                "generated_type".to_string(),
                VariableValue::String("range".to_string()),
            );

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn generate_grid(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let width = config
            .get_string("width")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3);
        let height = config
            .get_string("height")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3);

        let mut inputs = Vec::new();
        let mut index = 0;

        for y in 0..height {
            for x in 0..width {
                let mut input = ExecutionInput::new(
                    format!("grid_{}_{}", x, y),
                    InputType::Generated {
                        generator: "grid".to_string(),
                        config: serde_json::json!({
                            "width": width,
                            "height": height
                        }),
                    },
                );

                input.add_variable("x".to_string(), VariableValue::Number(x as i64));
                input.add_variable("y".to_string(), VariableValue::Number(y as i64));
                input.add_variable("index".to_string(), VariableValue::Number(index));
                input.add_variable(
                    "generated_type".to_string(),
                    VariableValue::String("grid".to_string()),
                );

                inputs.push(input);
                index += 1;
            }
        }

        Ok(inputs)
    }

    fn generate_fibonacci(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let count = config
            .get_string("count")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10);

        let mut inputs = Vec::new();
        let mut a: i64 = 0;
        let mut b: i64 = 1;

        for i in 0..count {
            let value = if i == 0 {
                a
            } else if i == 1 {
                b
            } else {
                let next = a + b;
                a = b;
                b = next;
                next
            };

            let mut input = ExecutionInput::new(
                format!("fib_{}", i),
                InputType::Generated {
                    generator: "fibonacci".to_string(),
                    config: serde_json::json!({
                        "count": count
                    }),
                },
            );

            input.add_variable("value".to_string(), VariableValue::Number(value));
            input.add_variable("index".to_string(), VariableValue::Number(i as i64));
            input.add_variable(
                "generated_type".to_string(),
                VariableValue::String("fibonacci".to_string()),
            );

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn generate_factorial(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let count = config
            .get_string("count")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10);

        let mut inputs = Vec::new();

        for i in 0..count {
            let value = (1..=i as i64).product::<i64>().max(1);

            let mut input = ExecutionInput::new(
                format!("factorial_{}", i),
                InputType::Generated {
                    generator: "factorial".to_string(),
                    config: serde_json::json!({
                        "count": count
                    }),
                },
            );

            input.add_variable("value".to_string(), VariableValue::Number(value));
            input.add_variable("n".to_string(), VariableValue::Number(i as i64));
            input.add_variable("index".to_string(), VariableValue::Number(i as i64));
            input.add_variable(
                "generated_type".to_string(),
                VariableValue::String("factorial".to_string()),
            );

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn generate_primes(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let count = config
            .get_string("count")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10);

        let mut inputs = Vec::new();
        let mut primes = Vec::new();
        let mut num = 2;

        while primes.len() < count {
            if self.is_prime(num) {
                primes.push(num);
            }
            num += 1;
        }

        for (i, prime) in primes.iter().enumerate() {
            let mut input = ExecutionInput::new(
                format!("prime_{}", i),
                InputType::Generated {
                    generator: "prime".to_string(),
                    config: serde_json::json!({
                        "count": count
                    }),
                },
            );

            input.add_variable("value".to_string(), VariableValue::Number(*prime));
            input.add_variable("index".to_string(), VariableValue::Number(i as i64));
            input.add_variable(
                "generated_type".to_string(),
                VariableValue::String("prime".to_string()),
            );

            inputs.push(input);
        }

        Ok(inputs)
    }

    fn is_prime(&self, n: i64) -> bool {
        if n <= 1 {
            return false;
        }
        if n <= 3 {
            return true;
        }
        if n % 2 == 0 || n % 3 == 0 {
            return false;
        }
        let mut i = 5;
        while i * i <= n {
            if n % i == 0 || n % (i + 2) == 0 {
                return false;
            }
            i += 6;
        }
        true
    }
}
