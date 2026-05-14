//! Knowledge base for domain-specific information
//!
//! This module provides a system for storing and retrieving domain-specific knowledge
//! that can enhance AI evaluations and verifications.
//!
//! # Examples
//!
//! ```
//! use kaccy_ai::knowledge_base::{KnowledgeBase, KnowledgeEntry, KnowledgeDomain};
//!
//! let mut kb = KnowledgeBase::new();
//!
//! // Add knowledge about Rust programming
//! let entry = KnowledgeEntry::new(
//!     KnowledgeDomain::Programming,
//!     "rust_best_practices",
//!     "Rust best practices include using the type system, avoiding unwrap() in production, \
//!      and using Result for error handling.",
//! );
//! kb.add_entry(entry);
//!
//! // Query knowledge
//! let results = kb.search("rust error handling");
//! assert!(results.len() > 0);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{AiError, Result};

/// Knowledge domain categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KnowledgeDomain {
    /// Programming and software development
    Programming,
    /// Blockchain and crypto
    Blockchain,
    /// Security and fraud detection
    Security,
    /// Content quality and writing
    Content,
    /// Social media and marketing
    SocialMedia,
    /// General domain knowledge
    General,
}

impl KnowledgeDomain {
    /// Get domain name
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            KnowledgeDomain::Programming => "Programming",
            KnowledgeDomain::Blockchain => "Blockchain",
            KnowledgeDomain::Security => "Security",
            KnowledgeDomain::Content => "Content",
            KnowledgeDomain::SocialMedia => "Social Media",
            KnowledgeDomain::General => "General",
        }
    }
}

