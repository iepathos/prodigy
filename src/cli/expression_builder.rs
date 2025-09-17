//! Expression builder CLI tool for testing and validating expressions

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use crate::cook::execution::data_pipeline::{DataPipeline, FilterExpression, Sorter};

/// Expression builder CLI commands
#[derive(Debug, Parser)]
#[clap(name = "expression-builder")]
pub struct ExpressionBuilderCommand {
    #[clap(subcommand)]
    pub command: ExpressionCommands,
}

#[derive(Debug, Subcommand)]
pub enum ExpressionCommands {
    /// Test a filter expression against sample data
    Filter {
        /// Filter expression to test
        #[clap(help = "Filter expression (e.g., 'priority > 5 && status == \"active\"')")]
        expression: String,

        /// Sample JSON data (or @file.json to read from file)
        #[clap(short, long, help = "JSON data to test against")]
        data: Option<String>,

        /// Verbose output showing evaluation steps
        #[clap(short, long)]
        verbose: bool,
    },

    /// Test a sort expression against sample data
    Sort {
        /// Sort expression to test
        #[clap(help = "Sort expression (e.g., 'priority DESC, name ASC')")]
        expression: String,

        /// Sample JSON data (or @file.json to read from file)
        #[clap(short, long, help = "JSON array to sort")]
        data: Option<String>,

        /// Verbose output showing sort process
        #[clap(short, long)]
        verbose: bool,
    },

    /// Test a complete pipeline (JSON path, filter, sort)
    Pipeline {
        /// JSON path expression
        #[clap(short = 'j', long)]
        json_path: Option<String>,

        /// Filter expression
        #[clap(short = 'f', long)]
        filter: Option<String>,

        /// Sort expression
        #[clap(short = 's', long)]
        sort_by: Option<String>,

        /// Maximum items to return
        #[clap(short = 'l', long)]
        limit: Option<usize>,

        /// Sample JSON data (or @file.json to read from file)
        #[clap(short, long, help = "JSON data to process")]
        data: Option<String>,

        /// Verbose output
        #[clap(short, long)]
        verbose: bool,
    },

    /// Validate an expression without executing it
    Validate {
        /// Expression to validate
        expression: String,

        /// Expression type (filter or sort)
        #[clap(short = 't', long, default_value = "filter")]
        expr_type: String,
    },

    /// Show examples of common expressions
    Examples {
        /// Category of examples to show
        #[clap(short, long)]
        category: Option<String>,
    },

    /// Analyze an expression and show metadata
    Analyze {
        /// Expression to analyze
        expression: String,
    },
}

impl ExpressionBuilderCommand {
    /// Execute the expression builder command
    pub async fn execute(&self) -> Result<()> {
        match &self.command {
            ExpressionCommands::Filter {
                expression,
                data,
                verbose,
            } => self.test_filter(expression, data.as_deref(), *verbose),
            ExpressionCommands::Sort {
                expression,
                data,
                verbose,
            } => self.test_sort(expression, data.as_deref(), *verbose),
            ExpressionCommands::Pipeline {
                json_path,
                filter,
                sort_by,
                limit,
                data,
                verbose,
            } => {
                self.test_pipeline(
                    json_path.as_deref(),
                    filter.as_deref(),
                    sort_by.as_deref(),
                    *limit,
                    data.as_deref(),
                    *verbose,
                )
            }
            ExpressionCommands::Validate {
                expression,
                expr_type,
            } => self.validate_expression(expression, expr_type),
            ExpressionCommands::Examples { category } => self.show_examples(category.as_deref()),
            ExpressionCommands::Analyze { expression } => self.analyze_expression(expression),
        }
    }

    /// Test a filter expression
    fn test_filter(&self, expression: &str, data: Option<&str>, verbose: bool) -> Result<()> {
        let filter = FilterExpression::parse(expression)?;

        // Get test data
        let items = self.get_test_data(data)?;

        if verbose {
            println!("Filter expression: {}", expression);
            println!("Input data: {} items", items.len());
            println!();
        }

        // Apply filter
        let mut matched = 0;
        let mut results = Vec::new();

        for (i, item) in items.iter().enumerate() {
            let matches = filter.evaluate(item);
            if matches {
                matched += 1;
                results.push(item.clone());
            }

            if verbose {
                println!(
                    "Item {}: {} => {}",
                    i,
                    serde_json::to_string(item)?,
                    if matches { "MATCH" } else { "SKIP" }
                );
            }
        }

        println!("\n=== Results ===");
        println!("Matched: {}/{} items", matched, items.len());

        if !results.is_empty() {
            println!("\nMatched items:");
            for item in results {
                println!("{}", serde_json::to_string_pretty(&item)?);
            }
        }

        Ok(())
    }

