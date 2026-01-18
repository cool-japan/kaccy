# kaccy-bitcoin Deployment Guide

This guide provides comprehensive instructions for deploying `kaccy-bitcoin` to production, including security hardening, performance tuning, and monitoring setup.

## Table of Contents

1. [Production Deployment Checklist](#production-deployment-checklist)
2. [Security Hardening Guide](#security-hardening-guide)
3. [Performance Tuning Guide](#performance-tuning-guide)
4. [Monitoring and Alerting Setup](#monitoring-and-alerting-setup)
5. [Disaster Recovery](#disaster-recovery)
6. [Scaling Strategies](#scaling-strategies)

---

## Production Deployment Checklist

### Pre-Deployment Requirements

#### Infrastructure

- [ ] **Bitcoin Core Node**
  - [ ] Fully synced Bitcoin Core node (mainnet/testnet)
  - [ ] Version 24.0+ recommended
  - [ ] Sufficient disk space (600GB+ for mainnet, growing)
  - [ ] Pruning configured if needed (minimum 550 blocks)
  - [ ] RPC enabled with strong authentication
  - [ ] Network connectivity verified
  - [ ] Backup node available (optional but recommended)

- [ ] **LND Node** (if using Lightning)
  - [ ] LND version 0.16.0+ recommended
  - [ ] Synced to Bitcoin Core
  - [ ] Channels opened with adequate liquidity
  - [ ] Macaroon authentication configured
  - [ ] REST API enabled and accessible
  - [ ] Watchtower configured (for channel monitoring)

- [ ] **Server Resources**
  - [ ] CPU: 4+ cores recommended
  - [ ] RAM: 8GB+ minimum, 16GB+ recommended
  - [ ] Disk: SSD storage for databases and caches
  - [ ] Network: Stable, low-latency connection
  - [ ] OS: Linux (Ubuntu 22.04 LTS recommended)

#### Security

- [ ] **Network Security**
  - [ ] Firewall configured (UFW/iptables)
  - [ ] Only necessary ports open (RPC, REST API)
  - [ ] SSH key-based authentication only
  - [ ] SSH on non-standard port
  - [ ] Fail2ban configured
  - [ ] DDoS protection in place

- [ ] **Application Security**
  - [ ] XPUB stored securely (environment variable or secrets manager)
  - [ ] RPC credentials in secure storage (NOT in code)
  - [ ] TLS certificates for all external connections
  - [ ] API rate limiting configured
  - [ ] CORS properly configured
  - [ ] Input validation on all endpoints

- [ ] **Key Management**
  - [ ] Hardware wallet for production signing
  - [ ] Multi-signature setup for high-value transactions
  - [ ] Cold storage for reserves
  - [ ] Key rotation policy documented
  - [ ] Recovery seed backed up offline (multiple secure locations)

#### Configuration

- [ ] **Environment Variables**
  ```bash
  # Bitcoin Core RPC
  BITCOIN_RPC_URL=http://localhost:8332
  BITCOIN_RPC_USER=<strong-username>
  BITCOIN_RPC_PASSWORD=<strong-password>
  BITCOIN_NETWORK=mainnet  # or testnet, regtest

  # HD Wallet
  XPUB=<your-xpub-here>  # NEVER share this publicly

  # LND (if using Lightning)
  LND_REST_URL=https://localhost:8080
  LND_MACAROON_PATH=/path/to/admin.macaroon
  LND_CERT_PATH=/path/to/tls.cert

  # Monitoring
  MONITOR_POLL_INTERVAL_SECS=30
  CONFIRMATION_THRESHOLD=6

  # Limits
  MAX_SINGLE_TX_SATS=100000000  # 1 BTC
  MAX_DAILY_TX_SATS=1000000000  # 10 BTC

  # Logging
  RUST_LOG=info,kaccy_bitcoin=debug
  LOG_FORMAT=json  # for structured logging

  # Metrics
  METRICS_ENABLED=true
  METRICS_PORT=9090
  ```

- [ ] **Configuration Files**
  - [ ] `bitcoin.conf` optimized (see Performance Tuning)
  - [ ] `lnd.conf` optimized (if using Lightning)
  - [ ] Application config reviewed
  - [ ] Backup configuration documented

#### Database

- [ ] **PostgreSQL Setup** (via kaccy-db)
  - [ ] PostgreSQL 14+ installed
  - [ ] Database created with appropriate permissions
  - [ ] Connection pooling configured
  - [ ] Backups automated (daily minimum)
  - [ ] Backup restoration tested
  - [ ] Replication configured (for high availability)

#### Testing

- [ ] **Pre-Production Testing**
  - [ ] All unit tests pass: `cargo test`
  - [ ] Integration tests pass on testnet
  - [ ] Load testing completed
  - [ ] Failover scenarios tested
  - [ ] Backup restoration tested
  - [ ] Transaction signing workflow tested
  - [ ] Hardware wallet integration tested

#### Monitoring

- [ ] **Observability Stack**
  - [ ] Prometheus configured and scraping metrics
  - [ ] Grafana dashboards imported
  - [ ] Alertmanager configured
  - [ ] Log aggregation setup (ELK/Loki)
  - [ ] Uptime monitoring (external service)
  - [ ] Error tracking (Sentry/similar)

### Deployment Steps

#### 1. Build and Package

```bash
# Build optimized release binary
cargo build --release --package kaccy-bitcoin

# Run tests one final time
cargo test --package kaccy-bitcoin

# Check for warnings (NO WARNINGS POLICY)
cargo clippy --package kaccy-bitcoin --all-targets --all-features

# Create deployment package
tar -czf kaccy-bitcoin-v0.1.0.tar.gz \
  target/release/kaccy-bitcoin \
  ARCHITECTURE.md \
  DEPLOYMENT.md \
  README.md
```

#### 2. Server Preparation

```bash
# Update system packages
sudo apt update && sudo apt upgrade -y

# Install dependencies
sudo apt install -y \
  build-essential \
  pkg-config \
  libssl-dev \
  postgresql-client

# Create application user
sudo useradd -r -s /bin/false kaccy

# Create directories
sudo mkdir -p /opt/kaccy-bitcoin
sudo mkdir -p /var/log/kaccy-bitcoin
sudo chown -R kaccy:kaccy /opt/kaccy-bitcoin /var/log/kaccy-bitcoin
```

#### 3. Deploy Application

```bash
# Copy binary
sudo cp target/release/kaccy-bitcoin /opt/kaccy-bitcoin/
sudo chown kaccy:kaccy /opt/kaccy-bitcoin/kaccy-bitcoin
sudo chmod 755 /opt/kaccy-bitcoin/kaccy-bitcoin

# Copy configuration
sudo cp .env.production /opt/kaccy-bitcoin/.env
sudo chown kaccy:kaccy /opt/kaccy-bitcoin/.env
sudo chmod 600 /opt/kaccy-bitcoin/.env  # Strict permissions

# Create systemd service
sudo nano /etc/systemd/system/kaccy-bitcoin.service
```

#### 4. Systemd Service Configuration

```ini
[Unit]
Description=Kaccy Bitcoin Integration Service
After=network.target postgresql.service bitcoind.service
Wants=postgresql.service bitcoind.service

[Service]
Type=simple
User=kaccy
Group=kaccy
WorkingDirectory=/opt/kaccy-bitcoin
EnvironmentFile=/opt/kaccy-bitcoin/.env
ExecStart=/opt/kaccy-bitcoin/kaccy-bitcoin
Restart=on-failure
RestartSec=10s
StandardOutput=append:/var/log/kaccy-bitcoin/stdout.log
StandardError=append:/var/log/kaccy-bitcoin/stderr.log

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/kaccy-bitcoin

# Resource limits
LimitNOFILE=65536
MemoryMax=4G

[Install]
WantedBy=multi-user.target
```

#### 5. Start Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service (start on boot)
sudo systemctl enable kaccy-bitcoin

# Start service
sudo systemctl start kaccy-bitcoin

# Check status
sudo systemctl status kaccy-bitcoin

# View logs
sudo journalctl -u kaccy-bitcoin -f
```

#### 6. Health Check

```bash
# Check Bitcoin RPC connectivity
curl -u $BITCOIN_RPC_USER:$BITCOIN_RPC_PASSWORD \
  -X POST \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"1.0","id":"test","method":"getnetworkinfo","params":[]}' \
  $BITCOIN_RPC_URL

# Check LND connectivity (if using Lightning)
curl --cacert $LND_CERT_PATH \
  --header "Grpc-Metadata-macaroon: $(xxd -ps -u -c 1000 $LND_MACAROON_PATH)" \
  $LND_REST_URL/v1/getinfo

# Check metrics endpoint
curl http://localhost:9090/metrics
```

### Post-Deployment Verification

- [ ] Service is running: `systemctl status kaccy-bitcoin`
- [ ] Bitcoin Core connectivity verified
- [ ] LND connectivity verified (if applicable)
- [ ] Database connectivity verified
- [ ] Metrics being collected
- [ ] Logs being written
- [ ] Address generation works
- [ ] Payment monitoring active
- [ ] Test transaction processed successfully
- [ ] Alerts triggering correctly
- [ ] Backup job running

---

## Security Hardening Guide

### 1. Network Security

#### Firewall Configuration (UFW)

```bash
# Default deny all
sudo ufw default deny incoming
sudo ufw default allow outgoing

# SSH (change 22 to your custom port)
sudo ufw allow 2222/tcp

# Bitcoin Core RPC (only from localhost or specific IPs)
sudo ufw allow from 127.0.0.1 to any port 8332

# LND REST API (only from localhost or specific IPs)
sudo ufw allow from 127.0.0.1 to any port 8080

# Prometheus metrics (only from monitoring server)
sudo ufw allow from <monitoring-server-ip> to any port 9090

# Enable firewall
sudo ufw enable
```

#### Fail2ban Configuration

```bash
# Install fail2ban
sudo apt install fail2ban

# Configure for SSH
sudo nano /etc/fail2ban/jail.local
```

```ini
[sshd]
enabled = true
port = 2222
filter = sshd
logpath = /var/log/auth.log
maxretry = 3
bantime = 3600
findtime = 600
```

```bash
# Restart fail2ban
sudo systemctl restart fail2ban
```

#### TLS/SSL Configuration

```bash
# Install certbot for Let's Encrypt
sudo apt install certbot

# Obtain certificate (for API endpoints)
sudo certbot certonly --standalone -d api.yourdomain.com

# Configure automatic renewal
sudo crontab -e
# Add: 0 3 * * * certbot renew --quiet
```

### 2. Bitcoin Core Security

#### bitcoin.conf Hardening

```ini
# Network
testnet=0  # Set to 1 for testnet
mainnet=1

# RPC
server=1
rpcuser=<strong-random-username>
rpcpassword=<strong-random-password>  # Use rpcauth for better security
rpcallowip=127.0.0.1
rpcbind=127.0.0.1
rpcport=8332

# Use rpcauth instead of rpcpassword (more secure)
# Generate with: bitcoin-cli rpcauth <username>
rpcauth=kaccy:7d9ba489ba890f98df89a7d890c7a8d$890c7d89c0a7d89c0a7d89c0a7d89c

# Connection limits
maxconnections=125
maxuploadtarget=5000  # MB per day (if bandwidth limited)

# Performance
dbcache=4096  # MB (adjust based on available RAM)
maxmempool=300  # MB
mempoolexpiry=72  # hours

# Privacy
blocksonly=0  # Set to 1 if you don't need transaction relay
listen=1
discover=1

# ZMQ (for real-time notifications, optional)
zmqpubrawblock=tcp://127.0.0.1:28332
zmqpubrawtx=tcp://127.0.0.1:28333

# Wallet (disable if not needed)
disablewallet=1

# Logging
debug=rpc
debug=mempool
```

### 3. LND Security (if using Lightning)

#### lnd.conf Hardening

```ini
[Application Options]
listen=localhost:9735
rpclisten=localhost:10009
restlisten=localhost:8080
alias=kaccy-node
color=#ff9900

[Bitcoin]
bitcoin.active=1
bitcoin.mainnet=1
bitcoin.node=bitcoind

[Bitcoind]
bitcoind.rpchost=localhost:8332
bitcoind.rpcuser=<bitcoin-rpc-user>
bitcoind.rpcpass=<bitcoin-rpc-password>
bitcoind.zmqpubrawblock=tcp://127.0.0.1:28332
bitcoind.zmqpubrawtx=tcp://127.0.0.1:28333

[tor]
tor.active=false  # Enable for enhanced privacy
tor.socks=9050
tor.dns=socks

[autopilot]
autopilot.active=false  # Manual channel management recommended

[watchtower]
watchtower.active=true  # Monitor channels for breaches
```

### 4. Key Management Best Practices

#### XPUB Storage

```bash
# Use environment variables (not in code)
export XPUB="xpub6..."

# Or use a secrets manager (AWS Secrets Manager, HashiCorp Vault, etc.)
# Example with Vault:
vault kv put secret/kaccy/bitcoin xpub="xpub6..."

# In application, retrieve:
vault kv get -field=xpub secret/kaccy/bitcoin
```

#### Hardware Wallet Setup

```bash
# Install HWI (Hardware Wallet Interface)
pip3 install hwi

# List connected devices
hwi enumerate

# Get master public key (XPUB) from hardware wallet
hwi -t ledger getmasterxpub "m/84'/0'/0'"

# Sign PSBT with hardware wallet
hwi -t ledger signtx <base64-psbt>
```

#### Multi-Signature Configuration

```rust
// Configure 2-of-3 multisig for high-value transactions
let multisig_config = MultisigConfig {
    threshold: 2,
    total_keys: 3,
    keys: vec![
        platform_pubkey,    // Hot wallet (platform-controlled)
        user_pubkey,        // User's hardware wallet
        cold_storage_pubkey // Cold storage (offline)
    ],
};

// Use custody manager for tiered security
let custody_manager = CustodyManager::new(multisig_wallet);

// Automatically move funds above threshold to multisig
if balance > 100_000_000 {  // 1 BTC
    custody_manager.move_to_cold_storage(amount)?;
}
```

### 5. Application-Level Security

#### Rate Limiting

```rust
// Configure rate limits to prevent abuse
let rate_limit_config = RateLimitConfig {
    max_requests_per_minute: 60,
    max_requests_per_hour: 1000,
    burst_size: 10,
};
```

#### Transaction Limits

```rust
// Configure transaction limits
let limit_config = LimitConfig {
    single_tx_max: Amount::from_sat(100_000_000),  // 1 BTC
    hourly_max: Amount::from_sat(500_000_000),     // 5 BTC
    daily_max: Amount::from_sat(1_000_000_000),    // 10 BTC
    weekly_max: Amount::from_sat(5_000_000_000),   // 50 BTC
};
```

#### Activity Monitoring

```rust
// Configure activity monitoring for suspicious patterns
let activity_config = ActivityMonitorConfig {
    large_tx_threshold: Amount::from_sat(50_000_000),  // 0.5 BTC
    rapid_succession_window: Duration::from_secs(300), // 5 minutes
    rapid_succession_count: 5,
    velocity_limit_per_hour: Amount::from_sat(200_000_000), // 2 BTC
    risk_score_threshold: 70,  // 0-100 scale
};
```

#### Audit Logging

```rust
// Enable comprehensive audit logging
let audit_logger = AuditLogger::new(
    FileAuditStorage::new("/var/log/kaccy-bitcoin/audit.log")?
);

// Log all critical operations
audit_logger.log(
    AuditEventBuilder::new(AuditEventType::TransactionCreated)
        .user_id(user_id)
        .severity(AuditSeverity::High)
        .resource_id(tx_id)
        .details("Withdrawal transaction created")
        .build()
)?;
```

### 6. Input Validation

```rust
// Always validate addresses
fn validate_withdrawal_address(address: &str, network: Network) -> Result<Address> {
    let addr = Address::from_str(address)
        .map_err(|_| Error::InvalidAddress)?;

    if addr.network != network {
        return Err(Error::NetworkMismatch);
    }

    // Additional validation for specific address types
    match addr.address_type() {
        Some(AddressType::P2pkh) |
        Some(AddressType::P2sh) |
        Some(AddressType::P2wpkh) |
        Some(AddressType::P2wsh) |
        Some(AddressType::P2tr) => Ok(addr),
        _ => Err(Error::UnsupportedAddressType),
    }
}

// Always validate amounts
fn validate_amount(amount: Amount) -> Result<()> {
    if amount.to_sat() < 546 {  // Dust threshold
        return Err(Error::AmountTooSmall);
    }

    if amount.to_sat() > 21_000_000 * 100_000_000 {  // Max BTC supply
        return Err(Error::AmountTooLarge);
    }

    Ok(())
}
```

### 7. Secrets Rotation

```bash
# Rotate RPC credentials quarterly
# 1. Generate new credentials
NEW_RPC_USER=$(openssl rand -hex 16)
NEW_RPC_PASS=$(openssl rand -hex 32)

# 2. Update bitcoin.conf with new rpcauth
bitcoin-cli rpcauth $NEW_RPC_USER

# 3. Update application environment
# 4. Restart Bitcoin Core
# 5. Restart application
# 6. Verify connectivity
# 7. Remove old credentials
```

---

## Performance Tuning Guide

### 1. Bitcoin Core Optimization

#### Recommended bitcoin.conf Settings

```ini
# Memory
dbcache=8192  # MB (use 50% of available RAM, max 16GB)
maxmempool=1024  # MB

# Network performance
maxconnections=125
maxuploadtarget=0  # Unlimited (remove if bandwidth-constrained)

# Block processing
par=4  # Script verification threads (adjust to CPU cores)

# RPC optimization
rpcthreads=16  # RPC worker threads
rpcworkqueue=32  # RPC work queue depth

# Transaction relay
minrelaytxfee=0.00001000  # Min relay fee (BTC/kB)
blockmaxweight=3996000  # Max block weight
```

### 2. Application Configuration

#### Connection Pooling

```rust
// Configure connection pool for high concurrency
let pool_config = PoolConfig {
    max_size: 20,  // Maximum connections
    min_idle: 5,   // Minimum idle connections
    max_lifetime: Duration::from_secs(1800),  // 30 minutes
    idle_timeout: Duration::from_secs(600),   // 10 minutes
    connection_timeout: Duration::from_secs(30),
};

let pool = ConnectionPool::new(bitcoin_client, pool_config)?;
```

#### Caching Strategy

```rust
// Configure caching for optimal performance
let cache_config = CacheConfig {
    // Transaction cache
    tx_cache_size: 10_000,
    tx_cache_ttl: Duration::from_secs(3600),  // 1 hour

    // UTXO cache
    utxo_cache_size: 50_000,
    utxo_cache_ttl: Duration::from_secs(300),  // 5 minutes

    // Block header cache
    block_header_cache_size: 1_000,
    block_header_ttl: None,  // Never expire (immutable)

    // Address cache
    address_cache_size: 100_000,
};

let cache_manager = CacheManager::new(cache_config);
```

#### Batch Processing

```rust
// Batch withdrawals to reduce on-chain fees
let batch_optimizer = BatchOptimizer::new(BatchStrategy::MinimizeFees);

// Collect withdrawal requests
let mut withdrawals = Vec::new();
withdrawals.push(WithdrawalRequest { address: addr1, amount: amt1 });
withdrawals.push(WithdrawalRequest { address: addr2, amount: amt2 });
// ... more withdrawals

// Optimize batch (combines compatible requests)
let optimized = batch_optimizer.optimize(withdrawals, fee_rate)?;

// Result: Fewer transactions, lower total fees
```

#### UTXO Management

```rust
// Configure UTXO consolidation
let consolidation_config = ConsolidationConfig {
    target_utxo_count: 100,  // Consolidate if more than this
    min_utxo_count: 50,      // Keep at least this many
    max_fee_rate: 5.0,       // sats/vbyte (only consolidate at low fees)
    min_consolidation_value: Amount::from_sat(10_000),
};

// Run consolidation during low-fee periods
if current_fee_rate <= consolidation_config.max_fee_rate {
    let plan = utxo_manager.get_consolidation_plan(&consolidation_config)?;
    if let Some(plan) = plan {
        utxo_manager.execute_consolidation(plan)?;
    }
}
```

### 3. Database Optimization (PostgreSQL)

#### postgresql.conf Tuning

```ini
# Memory
shared_buffers = 4GB  # 25% of system RAM
effective_cache_size = 12GB  # 75% of system RAM
work_mem = 64MB
maintenance_work_mem = 1GB

# Query planning
random_page_cost = 1.1  # Lower for SSD
effective_io_concurrency = 200  # Higher for SSD

# Write performance
wal_buffers = 16MB
checkpoint_completion_target = 0.9
max_wal_size = 4GB
min_wal_size = 1GB

# Connections
max_connections = 100
```

#### Index Optimization

```sql
-- Create indexes for common queries
CREATE INDEX idx_orders_btc_address ON orders(btc_address);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_created_at ON orders(created_at);
CREATE INDEX idx_transactions_txid ON transactions(txid);
CREATE INDEX idx_transactions_order_id ON transactions(order_id);

-- Analyze tables regularly
ANALYZE orders;
ANALYZE transactions;
```

### 4. System-Level Optimization

#### Increase File Descriptors

```bash
# Edit /etc/security/limits.conf
kaccy soft nofile 65536
kaccy hard nofile 65536

# Edit /etc/systemd/system/kaccy-bitcoin.service
[Service]
LimitNOFILE=65536
```

#### TCP Tuning

```bash
# Edit /etc/sysctl.conf
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 67108864
net.ipv4.tcp_wmem = 4096 65536 67108864
net.ipv4.tcp_congestion_control = bbr

# Apply changes
sudo sysctl -p
```

#### SSD Optimization

```bash
# Enable TRIM for SSD
sudo systemctl enable fstrim.timer
sudo systemctl start fstrim.timer

# Verify TRIM is enabled
sudo fstrim -v /
```

### 5. Monitoring Performance

#### Key Metrics to Track

```rust
// Track RPC call latency
metrics.record_histogram("bitcoin_rpc_latency_ms", latency.as_millis());

// Track cache hit rates
metrics.record_gauge("cache_hit_rate", cache.hit_rate());

// Track transaction throughput
metrics.increment_counter("transactions_processed_total");

// Track UTXO count
metrics.record_gauge("utxo_count", utxo_count);

// Track memory usage
metrics.record_gauge("memory_usage_bytes", memory_usage);
```

#### Benchmarking

```bash
# Run benchmarks
cargo bench --package kaccy-bitcoin

# Key benchmarks:
# - Address derivation: <100μs
# - Transaction parsing: <1ms
# - UTXO selection: <10ms
# - Batch optimization: <100ms
```

---

## Monitoring and Alerting Setup

### 1. Prometheus Configuration

#### prometheus.yml

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'kaccy-bitcoin'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: /metrics

  - job_name: 'bitcoin-core'
    static_configs:
      - targets: ['localhost:8332']
    metrics_path: /metrics  # Requires bitcoin-prometheus-exporter

  - job_name: 'lnd'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /v1/metrics  # LND built-in metrics

  - job_name: 'postgres'
    static_configs:
      - targets: ['localhost:9187']  # postgres_exporter
```

### 2. Grafana Dashboards

#### kaccy-bitcoin Dashboard

Create a Grafana dashboard with the following panels:

**Panel 1: Transaction Volume**
```promql
# Transactions processed per hour
rate(transactions_processed_total[1h])

# Transaction volume (BTC)
rate(transaction_volume_btc_total[1h])
```

**Panel 2: Payment Status**
```promql
# Payments by status
sum by (status) (payments_by_status)
```

**Panel 3: RPC Performance**
```promql
# RPC call latency (p50, p95, p99)
histogram_quantile(0.50, rate(bitcoin_rpc_latency_ms_bucket[5m]))
histogram_quantile(0.95, rate(bitcoin_rpc_latency_ms_bucket[5m]))
histogram_quantile(0.99, rate(bitcoin_rpc_latency_ms_bucket[5m]))
```

**Panel 4: Cache Performance**
```promql
# Cache hit rate
cache_hit_rate

# Cache size
cache_size_bytes
```

**Panel 5: Confirmation Times**
```promql
# Average confirmation time
avg(confirmation_time_seconds)
```

**Panel 6: UTXO Management**
```promql
# UTXO count
utxo_count

# UTXO value distribution
histogram_quantile(0.50, utxo_value_satoshis_bucket)
```

**Panel 7: Error Rates**
```promql
# Errors per minute
rate(errors_total[1m])

# Errors by type
sum by (error_type) (errors_total)
```

**Panel 8: Resource Usage**
```promql
# Memory usage
memory_usage_bytes

# CPU usage
process_cpu_seconds_total
```

### 3. Alerting Rules

#### alerting_rules.yml

```yaml
groups:
  - name: kaccy-bitcoin
    interval: 30s
    rules:
      # Bitcoin Core connectivity
      - alert: BitcoinCoreDown
        expr: up{job="bitcoin-core"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Bitcoin Core is down"
          description: "Bitcoin Core has been down for more than 1 minute"

      # LND connectivity
      - alert: LndDown
        expr: up{job="lnd"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "LND is down"
          description: "LND has been down for more than 1 minute"

      # High RPC latency
      - alert: HighRpcLatency
        expr: histogram_quantile(0.95, rate(bitcoin_rpc_latency_ms_bucket[5m])) > 1000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High RPC latency detected"
          description: "95th percentile RPC latency is {{ $value }}ms"

      # Low cache hit rate
      - alert: LowCacheHitRate
        expr: cache_hit_rate < 0.7
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Low cache hit rate"
          description: "Cache hit rate is {{ $value }}"

      # High error rate
      - alert: HighErrorRate
        expr: rate(errors_total[5m]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value }} errors/second"

      # Suspicious activity detected
      - alert: SuspiciousActivity
        expr: suspicious_activity_detected > 0
        for: 1m
        labels:
          severity: high
        annotations:
          summary: "Suspicious activity detected"
          description: "Risk score: {{ $value }}"

      # Large transaction
      - alert: LargeTransaction
        expr: large_transaction_detected > 0
        for: 1m
        labels:
          severity: info
        annotations:
          summary: "Large transaction detected"
          description: "Transaction value: {{ $value }} satoshis"

      # Payment underpaid
      - alert: PaymentUnderpaid
        expr: payment_underpaid_detected > 0
        for: 1m
        labels:
          severity: warning
        annotations:
          summary: "Payment underpaid"
          description: "Order ID: {{ $labels.order_id }}"

      # UTXO count too high
      - alert: HighUtxoCount
        expr: utxo_count > 1000
        for: 30m
        labels:
          severity: info
        annotations:
          summary: "High UTXO count"
          description: "UTXO count is {{ $value }}, consider consolidation"

      # Disk space low
      - alert: LowDiskSpace
        expr: node_filesystem_free_bytes{mountpoint="/"} / node_filesystem_size_bytes{mountpoint="/"} < 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Low disk space"
          description: "Disk usage is {{ $value | humanizePercentage }}"

      # Memory usage high
      - alert: HighMemoryUsage
        expr: memory_usage_bytes > 7 * 1024 * 1024 * 1024  # 7GB
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage"
          description: "Memory usage is {{ $value | humanize1024 }}"
```

### 4. Alertmanager Configuration

#### alertmanager.yml

```yaml
global:
  resolve_timeout: 5m
  smtp_smarthost: 'smtp.gmail.com:587'
  smtp_from: 'alerts@yourdomain.com'
  smtp_auth_username: 'alerts@yourdomain.com'
  smtp_auth_password: 'your-password'

route:
  receiver: 'default'
  group_by: ['alertname', 'cluster', 'service']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h

  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'
      continue: true

    - match:
        severity: warning
      receiver: 'slack'

receivers:
  - name: 'default'
    email_configs:
      - to: 'ops@yourdomain.com'

  - name: 'slack'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/WEBHOOK/URL'
        channel: '#alerts'
        title: 'Alert: {{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.summary }}\n{{ .Annotations.description }}\n{{ end }}'

  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: 'your-pagerduty-service-key'
```

### 5. Log Aggregation (Loki/ELK)

#### Promtail Configuration (for Loki)

```yaml
server:
  http_listen_port: 9080
  grpc_listen_port: 0

positions:
  filename: /tmp/positions.yaml

clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: kaccy-bitcoin
    static_configs:
      - targets:
          - localhost
        labels:
          job: kaccy-bitcoin
          __path__: /var/log/kaccy-bitcoin/*.log
    pipeline_stages:
      - json:
          expressions:
            level: level
            timestamp: timestamp
            message: message
            correlation_id: correlation_id
      - labels:
          level:
          correlation_id:
      - timestamp:
          source: timestamp
          format: RFC3339
```

### 6. Structured Logging

```rust
// Configure structured logging
use tracing::{info, warn, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_logging() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,kaccy_bitcoin=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();
}

// Log with structured fields
info!(
    order_id = %order_id,
    address = %address,
    amount_sats = amount.to_sat(),
    "Generated new payment address"
);

// Log with correlation ID for request tracing
let correlation_id = CorrelationId::new();
info!(
    correlation_id = %correlation_id,
    tx_id = %tx_id,
    confirmations = confirmations,
    "Transaction confirmed"
);
```

### 7. Health Checks

```rust
// Implement comprehensive health checks
let health_manager = HealthCheckManager::new(
    bitcoin_client.clone(),
    Some(lnd_client.clone()),
);

// Run health check
let health_status = health_manager.check_health().await?;

// Expose health endpoint for load balancers
// GET /health
// Response:
{
  "status": "healthy",
  "components": {
    "bitcoin_core": {
      "status": "healthy",
      "latency_ms": 15,
      "block_height": 780000
    },
    "lnd": {
      "status": "healthy",
      "latency_ms": 8,
      "synced": true
    },
    "database": {
      "status": "healthy",
      "latency_ms": 3
    },
    "cache": {
      "status": "healthy",
      "hit_rate": 0.92
    }
  },
  "timestamp": "2026-01-01T00:00:00Z"
}
```

---

## Disaster Recovery

### 1. Backup Strategy

#### What to Backup

- [ ] **Bitcoin Core**
  - Wallet.dat (if using Core wallet - NOT recommended for production)
  - bitcoin.conf configuration

- [ ] **LND**
  - channel.backup (critical!)
  - lnd.conf configuration
  - Macaroon files
  - TLS certificates

- [ ] **Application**
  - XPUB (securely encrypted)
  - Configuration files (.env, config.toml)
  - Address book / order-address mapping

- [ ] **Database**
  - Full PostgreSQL dumps
  - WAL (Write-Ahead Log) for point-in-time recovery

#### Backup Schedule

```bash
#!/bin/bash
# /opt/kaccy-bitcoin/backup.sh

DATE=$(date +%Y%m%d-%H%M%S)
BACKUP_DIR="/backup/kaccy-bitcoin"

# Database backup
pg_dump -U kaccy kaccy_db | gzip > "$BACKUP_DIR/db-$DATE.sql.gz"

# LND channel backup
cp /home/lnd/.lnd/data/chain/bitcoin/mainnet/channel.backup \
   "$BACKUP_DIR/channel-backup-$DATE"

# Configuration backup
tar -czf "$BACKUP_DIR/config-$DATE.tar.gz" \
  /opt/kaccy-bitcoin/.env \
  /etc/systemd/system/kaccy-bitcoin.service

# Encrypt and upload to S3
gpg --encrypt --recipient ops@yourdomain.com \
  "$BACKUP_DIR/db-$DATE.sql.gz"

aws s3 cp "$BACKUP_DIR/db-$DATE.sql.gz.gpg" \
  s3://kaccy-backups/bitcoin/

# Retention: Keep last 30 days locally, 1 year in S3
find $BACKUP_DIR -type f -mtime +30 -delete
```

#### Automate Backups

```bash
# Add to crontab
crontab -e

# Daily at 3 AM
0 3 * * * /opt/kaccy-bitcoin/backup.sh

# LND channel backup (every 6 hours - critical!)
0 */6 * * * cp /home/lnd/.lnd/data/chain/bitcoin/mainnet/channel.backup /backup/lnd/
```

### 2. Recovery Procedures

#### Database Recovery

```bash
# Stop application
sudo systemctl stop kaccy-bitcoin

# Restore database
gunzip < /backup/kaccy-bitcoin/db-20260101-030000.sql.gz | \
  psql -U kaccy kaccy_db

# Verify data integrity
psql -U kaccy kaccy_db -c "SELECT COUNT(*) FROM orders;"

# Start application
sudo systemctl start kaccy-bitcoin
```

#### LND Channel Recovery

```bash
# Stop LND
sudo systemctl stop lnd

# Restore channel backup
cp /backup/lnd/channel-backup-latest \
   /home/lnd/.lnd/data/chain/bitcoin/mainnet/channel.backup

# Start LND
sudo systemctl start lnd

# Verify channels
lncli listchannels
```

### 3. Failover Procedures

#### Active-Passive Setup

```
Primary Server (Active)
      │
      ├──► Bitcoin Core (Primary)
      ├──► LND (Primary)
      ├──► PostgreSQL (Primary with replication)
      │
      ▼
Secondary Server (Passive)
      │
      ├──► Bitcoin Core (Replica)
      ├──► LND (Standby)
      ├──► PostgreSQL (Replica with streaming replication)
```

#### Manual Failover

```bash
# 1. Promote replica database
sudo -u postgres /usr/lib/postgresql/14/bin/pg_ctl promote \
  -D /var/lib/postgresql/14/main

# 2. Update DNS/load balancer to point to secondary

# 3. Start application on secondary
sudo systemctl start kaccy-bitcoin

# 4. Verify health
curl http://secondary-server:9090/health

# 5. Investigate and fix primary server
```

---

## Scaling Strategies

### 1. Vertical Scaling

- Increase server resources (CPU, RAM, disk)
- Optimize PostgreSQL parameters for larger resources
- Increase connection pool sizes
- Increase cache sizes

### 2. Horizontal Scaling

#### Read Replicas for Bitcoin Core

```
Primary Bitcoin Core (RW)
      │
      ├──► Replica 1 (RO) - Handle payment monitoring
      ├──► Replica 2 (RO) - Handle withdrawals
      └──► Replica 3 (RO) - Handle analytics
```

#### Load Balancing

```nginx
# nginx load balancer config
upstream bitcoin_rpc {
    server 127.0.0.1:8332;
    server 127.0.0.1:8333;  # Replica
    server 127.0.0.1:8334;  # Replica
}

server {
    listen 8335;
    location / {
        proxy_pass http://bitcoin_rpc;
    }
}
```

### 3. Caching Layer (Redis)

```rust
// Shared cache for multiple instances
let redis_client = redis::Client::open("redis://127.0.0.1/")?;
let mut con = redis_client.get_connection()?;

// Cache transaction
con.set_ex(
    format!("tx:{}", txid),
    serde_json::to_string(&transaction)?,
    3600  // 1 hour TTL
)?;

// Retrieve from cache
if let Ok(cached) = con.get::<_, String>(format!("tx:{}", txid)) {
    return Ok(serde_json::from_str(&cached)?);
}
```

### 4. Microservices Separation

Split into focused services:
- **Payment Monitor Service**: Dedicated to monitoring payments
- **Transaction Service**: Handle withdrawals and transaction creation
- **Lightning Service**: LND integration
- **Analytics Service**: Reporting and analytics

### 5. Queue-Based Processing

```rust
// Use message queue for async processing
// Example with RabbitMQ
let connection = lapin::Connection::connect(
    "amqp://localhost:5672",
    ConnectionProperties::default(),
).await?;

// Publish withdrawal request
channel.basic_publish(
    "",
    "withdrawals",
    lapin::options::BasicPublishOptions::default(),
    serde_json::to_vec(&withdrawal_request)?,
    lapin::BasicProperties::default(),
).await?;

// Worker processes from queue
let consumer = channel.basic_consume(
    "withdrawals",
    "worker-1",
    lapin::options::BasicConsumeOptions::default(),
    lapin::types::FieldTable::default(),
).await?;
```

---

## Conclusion

This deployment guide provides comprehensive instructions for:
- ✅ Deploying kaccy-bitcoin to production securely
- ✅ Hardening security at all layers
- ✅ Optimizing performance for production workloads
- ✅ Setting up comprehensive monitoring and alerting
- ✅ Implementing disaster recovery procedures
- ✅ Planning for horizontal and vertical scaling

Follow this guide carefully and adapt configurations to your specific requirements. Always test in a staging environment before deploying to production.

For questions or issues, refer to the ARCHITECTURE.md for system design details or open an issue on GitHub.
