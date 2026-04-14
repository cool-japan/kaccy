//! Detection examples: plagiarism detection and image similarity detection.

use crate::error::Result;

/// Example: Plagiarism Detection
///
/// This example demonstrates how to detect plagiarism in code and text,
/// including semantic analysis and batch detection.
pub struct PlagiarismDetectionExample;

impl PlagiarismDetectionExample {
    /// Run basic code plagiarism detection
    #[allow(dead_code)]
    pub async fn run_basic_code_detection() -> Result<()> {
        println!("=== Code Plagiarism Detection Example ===");
        println!();

        // Plagiarism detector with default config
        let _code1 = r"
fn calculate_sum(numbers: &[i32]) -> i32 {
    numbers.iter().sum()
}
";

        let _code2 = r"
fn sum_array(nums: &[i32]) -> i32 {
    nums.iter().sum()
}
";

        println!("Comparing code samples...");
        println!("(Conceptual example - actual API calls omitted)");
        println!();
        println!("Results:");
        println!("Similarity score: 85.00%");
        println!("Is plagiarism: true");
        println!("Confidence: 90%");
        println!("Token similarity: 87.50%");

        Ok(())
    }

    /// Run text plagiarism detection with n-grams
    #[allow(dead_code)]
    pub async fn run_text_detection() -> Result<()> {
        println!("=== Text Plagiarism Detection Example ===");
        println!();

        let _text1 = "The blockchain is a distributed ledger that records transactions.";
        let _text2 = "A blockchain represents a distributed ledger for recording transactions.";

        println!("Comparing text samples...");
        println!("(Conceptual example - actual API calls omitted)");
        println!();
        println!("Results:");
        println!("Similarity score: 78.00%");
        println!("Is plagiarism: true");
        println!("Confidence: 85%");

        Ok(())
    }

    /// Semantic analysis with LLM
    #[allow(dead_code)]
    pub async fn semantic_analysis_example(api_key: &str) -> Result<()> {
        println!("=== Semantic Plagiarism Analysis (LLM-Powered) ===");
        println!();

        let _llm_client = crate::llm::LlmClientBuilder::new()
            .openai_api_key(api_key)
            .build()
            .expect("Failed to build LLM client");

        let _config = crate::plagiarism::PlagiarismConfig {
            similarity_threshold: 0.7,
            use_semantic_analysis: true,
            ngram_size: 3,
            min_token_overlap: 5,
        };

        let _text1 = "Machine learning models require large datasets for training.";
        let _text2 = "To train ML models effectively, you need substantial amounts of data.";

        println!("Running semantic analysis...");
        println!("(Conceptual example - actual API calls omitted)");
        println!();
        println!("Results:");
        println!("  Overall similarity: 72.00%");
        println!("  Ngram similarity: 65.00%");
        println!("  Semantic similarity (LLM): 82.00%");
        println!("  Verdict: PLAGIARISM DETECTED");

        Ok(())
    }

    /// Batch detection with similarity matrix
    #[allow(dead_code)]
    pub async fn batch_detection_example() -> Result<()> {
        println!("=== Batch Plagiarism Detection Example ===");
        println!();

        let _documents = [
            "The quick brown fox jumps over the lazy dog.".to_string(),
            "A fast brown fox leaps over a sleeping dog.".to_string(),
            "Blockchain technology enables decentralized transactions.".to_string(),
            "The rapid brown fox hops over the idle canine.".to_string(),
        ];

        println!("Analyzing 4 documents...");
        println!("Note: Batch comparison requires pairwise comparison of all documents");
        println!("For 4 documents, this would require 6 comparisons");

        // Simplified example - just show the concept
        println!();
        println!("Example similarity matrix (conceptual):");
        println!("     Doc0  Doc1  Doc2  Doc3");
        println!("Doc0 100.0  85.0  20.0  82.0");
        println!("Doc1  85.0 100.0  15.0  88.0");
        println!("Doc2  20.0  15.0 100.0  18.0");
        println!("Doc3  82.0  88.0  18.0 100.0");
        println!();
        println!("Potential plagiarism clusters (>80% similar):");
        println!("  * Cluster 1: Documents 0, 1, 3 (fox/canine theme)");
        println!("  * Cluster 2: Document 2 (unrelated - blockchain)");

        Ok(())
    }

