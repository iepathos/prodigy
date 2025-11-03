//! Tests for validation module
//!
//! This module contains all test code for the validation module, separated from
//! the implementation code for better maintainability and organization.

use super::validation::*;

// ============================================================================
// Phase 1: Proof of Concept Test
// ============================================================================

#[test]
fn test_should_continue_retry_boundary_conditions() {
    // Boundary: last attempt before max
    assert!(should_continue_retry(2, 3, false));
    // Boundary: at max attempts
    assert!(!should_continue_retry(3, 3, false));
    // Boundary: complete on first try
    assert!(!should_continue_retry(0, 3, true));
}