    /// Test a sort expression
    fn test_sort(&self, expression: &str, data: Option<&str>, verbose: bool) -> Result<()> {
        let sorter = Sorter::parse(expression)?;

        // Get test data
        let mut items = self.get_test_data(data)?;

        if verbose {
            println!("Sort expression: {}", expression);
            println!("Input data: {} items", items.len());
            println!("\nBefore sorting:");
            for (i, item) in items.iter().enumerate() {
                println!("{}: {}", i, serde_json::to_string(item)?);
            }
        }

        // Apply sort
        sorter.sort(&mut items);

        println!("\n=== Results ===");
        println!("Sorted {} items", items.len());
        println!("\nAfter sorting:");
        for (i, item) in items.iter().enumerate() {
            println!("{}: {}", i, serde_json::to_string_pretty(&item)?);
        }

        Ok(())
    }

    /// Test a complete pipeline
    fn test_pipeline(
        &self,
        json_path: Option<&str>,
        filter: Option<&str>,
        sort_by: Option<&str>,
        limit: Option<usize>,
        data: Option<&str>,
        verbose: bool,
    ) -> Result<()> {
        let pipeline = DataPipeline::from_config(
            json_path.map(String::from),
            filter.map(String::from),
            sort_by.map(String::from),
            limit,
        )?;

        // Get test data
        let input = self.get_single_test_data(data)?;

        if verbose {
            println!("Pipeline configuration:");
            if let Some(path) = json_path {
                println!("  JSON path: {}", path);
            }
            if let Some(f) = filter {
                println!("  Filter: {}", f);
            }
            if let Some(s) = sort_by {
                println!("  Sort: {}", s);
            }
            if let Some(l) = limit {
                println!("  Limit: {}", l);
            }
            println!();
        }

        // Process through pipeline
        let results = pipeline.process(&input)?;

        println!("=== Results ===");
        println!("Output: {} items", results.len());
        println!();
        for (i, item) in results.iter().enumerate() {
            println!("Item {}:", i);
            println!("{}", serde_json::to_string_pretty(&item)?);
        }

        Ok(())
    }

    /// Validate an expression
    fn validate_expression(&self, expression: &str, expr_type: &str) -> Result<()> {
        match expr_type {
            "filter" => {
                let filter = FilterExpression::parse(expression)?;
                println!("✓ Valid filter expression");
                println!("Expression: {:?}", filter);
            }
            "sort" => {
                let sorter = Sorter::parse(expression)?;
                println!("✓ Valid sort expression");
                println!("Sort keys: {:?}", sorter.fields);
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown expression type: {}", expr_type));
            }
        }

        Ok(())
    }

    /// Show expression examples
    fn show_examples(&self, category: Option<&str>) -> Result<()> {
        match category {
            Some("filter") | None => {
                println!("=== Filter Expression Examples ===\n");
                println!("Basic comparisons:");
                println!("  priority > 5");
                println!("  status == 'active'");
                println!("  score >= 7.5");
                println!();
                println!("Logical operators:");
                println!("  priority > 5 && status == 'active'");
                println!("  severity == 'high' || severity == 'critical'");
                println!("  !(status == 'archived')");
                println!();
                println!("Nested fields:");
                println!("  user.profile.age >= 18");
                println!("  location.coordinates.lat > 40.0");
                println!();
                println!("String operations:");
                println!("  contains(name, 'test')");
                println!("  starts_with(path, 'src/')");
                println!("  ends_with(file, '.rs')");
                println!();
                println!("Type checking:");
                println!("  is_null(optional_field)");
                println!("  is_not_null(required_field)");
                println!("  is_number(score)");
                println!("  is_string(name)");
                println!();
                println!("IN operator:");
                println!("  severity in ['high', 'critical']");
                println!("  status in ['pending', 'in_progress']");
            }
            Some("sort") => {
                println!("=== Sort Expression Examples ===\n");
                println!("Single field:");
                println!("  priority DESC");
                println!("  name ASC");
                println!("  created_at");
                println!();
                println!("Multiple fields:");
                println!("  severity DESC, priority ASC");
                println!("  category, score DESC, name");
                println!();
                println!("Nested fields:");
                println!("  user.profile.score DESC");
                println!("  location.city ASC");
                println!();
                println!("Null handling:");
                println!("  score DESC NULLS LAST");
                println!("  optional_field ASC NULLS FIRST");
            }
            Some("pipeline") => {
                println!("=== Pipeline Examples ===\n");
                println!("Extract and filter:");
                println!("  JSON path: $.items[*]");
                println!("  Filter: priority > 5");
                println!();
                println!("Full pipeline:");
                println!("  JSON path: $.data.records[*]");
                println!("  Filter: status == 'active' && score >= 7.0");
                println!("  Sort: score DESC, name ASC");
                println!("  Limit: 10");
            }
            Some(other) => {
                println!("Unknown category: {}", other);
                println!("Available categories: filter, sort, pipeline");
            }
        }

        Ok(())
    }

