# G: Detailed Implementation Plan

# Kaccy Protocol - Detailed Implementation Plan v1.0

Perfect! Let's create a **Detailed Implementation Plan**.

We'll break down the MVP (12 weeks) into weekly task breakdowns, technical details, risk management, and checkpoints.

---

## 🎯 Implementation Plan Overview

### Core Principles

```rust
pub struct ImplementationPrinciples {
    principle_1: "Weekly sprints (Agile)",
    principle_2: "Demoable deliverables every Friday",
    principle_3: "Minimize technical debt (design that doesn't need refactoring later)",
    principle_4: "Maximize parallel work (frontend/backend simultaneous development)",
    principle_5: "Incorporate user feedback weekly",
}

pub struct Timeline {
    total_duration: "12 weeks (84 days)",
    sprints: 12,
    sprint_length: "1 week (7 days)",
    team_size: "2-3 people (You + 1-2 contractors)",
    working_hours_per_week: "40-60 hours (aggressive but sustainable)",
}

```

---

## 📅 12-Week Sprint Plan (Detailed)

### **Week 0: Pre-Development (Before Day 1)**

```yaml
Goal: Complete development environment setup and initial preparation

Tasks:
  Infrastructure Setup:
    - [ ] VPS contract (Hetzner: CPX41, 8vCPU/16GB RAM, €26/month)
        • Generate SSH key, 2FA setup
        • Firewall setup (open only ports 22, 80, 443)
    - [ ] Domain DNS setup (kaccy.net)
        • A record: VPS IP
        • CNAME: www → kaccy.net
        • MX record: Email (SendGrid)
    - [ ] SSL certificate (Let's Encrypt + Certbot)
        • Auto-renewal setup (cron job)
    - [ ] PostgreSQL 15 installation
        • Create user (kaccy_admin)
        • Create database (kaccy_db)
        • Restrict external connections (localhost + specific IP)
    - [ ] Bitcoin Core setup
        • Start full node sync (~500GB, takes 2-3 days)
        • Run Testnet in parallel
        • bitcoin.conf configuration (enable RPC)

  Development Environment:
    - [ ] Create GitHub Organization
        • Private repo: kaccy-backend
        • Private repo: kaccy-frontend
        • Private repo: kaccy-docs
    - [ ] Project management setup
        • GitHub Projects OR Linear.app
        • Sprint board (To Do / In Progress / Review / Done)
    - [ ] Local development environment
        • Rust 1.75+ installation (rustup)
        • Node.js 20 LTS installation
        • Docker Desktop (local PostgreSQL)
        • VSCode + extensions (rust-analyzer, Prettier, ESLint)

  Team & Communication:
    - [ ] Hire frontend developer (if not solo)
        • Post on Upwork/Twitter: "React/Next.js dev, 3 months, $5K/month"
        • Screen 5-10 candidates
        • Technical interview: Build a simple form with validation
        • Hire decision by Week 0 Friday
    - [ ] Communication setup
        • Slack workspace OR Discord server
        • Daily standup time: 9:00 AM (async OK)
        • Weekly demo: Friday 4:00 PM

  Legal & Compliance (Parallel Track):
    - [ ] Begin Estonian VASP application preparation
        • Consult with local lawyer (€2,000 retainer)
        • Get list of required documents
        • Draft AML/CFT policy
    - [ ] Terms of Service draft
        • Based on Option C legal framework
        • Request lawyer review

  User Research (Parallel Track):
    - [ ] Create beta user candidate list (50 people)
        • LinkedIn connections (Rust devs)
        • GitHub followers
        • Twitter mutuals (crypto/tech)
    - [ ] Conduct initial interviews (5 people)
        • Question: "Would you tokenize your consulting services?"
        • Question: "What utility would you offer to token holders?"
        • Question: "What's your biggest concern about Web3 platforms?"

Deliverables by Week 0 End:
  ✅ Infrastructure live (VPS + DB + Bitcoin node syncing)
  ✅ GitHub repos initialized
  ✅ Team hired (if applicable)
  ✅ 50-person beta user target list
  ✅ VASP application in progress

Estimated Effort: 20-30 hours

```

---

### **Sprint 1 (Week 1): Foundation - Authentication & User Management**

```yaml
Sprint Goal: Users can register and login

Backend Tasks (Rust/Axum):
  Day 1-2:
    - [ ] Project initialization
        • cargo new kaccy-backend
        • Dependencies: axum, tokio, sqlx, serde, jsonwebtoken
        • Folder structure:
            src/
            ├── main.rs
            ├── routes/
            │   ├── mod.rs
            │   ├── auth.rs
            │   └── users.rs
            ├── models/
            │   ├── mod.rs
            │   └── user.rs
            ├── db/
            │   ├── mod.rs
            │   └── pool.rs
            ├── middleware/
            │   ├── mod.rs
            │   └── auth.rs
            └── utils/
                ├── mod.rs
                └── error.rs

    - [ ] Database schema v1
        • SQLx migrations setup
        • CREATE TABLE users (from Option E MVP schema)
        • CREATE TABLE sessions (JWT tracking, optional)

    - [ ] User model & CRUD
        • User struct with Serialize/Deserialize
        • create_user(email, password_hash, username)
        • get_user_by_email(email)
        • get_user_by_id(user_id)

  Day 3-4:
    - [ ] Password hashing
        • Use argon2 crate (NOT bcrypt - argon2 is newer, better)
        • hash_password(password: &str) -> String
        • verify_password(password: &str, hash: &str) -> bool

    - [ ] JWT authentication
        • Dependencies: jsonwebtoken, chrono
        • generate_token(user_id: Uuid) -> String (exp: 1 hour)
        • verify_token(token: &str) -> Result<Claims, JwtError>
        • Middleware: require_auth() for protected routes

    - [ ] API endpoints
        • POST /api/auth/register
            Body: { email, password, username }
            Response: { user_id, token }
        • POST /api/auth/login
            Body: { email, password }
            Response: { user_id, token }
        • GET /api/users/me (protected)
            Headers: Authorization: Bearer <token>
            Response: { user object }

  Day 5:
    - [ ] Testing
        • Unit tests: password hashing, JWT generation
        • Integration tests: register -> login flow
        • Manual test with curl/Postman

    - [ ] Error handling
        • Custom error types: AuthError, DbError
        • Consistent JSON error responses
        • Logging setup (tracing crate)

Frontend Tasks (Next.js/React):
  Day 1-2:
    - [ ] Project initialization
        • npx create-next-app@latest kaccy-frontend
        • TypeScript: Yes
        • Tailwind CSS: Yes
        • App Router: Yes
        • Dependencies: axios, zustand, react-query

    - [ ] Layout setup
        • app/layout.tsx (root layout)
        • components/Header.tsx
        • components/Footer.tsx
        • app/page.tsx (landing page - placeholder)

    - [ ] Styling foundation
        • Tailwind config (colors from Option F pitch deck)
        • Global CSS (fonts: Inter)
        • Component library decision: shadcn/ui OR Headless UI

  Day 3-4:
    - [ ] Authentication pages
        • app/register/page.tsx
            Form: Email, Password, Username
            Validation: Zod schema
            Submit: POST /api/auth/register
        • app/login/page.tsx
            Form: Email, Password
            Submit: POST /api/auth/login
        • app/dashboard/page.tsx (protected, placeholder)

    - [ ] State management
        • stores/authStore.ts (Zustand)
            State: { user, token, isAuthenticated }
            Actions: login(), logout(), setUser()
        • Persist token in localStorage
        • Axios interceptor (add Authorization header)

    - [ ] Protected route HOC
        • components/ProtectedRoute.tsx
        • Redirect to /login if not authenticated

  Day 5:
    - [ ] Testing
        • E2E test (Playwright): Register -> Login -> Dashboard
        • Manual testing on localhost

    - [ ] Polish
        • Loading states (spinners)
        • Error messages (toast notifications)
        • Responsive design (mobile check)

DevOps Tasks:
  Day 4-5:
    - [ ] CI/CD setup (GitHub Actions)
        • .github/workflows/backend.yml
            • Trigger: push to main
            • Steps: cargo test, cargo build --release
        • .github/workflows/frontend.yml
            • Steps: npm test, npm build

    - [ ] Deployment script
        • scripts/deploy_backend.sh
            • SSH to VPS
            • git pull
            • cargo build --release
            • systemd restart kaccy-backend
        • scripts/deploy_frontend.sh
            • npm run build
            • rsync to VPS /var/www/kaccy
            • nginx reload

Checkpoint (Friday Demo):
  Demo:
    1. Show registration form
    2. Create account
    3. Login
    4. Access protected dashboard (shows "Welcome, {username}")

  Metrics:
    - Backend tests passing: 100%
    - API response time: <100ms
    - Frontend loads: <2s

  Blockers/Risks:
    - Bitcoin Core still syncing? → OK, not needed yet
    - Team member unavailable? → Re-prioritize tasks

Estimated Effort:
  - Backend: 30 hours
  - Frontend: 25 hours
  - DevOps: 5 hours
  - Total: 60 hours (2 people * 30 hours/week)

```

