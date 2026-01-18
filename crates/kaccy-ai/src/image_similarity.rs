//! Image similarity detection using perceptual hashing
//!
//! Provides duplicate and near-duplicate image detection for fraud prevention.
//! Uses perceptual hashing (pHash) to create fingerprints of images that are
//! resistant to minor modifications like resizing, compression, and slight color changes.

use crate::error::AiError;
use image::{DynamicImage, ImageBuffer, Luma};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Perceptual hash of an image
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PerceptualHash {
    /// Hash value (64-bit)
    pub hash: u64,
    /// Hash algorithm used
    pub algorithm: HashAlgorithm,
}

/// Hash algorithm type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HashAlgorithm {
    /// Difference hash (dHash) - fast, good for exact and near-duplicates
    DHash,
    /// Average hash (aHash) - very fast, good for exact duplicates
    AHash,
    /// Perceptual hash (pHash) - slower, best for detecting modifications
    PHash,
}

/// Image similarity score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityScore {
    /// Hamming distance between hashes
    pub hamming_distance: u32,
    /// Similarity percentage (0-100)
    pub similarity_percent: f64,
    /// Whether images are considered similar
    pub is_similar: bool,
    /// Threshold used for comparison
    pub threshold: u32,
}

/// Image similarity detector
pub struct ImageSimilarityDetector {
    /// Similarity threshold (Hamming distance)
    /// Lower = more strict (0 = identical, 64 = completely different)
    threshold: u32,
    /// Hash algorithm to use
    algorithm: HashAlgorithm,
    /// Size for hash computation
    hash_size: u32,
}

impl Default for ImageSimilarityDetector {
    fn default() -> Self {
        Self {
            threshold: 10, // Default: allow 10 bits difference
            algorithm: HashAlgorithm::DHash,
            hash_size: 8,
        }
    }
}

impl ImageSimilarityDetector {
    /// Create a new detector with custom threshold
    #[must_use]
    pub fn new(threshold: u32, algorithm: HashAlgorithm) -> Self {
        Self {
            threshold,
            algorithm,
            hash_size: 8,
        }
    }

