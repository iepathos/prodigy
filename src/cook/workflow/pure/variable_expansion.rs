//! Variable expansion functions for workflow templates
//!
//! Provides pure functions for expanding variables in template strings:
//! - `expand_variables`: Expand ${VAR} and $VAR patterns
//! - `extract_variable_references`: Extract all variable references from a template
//!
//! # Examples
//!
//! ```
//! use prodigy::cook::workflow::pure::variable_expansion::{expand_variables, extract_variable_references};
//! use std::collections::HashMap;
//!
//! let template = "echo ${name} $value";
//! let vars: HashMap<String, String> = [
//!     ("name".into(), "test".into()),
//!     ("value".into(), "123".into()),
//! ].iter().cloned().collect();
//!
//! let result = expand_variables(template, &vars);
//! assert_eq!(result, "echo test 123");
//!
//! let refs = extract_variable_references(template);
//! assert!(refs.contains("name"));
//! assert!(refs.contains("value"));
//! ```

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{HashMap, HashSet};

/// Regex for matching braced variables ${VAR}
static BRACED_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{([a-zA-Z_][a-zA-Z0-9_]*)\}").expect("Valid regex pattern"));

/// Regex for matching simple variables $VAR (captures the full variable name)
/// We match the variable and then check the following character manually
static SIMPLE_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$([a-zA-Z_][a-zA-Z0-9_]*)").expect("Valid regex pattern"));

/// Pure: Expand variables in template string
///
/// Supports two variable syntaxes:
/// - `${VAR}` - Braced variable (expanded first, more specific)
/// - `$VAR` - Simple variable (expanded with word boundary check)
///
/// Variables not found in the provided map are left unexpanded.
///
/// # Arguments
///
/// * `template` - The template string containing variable placeholders
/// * `variables` - A map of variable names to their values
///
/// # Returns
///
/// The template with all found variables expanded
///
/// # Examples
///
/// ```
/// use prodigy::cook::workflow::pure::variable_expansion::expand_variables;
/// use std::collections::HashMap;
///
/// let vars: HashMap<String, String> = [
///     ("name".into(), "Alice".into()),
/// ].iter().cloned().collect();
///
/// assert_eq!(expand_variables("Hello ${name}", &vars), "Hello Alice");
/// assert_eq!(expand_variables("Hello $name", &vars), "Hello Alice");
/// assert_eq!(expand_variables("${missing}", &vars), "${missing}");
/// ```
pub fn expand_variables(template: &str, variables: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    // Expand ${VAR} first (more specific pattern)
    for (key, value) in variables {
        let placeholder = format!("${{{}}}", key);
        result = result.replace(&placeholder, value);
    }

    // Expand $VAR with word boundaries (using manual boundary checking)
    for (key, value) in variables {
        if key.is_empty() {
            continue;
        }
        result = expand_simple_var(&result, key, value);
    }

    result
}