    /// Use cases guide
    #[allow(dead_code)]
    pub async fn use_cases_guide() -> Result<()> {
        println!("=== Plagiarism Detection Use Cases ===");
        println!();

        println!("1. Fraud Detection");
        println!("   - Detect users copying code/content from others");
        println!("   - Identify reputation gaming through duplicate content");
        println!("   - Example: User submits same code for multiple commitments");
        println!();
        println!("2. Content Verification");
        println!("   - Verify commitment evidence is original");
        println!("   - Check if GitHub commits are copied");
        println!("   - Example: Detect forked repositories claimed as original work");
        println!();
        println!("3. Academic Integrity");
        println!("   - Verify educational commitments are original");
        println!("   - Detect code sharing between students");
        println!("   - Example: Multiple users submitting similar solutions");
        println!();
        println!("4. Code Review");
        println!("   - Find duplicate code blocks in codebase");
        println!("   - Suggest refactoring opportunities");
        println!("   - Example: Identify copy-pasted functions");
        println!();
        println!("Configuration tips:");
        println!("  * Token similarity: Good for exact/near-exact copies (threshold: 0.7)");
        println!("  * N-gram similarity: Detects paraphrasing (threshold: 0.6)");
        println!("  * Semantic similarity: Finds conceptual copies (threshold: 0.75)");

        Ok(())
    }
}

/// Example: Image Similarity Detection
///
/// This example demonstrates how to detect similar or duplicate images
/// using perceptual hashing.
pub struct ImageSimilarityExample;

impl ImageSimilarityExample {
    /// Run basic image similarity detection
    #[allow(dead_code)]
    pub async fn run_basic_detection() -> Result<()> {
        println!("=== Image Similarity Detection Example ===");
        println!();

        println!("Note: Using dHash algorithm for image hashing");
        println!("Computing perceptual hash for images...");
        println!("  Algorithm: dHash (difference hash)");
        println!();

        // Example hash comparison
        let hash1 = crate::image_similarity::PerceptualHash {
            hash: 0x1234_5678_9ABC_DEF0,
            algorithm: crate::image_similarity::HashAlgorithm::DHash,
        };
        let hash2 = crate::image_similarity::PerceptualHash {
            hash: 0x1234_5678_9ABC_DEF1,
            algorithm: crate::image_similarity::HashAlgorithm::DHash,
        };

        // Conceptual comparison (actual API may differ)
        let hamming_distance = (hash1.hash ^ hash2.hash).count_ones();
        let similarity_percent = (f64::from(64 - hamming_distance) / 64.0) * 100.0;

        println!("Hash 1: {:016X}", hash1.hash);
        println!("Hash 2: {:016X}", hash2.hash);
        println!("Hamming distance: {hamming_distance}");
        println!("Similarity score: {similarity_percent:.2}%");
        println!("Is similar: {}", similarity_percent > 90.0);

        Ok(())
    }

    /// Hash algorithm comparison
    #[allow(dead_code)]
    pub async fn algorithm_comparison() -> Result<()> {
        println!("=== Hash Algorithm Comparison ===");
        println!();

        println!("1. dHash (Difference Hash)");
        println!("   - Speed: Very fast");
        println!("   - Accuracy: Good");
        println!("   - Best for: Real-time detection, large datasets");
        println!("   - Resistant to: Scaling, slight cropping");
        println!();
        println!("2. aHash (Average Hash)");
        println!("   - Speed: Fastest");
        println!("   - Accuracy: Moderate");
        println!("   - Best for: Quick filtering, high performance");
        println!("   - Resistant to: Scaling, brightness changes");
        println!();
        println!("3. pHash (Perceptual Hash)");
        println!("   - Speed: Slower");
        println!("   - Accuracy: Best");
        println!("   - Best for: High-quality detection, critical use cases");
        println!("   - Resistant to: Rotation, compression, watermarks");
        println!();
        println!("Recommendation:");
        println!("  * Use dHash for most cases (good balance)");
        println!("  * Use pHash for fraud detection (highest accuracy)");
        println!("  * Use aHash for preliminary filtering (fastest)");

        Ok(())
    }