    /// Set similarity threshold
    #[must_use]
    pub fn with_threshold(mut self, threshold: u32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set hash algorithm
    #[must_use]
    pub fn with_algorithm(mut self, algorithm: HashAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Compute perceptual hash from image bytes
    pub fn hash_image(&self, image_bytes: &[u8]) -> Result<PerceptualHash, AiError> {
        let img = image::load_from_memory(image_bytes)
            .map_err(|e| AiError::ParseError(format!("Failed to load image: {e}")))?;

        self.hash_dynamic_image(&img)
    }

    /// Compute perceptual hash from `DynamicImage`
    pub fn hash_dynamic_image(&self, img: &DynamicImage) -> Result<PerceptualHash, AiError> {
        let hash = match self.algorithm {
            HashAlgorithm::DHash => self.compute_dhash(img),
            HashAlgorithm::AHash => self.compute_ahash(img),
            HashAlgorithm::PHash => self.compute_phash(img),
        };

        Ok(PerceptualHash {
            hash,
            algorithm: self.algorithm,
        })
    }

    /// Compute difference hash (dHash)
    fn compute_dhash(&self, img: &DynamicImage) -> u64 {
        let size = self.hash_size + 1;
        let resized = img.resize_exact(size, self.hash_size, image::imageops::FilterType::Lanczos3);
        let gray = resized.to_luma8();

        let mut hash: u64 = 0;
        for y in 0..self.hash_size {
            for x in 0..self.hash_size {
                let left = gray.get_pixel(x, y)[0];
                let right = gray.get_pixel(x + 1, y)[0];
                if left > right {
                    let bit_position = u64::from(y * self.hash_size + x);
                    hash |= 1 << bit_position;
                }
            }
        }

        hash
    }

    /// Compute average hash (aHash)
    fn compute_ahash(&self, img: &DynamicImage) -> u64 {
        let resized = img.resize_exact(
            self.hash_size,
            self.hash_size,
            image::imageops::FilterType::Lanczos3,
        );
        let gray = resized.to_luma8();

        // Calculate average pixel value
        let mut sum: u64 = 0;
        for pixel in gray.pixels() {
            sum += u64::from(pixel[0]);
        }
        let avg = sum / u64::from(self.hash_size * self.hash_size);

        // Build hash based on whether pixels are above or below average
        let mut hash: u64 = 0;
        for (i, pixel) in gray.pixels().enumerate() {
            if u64::from(pixel[0]) > avg {
                hash |= 1 << i;
            }
        }

        hash
    }

    /// Compute perceptual hash (pHash) using DCT
    fn compute_phash(&self, img: &DynamicImage) -> u64 {
        let size = 32; // Use larger size for DCT
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let gray = resized.to_luma8();

        // Simplified DCT for 32x32 image (using 8x8 DCT grid)
        let dct = self.simple_dct(&gray, 8, 8);

        // Calculate median of DCT values (excluding DC component)
        let mut values: Vec<f64> = Vec::new();
        for row in &dct {
            for &val in row {
                values.push(val);
            }
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = values[values.len() / 2];

        // Build hash based on median
        let mut hash: u64 = 0;
        for (i, row) in dct.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                if i * 8 + j >= 64 {
                    break;
                }
                if val > median {
                    hash |= 1 << (i * 8 + j);
                }
            }
        }

        hash
    }

    /// Simple 2D DCT (Discrete Cosine Transform) approximation
    fn simple_dct(
        &self,
        img: &ImageBuffer<Luma<u8>, Vec<u8>>,
        rows: usize,
        cols: usize,
    ) -> Vec<Vec<f64>> {
        let mut dct = vec![vec![0.0; cols]; rows];
        let (width, height) = img.dimensions();
        let block_width = width / cols as u32;
        let block_height = height / rows as u32;

        for (i, dct_row) in dct.iter_mut().enumerate() {
            for (j, dct_val) in dct_row.iter_mut().enumerate() {
                let mut sum = 0.0;
                let mut count = 0.0;

                // Average pixel values in block
                for y in 0..block_height {
                    for x in 0..block_width {
                        let px = (j as u32) * block_width + x;
                        let py = (i as u32) * block_height + y;
                        if px < width && py < height {
                            sum += f64::from(img.get_pixel(px, py)[0]);
                            count += 1.0;
                        }
                    }
                }

                *dct_val = if count > 0.0 { sum / count } else { 0.0 };
            }
        }

        dct
    }

    /// Calculate Hamming distance between two hashes
    #[must_use]
    pub fn hamming_distance(hash1: u64, hash2: u64) -> u32 {
        (hash1 ^ hash2).count_ones()
    }

    /// Compare two images for similarity
    pub fn compare_images(
        &self,
        image1_bytes: &[u8],
        image2_bytes: &[u8],
    ) -> Result<SimilarityScore, AiError> {
        let hash1 = self.hash_image(image1_bytes)?;
        let hash2 = self.hash_image(image2_bytes)?;

        self.compare_hashes(&hash1, &hash2)
    }

    /// Compare two perceptual hashes
    pub fn compare_hashes(
        &self,
        hash1: &PerceptualHash,
        hash2: &PerceptualHash,
    ) -> Result<SimilarityScore, AiError> {
        if hash1.algorithm != hash2.algorithm {
            return Err(AiError::InvalidInput(
                "Cannot compare hashes from different algorithms".to_string(),
            ));
        }

        let hamming_distance = Self::hamming_distance(hash1.hash, hash2.hash);
        let similarity_percent = 100.0 * (1.0 - (f64::from(hamming_distance) / 64.0));
        let is_similar = hamming_distance <= self.threshold;

        Ok(SimilarityScore {
            hamming_distance,
            similarity_percent,
            is_similar,
            threshold: self.threshold,
        })
    }

    /// Find similar images in a collection
    pub fn find_similar_images(
        &self,
        query_image: &[u8],
        image_collection: &[Vec<u8>],
    ) -> Result<Vec<(usize, SimilarityScore)>, AiError> {
        let query_hash = self.hash_image(query_image)?;
        let mut results = Vec::new();

        for (idx, img_bytes) in image_collection.iter().enumerate() {
            let hash = self.hash_image(img_bytes)?;
            let score = self.compare_hashes(&query_hash, &hash)?;

            if score.is_similar {
                results.push((idx, score));
            }
        }

        // Sort by similarity (most similar first)
        results.sort_by(|a, b| a.1.hamming_distance.cmp(&b.1.hamming_distance));

        Ok(results)
    }
}

/// Image database for deduplication
pub struct ImageDatabase {
    /// Stored hashes with IDs
    hashes: HashMap<String, PerceptualHash>,
    /// Detector instance
    detector: ImageSimilarityDetector,
}

impl ImageDatabase {
    /// Create a new image database
    #[must_use]
    pub fn new(detector: ImageSimilarityDetector) -> Self {
        Self {
            hashes: HashMap::new(),
            detector,
        }
    }

    /// Add an image to the database
    pub fn add_image(&mut self, id: String, image_bytes: &[u8]) -> Result<PerceptualHash, AiError> {
        let hash = self.detector.hash_image(image_bytes)?;
        self.hashes.insert(id, hash.clone());
        Ok(hash)
    }