---

### **Sprint 2 (Week 2): Token System - Data Models & Creation**

```yaml
Sprint Goal: Users can create tokens (not yet tradeable)

Backend Tasks:
  Day 1-2:
    - [ ] Database schema v2
        • CREATE TABLE tokens (from Option E)
        • CREATE TABLE balances
        • Migration: sqlx migrate add create_tokens_table

    - [ ] Token model
        • src/models/token.rs
            pub struct Token {
                pub token_id: Uuid,
                pub issuer_user_id: Uuid,
                pub symbol: String,       // e.g., "$KITASAN"
                pub name: String,
                pub description: String,
                pub total_supply: Decimal,
                pub circulating_supply: Decimal,
                pub initial_price_btc: Decimal,
                pub price_increment_btc: Decimal,
                pub created_at: DateTime<Utc>,
                pub status: TokenStatus,
            }
            pub enum TokenStatus {
                Active,
                Paused,
                Closed,
            }

    - [ ] Token CRUD operations
        • create_token(issuer_id, symbol, name, ...) -> Result<Token, DbError>
        • get_token_by_id(token_id) -> Result<Token, DbError>
        • get_token_by_symbol(symbol) -> Result<Token, DbError>
        • list_tokens(filters, pagination) -> Result<Vec<Token>, DbError>
        • update_token_status(token_id, status) -> Result<(), DbError>

  Day 3-4:
    - [ ] Bonding Curve logic (Linear for MVP)
        • src/pricing/bonding_curve.rs
            pub struct LinearBondingCurve {
                pub initial_price: Decimal,
                pub increment: Decimal,
            }

            impl LinearBondingCurve {
                // P(n) = initial_price + (n * increment)
                pub fn spot_price(&self, supply: Decimal) -> Decimal {
                    self.initial_price + (supply * self.increment)
                }

                // Cost to buy 'amount' tokens
                pub fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
                    // Integral: Σ P(n) for n in [supply, supply+amount]
                    // = amount * initial_price + increment * (amount * (2*supply + amount - 1) / 2)
                    let base_cost = amount * self.initial_price;
                    let incremental_cost = self.increment * amount * (dec!(2) * current_supply + amount - dec!(1)) / dec!(2);
                    base_cost + incremental_cost
                }

                pub fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
                    // Similar to buy, but selling
                    self.buy_price(current_supply - amount, amount) * dec!(0.95)  // 5% spread
                }
            }

    - [ ] Token creation validation
        • Check: User reputation score >= 400 (Phase 2 feature)
        • Check: Symbol uniqueness (UNIQUE constraint)
        • Check: Symbol format (regex: /^\$[A-Z0-9]{2,10}$/)
        • Check: Price params (initial_price > 0, increment > 0)

    - [ ] API endpoints
        • POST /api/tokens
            Body: {
                symbol: "$KITASAN",
                name: "Tetsuya Kita Token",
                description: "Access to Rust consulting...",
                total_supply: 1000,
                initial_price_btc: 0.0001,
                price_increment_btc: 0.00001
            }
            Response: { token object }

        • GET /api/tokens (public)
            Query params: ?category=developer&page=1&limit=20
            Response: { tokens: [...], total: 100, page: 1 }

        • GET /api/tokens/:token_id (public)
            Response: { token object + current_price + issuer_info }

  Day 5:
    - [ ] Testing
        • Unit tests: Bonding curve calculations
            • Test: buy_price(0, 10) with initial=0.0001, increment=0.00001
            • Expected: 0.0001*10 + 0.00001*(10*9/2) = 0.001 + 0.00045 = 0.00145 BTC
        • Integration tests: Create token -> List tokens

    - [ ] Seed data (development)
        • Insert 5 sample tokens for frontend development

Frontend Tasks:
  Day 1-2:
    - [ ] Token creation form
        • app/tokens/create/page.tsx
        • Form fields:
            - Symbol (input with "$" prefix)
            - Name (input)
            - Description (textarea)
            - Total Supply (number input)
            - Initial Price (BTC, number input with 8 decimals)
            - Price Increment (BTC, number input)
        • Real-time validation
            - Symbol: Uppercase, 2-10 chars, no special chars
            - Initial price: > 0, <= 0.01 BTC
        • Preview section:
            - "First token: X BTC"
            - "10th token: Y BTC"
            - "100th token: Z BTC"

  Day 3-4:
    - [ ] Token browsing page
        • app/tokens/page.tsx
        • Grid layout (4 columns desktop, 2 mobile)
        • Token card component:
            components/TokenCard.tsx
            - Token symbol & name
            - Issuer name (link to profile)
            - Current price
            - Supply (circulating / total)
            - "View Details" button
        • Filters (sidebar):
            - Search by symbol/name
            - Category (future)
            - Price range (slider)
        • Pagination (bottom)

    - [ ] Token detail page (skeleton)
        • app/tokens/[tokenId]/page.tsx
        • Fetch token data from API
        • Display:
            - Issuer info (avatar, name, reputation)
            - Token metadata
            - Current price
            - Supply chart (placeholder)
            - "Buy" button (disabled for now)

  Day 5:
    - [ ] Bonding curve visualization
        • components/BondingCurveChart.tsx
        • Use Chart.js or Recharts
        • X-axis: Token supply (0 to total_supply)
        • Y-axis: Price (BTC)
        • Line: P(n) = initial + n * increment
        • Current supply marker (red dot)

    - [ ] Polish & testing
        • Loading states
        • Empty states ("No tokens yet")
        • Error handling (API failures)

Checkpoint (Friday Demo):
  Demo:
    1. Login as test user
    2. Navigate to "Create Token"
    3. Fill form with sample data
    4. See bonding curve preview
    5. Submit (token appears in database)
    6. Browse tokens page (shows created token)
    7. Click token -> Detail page

  Metrics:
    - Token creation: <500ms
    - Bonding curve calculations: Correct (unit tested)
    - UI responsive: Mobile + desktop

  Risks:
    - Decimal precision issues? → Use rust_decimal crate (38 digits)
    - Chart rendering slow? → Optimize data points (max 100)

Estimated Effort: 60 hours (2-3 people)

```

---

### **Sprint 3 (Week 3): Bitcoin Integration - Payment Address Generation**