    /// Threshold tuning guide
    #[allow(dead_code)]
    pub async fn threshold_tuning_guide() -> Result<()> {
        println!("=== Similarity Threshold Tuning Guide ===");
        println!();

        println!("Hamming distance thresholds:");
        println!();
        println!("  Distance 0-5:  Nearly identical (99%+ similar)");
        println!("             -> Same image, minor compression/resize");
        println!();
        println!("  Distance 6-10: Very similar (95-99% similar)");
        println!("             -> Same image, different quality/format");
        println!();
        println!("  Distance 11-15: Similar (90-95% similar)");
        println!("             -> Same subject, different angle/crop");
        println!();
        println!("  Distance 16-20: Somewhat similar (85-90% similar)");
        println!("             -> Related content, different composition");
        println!();
        println!("  Distance 21+: Not similar (<85% similar)");
        println!("             -> Different images");
        println!();
        println!("Recommended thresholds:");
        println!("  * Exact duplicates: distance <= 5");
        println!("  * Near duplicates: distance <= 10");
        println!("  * Similar images: distance <= 15");
        println!("  * Fraud detection: distance <= 8 (strict)");

        Ok(())
    }

    /// Image deduplication database
    #[allow(dead_code)]
    pub async fn deduplication_example() -> Result<()> {
        println!("=== Image Deduplication Database Example ===");
        println!();

        println!("Note: Image database example (conceptual)");

        println!("Added 3 images to database");
        println!();

        println!("Finding duplicates for test image...");
        println!("Found 2 similar images:");
        println!("  - image1.jpg (hamming distance: 2)");
        println!("  - image2.jpg (hamming distance: 3)");

        Ok(())
    }

    /// Fraud prevention use cases
    #[allow(dead_code)]
    pub async fn fraud_prevention_guide() -> Result<()> {
        println!("=== Image Similarity for Fraud Prevention ===");
        println!();

        println!("Use Cases:");
        println!();
        println!("1. Screenshot Fraud Detection");
        println!("   - Detect users submitting same screenshot multiple times");
        println!("   - Identify edited/photoshopped evidence");
        println!("   - Example: Modified transaction screenshots");
        println!();
        println!("2. Duplicate Evidence Prevention");
        println!("   - Prevent reuse of evidence across commitments");
        println!("   - Track all submitted images");
        println!("   - Example: Same GitHub stats screenshot for different claims");
        println!();
        println!("3. Identity Verification");
        println!("   - Detect duplicate profile pictures");
        println!("   - Identify stock photo usage");
        println!("   - Example: Multiple accounts with similar avatars");
        println!();
        println!("4. Content Originality");
        println!("   - Verify image evidence is original");
        println!("   - Detect images copied from web");
        println!("   - Example: Reverse image search integration");
        println!();
        println!("Integration with kaccy-ai:");
        println!("  let detector = ImageSimilarityDetector::new(HashAlgorithm::PHash);");
        println!("  let fraud_detector = AiFraudDetector::new(llm_client);");
        println!("  // Use both together for comprehensive fraud detection");

        Ok(())
    }

    /// Performance optimization guide
    #[allow(dead_code)]
    pub async fn performance_optimization_guide() -> Result<()> {
        println!("=== Performance Optimization Guide ===");
        println!();

        println!("For large datasets:");
        println!();
        println!("1. Use fast algorithms first");
        println!("   - Filter with aHash (fastest)");
        println!("   - Confirm with pHash (most accurate)");
        println!();
        println!("2. Implement database indexing");
        println!("   - Use ImageDatabase with appropriate threshold");
        println!("   - Index by hash prefix for faster lookups");
        println!();
        println!("3. Batch processing");
        println!("   - Process images in parallel");
        println!("   - Use rayon for CPU parallelism");
        println!();
        println!("4. Caching");
        println!("   - Cache computed hashes");
        println!("   - Store hashes in database");
        println!();
        println!("Example performance:");
        println!("  * Hash computation: ~1ms per image");
        println!("  * Hash comparison: ~100ns per pair");
        println!("  * Database lookup: ~O(n) without indexing");
        println!("  * With indexing: ~O(log n)");

        Ok(())
    }
}
