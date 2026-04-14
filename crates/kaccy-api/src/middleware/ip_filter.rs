//! IP filtering middleware for security and access control

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::{
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use super::request_id::RequestId;

/// IP filter configuration
#[derive(Clone, Debug)]
pub struct IpFilterConfig {
    /// Enable IP filtering
    pub enabled: bool,
    /// Whitelist of allowed IPs (if not empty, only these IPs are allowed)
    pub whitelist: Vec<IpAddr>,
    /// Blacklist of blocked IPs
    pub blacklist: Vec<IpAddr>,
    /// Enable automatic blacklisting based on rate limits
    pub auto_blacklist: bool,
    /// Threshold for auto-blacklisting (requests per minute)
    pub auto_blacklist_threshold: usize,
    /// Duration to keep auto-blacklisted IPs
    pub auto_blacklist_duration: Duration,
}

impl Default for IpFilterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            whitelist: Vec::new(),
            blacklist: Vec::new(),
            auto_blacklist: false,
            auto_blacklist_threshold: 1000,
            auto_blacklist_duration: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// IP filter state
#[derive(Clone)]
pub struct IpFilter {
    config: Arc<IpFilterConfig>,
    // Track auto-blacklisted IPs with expiration time
    auto_blacklist: Arc<DashMap<IpAddr, Instant>>,
    // Track request counts per IP for auto-blacklisting
    request_counts: Arc<DashMap<IpAddr, (usize, Instant)>>,
}

impl IpFilter {
    /// Create a new IP filter with configuration
    pub fn new(config: IpFilterConfig) -> Self {
        Self {
            config: Arc::new(config),
            auto_blacklist: Arc::new(DashMap::new()),
            request_counts: Arc::new(DashMap::new()),
        }
    }

    /// Check if an IP is allowed
    pub fn is_allowed(&self, ip: IpAddr) -> bool {
        if !self.config.enabled {
            return true;
        }

        // Check permanent blacklist
        if self.config.blacklist.contains(&ip) {
            return false;
        }

        // Check auto-blacklist with expiration
        if let Some(expiry) = self.auto_blacklist.get(&ip) {
            if Instant::now() < *expiry {
                return false;
            } else {
                // Expired, remove from auto-blacklist
                self.auto_blacklist.remove(&ip);
            }
        }

        // If whitelist is not empty, only allow whitelisted IPs
        if !self.config.whitelist.is_empty() {
            return self.config.whitelist.contains(&ip);
        }

        true
    }

    /// Record a request from an IP (for auto-blacklisting)
    pub fn record_request(&self, ip: IpAddr) {
        if !self.config.auto_blacklist {
            return;
        }

        let now = Instant::now();
        let threshold_window = Duration::from_secs(60); // 1 minute window

        let mut should_blacklist = false;

        self.request_counts
            .entry(ip)
            .and_modify(|(count, last_reset)| {
                if now.duration_since(*last_reset) > threshold_window {
                    // Reset counter for new window
                    *count = 1;
                    *last_reset = now;
                } else {
                    *count += 1;
                    if *count > self.config.auto_blacklist_threshold {
                        should_blacklist = true;
                    }
                }
            })
            .or_insert((1, now));

        if should_blacklist {
            let expiry = now + self.config.auto_blacklist_duration;
            self.auto_blacklist.insert(ip, expiry);
            tracing::warn!(
                ip = %ip,
                threshold = self.config.auto_blacklist_threshold,
                "IP automatically blacklisted for excessive requests"
            );
        }
    }

    /// Manually add IP to blacklist
    pub fn add_to_blacklist(&self, ip: IpAddr, duration: Duration) {
        let expiry = Instant::now() + duration;
        self.auto_blacklist.insert(ip, expiry);
        tracing::info!(ip = %ip, "IP added to blacklist");
    }

    /// Remove IP from blacklist
    pub fn remove_from_blacklist(&self, ip: IpAddr) {
        self.auto_blacklist.remove(&ip);
        tracing::info!(ip = %ip, "IP removed from blacklist");
    }

    /// Get all currently blacklisted IPs
    pub fn get_blacklisted_ips(&self) -> Vec<IpAddr> {
        let now = Instant::now();
        let mut ips = Vec::new();

        // Clean up expired entries and collect active ones
        self.auto_blacklist.retain(|ip, expiry| {
            if now < *expiry {
                ips.push(*ip);
                true
            } else {
                false
            }
        });

        ips
    }

    /// Clean up expired entries
    pub fn cleanup(&self) {
        let now = Instant::now();

        // Clean auto-blacklist
        self.auto_blacklist.retain(|_, expiry| now < *expiry);

        // Clean request counts older than 5 minutes
        let threshold_window = Duration::from_secs(300);
        self.request_counts
            .retain(|_, (_, last_reset)| now.duration_since(*last_reset) < threshold_window);
    }
}

/// Extract IP address from request
pub fn extract_ip(req: &Request) -> Option<IpAddr> {
    // Try X-Forwarded-For header first (for proxies)
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // X-Forwarded-For can contain multiple IPs, take the first one
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse() {
                    return Some(ip);
                }
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            if let Ok(ip) = real_ip_str.parse() {
                return Some(ip);
            }
        }
    }

    // Note: Extracting from connection info would require access to ConnectInfo
    // which needs to be added via a layer. For now, we rely on headers.
    None
}

