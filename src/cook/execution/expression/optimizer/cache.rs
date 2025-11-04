//! Sub-expression caching for common subexpression elimination (CSE)

use crate::cook::execution::expression::ast::Expression;
use std::collections::HashMap;

/// Sub-expression cache for CSE
#[derive(Debug)]
pub(super) struct SubExpressionCache {
    #[allow(dead_code)]
    expressions: HashMap<u64, CachedExpression>,
    #[allow(dead_code)]
    access_count: HashMap<u64, usize>,
}

impl SubExpressionCache {
    pub(super) fn new() -> Self {
        Self {
            expressions: HashMap::new(),
            access_count: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub(super) fn get(&mut self, hash: u64) -> Option<&CachedExpression> {
        self.access_count
            .entry(hash)
            .and_modify(|c| *c += 1)
            .or_insert(1);
        self.expressions.get(&hash)
    }

    #[allow(dead_code)]
    pub(super) fn insert(&mut self, hash: u64, expr: CachedExpression) {
        self.expressions.insert(hash, expr);
        self.access_count.insert(hash, 1);
    }

    #[allow(dead_code)]
    pub(super) fn should_cache(&self, hash: u64, threshold: usize) -> bool {
        self.access_count.get(&hash).copied().unwrap_or(0) >= threshold
    }
}

/// Cached expression with metadata
#[derive(Debug, Clone)]
pub(super) struct CachedExpression {
    #[allow(dead_code)]
    pub(super) id: usize,
    #[allow(dead_code)]
    pub(super) expr: Expression,
    #[allow(dead_code)]
    pub(super) cost: u32,
}