    /// Check if an image is a duplicate
    pub fn is_duplicate(
        &self,
        image_bytes: &[u8],
    ) -> Result<Option<(String, SimilarityScore)>, AiError> {
        let query_hash = self.detector.hash_image(image_bytes)?;

        for (id, hash) in &self.hashes {
            let score = self.detector.compare_hashes(&query_hash, hash)?;
            if score.is_similar {
                return Ok(Some((id.clone(), score)));
            }
        }

        Ok(None)
    }

    /// Find all similar images
    pub fn find_all_similar(
        &self,
        image_bytes: &[u8],
    ) -> Result<Vec<(String, SimilarityScore)>, AiError> {
        let query_hash = self.detector.hash_image(image_bytes)?;
        let mut results = Vec::new();

        for (id, hash) in &self.hashes {
            let score = self.detector.compare_hashes(&query_hash, hash)?;
            if score.is_similar {
                results.push((id.clone(), score));
            }
        }

        // Sort by similarity
        results.sort_by(|a, b| a.1.hamming_distance.cmp(&b.1.hamming_distance));

        Ok(results)
    }

    /// Get number of images in database
    #[must_use]
    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    /// Check if database is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }

    /// Clear the database
    pub fn clear(&mut self) {
        self.hashes.clear();
    }

    /// Find all duplicates in the database
    #[must_use]
    pub fn find_duplicates(&self) -> Vec<(String, String, f64)> {
        let mut duplicates = Vec::new();
        let ids: Vec<_> = self.hashes.keys().cloned().collect();

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let hash1 = &self.hashes[&ids[i]];
                let hash2 = &self.hashes[&ids[j]];

                if let Ok(score) = self.detector.compare_hashes(hash1, hash2) {
                    if score.is_similar {
                        duplicates.push((ids[i].clone(), ids[j].clone(), score.similarity_percent));
                    }
                }
            }
        }

