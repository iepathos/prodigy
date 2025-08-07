//! Custom test assertions for complex validations
#![allow(clippy::uninlined_format_args)]

use anyhow::Result;
use std::path::Path;

/// Assert that a result is Ok and return the value
#[macro_export]
macro_rules! assert_ok {
    ($result:expr) => {{
        match $result {
            Ok(val) => val,
            Err(e) => panic!("Expected Ok, got Err: {:?}", e),
        }
    }};
}

/// Assert that a result is Err and return the error
#[macro_export]
macro_rules! assert_err {
    ($result:expr) => {{
        match $result {
            Ok(val) => panic!("Expected Err, got Ok: {:?}", val),
            Err(e) => e,
        }
    }};
}

/// Assert that an error message contains a specific string
#[macro_export]
macro_rules! assert_error_contains {
    ($result:expr, $substring:expr) => {{
        let err = assert_err!($result);
        let err_str = err.to_string();
        assert!(
            err_str.contains($substring),
            "Error '{}' does not contain '{}'",
            err_str,
            $substring
        );
    }};
}

/// Assert that a file exists
pub fn assert_file_exists(path: &Path) {
    assert!(
        path.exists() && path.is_file(),
        "File does not exist: {:?}",
        path
    );
}

/// Assert that a directory exists
pub fn assert_dir_exists(path: &Path) {
    assert!(
        path.exists() && path.is_dir(),
        "Directory does not exist: {:?}",
        path
    );
}

/// Assert that a file contains specific content
pub fn assert_file_contains(path: &Path, expected: &str) -> Result<()> {
    assert_file_exists(path);
    let content = std::fs::read_to_string(path)?;
    assert!(
        content.contains(expected),
        "File {:?} does not contain expected content: '{}'",
        path,
        expected
    );
    Ok(())
}

/// Assert that a file does not contain specific content
pub fn assert_file_not_contains(path: &Path, unexpected: &str) -> Result<()> {
    assert_file_exists(path);
    let content = std::fs::read_to_string(path)?;
    assert!(
        !content.contains(unexpected),
        "File {:?} contains unexpected content: '{}'",
        path,
        unexpected
    );
    Ok(())
}

/// Assert that two files have the same content
pub fn assert_files_equal(path1: &Path, path2: &Path) -> Result<()> {
    assert_file_exists(path1);
    assert_file_exists(path2);

    let content1 = std::fs::read_to_string(path1)?;
    let content2 = std::fs::read_to_string(path2)?;

    assert_eq!(
        content1, content2,
        "Files {:?} and {:?} have different content",
        path1, path2
    );
    Ok(())
}

/// Assert that a vector contains a specific element
pub fn assert_contains<T: PartialEq + std::fmt::Debug>(vec: &[T], item: &T) {
    assert!(
        vec.contains(item),
        "Vector {:?} does not contain {:?}",
        vec,
        item
    );
}

/// Assert that a vector does not contain a specific element
pub fn assert_not_contains<T: PartialEq + std::fmt::Debug>(vec: &[T], item: &T) {
    assert!(!vec.contains(item), "Vector {:?} contains {:?}", vec, item);
}

/// Assert that a string matches a regex pattern
#[cfg(test)]
pub fn assert_matches_pattern(text: &str, pattern: &str) -> Result<()> {
    use regex::Regex;
    let re = Regex::new(pattern)?;
    assert!(
        re.is_match(text),
        "Text '{}' does not match pattern '{}'",
        text,
        pattern
    );
    Ok(())
}

/// Assert that a value is within a range
pub fn assert_in_range<T: PartialOrd + std::fmt::Debug>(value: T, min: T, max: T) {
    assert!(
        value >= min && value <= max,
        "Value {:?} is not in range [{:?}, {:?}]",
        value,
        min,
        max
    );
}

/// Assert that a float is approximately equal to another
pub fn assert_approx_eq(a: f64, b: f64, epsilon: f64) {
    let diff = (a - b).abs();
    assert!(
        diff <= epsilon,
        "Values {} and {} differ by {} (max allowed: {})",
        a,
        b,
        diff,
        epsilon
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_assert_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        assert_file_exists(&file_path);
    }

    #[test]
    #[should_panic(expected = "File does not exist")]
    fn test_assert_file_exists_panic() {
        assert_file_exists(Path::new("/nonexistent/file.txt"));
    }

    #[test]
    fn test_assert_file_contains() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello, world!").unwrap();

        assert_file_contains(&file_path, "Hello").unwrap();
    }

    #[test]
    fn test_assert_contains() {
        let vec = vec![1, 2, 3, 4, 5];
        assert_contains(&vec, &3);
        assert_not_contains(&vec, &6);
    }

    #[test]
    fn test_assert_in_range() {
        assert_in_range(5, 1, 10);
        assert_in_range(5.5, 5.0, 6.0);
    }

    #[test]
    fn test_assert_approx_eq() {
        assert_approx_eq(1.0, 1.00001, 0.0001);
        assert_approx_eq(3.0, 3.001, 0.01);
    }

    #[test]
    fn test_assert_ok_macro() {
        let result: Result<i32> = Ok(42);
        let value = assert_ok!(result);
        assert_eq!(value, 42);
    }

    #[test]
    fn test_assert_err_macro() {
        let result: Result<i32> = Err(anyhow::anyhow!("error"));
        let _err = assert_err!(result);
    }
}
