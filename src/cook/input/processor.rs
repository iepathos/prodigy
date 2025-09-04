use super::arguments::ArgumentsInputProvider;
use super::config::{
    CachingConfig, FilterType, InputConfig, InputFilter, InputSource, MergeStrategy, SortConfig,
    TransformationConfig,
};
use super::file_pattern::FilePatternInputProvider;
use super::provider::{InputProvider, ValidationIssue, ValidationSeverity};
use super::types::{ExecutionInput, VariableValue};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
struct CachedInput {
    inputs: Vec<ExecutionInput>,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

pub struct InputProcessor {
    providers: HashMap<String, Box<dyn InputProvider>>,
    cache: Arc<RwLock<HashMap<String, CachedInput>>>,
}

impl InputProcessor {
    pub fn new() -> Self {
        let mut providers: HashMap<String, Box<dyn InputProvider>> = HashMap::new();

        providers.insert("arguments".to_string(), Box::new(ArgumentsInputProvider));
        providers.insert(
            "file_pattern".to_string(),
            Box::new(FilePatternInputProvider::new()),
        );

        Self {
            providers,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn process_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let mut all_inputs = Vec::new();

        for source in &config.sources {
            let inputs = self.process_input_source(source, config).await?;
            all_inputs.extend(inputs);
        }

        // Apply transformations
        let transformed_inputs = self
            .apply_transformations(&all_inputs, &config.transformation)
            .await?;

        // Apply validation
        self.validate_inputs(&transformed_inputs, &config.validation)
            .await?;

        Ok(transformed_inputs)
    }

    fn process_input_source<'a>(
        &'a self,
        source: &'a InputSource,
        config: &'a InputConfig,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<ExecutionInput>>> + 'a>> {
        Box::pin(async move {
            match source {
                InputSource::Empty => Ok(vec![ExecutionInput::new(
                    "empty".to_string(),
                    super::types::InputType::Empty,
                )]),
                InputSource::Arguments {
                    value,
                    separator,
                    validation: _,
                } => {
                    let provider = self
                        .providers
                        .get("arguments")
                        .ok_or_else(|| anyhow::anyhow!("Arguments provider not found"))?;

                    let mut source_config = super::provider::InputConfig::new();
                    source_config.set("args".to_string(), json!(value));
                    if let Some(sep) = separator {
                        source_config.set("separator".to_string(), json!(sep));
                    }

                    provider.generate_inputs(&source_config).await
                }
                InputSource::FilePattern {
                    patterns,
                    recursive,
                    filters: _,
                } => {
                    let provider = self
                        .providers
                        .get("file_pattern")
                        .ok_or_else(|| anyhow::anyhow!("File pattern provider not found"))?;

                    let mut source_config = super::provider::InputConfig::new();
                    source_config.set("patterns".to_string(), json!(patterns));
                    source_config.set("recursive".to_string(), json!(recursive));

                    provider.generate_inputs(&source_config).await
                }
                InputSource::Composite {
                    sources,
                    merge_strategy,
                } => {
                    self.process_composite_source(sources, merge_strategy, config)
                        .await
                }
                _ => {
                    // Other input types not implemented yet
                    Ok(vec![])
                }
            }
        })
    }

    async fn process_composite_source(
        &self,
        sources: &[InputSource],
        merge_strategy: &MergeStrategy,
        config: &InputConfig,
    ) -> Result<Vec<ExecutionInput>> {
        let mut all_inputs = Vec::new();

        for source in sources {
            let inputs = self.process_input_source(source, config).await?;
            all_inputs.push(inputs);
        }

        // Apply merge strategy
        match merge_strategy {
            MergeStrategy::Sequential => Ok(all_inputs.into_iter().flatten().collect()),
            MergeStrategy::Interleaved => {
                let mut result = Vec::new();
                let max_len = all_inputs.iter().map(|v| v.len()).max().unwrap_or(0);

                for i in 0..max_len {
                    for input_vec in &all_inputs {
                        if i < input_vec.len() {
                            result.push(input_vec[i].clone());
                        }
                    }
                }
                Ok(result)
            }
            MergeStrategy::Grouped => {
                // Groups are kept as separate batches
                Ok(all_inputs.into_iter().flatten().collect())
            }
            MergeStrategy::Custom { handler: _ } => {
                // Custom merge strategy would be implemented here
                Ok(all_inputs.into_iter().flatten().collect())
            }
        }
    }

    async fn apply_transformations(
        &self,
        inputs: &[ExecutionInput],
        config: &TransformationConfig,
    ) -> Result<Vec<ExecutionInput>> {
        let mut transformed = inputs.to_vec();

        // Apply variable transformations
        for input in &mut transformed {
            for (var_name, transformation) in &config.variable_transformations {
                if let Some(value) = input.variables.get(var_name) {
                    let transformed_value = self.apply_transformation(value, transformation)?;
                    input.variables.insert(var_name.clone(), transformed_value);
                }
            }
        }

        // Apply input filters
        for filter in &config.input_filters {
            transformed = self.apply_input_filter(transformed, filter)?;
        }

        // Apply sorting if configured
        if let Some(sort_config) = &config.sorting {
            transformed = self.sort_inputs(transformed, sort_config)?;
        }

        Ok(transformed)
    }

    fn apply_transformation(
        &self,
        value: &VariableValue,
        transformation: &str,
    ) -> Result<VariableValue> {
        // Simple transformations for now
        match transformation {
            "uppercase" => Ok(VariableValue::String(value.to_string().to_uppercase())),
            "lowercase" => Ok(VariableValue::String(value.to_string().to_lowercase())),
            "trim" => Ok(VariableValue::String(value.to_string().trim().to_string())),
            _ => Ok(value.clone()),
        }
    }

    fn apply_input_filter(
        &self,
        inputs: Vec<ExecutionInput>,
        filter: &InputFilter,
    ) -> Result<Vec<ExecutionInput>> {
        match &filter.filter_type {
            FilterType::Include { pattern } => {
                let re = regex::Regex::new(pattern)?;
                Ok(inputs
                    .into_iter()
                    .filter(|input| {
                        input
                            .variables
                            .values()
                            .any(|v| re.is_match(&v.to_string()))
                    })
                    .collect())
            }
            FilterType::Exclude { pattern } => {
                let re = regex::Regex::new(pattern)?;
                Ok(inputs
                    .into_iter()
                    .filter(|input| {
                        !input
                            .variables
                            .values()
                            .any(|v| re.is_match(&v.to_string()))
                    })
                    .collect())
            }
            FilterType::Custom { name: _ } => {
                // Custom filters would be implemented here
                Ok(inputs)
            }
        }
    }

    fn sort_inputs(
        &self,
        mut inputs: Vec<ExecutionInput>,
        config: &SortConfig,
    ) -> Result<Vec<ExecutionInput>> {
        inputs.sort_by(|a, b| {
            let a_val = a.variables.get(&config.field).map(|v| v.to_string());
            let b_val = b.variables.get(&config.field).map(|v| v.to_string());

            match (a_val, b_val) {
                (Some(a), Some(b)) => {
                    if config.numeric {
                        a.parse::<f64>()
                            .unwrap_or(0.0)
                            .partial_cmp(&b.parse::<f64>().unwrap_or(0.0))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.cmp(&b)
                    }
                }
                _ => std::cmp::Ordering::Equal,
            }
        });

        if !config.ascending {
            inputs.reverse();
        }

        Ok(inputs)
    }

    async fn validate_inputs(
        &self,
        inputs: &[ExecutionInput],
        config: &super::config::ValidationConfig,
    ) -> Result<()> {
        let mut all_issues: Vec<ValidationIssue> = Vec::new();

        for _input in inputs {
            // Basic validation - check required variables exist
            // More complex validation would be added here
        }

        if config.strict && !all_issues.is_empty() {
            let errors: Vec<_> = all_issues
                .iter()
                .filter(|i| matches!(i.severity, ValidationSeverity::Error))
                .collect();

            if !errors.is_empty() {
                return Err(anyhow::anyhow!(
                    "Validation failed with {} errors",
                    errors.len()
                ));
            }
        }

        Ok(())
    }

    async fn check_cache(
        &self,
        config: &super::provider::InputConfig,
        cache_config: &CachingConfig,
    ) -> Result<Option<CachedInput>> {
        if !cache_config.enabled {
            return Ok(None);
        }

        let cache_key = self.generate_cache_key(config, cache_config)?;
        let cache = self.cache.read().await;

        if let Some(cached) = cache.get(&cache_key) {
            if let Some(expires_at) = cached.expires_at {
                if Utc::now() < expires_at {
                    return Ok(Some(cached.clone()));
                }
            }
        }

        Ok(None)
    }

    async fn store_in_cache(
        &self,
        config: &super::provider::InputConfig,
        inputs: &[ExecutionInput],
        cache_config: &CachingConfig,
    ) -> Result<()> {
        if !cache_config.enabled {
            return Ok(());
        }

        let cache_key = self.generate_cache_key(config, cache_config)?;
        let expires_at = cache_config
            .ttl_seconds
            .map(|ttl| Utc::now() + chrono::Duration::seconds(ttl as i64));

        let cached = CachedInput {
            inputs: inputs.to_vec(),
            created_at: Utc::now(),
            expires_at,
        };

        let mut cache = self.cache.write().await;
        cache.insert(cache_key, cached);

        Ok(())
    }

    fn generate_cache_key(
        &self,
        _config: &super::provider::InputConfig,
        cache_config: &CachingConfig,
    ) -> Result<String> {
        if let Some(template) = &cache_config.cache_key_template {
            // Use template for cache key
            Ok(template.clone())
        } else {
            // Generate default cache key
            Ok(format!("input_cache_{}", Utc::now().timestamp()))
        }
    }
}