```yaml
Sprint Goal: Users can generate BTC addresses and deposits can be detected

Backend Tasks:
  Day 1-2:
    - [ ] Bitcoin Core RPC integration
        • Dependencies: bitcoincore-rpc = "0.18"
        • src/bitcoin/client.rs
            pub struct BitcoinClient {
                rpc: Client,
            }

            impl BitcoinClient {
                pub fn new(url: &str, user: &str, password: &str) -> Self {
                    let rpc = Client::new(url, Auth::UserPass(user.to_string(), password.to_string())).unwrap();
                    Self { rpc }
                }

                pub fn get_new_address(&self, label: &str) -> Result<Address, BitcoinError> {
                    self.rpc.get_new_address(Some(label), Some(AddressType::Bech32))
                }

                pub fn get_balance(&self) -> Result<Amount, BitcoinError> {
                    self.rpc.get_balance(None, None)
                }

                pub fn list_unspent(&self) -> Result<Vec<Utxo>, BitcoinError> {
                    self.rpc.list_unspent(None, None, None, None, None)
                }
            }

    - [ ] Order model & table
        • src/models/order.rs
        • CREATE TABLE orders (from Option E schema)
            - order_id, user_id, token_id
            - order_type ('buy' or 'sell')
            - amount, price_btc, total_btc
            - status ('pending', 'completed', 'cancelled')
            - btc_address (for buy orders)
            - btc_txid (transaction ID)
            - created_at, completed_at

    - [ ] Order creation logic
        • create_buy_order(user_id, token_id, amount) -> Result<Order, OrderError>
            1. Get token current price (bonding curve)
            2. Calculate total_btc = buy_price(supply, amount)
            3. Generate BTC address (label: "order_{order_id}")
            4. Insert into orders table
            5. Return order with btc_address

  Day 3-4:
    - [ ] Payment monitoring worker
        • src/workers/payment_monitor.rs
        • Background task (tokio::spawn)
        • Loop every 30 seconds:
            1. Query orders with status='pending'
            2. For each order:
                - Call Bitcoin RPC: listunspent(0, 9999999, [order.btc_address])
                - Check if any UTXO with confirmations >= 1
                - If found:
                    a. Verify amount >= order.total_btc
                    b. Update order status = 'completed'
                    c. Record btc_txid
                    d. Execute trade (next sprint)
        • Error handling: Log failures, don't crash
        • Rate limiting: Max 100 orders per scan

    - [ ] API endpoints
        • POST /api/orders/buy
            Body: { token_id, amount }
            Response: {
                order_id,
                btc_address,
                total_btc,
                expires_at (1 hour)
            }

        • GET /api/orders/:order_id
            Response: {
                order object,
                status ('pending', 'completed', ...),
                confirmations (0, 1, 2, ...)
            }

  Day 5:
    - [ ] Testing
        • Testnet testing:
            1. Create buy order
            2. Send testnet BTC to address (from testnet faucet)
            3. Wait for 1 confirmation (~10 min)
            4. Verify order status updates to 'completed'
        • Unit tests: Order creation logic
        • Mock Bitcoin RPC for faster tests

Frontend Tasks:
  Day 1-3:
    - [ ] Buy modal component
        • components/BuyModal.tsx
        • Triggered from token detail page
        • Steps:
            1. Amount input (number of tokens)
            2. Show calculated price (API call to /api/tokens/:id/price?amount=X)
            3. Confirm button → POST /api/orders/buy
            4. Show BTC address + QR code
            5. Poll order status every 10 seconds
            6. Show "Payment received!" when completed

    - [ ] QR code generation
        • npm install qrcode.react
        • Display btc_address as QR code
        • Copy address button (clipboard API)

    - [ ] Payment status polling
        • useEffect hook: setInterval(() => fetch order status, 10000)
        • States:
            - "Waiting for payment..." (pending, 0 confirmations)
            - "Payment detected! Waiting for confirmation..." (pending, 1+ conf)
            - "Completed! Tokens added to your portfolio" (completed)

  Day 4-5:
    - [ ] Portfolio page
        • app/portfolio/page.tsx
        • List user's token balances
        • Table:
            | Token | Amount | Current Value (BTC) | Actions |
        • "Sell" button (disabled, next sprint)

    - [ ] Transaction history
        • app/portfolio/history/page.tsx
        • List user's past orders
        • Columns: Date, Type (Buy/Sell), Token, Amount, Price, Status

Checkpoint (Friday Demo):
  Demo:
    1. Login
    2. Browse tokens, select one
    3. Click "Buy"
    4. Enter amount (e.g., 10 tokens)
    5. See total cost (e.g., 0.0015 BTC)
    6. Generate BTC address + QR code
    7. (Manually send testnet BTC from another wallet)
    8. Wait ~10 minutes
    9. Order status updates to "Completed"
    10. Check portfolio → Balance increased

  Metrics:
    - Address generation: <1s
    - Payment detection: Within 1 minute of confirmation
    - No false positives (wrong address credited)

  Risks:
    - Bitcoin Core not synced? → Use Blockstream.info API as fallback
    - Testnet faucet down? → Use mainnet with tiny amounts (0.0001 BTC)

Estimated Effort: 70 hours

```

---

### **Sprint 4 (Week 4): Trading Execution - Balances & Trades**

```yaml
Sprint Goal: Tokens are transferred to users upon purchase completion

Backend Tasks:
  Day 1-2:
    - [ ] Balance management
        • src/models/balance.rs
        • CRUD operations:
            - get_balance(user_id, token_id) -> Decimal
            - update_balance(user_id, token_id, delta) -> Result<(), DbError>
                • Use DB transaction (BEGIN...COMMIT)
                • Check: balance + delta >= 0 (no negative)
        • Atomic operations (ACID guarantee)

    - [ ] Trade execution logic
        • src/trading/executor.rs
            pub async fn execute_buy_order(order_id: Uuid) -> Result<Trade, TradeError> {
                // Within a DB transaction:
                let order = get_order(order_id)?;
                let token = get_token(order.token_id)?;

                // 1. Update token circulating supply
                token.circulating_supply += order.amount;
                update_token(token)?;

                // 2. Credit buyer's balance
                update_balance(order.user_id, order.token_id, order.amount)?;

                // 3. Record trade
                let trade = Trade {
                    trade_id: Uuid::new_v4(),
                    buyer_user_id: order.user_id,
                    seller_user_id: None,  // Bonding curve = no seller
                    token_id: order.token_id,
                    amount: order.amount,
                    price_btc: order.price_btc,
                    total_btc: order.total_btc,
                    executed_at: Utc::now(),
                };
                insert_trade(trade)?;

                // 4. Update order status
                order.status = OrderStatus::Completed;
                order.completed_at = Some(Utc::now());
                update_order(order)?;

                Ok(trade)
            }

    - [ ] Payment monitor integration
        • Update src/workers/payment_monitor.rs
        • When payment detected:
            → Call execute_buy_order(order_id)
            → Log result (success or error)

  Day 3-4:
    - [ ] Sell order functionality
        • create_sell_order(user_id, token_id, amount)
            1. Check user balance >= amount
            2. Lock tokens (balance.locked_amount += amount)
            3. Calculate sell price (bonding curve with 5% spread)
            4. Create order with status='pending_payout'
            5. Generate Bitcoin withdrawal transaction
                • Get user's BTC withdrawal address (new field: users.btc_withdrawal_address)
                • Create transaction: self.bitcoin_client.send_to_address(address, amount)
                • Update order.btc_txid
            6. Update order status='completed'
            7. Reduce balance and circulating supply

    - [ ] API endpoints
        • POST /api/orders/sell
            Body: { token_id, amount }
            Response: { order_id, btc_amount, btc_address (user's), eta: "10-60 min" }

        • GET /api/balances (protected)
            Response: [{ token_id, amount, locked_amount, current_value_btc }]

  Day 5:
    - [ ] Testing
        • End-to-end test:
            1. Buy 10 tokens (testnet BTC)
            2. Verify balance = 10
            3. Sell 5 tokens
            4. Verify balance = 5
            5. Verify BTC payout received
        • Edge cases:
            - Insufficient balance
            - Insufficient BTC for gas fees (need reserve)

Frontend Tasks:
  Day 1-3:
    - [ ] Sell modal
        • Similar to buy modal
        • Input: Amount to sell
        • Show: "You'll receive X BTC (after 5% spread)"
        • BTC withdrawal address field (saved in user profile)
        • Confirm → POST /api/orders/sell

    - [ ] User profile page
        • app/profile/page.tsx
        • Editable fields:
            - Display name
            - Bio
            - Avatar (image upload, Phase 2)
            - **BTC withdrawal address** (important!)
        • Validation: Valid Bitcoin address (regex or library)

  Day 4-5:
    - [ ] Real-time balance updates
        • WebSocket OR polling (polling easier for MVP)
        • After buy/sell, refresh portfolio
        • Toast notification: "10 $KITASAN tokens added to your portfolio"

    - [ ] Order status page
        • app/orders/[orderId]/page.tsx
        • Real-time status (pending → completed)
        • Show BTC address, QR code, confirmations
        • For sell orders: Show payout status

Checkpoint (Friday Demo):
  Demo:
    1. Buy 10 tokens (testnet BTC)
    2. Wait for confirmation
    3. Portfolio shows 10 tokens
    4. Sell 5 tokens
    5. Enter BTC withdrawal address
    6. Confirm
    7. Verify: Balance now 5 tokens
    8. Check Bitcoin wallet: Received ~0.0007 BTC (5 tokens * price - 5% spread)

  Metrics:
    - Trade execution: <500ms (after payment confirmed)
    - Balance updates: Real-time (within 1s)
    - No double-spending or race conditions (DB transactions)

Estimated Effort: 65 hours

```

