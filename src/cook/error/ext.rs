//! Extension trait for adding context to Results
//!
//! Provides convenient methods for wrapping errors with context messages.

use stillwater::ContextError;

/// Extension trait for adding context to Results
///
/// # Examples
///
/// ```
/// use prodigy::cook::error::{ResultExt, ContextResult};
///
/// fn process_file(path: &str) -> ContextResult<String, std::io::Error> {
///     let _content = std::fs::read_to_string(path)
///         .context("Reading input file")?;
///     Ok("data".to_string())
/// }
/// ```
pub trait ResultExt<T, E> {
    /// Add static context to error
    ///
    /// Use this when the context message is known at compile time.
    ///
    /// # Examples
    ///
    /// ```
    /// use prodigy::cook::error::{ResultExt, ContextResult};
    /// use stillwater::ContextError;
    ///
    /// fn do_something() -> Result<(), std::io::Error> {
    ///     Ok(())
    /// }
    ///
    /// fn example() -> Result<(), ContextError<std::io::Error>> {
    ///     do_something()
    ///         .context("Operation description")?;
    ///     Ok(())
    /// }
    /// ```
    fn context(self, msg: impl Into<String>) -> Result<T, ContextError<E>>;

    /// Add dynamic context to error
    ///
    /// Use this when the context message needs to be computed at runtime.
    /// The closure is only evaluated if an error occurs, making it zero-cost
    /// in the success path.
    ///
    /// # Examples
    ///
    /// ```
    /// use prodigy::cook::error::{ResultExt, ContextResult};
    /// use stillwater::ContextError;
    ///
    /// fn do_something_with_id(id: &str) -> Result<(), std::io::Error> {
    ///     Ok(())
    /// }
    ///
    /// fn example(id: &str) -> Result<(), ContextError<std::io::Error>> {
    ///     do_something_with_id(id)
    ///         .with_context(|| format!("Processing item {}", id))?;
    ///     Ok(())
    /// }
    /// ```
    fn with_context<F>(self, f: F) -> Result<T, ContextError<E>>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn context(self, msg: impl Into<String>) -> Result<T, ContextError<E>> {
        self.map_err(|e| ContextError::new(e).context(msg))
    }

    fn with_context<F>(self, f: F) -> Result<T, ContextError<E>>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ContextError::new(e).context(f()))
    }
}

/// Alias for context-aware results
///
/// Use this type alias for functions that return context-aware errors.
///
/// # Examples
///
/// ```
/// use prodigy::cook::error::ContextResult;
///
/// fn process_item(id: &str) -> ContextResult<String, std::io::Error> {
///     // ... implementation
///     Ok("result".to_string())
/// }
/// ```
pub type ContextResult<T, E> = Result<T, ContextError<E>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_preservation() {
        fn inner() -> Result<(), String> {
            Err("base error".to_string())
        }

        fn middle() -> ContextResult<(), String> {
            inner().context("middle operation")
        }

        fn outer() -> ContextResult<(), String> {
            middle().map_err(|e| e.context("outer operation"))?;
            Ok(())
        }

        let result = outer();
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.inner(), "base error");
        assert_eq!(error.context_trail().len(), 2);
        assert!(error
            .context_trail()
            .contains(&"middle operation".to_string()));
        assert!(error
            .context_trail()
            .contains(&"outer operation".to_string()));
    }

    #[test]
    fn test_with_context_lazy_evaluation() {
        let mut call_count = 0;

        let success: Result<i32, String> = Ok(42);
        let _ = success.with_context(|| {
            call_count += 1;
            "should not be called".to_string()
        });

        // Closure should not be called on success
        assert_eq!(call_count, 0);

        let failure: Result<i32, String> = Err("error".to_string());
        let _ = failure.with_context(|| {
            call_count += 1;
            "should be called".to_string()
        });

        // Closure should be called on failure
        assert_eq!(call_count, 1);
    }

    #[test]
    fn test_multiple_context_layers() {
        fn layer1() -> Result<(), String> {
            Err("root cause".to_string())
        }

        fn layer2() -> ContextResult<(), String> {
            layer1().context("layer 2 context")
        }

        fn layer3() -> ContextResult<(), String> {
            layer2().map_err(|e| e.context("layer 3 context"))?;
            Ok(())
        }

        fn layer4() -> ContextResult<(), String> {
            layer3().map_err(|e| e.context("layer 4 context"))?;
            Ok(())
        }

        let result = layer4();
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.inner(), "root cause");

        // Should have context from all layers
        let trail = error.context_trail();
        assert_eq!(trail.len(), 3);
        assert!(trail.contains(&"layer 2 context".to_string()));
        assert!(trail.contains(&"layer 3 context".to_string()));
        assert!(trail.contains(&"layer 4 context".to_string()));
    }
}
