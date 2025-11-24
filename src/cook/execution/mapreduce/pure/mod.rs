// ! Pure functional utilities for MapReduce execution
//!
//! This module contains pure functions extracted from the MapReduce executor
//! to improve testability, reusability, and maintainability.

pub mod aggregation;
pub mod dependency_analysis;
pub mod formatting;
pub mod interpolation;
pub mod planning;
pub mod work_planning;