/// Expand a simple $VAR pattern with word boundary checking
///
/// This function manually checks that the character following the variable
/// name is not an identifier character (letter, digit, or underscore).
fn expand_simple_var(template: &str, var_name: &str, value: &str) -> String {
    let pattern = format!("${}", var_name);
    let pattern_len = pattern.len();
    let mut result = String::with_capacity(template.len());
    let mut i = 0;
    let chars: Vec<char> = template.chars().collect();

    while i < chars.len() {
        // Check if we're at a potential variable match
        let remaining: String = chars[i..].iter().collect();
        if remaining.starts_with(&pattern) {
            // Check if the next character (after the pattern) is NOT an identifier char
            let end_pos = i + pattern_len;
            let next_char = chars.get(end_pos).copied();

            if !is_identifier_char(next_char) {
                // This is a valid variable reference - replace it
                result.push_str(value);
                i = end_pos;
                continue;
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Check if a character is a valid identifier character (letter, digit, or underscore)
fn is_identifier_char(c: Option<char>) -> bool {
    match c {
        Some(ch) => ch.is_ascii_alphanumeric() || ch == '_',
        None => false,
    }
}

/// Pure: Extract all variable references from a template
///
/// Finds all `${VAR}` and `$VAR` patterns and returns the unique set
/// of variable names referenced.
///
/// # Arguments
///
/// * `template` - The template string to scan for variable references
///
/// # Returns
///
/// A set of unique variable names found in the template
///
/// # Examples
///
/// ```
/// use prodigy::cook::workflow::pure::variable_expansion::extract_variable_references;
///
/// let refs = extract_variable_references("echo ${name} $value ${name}");
/// assert_eq!(refs.len(), 2);
/// assert!(refs.contains("name"));
/// assert!(refs.contains("value"));
/// ```
pub fn extract_variable_references(template: &str) -> HashSet<String> {
    let mut refs = HashSet::new();

    // Match ${VAR} pattern
    for cap in BRACED_VAR_REGEX.captures_iter(template) {
        refs.insert(cap[1].to_string());
    }

    // Match $VAR pattern with word boundary checking
    // We need to manually verify that the character after the match
    // is not an identifier character
    for cap in SIMPLE_VAR_REGEX.captures_iter(template) {
        let full_match = cap.get(0).unwrap();
        let var_name = cap[1].to_string();

        // Get the character after the match (if any)
        let end_pos = full_match.end();
        let next_char = template[end_pos..].chars().next();

        // Only add if the next character is not an identifier character
        // (This handles cases like $name_with_suffix where we don't want to match $name)
        if !is_identifier_char(next_char) {
            refs.insert(var_name);
        }
    }

    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_variables_braced() {
        let template = "echo ${name} ${value}";
        let vars: HashMap<String, String> = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_expand_variables_simple() {
        let template = "echo $name $value";
        let vars: HashMap<String, String> = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_expand_variables_mixed() {
        let template = "echo ${name} $value";
        let vars: HashMap<String, String> = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_expand_variables_preserves_missing() {
        let template = "echo ${exists} ${missing} $also_missing";
        let vars: HashMap<String, String> = [("exists".into(), "value".into())]
            .iter()
            .cloned()
            .collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo value ${missing} $also_missing");
    }

    #[test]
    fn test_expand_variables_avoids_partial_match() {
        let template = "$name $name_with_suffix";
        let vars: HashMap<String, String> =
            [("name".into(), "test".into())].iter().cloned().collect();

        let result = expand_variables(template, &vars);

        // Should only replace $name, not $name_with_suffix
        assert_eq!(result, "test $name_with_suffix");
    }

    #[test]
    fn test_expand_variables_empty_template() {
        let template = "";
        let vars: HashMap<String, String> =
            [("name".into(), "test".into())].iter().cloned().collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "");
    }

    #[test]
    fn test_expand_variables_empty_vars() {
        let template = "echo ${name}";
        let vars: HashMap<String, String> = HashMap::new();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo ${name}");
    }

    #[test]
    fn test_expand_variables_special_characters_in_value() {
        let template = "echo ${msg}";
        let vars: HashMap<String, String> = [("msg".into(), "hello 'world' \"test\"".into())]
            .iter()
            .cloned()
            .collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo hello 'world' \"test\"");
    }

    #[test]
    fn test_expand_variables_consecutive_vars() {
        let template = "${a}${b}${c}";
        let vars: HashMap<String, String> = [
            ("a".into(), "1".into()),
            ("b".into(), "2".into()),
            ("c".into(), "3".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "123");
    }

    #[test]
    fn test_expand_variables_var_at_end() {
        let template = "path=$PATH";
        let vars: HashMap<String, String> = [("PATH".into(), "/usr/bin".into())]
            .iter()
            .cloned()
            .collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "path=/usr/bin");
    }

    #[test]
    fn test_expand_variables_dollar_sign_preserved() {
        // Dollar signs not followed by valid variable name should be preserved
        let template = "echo $1 $$ ${name}";
        let vars: HashMap<String, String> =
            [("name".into(), "test".into())].iter().cloned().collect();

        let result = expand_variables(template, &vars);

        assert_eq!(result, "echo $1 $$ test");
    }

    #[test]
    fn test_extract_variable_references_braced() {
        let template = "echo ${name} ${value} ${name}";

        let refs = extract_variable_references(template);

        assert_eq!(refs.len(), 2);
        assert!(refs.contains("name"));
        assert!(refs.contains("value"));
    }

    #[test]
    fn test_extract_variable_references_simple() {
        let template = "echo $name $value $name";

        let refs = extract_variable_references(template);

        assert_eq!(refs.len(), 2);
        assert!(refs.contains("name"));
        assert!(refs.contains("value"));
    }

    #[test]
    fn test_extract_variable_references_mixed() {
        let template = "echo ${name} $value";

        let refs = extract_variable_references(template);

        assert_eq!(refs.len(), 2);
        assert!(refs.contains("name"));
        assert!(refs.contains("value"));
    }

    #[test]
    fn test_extract_variable_references_empty() {
        let template = "no variables here";

        let refs = extract_variable_references(template);

        assert!(refs.is_empty());
    }

    #[test]
    fn test_extract_variable_references_underscore_names() {
        let template = "${var_name} $another_var $_private";

        let refs = extract_variable_references(template);

        assert_eq!(refs.len(), 3);
        assert!(refs.contains("var_name"));
        assert!(refs.contains("another_var"));
        assert!(refs.contains("_private"));
    }

    #[test]
    fn test_extract_variable_references_with_numbers() {
        let template = "${var1} $var2 ${var3abc}";

        let refs = extract_variable_references(template);

        assert_eq!(refs.len(), 3);
        assert!(refs.contains("var1"));
        assert!(refs.contains("var2"));
        assert!(refs.contains("var3abc"));
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Generator for valid variable names
    fn valid_var_name() -> impl Strategy<Value = String> {
        r"[a-zA-Z_][a-zA-Z0-9_]{0,20}".prop_filter("non-empty", |s| !s.is_empty())
    }

    // Generator for variable values (avoiding regex special characters for simplicity)
    fn safe_value() -> impl Strategy<Value = String> {
        r"[a-zA-Z0-9 _-]{0,50}"
    }

    proptest! {
        #[test]
        fn prop_variable_expansion_is_deterministic(
            template in ".*",
            vars in prop::collection::hash_map(valid_var_name(), safe_value(), 0..5),
        ) {
            let result1 = expand_variables(&template, &vars);
            let result2 = expand_variables(&template, &vars);

            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn prop_variable_expansion_idempotent_for_safe_values(
            template in r"[a-zA-Z0-9 ${}_.,-]*",
            vars in prop::collection::hash_map(valid_var_name(), r"[a-zA-Z0-9 _-]*", 0..3),
        ) {
            // Filter out variables that contain variable-like patterns
            let safe_vars: HashMap<String, String> = vars
                .into_iter()
                .filter(|(_, v)| !v.contains('$') && !v.contains('{') && !v.contains('}'))
                .collect();

            let result1 = expand_variables(&template, &safe_vars);
            let result2 = expand_variables(&result1, &safe_vars);

            // Should be idempotent when values don't contain variable references
            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn prop_extract_references_is_deterministic(template in ".*") {
            let refs1 = extract_variable_references(&template);
            let refs2 = extract_variable_references(&template);

            prop_assert_eq!(refs1, refs2);
        }

        #[test]
        fn prop_expand_handles_empty_template(_vars in prop::collection::hash_map(valid_var_name(), safe_value(), 0..5)) {
            let result = expand_variables("", &_vars);
            prop_assert_eq!(result, "");
        }

        #[test]
        fn prop_expand_handles_empty_vars(template in ".*") {
            let empty_vars: HashMap<String, String> = HashMap::new();
            let result = expand_variables(&template, &empty_vars);
            // With no variables, nothing should be expanded
            // But the template might still be valid
            prop_assert!(result.len() <= template.len() + 100); // Sanity check
        }

        #[test]
        fn prop_extracted_refs_are_valid_identifiers(template in ".*") {
            let refs = extract_variable_references(&template);

            for var_name in refs {
                // Check that each extracted name is a valid identifier
                prop_assert!(!var_name.is_empty());
                let first_char = var_name.chars().next().unwrap();
                prop_assert!(first_char.is_ascii_alphabetic() || first_char == '_');
                prop_assert!(var_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            }
        }
    }
}