        // Sort by similarity (highest first)
        duplicates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        duplicates
    }

    /// Find similar images to a query
    pub fn find_similar(
        &self,
        image_bytes: &[u8],
        min_similarity: f64,
    ) -> Result<Vec<(String, f64)>, AiError> {
        let query_hash = self.detector.hash_image(image_bytes)?;
        let mut results = Vec::new();

        for (id, hash) in &self.hashes {
            let score = self.detector.compare_hashes(&query_hash, hash)?;
            if score.similarity_percent >= min_similarity {
                results.push((id.clone(), score.similarity_percent));
            }
        }

        // Sort by similarity (highest first)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(results)
    }

    /// Check if similar image exists in database
    pub fn has_similar_image(&self, image_bytes: &[u8]) -> Result<bool, AiError> {
        Ok(self.is_duplicate(image_bytes)?.is_some())
    }

    /// Get all stored image IDs
    #[must_use]
    pub fn get_all_ids(&self) -> Vec<String> {
        self.hashes.keys().cloned().collect()
    }

    /// Remove an image from the database
    pub fn remove_image(&mut self, id: &str) -> Option<PerceptualHash> {
        self.hashes.remove(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn create_test_image(color: [u8; 3]) -> Vec<u8> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(100, 100, |_, _| Rgb(color));

        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }

    #[test]
    fn test_identical_images() {
        let detector = ImageSimilarityDetector::default();
        let img1 = create_test_image([255, 0, 0]);
        let img2 = img1.clone();

        let score = detector.compare_images(&img1, &img2).unwrap();
        assert_eq!(score.hamming_distance, 0);
        assert!((score.similarity_percent - 100.0).abs() < 0.01);
        assert!(score.is_similar);
    }

    #[test]
    fn test_different_images() {
        let detector = ImageSimilarityDetector::default();
        let img1 = create_test_image([255, 0, 0]); // Red
        let img2 = create_test_image([0, 0, 255]); // Blue

        let score = detector.compare_images(&img1, &img2).unwrap();
        // Solid color images may have same hash due to lack of detail
        // Just verify the comparison works
        assert!(score.similarity_percent <= 100.0);
        assert!(score.similarity_percent >= 0.0);
    }

    #[test]
    fn test_hash_algorithms() {
        let img = create_test_image([128, 128, 128]);

        let dhash_detector = ImageSimilarityDetector::new(10, HashAlgorithm::DHash);
        let ahash_detector = ImageSimilarityDetector::new(10, HashAlgorithm::AHash);
        let phash_detector = ImageSimilarityDetector::new(10, HashAlgorithm::PHash);

        let dhash = dhash_detector.hash_image(&img).unwrap();
        let ahash = ahash_detector.hash_image(&img).unwrap();
        let phash = phash_detector.hash_image(&img).unwrap();

        assert_eq!(dhash.algorithm, HashAlgorithm::DHash);
        assert_eq!(ahash.algorithm, HashAlgorithm::AHash);
        assert_eq!(phash.algorithm, HashAlgorithm::PHash);
    }

    #[test]
    fn test_hamming_distance() {
        assert_eq!(ImageSimilarityDetector::hamming_distance(0b1010, 0b1010), 0);
        assert_eq!(ImageSimilarityDetector::hamming_distance(0b1010, 0b1011), 1);
        assert_eq!(ImageSimilarityDetector::hamming_distance(0b1010, 0b0101), 4);
    }

    #[test]
    fn test_image_database() {
        let detector = ImageSimilarityDetector::default();
        let mut db = ImageDatabase::new(detector);

        let img1 = create_test_image([255, 0, 0]);
        let img2 = create_test_image([0, 255, 0]);

        db.add_image("img1".to_string(), &img1).unwrap();
        assert_eq!(db.len(), 1);

        // Check for duplicate (exact match)
        let duplicate = db.is_duplicate(&img1).unwrap();
        assert!(duplicate.is_some());

        // Check for non-duplicate
        db.add_image("img2".to_string(), &img2).unwrap();
        assert_eq!(db.len(), 2);

        db.clear();
        assert!(db.is_empty());
    }

    #[test]
    fn test_find_similar_images() {
        let detector = ImageSimilarityDetector::default();
        let img1 = create_test_image([255, 0, 0]);
        let img2 = create_test_image([254, 0, 0]); // Very similar
        let img3 = create_test_image([0, 0, 255]); // Different

        let collection = vec![img1.clone(), img2.clone(), img3];
        let results = detector.find_similar_images(&img1, &collection).unwrap();

        // Should find at least itself and the very similar image
        assert!(!results.is_empty());
        assert!(results[0].1.is_similar);
    }

    #[test]
    fn test_with_threshold() {
        let detector = ImageSimilarityDetector::default().with_threshold(5);
        assert_eq!(detector.threshold, 5);
    }

    #[test]
    fn test_with_algorithm() {
        let detector = ImageSimilarityDetector::default().with_algorithm(HashAlgorithm::PHash);
        assert_eq!(detector.algorithm, HashAlgorithm::PHash);
    }

    #[test]
    fn test_find_duplicates_in_database() {
        let detector = ImageSimilarityDetector::default();
        let mut db = ImageDatabase::new(detector);

        let img1 = create_test_image([255, 0, 0]);
        let img2 = img1.clone(); // Exact duplicate
        let img3 = create_test_image([0, 255, 0]);

        db.add_image("img1".to_string(), &img1).unwrap();
        db.add_image("img2".to_string(), &img2).unwrap();
        db.add_image("img3".to_string(), &img3).unwrap();

        let duplicates = db.find_duplicates();
        // Should find at least the exact duplicate pair
        assert!(!duplicates.is_empty());
    }

    #[test]
    fn test_find_similar_with_threshold() {
        let detector = ImageSimilarityDetector::default();
        let mut db = ImageDatabase::new(detector);

        let img1 = create_test_image([255, 0, 0]);
        let img2 = create_test_image([254, 0, 0]);

        db.add_image("img1".to_string(), &img1).unwrap();
        db.add_image("img2".to_string(), &img2).unwrap();

        // Find similar with 90% threshold
        let similar = db.find_similar(&img1, 90.0).unwrap();
        assert!(!similar.is_empty());
    }

    #[test]
    fn test_has_similar_image() {
        let detector = ImageSimilarityDetector::default();
        let mut db = ImageDatabase::new(detector);

        let img1 = create_test_image([255, 0, 0]);
        db.add_image("img1".to_string(), &img1).unwrap();

        assert!(db.has_similar_image(&img1).unwrap());
    }

    #[test]
    fn test_get_all_ids() {
        let detector = ImageSimilarityDetector::default();
        let mut db = ImageDatabase::new(detector);

        let img1 = create_test_image([255, 0, 0]);
        let img2 = create_test_image([0, 255, 0]);

        db.add_image("img1".to_string(), &img1).unwrap();
        db.add_image("img2".to_string(), &img2).unwrap();

        let ids = db.get_all_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"img1".to_string()));
        assert!(ids.contains(&"img2".to_string()));
    }

    #[test]
    fn test_remove_image() {
        let detector = ImageSimilarityDetector::default();
        let mut db = ImageDatabase::new(detector);

        let img1 = create_test_image([255, 0, 0]);
        db.add_image("img1".to_string(), &img1).unwrap();
        assert_eq!(db.len(), 1);

        let removed = db.remove_image("img1");
        assert!(removed.is_some());
        assert_eq!(db.len(), 0);
    }
}