---

### **Sprint 5 (Week 5): Reputation System - Commitments (Manual)**

```yaml
Sprint Goal: Users can create commitments and admins can verify them

Backend Tasks:
  Day 1-2:
    - [ ] Commitment model
        • src/models/commitment.rs
        • CREATE TABLE output_commitments (from Option E)
        • CRUD operations:
            - create_commitment(user_id, token_id, title, description, deadline)
            - get_commitment(commitment_id)
            - list_commitments(user_id, status)
            - update_commitment_status(commitment_id, status, verifier_id, notes)

    - [ ] Commitment API endpoints
        • POST /api/commitments (protected)
            Body: {
                token_id,
                title: "Release v2.0",
                description: "Ship new features X, Y, Z",
                deadline: "2026-12-31"
            }
            Response: { commitment object }

        • GET /api/commitments (public, filterable)
            Query: ?user_id=X&status=pending

        • PUT /api/commitments/:id/evidence (protected)
            Body: {
                evidence_url: "https://github.com/user/repo/releases/v2.0",
                evidence_description: "Released on GitHub"
            }
            Response: { commitment with evidence }

  Day 3-4:
    - [ ] Reputation scoring (simple)
        • src/reputation/scoring.rs
        • CREATE TABLE reputation_events
        • Functions:
            - adjust_reputation(user_id, delta, reason)
                • Update users.reputation_score
                • Insert into reputation_events (audit log)
            - calculate_reputation(user_id) -> f64
                • Sum of all events (for now, simple)
        • Initial score: 500 (neutral)
        • Fulfilled commitment: +10
        • Failed commitment: -20

  Day 5:
    - [ ] Admin verification API
        • POST /api/admin/commitments/:id/verify (admin only)
            Body: {
                verified: true/false,
                verification_notes: "Checked GitHub, looks good"
            }
            Side effects:
                1. Update commitment status = 'verified' or 'failed'
                2. Adjust issuer reputation (+10 or -20)
                3. Log admin action

        • GET /api/admin/commitments/pending
            Response: [{ commitment objects pending verification }]

    - [ ] Admin role check middleware
        • users table: Add column `role` ('user', 'admin')
        • Middleware: require_admin() → checks JWT user_id → query user role
        • For MVP: Manually set your user as admin in DB

Frontend Tasks:
  Day 1-2:
    - [ ] Commitment creation form
        • app/commitments/create/page.tsx
        • Form fields:
            - Token (dropdown of user's issued tokens)
            - Title (input)
            - Description (textarea)
            - Deadline (date picker)
        • Submit → POST /api/commitments

    - [ ] Commitment list on token page
        • app/tokens/[tokenId]/page.tsx
        • Add section: "Issuer Commitments"
        • List:
            | Title | Deadline | Status | Evidence |
        • Status badge colors:
            - Pending: Yellow
            - Completed (awaiting verification): Blue
            - Verified: Green
            - Failed: Red

  Day 3-4:
    - [ ] Evidence submission form
        • app/commitments/[commitmentId]/submit-evidence/page.tsx
        • Fields:
            - Evidence URL (input, e.g., GitHub link)
            - Description (textarea)
        • Submit → PUT /api/commitments/:id/evidence

    - [ ] Admin dashboard
        • app/admin/page.tsx (protected, admin only)
        • Tabs:
            1. Pending Commitments
            2. KYC Queue (next sprint)
        • Pending Commitments tab:
            - Table: User, Token, Title, Deadline, Evidence
            - Actions: [Verify] [Reject]
            - On click:
                → Show evidence (open URL in new tab)
                → Confirmation modal: "Verify this commitment? +10 rep"
                → POST /api/admin/commitments/:id/verify

  Day 5:
    - [ ] Reputation display
        • User profile page: Show reputation score (e.g., "850 ⭐")
        • Token detail page: Show issuer reputation
        • Reputation tooltip: Hover to see breakdown (future: events log)

Checkpoint (Friday Demo):
  Demo:
    1. Login as token issuer
    2. Navigate to "Create Commitment"
    3. Fill form: "Ship feature X by 2026-12-31"
    4. Submit
    5. Commitment appears on token detail page (status: Pending)
    6. (Simulate completing work)
    7. Submit evidence: GitHub URL
    8. Login as admin
    9. Go to admin dashboard
    10. See pending commitment
    11. Click "Verify"
    12. Issuer's reputation increases from 500 → 510

  Metrics:
    - Commitment creation: <500ms
    - Admin can verify within 2 clicks
    - Reputation updates immediately

Estimated Effort: 55 hours

```

---

### **Sprint 6 (Week 6): KYC & User Profiles**

```yaml
Sprint Goal: Users can upload KYC documents and admins can approve them

Backend Tasks:
  Day 1-2:
    - [ ] File upload handling
        • Use multer or similar (for Node.js) OR
        • Rust: multipart/form-data parsing (actix-multipart or multer crate)
        • Storage: Local filesystem `/var/kaccy/uploads/kyc/` (Phase 2: S3)
        • File validation:
            - Max size: 10MB
            - Allowed types: image/jpeg, image/png, application/pdf
        • Generate unique filename: `{user_id}_{timestamp}_{original_name}`

    - [ ] KYC submission API
        • POST /api/kyc/submit (protected)
            Multipart form data:
                - document_type: 'passport' | 'drivers_license' | 'national_id'
                - front_image: File
                - back_image: File (optional)
            Response: { kyc_submission_id, status: 'pending' }
        • Update users.kyc_status = 'pending'

    - [ ] KYC review API
        • GET /api/admin/kyc/pending
            Response: [{
                user_id,
                email,
                submitted_at,
                document_urls: [...]
            }]

        • POST /api/admin/kyc/:user_id/review
            Body: { approved: true/false, notes: "..." }
            Side effects:
                - Update users.kyc_status = 'verified' or 'rejected'
                - Send email notification (SendGrid)

  Day 3-4:
    - [ ] User profile CRUD
        • PUT /api/users/me
            Body: {
                display_name,
                bio,
                btc_withdrawal_address,
                avatar_url (future)
            }
        • GET /api/users/:user_id (public profile)
            Response: {
                user_id,
                username,
                display_name,
                bio,
                reputation_score,
                tokens_issued: [{ token objects }],
                created_at
            }

    - [ ] Email notifications
        • Integrate SendGrid (free tier: 100 emails/day)
        • Templates:
            - KYC submitted (confirmation to user)
            - KYC approved (congratulations)
            - KYC rejected (with reason)
        • src/email/templates.rs

  Day 5:
    - [ ] Testing
        • Upload test documents
        • Admin review flow
        • Email delivery test (check spam folder)

Frontend Tasks:
  Day 1-2:
    - [ ] KYC submission page
        • app/kyc/submit/page.tsx
        • Form:
            - Document type (dropdown)
            - Front image (file upload, drag-and-drop)
            - Back image (optional)
            - Preview uploaded images
        • Submit → POST /api/kyc/submit
        • Success: "KYC submitted! Review takes 24-48 hours."

    - [ ] KYC status indicator
        • components/KycStatusBadge.tsx
        • Display in header or profile:
            - "✓ Verified" (green)
            - "⏳ Pending" (yellow)
            - "✗ Not Verified" (red)
        • If not verified: Prompt to submit KYC

  Day 3-4:
    - [ ] Admin KYC review page
        • app/admin/kyc/page.tsx
        • Table: User, Email, Submitted, Documents
        • Click row → Modal:
            - Display uploaded images (inline or new tab)
            - Form: Approve / Reject + Notes
            - Submit → POST /api/admin/kyc/:user_id/review

    - [ ] User profile edit page
        • app/profile/edit/page.tsx
        • Editable fields (from backend API)
        • Save button → PUT /api/users/me

  Day 5:
    - [ ] Public profile page
        • app/users/[userId]/page.tsx
        • Display:
            - Avatar (placeholder for now)
            - Display name, username
            - Bio
            - Reputation score
            - Issued tokens (grid of TokenCards)
        • Link from token detail page (issuer name clickable)

Checkpoint (Friday Demo):
  Demo:
    1. Login as new user
    2. Navigate to "Submit KYC"
    3. Upload passport image
    4. Submit
    5. Status shows "Pending"
    6. Login as admin
    7. Go to KYC queue
    8. Review uploaded document
    9. Approve
    10. User receives email notification
    11. User's status now "Verified ✓"

  Metrics:
    - File upload: <5s for 5MB image
    - Admin review: <3 clicks to approve
    - Email delivery: <1 minute

Estimated Effort: 60 hours

```