/// IP filtering middleware
pub async fn ip_filter_middleware(
    filter: IpFilter,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
+ Clone {
    move |req: Request, next: Next| {
        let filter = filter.clone();
        Box::pin(async move {
            let ip = extract_ip(&req);

            let request_id = req
                .extensions()
                .get::<RequestId>()
                .map(|r| r.0.clone())
                .unwrap_or_else(|| "unknown".to_string());

            if let Some(ip_addr) = ip {
                filter.record_request(ip_addr);

                if !filter.is_allowed(ip_addr) {
                    tracing::warn!(
                        request_id = %request_id,
                        ip = %ip_addr,
                        "Request blocked by IP filter"
                    );
                    return (
                        StatusCode::FORBIDDEN,
                        "Access denied: IP address is blacklisted",
                    )
                        .into_response();
                }
            }

            next.run(req).await
        })
    }
}

/// IP filter statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct IpFilterStats {
    /// Number of dynamically blacklisted IP addresses
    pub blacklisted_ips: usize,
    /// Total number of IP addresses with tracked requests
    pub total_requests_tracked: usize,
    /// Number of IPs in the whitelist
    pub whitelist_size: usize,
    /// Number of IPs in the permanent blacklist
    pub permanent_blacklist_size: usize,
}

impl IpFilter {
    /// Get statistics about IP filtering
    pub fn get_stats(&self) -> IpFilterStats {
        IpFilterStats {
            blacklisted_ips: self.get_blacklisted_ips().len(),
            total_requests_tracked: self.request_counts.len(),
            whitelist_size: self.config.whitelist.len(),
            permanent_blacklist_size: self.config.blacklist.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = IpFilterConfig::default();
        assert!(!config.enabled);
        assert!(config.whitelist.is_empty());
        assert!(config.blacklist.is_empty());
    }

    #[test]
    fn test_disabled_filter_allows_all() {
        let config = IpFilterConfig {
            enabled: false,
            ..Default::default()
        };
        let filter = IpFilter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        assert!(filter.is_allowed(ip));
    }

    #[test]
    fn test_permanent_blacklist() {
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let config = IpFilterConfig {
            enabled: true,
            blacklist: vec![ip],
            ..Default::default()
        };
        let filter = IpFilter::new(config);
        assert!(!filter.is_allowed(ip));
    }

    #[test]
    fn test_whitelist_only_mode() {
        let allowed_ip: IpAddr = "192.168.1.1".parse().unwrap();
        let blocked_ip: IpAddr = "192.168.1.2".parse().unwrap();
        let config = IpFilterConfig {
            enabled: true,
            whitelist: vec![allowed_ip],
            ..Default::default()
        };
        let filter = IpFilter::new(config);
        assert!(filter.is_allowed(allowed_ip));
        assert!(!filter.is_allowed(blocked_ip));
    }

    #[test]
    fn test_manual_blacklist_addition() {
        let config = IpFilterConfig {
            enabled: true,
            ..Default::default()
        };
        let filter = IpFilter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        assert!(filter.is_allowed(ip));
        filter.add_to_blacklist(ip, Duration::from_secs(3600));
        assert!(!filter.is_allowed(ip));
    }

    #[test]
    fn test_blacklist_removal() {
        let config = IpFilterConfig {
            enabled: true,
            ..Default::default()
        };
        let filter = IpFilter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        filter.add_to_blacklist(ip, Duration::from_secs(3600));
        assert!(!filter.is_allowed(ip));

        filter.remove_from_blacklist(ip);
        assert!(filter.is_allowed(ip));
    }

    #[test]
    fn test_auto_blacklist() {
        let config = IpFilterConfig {
            enabled: true,
            auto_blacklist: true,
            auto_blacklist_threshold: 10,
            ..Default::default()
        };
        let filter = IpFilter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // Should be allowed initially
        assert!(filter.is_allowed(ip));

        // Simulate excessive requests
        for _ in 0..15 {
            filter.record_request(ip);
        }

        // Should be auto-blacklisted
        assert!(!filter.is_allowed(ip));
    }

    #[test]
    fn test_get_stats() {
        let config = IpFilterConfig {
            enabled: true,
            auto_blacklist: true,
            ..Default::default()
        };
        let filter = IpFilter::new(config);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        filter.add_to_blacklist(ip, Duration::from_secs(3600));
        filter.record_request(ip);

        let stats = filter.get_stats();
        assert_eq!(stats.blacklisted_ips, 1);
        assert!(stats.total_requests_tracked > 0);
    }
}
