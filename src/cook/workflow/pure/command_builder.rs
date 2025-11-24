//! Command building functions for workflow templates
//!
//! Provides pure functions for building command strings from templates:
//! - `build_command`: Build a command by expanding variables in a template
//!
//! # Examples
//!
//! ```
//! use prodigy::cook::workflow::pure::command_builder::build_command;
//! use std::collections::HashMap;
//!
//! let template = "echo ${name} ${value}";
//! let vars: HashMap<String, String> = [
//!     ("name".into(), "test".into()),
//!     ("value".into(), "123".into()),
//! ].iter().cloned().collect();
//!
//! let result = build_command(template, &vars);
//! assert_eq!(result, "echo test 123");
//! ```

use std::collections::HashMap;

use super::variable_expansion::expand_variables;

/// Pure: Build command string from template
///
/// This is the primary entry point for command building. It expands
/// all variables in the template using the provided variable map.
///
/// Variables not found in the map are left unexpanded, which allows
/// for multi-stage expansion where some variables may be resolved
/// at different stages of execution.
///
/// # Arguments
///
/// * `template` - The command template containing variable placeholders
/// * `variables` - A map of variable names to their values
///
/// # Returns
///
/// The command string with all found variables expanded
///
/// # Examples
///
/// ```
/// use prodigy::cook::workflow::pure::command_builder::build_command;
/// use std::collections::HashMap;
///
/// let vars: HashMap<String, String> = [
///     ("item".into(), "spec-01".into()),
///     ("output".into(), "result.txt".into()),
/// ].iter().cloned().collect();
///
/// let cmd = build_command("process ${item} > ${output}", &vars);
/// assert_eq!(cmd, "process spec-01 > result.txt");
/// ```
pub fn build_command(template: &str, variables: &HashMap<String, String>) -> String {
    expand_variables(template, variables)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_command() {
        let template = "echo ${name} ${value}";
        let vars: HashMap<String, String> = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_build_command_with_missing_vars() {
        let template = "echo ${exists} ${missing}";
        let vars: HashMap<String, String> = [("exists".into(), "value".into())]
            .iter()
            .cloned()
            .collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "echo value ${missing}");
    }

    #[test]
    fn test_build_command_empty_template() {
        let vars: HashMap<String, String> =
            [("name".into(), "test".into())].iter().cloned().collect();

        let result = build_command("", &vars);

        assert_eq!(result, "");
    }

    #[test]
    fn test_build_command_no_variables() {
        let vars: HashMap<String, String> = HashMap::new();

        let result = build_command("echo hello world", &vars);

        assert_eq!(result, "echo hello world");
    }

    #[test]
    fn test_build_command_simple_vars() {
        let template = "echo $name $value";
        let vars: HashMap<String, String> = [
            ("name".into(), "test".into()),
            ("value".into(), "123".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "echo test 123");
    }

    #[test]
    fn test_build_command_mixed_vars() {
        let template = "process ${item} with $mode";
        let vars: HashMap<String, String> = [
            ("item".into(), "data.json".into()),
            ("mode".into(), "verbose".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "process data.json with verbose");
    }

    #[test]
    fn test_build_command_with_paths() {
        let template = "cp ${src} ${dest}";
        let vars: HashMap<String, String> = [
            ("src".into(), "/home/user/file.txt".into()),
            ("dest".into(), "/tmp/backup/".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "cp /home/user/file.txt /tmp/backup/");
    }

    #[test]
    fn test_build_command_with_special_chars() {
        let template = "echo ${msg}";
        let vars: HashMap<String, String> = [("msg".into(), "Hello, World! @#$%".into())]
            .iter()
            .cloned()
            .collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "echo Hello, World! @#$%");
    }

    #[test]
    fn test_build_command_multi_expansion() {
        // Test that the same variable can appear multiple times
        let template = "${x} + ${x} = ${result}";
        let vars: HashMap<String, String> =
            [("x".into(), "2".into()), ("result".into(), "4".into())]
                .iter()
                .cloned()
                .collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "2 + 2 = 4");
    }

    #[test]
    fn test_build_command_iteration_context() {
        // Test typical workflow iteration variables
        let template = "/process-item ${item} --iteration ${ITERATION}";
        let vars: HashMap<String, String> = [
            ("item".into(), "spec-42".into()),
            ("ITERATION".into(), "3".into()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = build_command(template, &vars);

        assert_eq!(result, "/process-item spec-42 --iteration 3");
    }
}