---

### **Sprint 7 (Week 7): Polish & Testing - Security & Performance**

```yaml
Sprint Goal: Bring product to production-ready quality

Backend Tasks:
  Day 1-2:
    - [ ] Security hardening
        • Rate limiting (per IP and per user)
            - Use tower_governor crate
            - Limits: 100 req/min per IP, 1000 req/hour per user
        • Input sanitization
            - Trim whitespace
            - Escape HTML (prevent XSS in display name/bio)
        • SQL injection prevention (already using SQLx parameterized queries)
        • CORS configuration
            - Allow: https://kaccy.net, http://localhost:3000 (dev)
        • Helmet headers (CSP, X-Frame-Options, etc.)

    - [ ] Error handling audit
        • Consistent error responses:
            {
                "error": {
                    "code": "INVALID_TOKEN_SYMBOL",
                    "message": "Symbol must start with $ and contain 2-10 uppercase letters",
                    "details": {}
                }
            }
        • HTTP status codes:
            - 400: Bad Request (validation errors)
            - 401: Unauthorized (no token / invalid token)
            - 403: Forbidden (no permission)
            - 404: Not Found
            - 429: Too Many Requests (rate limit)
            - 500: Internal Server Error (log to file, don't expose details)

  Day 3-4:
    - [ ] Performance optimization
        • Database indexes review (EXPLAIN ANALYZE queries)
            - users: email, username
            - tokens: symbol, issuer_user_id, status
            - orders: user_id, token_id, status
            - trades: token_id, executed_at
        • Query optimization
            - N+1 problem? Use JOINs or batch queries
            - List tokens: Include issuer name in one query
        • Caching (optional for MVP, but check):
            - Token list: Cache for 60 seconds (Redis future)
        • Connection pooling
            - PostgreSQL: Max 20 connections (PgPoolOptions)

    - [ ] Logging & monitoring
        • Structured logging (tracing crate)
            - Log levels: ERROR, WARN, INFO, DEBUG
            - Log format: JSON (for parsing)
        • Log critical events:
            - User registration/login
            - Token creation
            - Order creation/completion
            - Admin actions
            - Errors (with stack trace)
        • Log rotation (logrotate)
            - Keep last 7 days
            - Max 100MB per file

  Day 5:
    - [ ] Load testing
        • Use Apache Bench or wrk
        • Endpoints to test:
            - GET /api/tokens (list)
            - POST /api/auth/login
            - POST /api/orders/buy
        • Target: 100 req/sec with <200ms response time
        • Check: No database connection exhaustion

Frontend Tasks:
  Day 1-3:
    - [ ] Error handling & UX
        • Global error boundary (React)
        • API error handling:
            - 401 → Redirect to login
            - 403 → Show "Insufficient permissions" toast
            - 500 → Show "Something went wrong, try again"
        • Network error handling (axios interceptor)
            - Retry failed requests (3x with exponential backoff)
            - Offline indicator

    - [ ] Loading states
        • Skeleton screens (instead of spinners)
            - Token list: Skeleton cards
            - Profile: Skeleton layout
        • Progress indicators
            - File upload: Progress bar
            - Payment waiting: Animated icon + countdown

    - [ ] Empty states
        • No tokens found: "No tokens match your search. Try different filters."
        • No balances: "You don't own any tokens yet. Browse tokens to buy."
        • No commitments: "This issuer hasn't made any commitments yet."

  Day 4-5:
    - [ ] Responsive design audit
        • Test on:
            - Desktop (1920x1080, 1366x768)
            - Tablet (768x1024)
            - Mobile (375x667, 414x896)
        • Fix issues:
            - Horizontal scroll
            - Tiny touch targets (<44px)
            - Overlapping elements

    - [ ] Accessibility (basic)
        • Semantic HTML (h1, h2, nav, main, footer)
        • ARIA labels (buttons, inputs)
        • Keyboard navigation (Tab, Enter)
        • Color contrast (WCAG AA: 4.5:1 for text)

    - [ ] Performance optimization
        • Image optimization
            - Next.js Image component (automatic optimization)
            - Lazy loading (below the fold)
        • Code splitting (Next.js automatic)
        • Bundle size audit (next bundle-analyzer)
            - Target: <500KB initial load

Checkpoint (Friday Demo):
  Demo:
    1. Show Lighthouse score (Chrome DevTools)
       - Performance: 90+
       - Accessibility: 90+
       - Best Practices: 90+
       - SEO: 90+
    2. Load testing results: 100 req/sec, <200ms
    3. Error handling: Trigger 500 error → User sees friendly message
    4. Mobile responsiveness: Browse on iPhone simulator

  Metrics:
    - API response time (p95): <100ms
    - Frontend load time: <2s
    - Zero critical security vulnerabilities (npm audit, cargo audit)

Estimated Effort: 55 hours

```

---

### **Sprint 8 (Week 8): Legal & Compliance - Terms, Privacy, VASP**

```yaml
Sprint Goal: Complete legally required documents and prepare for public launch

Backend Tasks:
  Day 1-2:
    - [ ] Terms of Service API
        • Static content endpoint:
            GET /api/legal/terms
            Response: { html: "...", version: "1.0", updated_at: "2026-01-01" }
        • User acceptance tracking:
            - users table: Add `terms_accepted_version` and `terms_accepted_at`
            - On registration: Require checkbox "I accept ToS"
            - POST /api/users/accept-terms
                Body: { version: "1.0" }
                Side effect: Update user record

    - [ ] Privacy Policy API
        • Similar to ToS
        • GET /api/legal/privacy

    - [ ] Cookie consent
        • Minimal for MVP (only essential cookies: session)
        • GET /api/legal/cookies
        • Frontend: Simple banner "We use cookies for authentication. By continuing, you accept our use of cookies."

  Day 3-5:
    - [ ] VASP compliance documentation
        • Prepare for Estonian FIU submission:
            1. AML/CFT Policy (PDF)
                - Customer Due Diligence (CDD) procedures
                - Transaction monitoring thresholds
                - Suspicious Activity Reporting (SAR)
                - Record keeping (5 years)
            2. Data Protection Policy (GDPR)
                - User data collected
                - Storage location (Estonia VPS)
                - Retention periods
                - Right to erasure procedures
            3. Business Plan (1-pager)
                - Company overview
                - Services offered
                - Target market
                - Revenue model
            4. Management structure
                - Org chart (even if just you)
                - CV of key personnel
            5. IT Security Policy
                - Infrastructure diagram
                - Security measures (encryption, 2FA, etc.)
                - Incident response plan
        • Work with Estonian legal counsel to finalize
        • Submit application (online portal)

    - [ ] Backup & disaster recovery
        • Automated daily backups
            - PostgreSQL: pg_dump to S3 (or local + rsync)
            - Script: /opt/kaccy/scripts/backup_db.sh
            - Cron: 0 2 * * * (2 AM daily)
        • Backup verification (test restore monthly)
        • Document recovery procedures (in wiki)

Frontend Tasks:
  Day 1-2:
    - [ ] Terms of Service page
        • app/legal/terms/page.tsx
        • Fetch content from API
        • Display in readable format (Markdown or HTML)
        • Footer link: "Terms of Service"

    - [ ] Privacy Policy page
        • app/legal/privacy/page.tsx
        • Similar to ToS

    - [ ] Cookie consent banner
        • components/CookieConsent.tsx
        • Display at bottom of screen (if not accepted)
        • Button: "Accept" → Set localStorage flag → Hide banner

  Day 3-4:
    - [ ] Registration flow update
        • app/register/page.tsx
        • Add checkbox: "I have read and accept the Terms of Service"
        • Link to /legal/terms (open in new tab)
        • Disable submit button if not checked
        • On submit: POST /api/users/accept-terms

    - [ ] User settings page
        • app/settings/page.tsx
        • Tabs:
            1. Profile (display name, bio)
            2. Security (change password, 2FA future)
            3. Privacy (data export, account deletion)
        • Privacy tab:
            - Button: "Export my data" → GET /api/users/me/export (Phase 2)
            - Button: "Delete my account" → Confirmation modal → POST /api/users/me/delete

  Day 5:
    - [ ] Footer content
        • components/Footer.tsx
        • Links:
            - About
            - Terms of Service
            - Privacy Policy
            - Contact (email)
            - Status page (future)
        • Copyright: "© 2026 Kaccy Protocol. All rights reserved."
        • Social links: Twitter, GitHub (if applicable)

Checkpoint (Friday Demo):
  Demo:
    1. Show Terms of Service page (readable, formatted)
    2. Registration flow: Checkbox required
    3. Cookie consent banner (accept → disappears)
    4. Settings page: Privacy options visible
    5. Estonian VASP application submitted (show confirmation email/PDF)

  Metrics:
    - Legal pages load: <1s
    - ToS word count: 3,000-5,000 (comprehensive but readable)
    - VASP application: All documents uploaded

Estimated Effort: 50 hours (+ legal counsel time)

```