/// Knowledge entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    /// Unique identifier
    pub id: String,
    /// Domain category
    pub domain: KnowledgeDomain,
    /// Entry title/key
    pub title: String,
    /// Knowledge content
    pub content: String,
    /// Tags for searching
    pub tags: Vec<String>,
    /// Source/reference
    pub source: Option<String>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last updated timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl KnowledgeEntry {
    /// Create a new knowledge entry
    pub fn new(
        domain: KnowledgeDomain,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now();
        let title = title.into();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            domain,
            title: title.clone(),
            content: content.into(),
            tags: Self::extract_tags(&title),
            source: None,
            confidence: 1.0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create with explicit ID
    pub fn with_id(
        id: impl Into<String>,
        domain: KnowledgeDomain,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now();
        let title = title.into();
        Self {
            id: id.into(),
            domain,
            title: title.clone(),
            content: content.into(),
            tags: Self::extract_tags(&title),
            source: None,
            confidence: 1.0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add tags
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add source
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Extract tags from text (simple word tokenization)
    fn extract_tags(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .take(10)
            .map(std::string::ToString::to_string)
            .collect()
    }

    /// Update content
    pub fn update_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
        self.updated_at = chrono::Utc::now();
    }
}

/// Knowledge base storage and retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeBase {
    entries: HashMap<String, KnowledgeEntry>,
    domain_index: HashMap<KnowledgeDomain, Vec<String>>,
}

impl Default for KnowledgeBase {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeBase {
    /// Create a new knowledge base
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            domain_index: HashMap::new(),
        }
    }

    /// Add a knowledge entry
    pub fn add_entry(&mut self, entry: KnowledgeEntry) -> Result<()> {
        let id = entry.id.clone();
        let domain = entry.domain;

        // Add to main storage
        self.entries.insert(id.clone(), entry);

        // Add to domain index
        self.domain_index.entry(domain).or_default().push(id);

        Ok(())
    }

    /// Get entry by ID
    #[must_use]
    pub fn get_entry(&self, id: &str) -> Option<&KnowledgeEntry> {
        self.entries.get(id)
    }

    /// Get mutable entry by ID
    pub fn get_entry_mut(&mut self, id: &str) -> Option<&mut KnowledgeEntry> {
        self.entries.get_mut(id)
    }

    /// Remove entry by ID
    pub fn remove_entry(&mut self, id: &str) -> Option<KnowledgeEntry> {
        if let Some(entry) = self.entries.remove(id) {
            // Remove from domain index
            if let Some(ids) = self.domain_index.get_mut(&entry.domain) {
                ids.retain(|i| i != id);
            }
            Some(entry)
        } else {
            None
        }
    }

    /// Search for entries matching query
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&KnowledgeEntry> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut results: Vec<(f64, &KnowledgeEntry)> = self
            .entries
            .values()
            .filter_map(|entry| {
                let score = self.calculate_relevance(entry, &query_words, &query_lower);
                if score > 0.0 {
                    Some((score, entry))
                } else {
                    None
                }
            })
            .collect();

        // Sort by relevance score
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        results.into_iter().map(|(_, entry)| entry).collect()
    }

    /// Calculate relevance score for an entry
    fn calculate_relevance(
        &self,
        entry: &KnowledgeEntry,
        query_words: &[&str],
        query_full: &str,
    ) -> f64 {
        let mut score = 0.0;

        let title_lower = entry.title.to_lowercase();
        let content_lower = entry.content.to_lowercase();

        // Exact title match
        if title_lower.contains(query_full) {
            score += 10.0;
        }

        // Exact content match
        if content_lower.contains(query_full) {
            score += 5.0;
        }

        // Word matches in title
        for word in query_words {
            if title_lower.contains(word) {
                score += 2.0;
            }
        }

        // Word matches in content
        for word in query_words {
            if content_lower.contains(word) {
                score += 0.5;
            }
        }

        // Tag matches
        for tag in &entry.tags {
            for word in query_words {
                if tag.contains(word) {
                    score += 1.0;
                }
            }
        }

        // Apply confidence multiplier
        score * entry.confidence
    }

    /// Get entries by domain
    #[must_use]
    pub fn get_by_domain(&self, domain: KnowledgeDomain) -> Vec<&KnowledgeEntry> {
        self.domain_index
            .get(&domain)
            .map(|ids| ids.iter().filter_map(|id| self.entries.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all entries
    #[must_use]
    pub fn all_entries(&self) -> Vec<&KnowledgeEntry> {
        self.entries.values().collect()
    }

    /// Get total entry count
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Save knowledge base to file
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AiError::Internal(format!("Failed to serialize knowledge base: {e}")))?;

        std::fs::write(path, json)
            .map_err(|e| AiError::Internal(format!("Failed to write knowledge base: {e}")))?;

        Ok(())
    }

    /// Load knowledge base from file
    pub fn load_from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| AiError::Internal(format!("Failed to read knowledge base: {e}")))?;

        let kb: KnowledgeBase = serde_json::from_str(&json)
            .map_err(|e| AiError::Internal(format!("Failed to deserialize knowledge base: {e}")))?;

        Ok(kb)
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.domain_index.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_entry_creation() {
        let entry = KnowledgeEntry::new(
            KnowledgeDomain::Programming,
            "Rust Best Practices",
            "Always use Result for error handling",
        );

        assert_eq!(entry.domain, KnowledgeDomain::Programming);
        assert_eq!(entry.title, "Rust Best Practices");
        assert!(!entry.tags.is_empty());
    }

    #[test]
    fn test_knowledge_base_add_and_get() {
        let mut kb = KnowledgeBase::new();

        let entry = KnowledgeEntry::new(
            KnowledgeDomain::Programming,
            "Rust Ownership",
            "Ownership is a key concept in Rust",
        );

        let id = entry.id.clone();
        kb.add_entry(entry).unwrap();

        assert_eq!(kb.len(), 1);
        assert!(kb.get_entry(&id).is_some());
    }

    #[test]
    fn test_knowledge_base_search() {
        let mut kb = KnowledgeBase::new();

        kb.add_entry(KnowledgeEntry::new(
            KnowledgeDomain::Programming,
            "Rust Ownership",
            "Ownership prevents memory issues",
        ))
        .unwrap();

        kb.add_entry(KnowledgeEntry::new(
            KnowledgeDomain::Programming,
            "Python GIL",
            "Global Interpreter Lock in Python",
        ))
        .unwrap();

        let results = kb.search("rust ownership");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Ownership");
    }

    #[test]
    fn test_knowledge_base_by_domain() {
        let mut kb = KnowledgeBase::new();

        kb.add_entry(KnowledgeEntry::new(
            KnowledgeDomain::Programming,
            "Test1",
            "Content1",
        ))
        .unwrap();

        kb.add_entry(KnowledgeEntry::new(
            KnowledgeDomain::Blockchain,
            "Test2",
            "Content2",
        ))
        .unwrap();

        let prog_entries = kb.get_by_domain(KnowledgeDomain::Programming);
        assert_eq!(prog_entries.len(), 1);

        let blockchain_entries = kb.get_by_domain(KnowledgeDomain::Blockchain);
        assert_eq!(blockchain_entries.len(), 1);
    }

    #[test]
    fn test_knowledge_base_persistence() {
        let mut kb = KnowledgeBase::new();

        kb.add_entry(KnowledgeEntry::new(
            KnowledgeDomain::Security,
            "Security Best Practices",
            "Always validate input",
        ))
        .unwrap();

        let temp_path = "/tmp/kb_test.json";
        kb.save_to_file(temp_path).unwrap();

        let kb2 = KnowledgeBase::load_from_file(temp_path).unwrap();
        assert_eq!(kb2.len(), 1);

        // Cleanup
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn test_entry_update() {
        let mut kb = KnowledgeBase::new();

        let entry = KnowledgeEntry::new(KnowledgeDomain::Programming, "Test", "Original content");

        let id = entry.id.clone();
        kb.add_entry(entry).unwrap();

        if let Some(entry) = kb.get_entry_mut(&id) {
            entry.update_content("Updated content");
        }

        let updated = kb.get_entry(&id).unwrap();
        assert_eq!(updated.content, "Updated content");
    }

    #[test]
    fn test_entry_removal() {
        let mut kb = KnowledgeBase::new();

        let entry = KnowledgeEntry::new(KnowledgeDomain::Programming, "Test", "Content");

        let id = entry.id.clone();
        kb.add_entry(entry).unwrap();

        assert_eq!(kb.len(), 1);

        let removed = kb.remove_entry(&id);
        assert!(removed.is_some());
        assert_eq!(kb.len(), 0);
    }
}