    /// Analyze an expression
    fn analyze_expression(&self, expression: &str) -> Result<()> {
        // Try parsing as filter first
        if let Ok(filter) = FilterExpression::parse(expression) {
            println!("Expression type: Filter");
            println!("Parsed AST: {:#?}", filter);

            // Estimate complexity
            let complexity = self.estimate_filter_complexity(&filter);
            println!("\nEstimated complexity: {}", complexity);

            // Check if indexable (simple field comparisons)
            let indexable = matches!(
                filter,
                FilterExpression::Comparison { .. } | FilterExpression::In { .. }
            );
            println!("Indexable: {}", indexable);

            return Ok(());
        }

        // Try parsing as sort
        if let Ok(sorter) = Sorter::parse(expression) {
            println!("Expression type: Sort");
            println!("Sort fields:");
            for field in &sorter.fields {
                println!(
                    "  - {} ({:?}, nulls {:?})",
                    field.path, field.order, field.null_position
                );
            }
            return Ok(());
        }

        Err(anyhow::anyhow!("Could not parse expression"))
    }

    /// Get test data from string or default
    fn get_test_data(&self, data: Option<&str>) -> Result<Vec<Value>> {
        if let Some(data_str) = data {
            // Check if it's a file reference
            if data_str.starts_with('@') {
                let path = &data_str[1..];
                let content = std::fs::read_to_string(path)?;
                let value: Value = serde_json::from_str(&content)?;
                return self.extract_array(value);
            }

            // Parse as JSON
            let value: Value = serde_json::from_str(data_str)?;
            self.extract_array(value)
        } else {
            // Default test data
            Ok(vec![
                json!({"id": 1, "priority": 3, "status": "active", "name": "Item A"}),
                json!({"id": 2, "priority": 7, "status": "pending", "name": "Item B"}),
                json!({"id": 3, "priority": 5, "status": "active", "name": "Item C"}),
                json!({"id": 4, "priority": 9, "status": "archived", "name": "Item D"}),
                json!({"id": 5, "priority": 2, "status": "active", "name": "Item E"}),
            ])
        }
    }

    /// Get single test data object
    fn get_single_test_data(&self, data: Option<&str>) -> Result<Value> {
        if let Some(data_str) = data {
            // Check if it's a file reference
            if data_str.starts_with('@') {
                let path = &data_str[1..];
                let content = std::fs::read_to_string(path)?;
                return Ok(serde_json::from_str(&content)?);
            }

            // Parse as JSON
            Ok(serde_json::from_str(data_str)?)
        } else {
            // Default test data
            Ok(json!({
                "items": [
                    {"id": 1, "priority": 3, "status": "active"},
                    {"id": 2, "priority": 7, "status": "pending"},
                    {"id": 3, "priority": 5, "status": "active"},
                ]
            }))
        }
    }

    /// Extract array from JSON value
    fn extract_array(&self, value: Value) -> Result<Vec<Value>> {
        match value {
            Value::Array(arr) => Ok(arr),
            single => Ok(vec![single]),
        }
    }

    /// Estimate filter complexity
    fn estimate_filter_complexity(&self, filter: &FilterExpression) -> u32 {
        match filter {
            FilterExpression::Comparison { .. } => 1,
            FilterExpression::Logical { operands, .. } => {
                operands.iter().map(|f| self.estimate_filter_complexity(f)).sum::<u32>() + 1
            }
            FilterExpression::Function { .. } => 3,
            FilterExpression::In { values, .. } => 1 + values.len() as u32,
        }
    }
}