---

### **Sprint 9 (Week 9): Beta User Onboarding - Invitations & Support**

```yaml
Sprint Goal: Invite first 10-20 beta users and onboard them

Backend Tasks:
  Day 1-2:
    - [ ] Invitation system
        • CREATE TABLE invitations (
            invitation_id UUID PRIMARY KEY,
            code VARCHAR(20) UNIQUE,  -- e.g., "KACCY-BETA-A1B2C3"
            invited_by UUID REFERENCES users(user_id),
            email VARCHAR(255),
            used_by UUID REFERENCES users(user_id),
            created_at TIMESTAMPTZ,
            used_at TIMESTAMPTZ
          )
        • generate_invitation(admin_id, email) -> String
            - Generate random code
            - Insert into table
            - Send email (SendGrid template)
        • validate_invitation(code) -> Result<Invitation, InvitationError>
            - Check: not used
            - Check: not expired (7 days)

    - [ ] Invitation API
        • POST /api/admin/invitations (admin only)
            Body: { email: "user@example.com" }
            Response: { code, expires_at }
        • GET /api/invitations/:code (public)
            Response: { valid: true/false, email }
        • POST /api/auth/register-with-invitation
            Body: { invitation_code, email, password, username }
            Side effect: Mark invitation as used

  Day 3-4:
    - [ ] Onboarding checklist
        • users table: Add `onboarding_completed` JSON field
            {
              "steps": {
                "profile_completed": false,
                "kyc_submitted": false,
                "first_token_browsed": false,
                "first_token_purchased": false,
                "first_commitment_created": false
              }
            }
        • Track progress: Update on relevant actions
        • GET /api/users/me/onboarding
            Response: { steps, completion_percentage }

    - [ ] Welcome email sequence
        • Day 0: Welcome + invitation link
        • Day 1: "Complete your profile" (if not done)
        • Day 3: "Browse tokens and make your first purchase"
        • Day 7: Feedback survey

  Day 5:
    - [ ] Feedback collection API
        • POST /api/feedback
            Body: {
                category: 'bug' | 'feature' | 'general',
                message: "...",
                contact_email: "..." (optional)
            }
        • Store in database OR send to admin email

Frontend Tasks:
  Day 1-2:
    - [ ] Invitation landing page
        • app/invite/[code]/page.tsx
        • Fetch invitation details (GET /api/invitations/:code)
        • If valid:
            - Show: "You've been invited to Kaccy Beta!"
            - Button: "Join Now" → Redirect to register (pre-fill email)
        • If invalid/used:
            - Show: "This invitation is no longer valid."

    - [ ] Onboarding modal/checklist
        • components/OnboardingChecklist.tsx
        • Display on dashboard (if not completed)
        • Checklist items:
            ☐ Complete your profile
            ☐ Submit KYC
            ☐ Browse tokens
            ☐ Make your first purchase
            ☐ Create a commitment (if you're an issuer)
        • Each item: Link to relevant page
        • Dismissible (but comes back until 100% complete)

  Day 3-4:
    - [ ] Help & support page
        • app/help/page.tsx
        • FAQ sections:
            - Getting Started
            - Buying & Selling Tokens
            - KYC Verification
            - Reputation System
            - Technical Issues
        • Contact form (POST /api/feedback)

    - [ ] Discord community setup
        • Create Discord server: "Kaccy Beta"
        • Channels:
            - #announcements (read-only)
            - #general
            - #support
            - #feedback
        • Invite beta users (link in welcome email)

  Day 5:
    - [ ] In-app notifications (basic)
        • components/NotificationBell.tsx (in header)
        • Notifications:
            - "Your KYC has been approved!"
            - "Order completed: 10 $KITASAN tokens added"
            - "New commitment verified: +10 reputation"
        • GET /api/notifications (backend API needed)
        • Mark as read: PUT /api/notifications/:id/read

Real-World Testing:
  Day 1-5 (parallel):
    - [ ] Send 10 invitations (to trusted network)
        • Personal email to each invitee
        • 1-on-1 onboarding call (optional, but recommended)
        • Ask them to:
            1. Complete profile
            2. Submit KYC (you manually approve)
            3. Browse tokens
            4. (Optional) Buy testnet tokens
        • Collect feedback in shared Google Doc

    - [ ] Bug tracking
        • Use GitHub Issues OR Linear
        • Label: "bug", "feedback", "enhancement"
        • Triage daily: Prioritize P0 (blockers) → Fix within 24h

Checkpoint (Friday Demo):
  Demo:
    1. Generate invitation (admin panel)
    2. Open invitation link (as new user)
    3. Register with invitation code
    4. See onboarding checklist
    5. Complete each step
    6. Checklist updates in real-time
    7. Submit feedback via form
    8. Show Discord community (5+ members)

  Metrics:
    - 10 invitations sent
    - 5+ users registered
    - 3+ users completed profile
    - 2+ users submitted KYC
    - 1+ user bought tokens

Estimated Effort: 55 hours

```

---

### **Sprint 10 (Week 10): Admin Tools & Analytics**

```yaml
Sprint Goal: Admin can efficiently operate with tools and dashboards

Backend Tasks:
  Day 1-2:
    - [ ] Admin analytics API
        • GET /api/admin/analytics/overview
            Response: {
                total_users,
                active_users_7d,
                active_users_30d,
                total_tokens,
                total_trades,
                total_volume_btc,
                avg_reputation_score,
                pending_kyc_count,
                pending_commitment_count
            }
        • GET /api/admin/analytics/users
            Query: ?date_from=2026-01-01&date_to=2026-01-31
            Response: {
                registrations_by_day: [{date, count}],
                active_users_by_day: [{date, count}],
                retention: {
                    day_1: 0.80,
                    day_7: 0.60,
                    day_30: 0.40
                }
            }
        • GET /api/admin/analytics/tokens
            Response: {
                tokens_by_day: [{date, count}],
                top_tokens_by_volume: [{token, volume_btc}],
                avg_token_price_btc
            }

    - [ ] Admin action logs
        • CREATE TABLE admin_actions (from Option E)
        • Log all admin actions:
            - KYC approval/rejection
            - Commitment verification
            - User ban/unban (future)
            - Token pause/unpause
        • GET /api/admin/actions
            Query: ?admin_id=X&action_type=verify_commitment&page=1
            Response: [{ action objects }]

  Day 3-4:
    - [ ] User management API
        • GET /api/admin/users
            Query: ?search=email&kyc_status=verified&page=1
            Response: { users: [...], total, page }
        • PUT /api/admin/users/:user_id
            Body: {
                kyc_status: 'verified' | 'rejected',
                reputation_score: 850,
                notes: "..."
            }
        • POST /api/admin/users/:user_id/ban (future, optional for MVP)

    - [ ] Token moderation API
        • PUT /api/admin/tokens/:token_id
            Body: { status: 'paused' | 'active' | 'closed' }
            Use case: Pause trading if issuer violates ToS

  Day 5:
    - [ ] Data export
        • GET /api/admin/export/users (CSV)
        • GET /api/admin/export/tokens (CSV)
        • GET /api/admin/export/trades (CSV)
        • Use case: Compliance reporting, data analysis

Frontend Tasks:
  Day 1-3:
    - [ ] Admin analytics dashboard
        • app/admin/analytics/page.tsx
        • Metrics cards (top row):
            - Total Users
            - Active Users (30d)
            - Total Tokens
            - Total Volume (BTC)
        • Charts:
            - User registrations over time (line chart)
            - Trading volume over time (bar chart)
            - Top tokens by volume (table)
        • Use Chart.js or Recharts

    - [ ] User management page
        • app/admin/users/page.tsx
        • Search bar (email, username)
        • Filters: KYC status, Reputation range
        • Table:
            | Email | Username | KYC Status | Reputation | Actions |
        • Actions:
            - [View Profile]
            - [Edit] (opens modal)
            - [View Activity Log]

  Day 4-5:
    - [ ] Token management page
        • app/admin/tokens/page.tsx
        • Table: Symbol, Issuer, Created, Volume, Status
        • Actions:
            - [View Details]
            - [Pause Trading]
            - [View Trades]

    - [ ] Admin action log viewer
        • app/admin/logs/page.tsx
        • Filterable by:
            - Admin (yourself, or other admins if team)
            - Action type
            - Date range
        • Table: Timestamp, Admin, Action, Target, Details

Checkpoint (Friday Demo):
  Demo:
    1. Login as admin
    2. Go to analytics dashboard
    3. Show metrics: "10 users, 5 tokens, 0.01 BTC volume"
    4. Charts: User registrations (upward trend)
    5. Go to user management
    6. Search for specific user
    7. Edit reputation score
    8. Go to action logs
    9. See all actions (KYC approvals, commitment verifications)

  Metrics:
    - Dashboard loads: <2s (even with charts)
    - CSV export: Works for 100+ rows
    - Action logs: Audit trail complete

Estimated Effort: 60 hours

```

---

### **Sprint 11 (Week 11): Final Testing & Bug Fixes**

```yaml
Sprint Goal: Test all features and fix bugs

Testing Tasks:
  Day 1-2: End-to-End Testing
    - [ ] User journey 1: New user registration → KYC → Browse → Buy
        • Register new account
        • Submit KYC documents
        • Admin approves KYC
        • Browse tokens
        • Buy 5 tokens (testnet BTC)
        • Verify balance updated
        • Check portfolio

    - [ ] User journey 2: Token issuer → Create token → Make commitment → Fulfill
        • Login as verified user
        • Create token "$TEST"
        • Browse → See own token listed
        • Create commitment "Deliver X by DATE"
        • Submit evidence
        • Admin verifies
        • Reputation increases

    - [ ] User journey 3: Sell tokens → Withdraw BTC
        • Login as user with tokens
        • Sell 3 tokens
        • Enter BTC withdrawal address
        • Verify BTC received (testnet)

    - [ ] Edge cases:
        • Try to buy with insufficient BTC
        • Try to sell more tokens than balance
        • Try to create token with duplicate symbol
        • Try to access admin pages as non-admin
        • Try to submit KYC twice

  Day 3: Security Testing
    - [ ] Manual security audit
        • SQL injection attempts (in search fields, inputs)
        • XSS attempts (in bio, descriptions)
        • CSRF attempts (without CSRF token)
        • Auth bypass attempts (manipulate JWT)
        • Rate limiting test (send 200 req/sec, verify 429 error)

    - [ ] Automated security scan
        • npm audit (frontend)
        • cargo audit (backend)
        • OWASP ZAP (web app security scanner)
        • Fix critical/high vulnerabilities

    - [ ] Penetration testing (if budget allows)
        • Hire on Upwork ($500-1000)
        • Report vulnerabilities
        • Fix within 48 hours

  Day 4: Performance Testing
    - [ ] Load testing (Apache Bench or k6)
        • GET /api/tokens: 100 req/sec for 60 seconds
        • POST /api/auth/login: 50 req/sec for 60 seconds
        • POST /api/orders/buy: 20 req/sec for 60 seconds
        • Check: Response time <200ms (p95)
        • Check: No 500 errors

    - [ ] Database stress test
        • Insert 1,000 fake users (script)
        • Insert 100 fake tokens
        • Insert 10,000 fake trades
        • Query performance: Still <50ms?

    - [ ] Frontend performance
        • Lighthouse audit (all pages)
        • Target: 90+ on Performance, Accessibility, Best Practices
        • Fix issues: Large images, unused JS, etc.

  Day 5: Bug Bash
    - [ ] Internal bug bash (team + beta users)
        • Invite 10 beta users to test for 2 hours
        • Provide test scenarios (Google Doc)
        • Report bugs in GitHub Issues
        • Categorize: P0 (blocker), P1 (high), P2 (medium), P3 (low)

    - [ ] Bug triage & fixing
        • P0: Fix immediately (same day)
        • P1: Fix within 2 days
        • P2: Add to backlog (post-MVP)
        • P3: "Won't fix" or "Nice to have"

Bug Fixing (Continuous):
  - [ ] Daily standup: Review new bugs
  - [ ] Prioritize: Blockers first, then high priority
  - [ ] Test fixes: Before deploying
  - [ ] Regression testing: Ensure fixes don't break other features

Checkpoint (Friday Demo):
  Demo:
    1. Walk through all 3 user journeys (live)
    2. Show security scan results (no critical issues)
    3. Show load testing results (graphs)
    4. Show bug tracker: P0/P1 bugs fixed

  Metrics:
    - End-to-end tests: 100% passing
    - Security vulnerabilities: 0 critical, 0 high
    - Load testing: 100 req/sec, <200ms response
    - Bugs found: 20+
    - Bugs fixed (P0/P1): 100%

Estimated Effort: 70 hours (includes bug fixing time)

```

---

### **Sprint 12 (Week 12): Launch Preparation & Public Beta**

```yaml
Sprint Goal: Production environment deployment and public beta launch

Backend Tasks:
  Day 1-2: Production Environment Setup
    - [ ] Production VPS configuration
        • If using same VPS as dev: Create separate DB (kaccy_production)
        • If new VPS: Repeat Week 0 setup (but with mainnet Bitcoin)
        • Environment variables:
            - DATABASE_URL (production)
            - BITCOIN_RPC_URL (mainnet)
            - JWT_SECRET (different from dev!)
            - SENDGRID_API_KEY

    - [ ] Bitcoin mainnet switch
        • bitcoin.conf: Remove testnet=1
        • Restart Bitcoin Core
        • Wait for mainnet sync (if not already synced)
        • Update code: Remove testnet references

    - [ ] SSL & Domain
        • Verify HTTPS: https://kaccy.net
        • Force HTTPS redirect (Nginx config)
        • HSTS header (Strict-Transport-Security)

    - [ ] Database migration
        • Run all migrations on production DB
        • Seed initial data:
            - Admin user (you)
            - Sample tokens (optional, or wait for real users)

  Day 3: Deployment & Smoke Testing
    - [ ] Deploy backend
        • Build: cargo build --release
        • Copy binary to VPS: /opt/kaccy/kaccy-backend
        • Systemd service: sudo systemctl start kaccy-backend
        • Verify: curl https://kaccy.net/api/health

    - [ ] Deploy frontend
        • Build: npm run build
        • Copy to VPS: rsync -avz out/ user@vps:/var/www/kaccy
        • Nginx restart: sudo systemctl restart nginx
        • Verify: Open https://kaccy.net in browser

    - [ ] Smoke testing (production)
        • Register new account (real email)
        • Login
        • Browse tokens
        • Create token (if you want to be first issuer)
        • DO NOT buy tokens yet (mainnet BTC is real!)

  Day 4: Marketing & Announcements
    - [ ] Landing page content update
        • Emphasize: "Now in Public Beta"
        • Add: Waitlist form (if throttling signups)
        • Add: FAQ section
        • Add: Demo video (2-3 min, Loom)

    - [ ] Social media announcements
        • Twitter/X:
            "🚀 Kaccy Protocol is now LIVE!

            The first Bitcoin-based platform where your reputation = your currency.

            ✅ Tokenize your expertise
            ✅ Proof of Output (not hype)
            ✅ Lightning Network micropayments

            Join beta: https://kaccy.net

            #Bitcoin #Web3 #CreatorEconomy"

        • LinkedIn:
            Professional tone, focus on use case for consultants/engineers

        • Reddit (r/rust, r/bitcoin):
            Soft launch post, not spammy, provide value

        • Hacker News (Show HN):
            "Show HN: Kaccy – Tokenize your consulting services on Bitcoin"
            Link to landing page + demo video

    - [ ] Email to beta waitlist
        • Subject: "Kaccy is LIVE – Your Invitation Inside"
        • Body:
            - Thank them for waiting
            - Explain what Kaccy does (1 paragraph)
            - CTA: "Join Beta" button
            - Mention: Limited to 100 users (creates urgency)

    - [ ] Press release (optional)
        • Publish on company blog (kaccy.net/blog)
        • Submit to crypto news sites (Coindesk, Cointelegraph)
            - Unlikely to get picked up as unknown startup, but worth trying

  Day 5: Monitoring & Support
    - [ ] Real-time monitoring setup
        • Use Uptime Robot (free): Ping https://kaccy.net every 5 minutes
        • Email/SMS alert if down
        • StatusPage (optional): status.kaccy.net

    - [ ] Error tracking
        • Integrate Sentry (free tier):
            - Backend: sentry crate
            - Frontend: @sentry/nextjs
        • Get alerts on errors

    - [ ] User support readiness
        • Monitor Discord #support channel
        • Check email: support@kaccy.net (setup forwarding)
        • Response SLA: <24 hours

    - [ ] Daily metrics review
        • Check analytics dashboard:
            - Signups
            - Active users
            - Tokens created
            - Trades executed
        • Document in Google Sheets (daily log)

Post-Launch Activities (Week 12-16):
  - [ ] Weekly user interviews (5 users/week)
      • Questions:
          - What was confusing?
          - What feature is missing?
          - Would you recommend Kaccy to a friend? (NPS)

  - [ ] Weekly product updates
      • Fix P1 bugs within 48 hours
      • Ship P2 features if quick wins
      • Announce updates on Discord

  - [ ] Weekly blog post / social media
      • Case study: "Meet Alex, the first Rust dev on Kaccy"
      • Tutorial: "How to tokenize your consulting services"
      • Transparency: "Week 1 metrics: 50 users, 5 tokens, 0.01 BTC volume"

Checkpoint (Friday Demo):
  Demo:
    1. Show production site (live on kaccy.net)
    2. Walk through user flow (real mainnet, but small amounts)
    3. Show social media posts (published)
    4. Show analytics: Real users signing up
    5. Celebrate! 🎉

  Metrics (End of Week 12):
    - 50+ registered users (target)
    - 10+ KYC-verified users
    - 5+ tokens issued
    - 10+ trades executed
    - $1,000+ trading volume (BTC equivalent)
    - 0 downtime
    - <5 P0 bugs (and all fixed)

Estimated Effort: 60 hours

```

---

## 📊 Project Management Tools

### Recommended Stack

```yaml
Code Repository:
  - GitHub (private repos)
  - Branch strategy:
      • main (production)
      • develop (staging)
      • feature/* (feature branches)
  - PR reviews required (if team > 1)

Task Management:
  - GitHub Projects OR Linear.app
  - Board columns:
      • Backlog
      • To Do (current sprint)
      • In Progress
      • In Review (PR open)
      • Done
  - Labels: bug, feature, enhancement, p0, p1, p2, p3

Communication:
  - Slack OR Discord
  - Channels:
      • #general
      • #dev (technical discussions)
      • #standup (daily updates)
      • #bugs (bug reports)

Documentation:
  - Notion OR GitHub Wiki
  - Sections:
      • Architecture (diagrams)
      • API documentation (OpenAPI)
      • Deployment guide
      • Troubleshooting

Time Tracking (Optional):
  - Toggl Track (free for solo)
  - Log hours by task
  - Review weekly: Actual vs estimated

```

---

## 🎯 Success Criteria & Go/No-Go

### Weekly Check-In Questions

```yaml
Every Friday 4 PM:
  1. Did we achieve this week's sprint goal?
  2. What blockers emerged?
  3. Are we on track for Week 12 launch?
  4. Do we need to cut scope?
  5. Team morale: 1-10? (if > 1 person)

Every Monday 9 AM:
  1. Review last week's demo
  2. Plan this week's tasks
  3. Assign responsibilities
  4. Set expectations

```

### Go/No-Go Decision (Week 12)

```rust
pub struct LaunchReadiness {
    criteria: Vec<Criterion>,
}

impl LaunchReadiness {
    pub fn evaluate() -> Decision {
        let criteria = vec![
            // Must-Have (Hard Requirements)
            Criterion {
                name: "All core features working",
                status: Status::Pass,  // ✅
                required: true,
            },
            Criterion {
                name: "Zero P0 bugs",
                status: Status::Pass,  // ✅
                required: true,
            },
            Criterion {
                name: "Security audit clean (critical/high)",
                status: Status::Pass,  // ✅
                required: true,
            },
            Criterion {
                name: "Terms of Service finalized",
                status: Status::Pass,  // ✅
                required: true,
            },
            Criterion {
                name: "Production environment ready",
                status: Status::Pass,  // ✅
                required: true,
            },

            // Should-Have (Soft Requirements)
            Criterion {
                name: "10+ beta users onboarded",
                status: Status::Pass,  // ✅ (target: 10, actual: 12)
                required: false,
            },
            Criterion {
                name: "Estonian VASP application submitted",
                status: Status::InProgress,  // ⏳ (submitted, awaiting approval)
                required: false,  // Can launch without license for closed beta
            },
        ];

        let all_required_pass = criteria.iter()
            .filter(|c| c.required)
            .all(|c| c.status == Status::Pass);

        if all_required_pass {
            Decision::Go
        } else {
            Decision::Delay { reason: "Critical criteria not met".to_string() }
        }
    }
}

pub enum Decision {
    Go,                               // Launch as planned
    Delay { reason: String },         // Delay 1-2 weeks, fix issues
    Pivot { reason: String },         // Major change needed
}

```

---

## 📝 Summary & Handoff

### What We've Built

✅ **12-week detailed implementation plan**

- Week-by-week breakdown
- Day-by-day tasks
- Backend + Frontend + DevOps
- Checkpoints every Friday
- Estimated effort per sprint

✅ **Technical details**

- Database schemas
- API endpoints
- Bonding curve calculations
- Bitcoin integration steps
- Security measures

✅ **Process & Governance**

- Sprint structure
- Testing strategy
- Bug triage process
- Launch readiness criteria

✅ **Risk mitigation**

- Blockers identified
- Fallback plans
- Go/No-Go decision framework

---

## 🚀 Next Immediate Actions (This Week)

### Day 1 (Monday):

```yaml
Morning (3 hours):
  - [ ] Set up GitHub Organization
  - [ ] Create 3 repos (backend, frontend, docs)
  - [ ] Initialize project management (GitHub Projects or Linear)
  - [ ] Create Sprint 1 board with tasks from Week 1

Afternoon (3 hours):
  - [ ] Contract VPS (Hetzner)
  - [ ] SSH setup, firewall
  - [ ] PostgreSQL installation
  - [ ] Bitcoin Core download (start syncing overnight)

Evening (2 hours):
  - [ ] If hiring: Post job on Upwork/Twitter
  - [ ] Send 10 emails to potential beta users (gauge interest)
  - [ ] Create beta user list spreadsheet

```

### Day 2-5:

Follow **Sprint 1 (Week 1)** plan above
