# E. MVP Scope Definition

# Kaccy Protocol - MVP Scope Definition v1.0

This document integrates all previous analyses (A-D: Technical, Economics, Legal, Competitive) to define **"the minimal configuration that can be implemented in 3 months and validated in the market"**.

---

## 🎯 MVP Philosophy

### Core Principles

```rust
pub struct MVPPrinciples {
    principle_1: "Prove core value with minimal features",
    principle_2: "Launch within 3 months",
    principle_3: "Validate with 50-100 closed beta users",
    principle_4: "Minimize technical debt (extensible design)",
    principle_5: "Eliminate legal risks (parallel with Estonian VASP application)",
}

impl MVPPrinciples {
    pub fn must_have_vs_nice_to_have() -> Categorization {
        Categorization {
            must_have: vec![
                "User registration (DID)",
                "Token issuance (simplified Bonding Curve)",
                "Token trading (BTC settlement)",
                "Reputation score (manual verification)",
                "Basic governance",
            ],
            should_have: vec![
                "Lightning Network integration",
                "GitHub integration (Output verification)",
                "AI evaluation (simplified)",
            ],
            nice_to_have: vec![
                "Mobile app",
                "Advanced AI evaluation",
                "Fully automated Oracle",
                "DAO voting system",
            ],
            future: vec![
                "L2 integration (Stacks/RSK)",
                "NFT integration",
                "Multi-chain support",
            ],
        }
    }
}

```

---

## 📋 MVP Feature Matrix

### Phase 0 vs Full Vision

| Feature Category | Full Vision (Option A/B) | MVP (3 Months) | Phase 2 (6-12 Months) |
| --- | --- | --- | --- |
| **Authentication** | DID + Multi-wallet | Email + DID (simplified) | Full DID + Hardware Wallet |
| **Token Issuance** | Adaptive Bonding Curve | Linear Bonding Curve | Adaptive Curve |
| **Payment** | Lightning + BTC on-chain | BTC on-chain only | Lightning integration |
| **Reputation** | SBT + Auto Oracle | Manual verification | Semi-auto Oracle |
| **AI Integration** | Full AI Agent access control | None (add in Phase 2) | AI Quality evaluation |
| **KYC/AML** | Level 0-3, Full compliance | Level 1 only (Email + ID) | Add Level 2-3 |
| **Frontend** | Web + Mobile + CLI | Web only (React) | Add Mobile |
| **Backend** | Rust (Axum) full stack | Rust core + Python auxiliary | Full Rust migration |
| **Database** | PostgreSQL + Redis + TimescaleDB | PostgreSQL only | Add Redis + TimescaleDB |
| **Blockchain** | BTC + Lightning + L2 | BTC on-chain only | Add Lightning |
| **Governance** | Full DAO + On-chain voting | Admin panel | Token-weighted voting |
| **Analytics** | Real-time dashboard | Basic metrics | Advanced analytics |

---

## 🏗️ MVP Architecture (Simplified)

### Simplified System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Client Layer                            │
│                    Web App (React + TypeScript)                 │
│              - User registration & login                        │
│              - Token browsing & purchase                        │
│              - Profile management                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ REST API
┌─────────────────────────────────────────────────────────────────┐
│                      API Gateway (Rust: Axum)                   │
│              - JWT authentication                               │
│              - Rate limiting (basic)                            │
│              - Request validation                               │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌───────────────┐   ┌──────────────────┐   ┌──────────────────┐
│  Core Engine  │   │  Payment Service │   │ Reputation       │
│  (Rust)       │   │  (BTC on-chain)  │   │ (Manual)         │
│  - Tokens     │   │  - Bitcoin Core  │   │ - Admin review   │
│  - Orders     │   │  - Address gen   │   │ - Score calc     │
│  - Matching   │   │  - Confirmation  │   │                  │
└───────────────┘   └──────────────────┘   └──────────────────┘
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PostgreSQL Database                         │
│  - users, tokens, balances, orders, trades                      │
│  - reputation_scores, commitments                               │
└─────────────────────────────────────────────────────────────────┘

```

**Removed components:**

- ❌ Lightning Network (move to Phase 2)
- ❌ Redis Cache (performance optimization later)
- ❌ TimescaleDB (basic SQL analytics for now)
- ❌ AI Services (replace with manual verification)
- ❌ Complex Circuit Breaker (basic limits only)

---

## 💻 MVP Technical Stack

### Core Technologies

```yaml
Backend:
  Language: Rust 1.75+
  Framework: Axum 0.7
  Database: PostgreSQL 15
  ORM: SQLx (compile-time checked queries)
  Authentication: JWT + Argon2 password hashing
  Bitcoin: rust-bitcoin + bitcoincore-rpc

Frontend:
  Framework: Next.js 14 (React 18)
  Language: TypeScript 5
  Styling: Tailwind CSS
  State Management: Zustand
  API Client: TanStack Query (React Query)
  Wallet: Web3.js (for future DID integration)

Infrastructure:
  Hosting: AWS EC2 (t3.medium) or Hetzner VPS
  Database: Managed PostgreSQL (AWS RDS or self-hosted)
  CDN: CloudFlare
  Domain: kaccy.net (already owned)
  SSL: Let's Encrypt (free)

DevOps:
  CI/CD: GitHub Actions
  Containerization: Docker
  Monitoring: Simple logging (structured JSON)
  Backup: Daily PostgreSQL dumps to S3

```

---

## 📐 MVP Data Model (Simplified)

### Core Tables Only

```sql
-- ============================================
-- MVP SCHEMA (Simplified from Option A)
-- ============================================

-- Users (minimal fields)
CREATE TABLE users (
    user_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,  -- Argon2
    username VARCHAR(50) UNIQUE NOT NULL,
    display_name VARCHAR(100),
    bio TEXT,
    avatar_url TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    kyc_status VARCHAR(20) DEFAULT 'pending',  -- pending, verified, rejected
    reputation_score NUMERIC(10, 2) DEFAULT 500.0,
    INDEX idx_email (email),
    INDEX idx_username (username)
);

-- Tokens (simplified)
CREATE TABLE tokens (
    token_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issuer_user_id UUID REFERENCES users(user_id) ON DELETE CASCADE,
    symbol VARCHAR(20) UNIQUE NOT NULL,  -- e.g., "$KITASAN"
    name VARCHAR(100) NOT NULL,
    description TEXT,
    total_supply NUMERIC(38, 18) NOT NULL DEFAULT 100,
    circulating_supply NUMERIC(38, 18) NOT NULL DEFAULT 0,

    -- Bonding Curve Params (Linear for MVP)
    initial_price_btc NUMERIC(38, 18) NOT NULL,  -- e.g., 0.0001
    price_increment_btc NUMERIC(38, 18) NOT NULL,  -- e.g., 0.00001

    created_at TIMESTAMPTZ DEFAULT NOW(),
    status VARCHAR(20) DEFAULT 'active',  -- active, paused, closed

    INDEX idx_issuer (issuer_user_id),
    INDEX idx_symbol (symbol),
    INDEX idx_status (status)
);

-- Balances
CREATE TABLE balances (
    balance_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(user_id) ON DELETE CASCADE,
    token_id UUID REFERENCES tokens(token_id) ON DELETE CASCADE,
    amount NUMERIC(38, 18) NOT NULL CHECK (amount >= 0),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(user_id, token_id),
    INDEX idx_user_balances (user_id)
);

-- Orders (simplified: Market orders only)
CREATE TABLE orders (
    order_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(user_id),
    token_id UUID REFERENCES tokens(token_id),
    order_type VARCHAR(10) NOT NULL,  -- 'buy' or 'sell'
    amount NUMERIC(38, 18) NOT NULL,
    price_btc NUMERIC(38, 18) NOT NULL,
    total_btc NUMERIC(38, 18) NOT NULL,
    status VARCHAR(20) NOT NULL,  -- 'pending', 'completed', 'cancelled'

    -- Bitcoin payment info
    btc_address VARCHAR(100),  -- deposit address for purchases
    btc_txid VARCHAR(64),  -- Bitcoin transaction ID

    created_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    INDEX idx_user_orders (user_id, created_at DESC),
    INDEX idx_token_orders (token_id, status),
    INDEX idx_btc_address (btc_address)
);

-- Trades (simplified)
CREATE TABLE trades (
    trade_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    buyer_user_id UUID REFERENCES users(user_id),
    seller_user_id UUID REFERENCES users(user_id),  -- NULL for bonding curve buys
    token_id UUID REFERENCES tokens(token_id),
    amount NUMERIC(38, 18) NOT NULL,
    price_btc NUMERIC(38, 18) NOT NULL,
    total_btc NUMERIC(38, 18) NOT NULL,
    executed_at TIMESTAMPTZ DEFAULT NOW(),

    INDEX idx_token_trades (token_id, executed_at DESC),
    INDEX idx_user_trades (buyer_user_id, executed_at DESC)
);

-- Reputation Events (for manual verification)
CREATE TABLE reputation_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(user_id),
    event_type VARCHAR(50) NOT NULL,  -- 'commitment_fulfilled', 'commitment_failed', 'quality_rating'
    score_delta NUMERIC(10, 2) NOT NULL,  -- +10, -20, etc.
    description TEXT,
    verified_by UUID REFERENCES users(user_id),  -- Admin who verified
    created_at TIMESTAMPTZ DEFAULT NOW(),

    INDEX idx_user_events (user_id, created_at DESC)
);

-- Output Commitments (manual verification version)
CREATE TABLE output_commitments (
    commitment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(user_id),
    token_id UUID REFERENCES tokens(token_id),
    title VARCHAR(200) NOT NULL,
    description TEXT NOT NULL,
    deadline TIMESTAMPTZ NOT NULL,
    status VARCHAR(20) DEFAULT 'pending',  -- pending, completed, failed, verified

    -- Evidence (user uploads)
    evidence_url TEXT,
    evidence_description TEXT,

    -- Admin verification
    verified_by UUID REFERENCES users(user_id),
    verified_at TIMESTAMPTZ,
    verification_notes TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    INDEX idx_user_commitments (user_id, status),
    INDEX idx_deadline (deadline)
);

-- Admin Actions Log
CREATE TABLE admin_actions (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admin_user_id UUID REFERENCES users(user_id),
    action_type VARCHAR(50) NOT NULL,  -- 'verify_commitment', 'adjust_reputation', 'pause_token'
    target_user_id UUID REFERENCES users(user_id),
    target_token_id UUID REFERENCES tokens(token_id),
    details JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),

    INDEX idx_admin_actions (admin_user_id, created_at DESC)
);

```

**Removed components:**

- ❌ Complex Bonding Curve params (Linear only)
- ❌ Lightning invoices table (add in Phase 2)
- ❌ OrderBook details (no Limit orders)
- ❌ Token metadata JSONB (simple columns only)
- ❌ Dedicated SBT table (use reputation_events instead)

---

## 🎨 MVP User Flows

### Core User Journeys

### 1. **Issuer Journey: Token Issuance**

```
Step 1: Sign Up & KYC
├─ Email + Password registration
├─ Email verification
├─ Upload ID document (manual admin review)
└─ Wait for approval (24-48 hours)

Step 2: Create Token
├─ Navigate to "Issue Token"
├─ Fill form:
│   ├─ Symbol (e.g., "$KITASAN")
│   ├─ Name
│   ├─ Description
│   ├─ Initial Price (0.0001 BTC)
│   ├─ Price Increment (0.00001 BTC per token)
│   └─ Total Supply (100-10,000)
├─ Preview bonding curve
└─ Submit (requires reputation score > 400)

Step 3: Set Utility
├─ Describe what token holders get:
│   ├─ "Access to my Rust consulting (1 hour/month)"
│   ├─ "Early access to my GitHub private repos"
│   └─ "Priority support on Discord"
└─ Create initial commitment (optional):
    ├─ "I will release X by DATE"
    └─ Upload evidence when completed

Step 4: Promote
├─ Share token page on Twitter/LinkedIn
├─ Engage with early supporters
└─ Fulfill commitments to build reputation

```

### 2. **Buyer Journey: Token Purchase**

```
Step 1: Browse Tokens
├─ Landing page shows featured tokens
├─ Filter by:
│   ├─ Category (Developer, Consultant, Educator)
│   ├─ Price range
│   └─ Issuer reputation score
└─ Click token to view details

Step 2: Token Detail Page
├─ See issuer profile
├─ View bonding curve (current price)
├─ Read utility description
├─ Check commitment history
└─ Decide to buy

Step 3: Purchase (Simplified)
├─ Click "Buy" button
├─ Enter amount (e.g., 10 tokens)
├─ See total cost (in BTC)
├─ Generate BTC payment address
├─ Copy address & send BTC from external wallet
└─ Wait for 1 confirmation (~10 minutes)

Step 4: After Purchase
├─ Tokens appear in "My Portfolio"
├─ Can send message to issuer (basic feature)
├─ Access promised utilities
└─ Can sell back to bonding curve anytime

```

### 3. **Admin Journey: Verification Tasks**

```
Daily Tasks:
├─ Review KYC applications
│   ├─ Check ID document
│   ├─ Verify email domain
│   └─ Approve or reject
│
├─ Review commitment evidence
│   ├─ User submits "I completed X"
│   ├─ Check evidence (URL, description)
│   ├─ Verify authenticity
│   └─ Update reputation score
│
└─ Monitor suspicious activity
    ├─ Large orders
    ├─ Rapid trading
    └─ User complaints

```

---

## 🚀 MVP Feature List (Detailed)

### Must-Have Features (3 Months)

### **Phase 0.1: Foundation (Week 1-2)**

```yaml
Infrastructure:
  - [x] AWS/Hetzner VPS setup
  - [x] PostgreSQL installation
  - [x] Domain (kaccy.net) DNS setup
  - [x] SSL certificate (Let's Encrypt)
  - [x] GitHub repo setup (private)

Backend Core:
  - [x] Rust project initialization (Axum)
  - [x] Database schema creation (SQL migrations)
  - [x] User registration API
  - [x] JWT authentication
  - [x] Basic error handling
  - [x] Logging setup (tracing)

Frontend Core:
  - [x] Next.js project initialization
  - [x] Tailwind CSS setup
  - [x] Layout components (Header, Footer)
  - [x] Login/Register pages
  - [x] Home page (static)

```

### **Phase 0.2: Token System (Week 3-5)**

```yaml
Backend:
  - [ ] Token creation API
    - [ ] POST /api/tokens
    - [ ] Validation (symbol uniqueness, price params)
    - [ ] Reputation check (score >= 400)

  - [ ] Token listing API
    - [ ] GET /api/tokens (with pagination)
    - [ ] GET /api/tokens/:id (detail)
    - [ ] Filter by category, price

  - [ ] Bonding Curve calculation
    - [ ] Linear curve: P(n) = initial_price + n * increment
    - [ ] Calculate buy price (integral)
    - [ ] Calculate sell price (integral with small spread)

Frontend:
  - [ ] Token creation form
    - [ ] Multi-step wizard
    - [ ] Price curve preview (Chart.js)
    - [ ] Utility description textarea

  - [ ] Token browsing page
    - [ ] Card grid layout
    - [ ] Filter sidebar
    - [ ] Search functionality

  - [ ] Token detail page
    - [ ] Issuer profile section
    - [ ] Price chart
    - [ ] Utility description
    - [ ] Buy/Sell buttons

```

### **Phase 0.3: Bitcoin Payment (Week 6-7)**

```yaml
Backend:
  - [ ] Bitcoin Core integration
    - [ ] RPC connection setup
    - [ ] Address generation (HD wallet)
    - [ ] Transaction monitoring
    - [ ] Confirmation tracking

  - [ ] Order creation API
    - [ ] POST /api/orders/buy
    - [ ] Generate BTC address
    - [ ] Calculate total cost
    - [ ] Store order in DB

  - [ ] Payment confirmation worker
    - [ ] Poll Bitcoin Core every 30s
    - [ ] Check incoming transactions
    - [ ] Update order status
    - [ ] Execute trade (update balances)

Frontend:
  - [ ] Buy modal
    - [ ] Amount input
    - [ ] Price calculation display
    - [ ] QR code generation (BTC address)
    - [ ] Copy address button
    - [ ] Payment status polling

  - [ ] Portfolio page
    - [ ] List of owned tokens
    - [ ] Total value in BTC
    - [ ] Sell functionality

```

### **Phase 0.4: Reputation (Week 8-9)**

```yaml
Backend:
  - [ ] Commitment creation API
    - [ ] POST /api/commitments
    - [ ] Link to token
    - [ ] Set deadline

  - [ ] Commitment evidence submission
    - [ ] PUT /api/commitments/:id/evidence
    - [ ] Upload evidence URL
    - [ ] Mark as "awaiting verification"

  - [ ] Admin verification API
    - [ ] GET /api/admin/commitments (pending)
    - [ ] PUT /api/admin/commitments/:id/verify
    - [ ] Update reputation score
    - [ ] Log admin action

Frontend:
  - [ ] Commitment creation form (issuer)
  - [ ] Commitment list on token page
  - [ ] Evidence upload form
  - [ ] Admin dashboard
    - [ ] KYC queue
    - [ ] Commitment verification queue
    - [ ] User reputation management

```

### **Phase 0.5: Polish & Testing (Week 10-12)**

```yaml
Backend:
  - [ ] API documentation (OpenAPI/Swagger)
  - [ ] Rate limiting (basic: 100 req/min per IP)
  - [ ] Input sanitization
  - [ ] Security audit (self-review)
  - [ ] Database indexes optimization
  - [ ] Backup script (daily to S3)

Frontend:
  - [ ] Responsive design (mobile)
  - [ ] Loading states
  - [ ] Error messages
  - [ ] Success notifications
  - [ ] FAQ page
  - [ ] Terms of Service page (simplified)

Testing:
  - [ ] Unit tests (Rust: critical functions)
  - [ ] Integration tests (API endpoints)
  - [ ] End-to-end tests (Playwright: core flows)
  - [ ] Manual testing (beta testers)

Legal:
  - [ ] Basic ToS (based on Option C)
  - [ ] Privacy Policy
  - [ ] Cookie consent banner
  - [ ] Estonian VASP application submission (parallel)

```

---

## 👥 MVP Team & Resources

### Minimal Team Structure

```yaml
Scenario A: Solo Founder (You)
  Role: Full-stack developer + Product + Legal
  Timeline: 3-4 months (aggressive)
  Risk: High burnout risk, slower iteration
  Recommended: Contract specific tasks

Scenario B: You + 1 Developer
  Roles:
    - You: Backend (Rust) + Bitcoin + Product
    - Developer: Frontend (React/Next.js)
  Timeline: 2-3 months
  Cost: $15K-30K (contractor for 3 months)
  Risk: Medium

Scenario C: Small Team (Recommended)
  Roles:
    - You: Architecture + Backend Core + Product
    - Frontend Developer: React/Next.js
    - Designer (Part-time): UI/UX
    - Legal Advisor (Retainer): ToS, VASP application
  Timeline: 2-3 months
  Cost: $30K-50K
  Risk: Low, high quality output

```

### Development Resources Estimate

```yaml
Infrastructure (Monthly):
  - VPS (Hetzner): €20/month
  - PostgreSQL (managed): €30/month
  - Domain + SSL: €10/month
  - CloudFlare (free tier): €0
  - Bitcoin node (self-hosted): €0 (runs on same VPS)
  - Total: ~€60/month (~$65)

External Services:
  - KYC Provider (Onfido/Sumsub): $2-5 per verification
    - 50 users = $100-250
  - Transactional Email (SendGrid): Free tier (100 emails/day)
  - Analytics (Plausible): €9/month

Development Tools:
  - GitHub (private repos): Free
  - Figma (design): Free tier
  - Postman (API testing): Free tier

Total MVP Cost (3 months):
  - Infrastructure: $195
  - External Services: $300
  - Developer(s): $15K-50K (depending on team)
  - Legal: $5K-10K (Estonian VASP application)
  - Total: $20.5K - $60.5K

```

---

## 📊 MVP Success Metrics

### Definition of Success (Month 3)

```yaml
Minimum Success (Validation):
  Users:
    - 50 registered users
    - 20 KYC-verified users
    - 10 token issuers

  Tokens:
    - 10 tokens issued
    - 5 tokens with active trading

  Trading:
    - 100 total trades
    - $5,000 total trading volume (in BTC equivalent)
    - 50% of buyers return for 2nd purchase

  Reputation:
    - 15 commitments created
    - 10 commitments fulfilled
    - 66% fulfillment rate

  Platform:
    - 0 security incidents
    - 95% uptime
    - <2s page load time

Target Success (Strong Validation):
  Users: 100 registered, 50 verified, 20 issuers
  Tokens: 20 issued, 10 active
  Trading: 300 trades, $20K volume
  Reputation: 80% fulfillment rate

Stretch Goal (Product-Market Fit):
  Users: 200 registered, 100 verified, 50 issuers
  Tokens: 50 issued, 30 active
  Trading: 1000 trades, $100K volume
  Reputation: 85% fulfillment rate
  Retention: 70% of users active after 30 days

```

### Key Metrics Dashboard

```rust
// src/analytics/mvp_metrics.rs

#[derive(Debug, Serialize)]
pub struct MVPMetrics {
    // User Metrics
    pub total_users: u64,
    pub verified_users: u64,
    pub token_issuers: u64,

    // Token Metrics
    pub total_tokens: u64,
    pub active_tokens: u64,  // at least 1 trade in last 7 days

    // Trading Metrics
    pub total_trades: u64,
    pub total_volume_btc: Decimal,
    pub total_volume_usd: Decimal,
    pub avg_trade_size_btc: Decimal,

    // Reputation Metrics
    pub total_commitments: u64,
    pub fulfilled_commitments: u64,
    pub fulfillment_rate: f64,  // percentage

    // Engagement Metrics
    pub dau: u64,  // Daily Active Users
    pub wau: u64,  // Weekly Active Users
    pub mau: u64,  // Monthly Active Users
    pub retention_30d: f64,  // percentage

    // Platform Health
    pub avg_response_time_ms: f64,
    pub error_rate: f64,
    pub uptime_percentage: f64,
}

impl MVPMetrics {
    pub async fn calculate(db: &PgPool) -> Result<Self, DbError> {
        // SQL queries to calculate each metric
        todo!("Implement metric calculation")
    }

    pub fn health_score(&self) -> f64 {
        let mut score = 0.0;

        // User growth (25 points)
        if self.verified_users >= 50 { score += 25.0; }
        else { score += (self.verified_users as f64 / 50.0) * 25.0; }

        // Trading activity (25 points)
        if self.total_trades >= 100 { score += 25.0; }
        else { score += (self.total_trades as f64 / 100.0) * 25.0; }

        // Reputation (25 points)
        score += self.fulfillment_rate * 0.25;

        // Platform health (25 points)
        if self.uptime_percentage >= 95.0 && self.avg_response_time_ms < 2000.0 {
            score += 25.0;
        }

        score
    }
}

```

---

## 🎯 MVP Go-to-Market (Closed Beta)

### Beta User Recruitment Strategy

```yaml
Phase 1: Hand-picked Seeds (Week 1-2, 10 users)
  Target:
    - Your existing network (LinkedIn, GitHub)
    - Trusted developers, consultants
    - Early crypto adopters you know personally

  Criteria:
    - Must have professional expertise to tokenize
    - Willing to give detailed feedback
    - Active on social media (for later promotion)

  Approach:
    - Personal email invitation
    - 1-on-1 onboarding call
    - Free KYC verification
    - No platform fees for first month

Phase 2: Warm Outreach (Week 3-6, 40 users)
  Target:
    - Rust community (Reddit r/rust, Discord)
    - Indie hackers (IndieHackers.com)
    - Technical Twitter/X accounts

  Criteria:
    - GitHub profile with 50+ repos
    - Or LinkedIn with "Consultant/Freelancer"
    - Or technical blog/YouTube channel

  Approach:
    - Application form (Google Forms)
    - Review applications manually
    - Send invitation codes
    - Weekly cohort onboarding (group calls)

Phase 3: Expansion (Week 7-12, 50 more users)
  Target:
    - Referrals from Phase 1-2 users
    - Waitlist from landing page

  Criteria:
    - Referral from existing user (priority)
    - Or application with strong profile

  Approach:
    - Automated invitation system
    - Self-service onboarding
    - Community Discord for support

```

### Beta Incentives

```rust
pub struct BetaIncentives {
    early_adopter_benefits: vec![
        "Lifetime 50% fee discount",
        "Priority customer support",
        "Input on roadmap (Discord polls)",
        "Early access to new features",
        "Recognition badge (public profile)",
    ],

    issuer_incentives: vec![
        "First 10 issuers: No token creation fee",
        "Featured on homepage",
        "Personal success manager (You)",
    ],

    referral_program: ReferralProgram {
        reward: "Refer 3 users → Get $KACCY airdrop (Phase 2)",
        tracking: "Unique referral codes",
    },
}

```

---

## 🔄 Feedback Loop & Iteration

### Weekly Cadence

```yaml
Monday:
  - Review previous week metrics
  - Prioritize bug fixes
  - Plan week's tasks

Wednesday:
  - User feedback review session
    - Read support tickets
    - Review Discord messages
    - Check Twitter mentions
  - Quick bug fixes deployment

Friday:
  - Feature deployment (if ready)
  - Metrics review
  - User announcement (email + Discord)

Every 2 Weeks:
  - Beta user survey
    - NPS score
    - Feature requests
    - Pain points
  - Group feedback call (optional attendance)

```

### Key Questions to Answer

```rust
pub enum ValidationQuestions {
    Q1 => "Do users understand the 'Proof of Output' concept?",
    Q2 => "Is the KYC process too burdensome?",
    Q3 => "Is Bitcoin-only payment acceptable, or do they demand Lightning?",
    Q4 => "What utilities do issuers actually offer? (Are they creative?)",
    Q5 => "Do buyers feel they get value, or is it pure speculation?",
    Q6 => "Is manual admin verification sustainable, or must we automate?",
    Q7 => "What's the biggest blocker to issuing a token?",
    Q8 => "What's the biggest blocker to buying a token?",
    Q9 => "How's the reputation system perceived? Fair? Gameable?",
    Q10 => "Would users pay for this long-term, or just experimenting?",
}

```

---

## 🚨 MVP Risks & Mitigation

### Top Risks

```rust
pub enum MVPRisks {
    TechnicalRisk {
        risk: "Bitcoin Core sync takes days, payments are slow",
        probability: "High (80%)",
        impact: "Medium",
        mitigation: vec![
            "Use Blockstream.info API as fallback",
            "Set expectations: 10-60 min confirmation",
            "Phase 2: Lightning fixes this",
        ],
    },

    LegalRisk {
        risk: "Estonian VASP license delayed beyond 3 months",
        probability: "Medium (40%)",
        impact: "High (can't launch publicly)",
        mitigation: vec![
            "Start application NOW (parallel to dev)",
            "Keep beta private (50-100 users = not public offering)",
            "Backup: Launch without EU users, focus on Asia",
        ],
    },

    UserAcquisitionRisk {
        risk: "Can't find 50 quality beta users",
        probability: "Low (20%)",
        impact: "High (no validation)",
        mitigation: vec![
            "Leverage your existing network first",
            "Offer strong incentives (lifetime discounts)",
            "Start outreach early (Week 1)",
        ],
    },

    ProductRisk {
        risk: "Users don't see value in tokenizing themselves",
        probability: "Medium (30%)",
        impact: "Critical (product-market fit failure)",
        mitigation: vec![
            "Extensive user interviews before building",
            "Start with narrow niche (Rust devs only)",
            "Be prepared to pivot utility definition",
        ],
    },

    CompetitionRisk {
        risk: "Friend.tech v2 launches during our beta",
        probability: "Low (15%)",
        impact: "Medium",
        mitigation: vec![
            "Our differentiation (Proof of Output) is strong",
            "Focus on different audience (pros, not influencers)",
            "Speed matters: Launch fast",
        ],
    },

    BurnoutRisk {
        risk: "Solo founder burns out before completion",
        probability: "Medium (40% if solo)",
        impact: "Critical",
        mitigation: vec![
            "Hire at least 1 contractor (frontend)",
            "Cut scope aggressively if behind schedule",
            "Set hard 3-month deadline, no extensions",
        ],
    },
}

```

---

## 📅 Detailed 12-Week Timeline

### Gantt Chart (Text Format)

```
Week │ Backend                  │ Frontend                │ Other
─────┼─────────────────────────┼─────────────────────────┼──────────────────────
  1  │ Setup + Auth            │ Setup + Login pages     │ VASP application start
  2  │ Auth + User DB          │ Home + Layout           │ User research calls
  3  │ Token creation API      │ Token creation form     │ Beta user outreach
  4  │ Token listing API       │ Token browsing page     │ First 10 invitations
  5  │ Bonding Curve logic     │ Token detail page       │ Weekly feedback calls
  6  │ Bitcoin integration     │ Buy modal + QR code     │ First tokens issued
  7  │ Payment confirmation    │ Portfolio page          │ First trades executed
  8  │ Commitment APIs         │ Commitment UI           │ Legal docs finalization
  9  │ Admin verification      │ Admin dashboard         │ Security self-audit
 10  │ Polish + Testing        │ Responsive design       │ Prepare public beta
 11  │ Bug fixes               │ Bug fixes               │ Marketing content
 12  │ Final testing           │ Final testing           │ Public beta launch

```

### Milestone Checklist

```yaml
✅ Month 1 (Week 1-4):
  - [ ] Infrastructure live
  - [ ] Users can register & login
  - [ ] Users can create tokens (basic)
  - [ ] Tokens visible on browse page
  - [ ] 10 beta users onboarded

✅ Month 2 (Week 5-8):
  - [ ] Bitcoin payment working (testnet)
  - [ ] First successful buy transaction
  - [ ] Balances update correctly
  - [ ] Commitment system functional
  - [ ] 30 beta users onboarded
  - [ ] 5 tokens issued

✅ Month 3 (Week 9-12):
  - [ ] Bitcoin mainnet enabled
  - [ ] Admin verification workflow complete
  - [ ] Reputation scores updating
  - [ ] 50+ beta users
  - [ ] 10+ tokens issued
  - [ ] 50+ trades executed
  - [ ] Estonian VASP application submitted
  - [ ] Public beta launch (if license approved)

```

---

## 🎨 MVP UI/UX Wireframes (Description)

### Key Screens

### 1. **Landing Page**

```
┌─────────────────────────────────────────────────────────┐
│  [Logo] Kaccy                    Login | Sign Up        │
├─────────────────────────────────────────────────────────┤
│                                                          │
│              Tokenize Your Expertise                     │
│         Turn Your Skills into Tradable Value            │
│                                                          │
│           [Get Started - Beta Access]                    │
│                                                          │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  How It Works:                                           │
│  1. Create Your Token  2. Offer Utility  3. Get Paid    │
│                                                          │
│  Featured Tokens:                                        │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                 │
│  │$KITASAN │  │ $ALICE  │  │  $BOB   │                 │
│  │Rust Dev │  │Designer │  │Consultant│                 │
│  │0.001 BTC│  │0.0005.. │  │0.002 BTC│                 │
│  └─────────┘  └─────────┘  └─────────┘                 │
│                                                          │
└─────────────────────────────────────────────────────────┘

```

### 2. **Token Detail Page**

```
┌─────────────────────────────────────────────────────────┐
│  [Back to Browse]                                        │
├─────────────────────────────────────────────────────────┤
│  [Avatar]  $KITASAN                    [Buy] [Sell]     │
│            Tetsuya Kita                                  │
│            Reputation: 850 ⭐                            │
├─────────────────────────────────────────────────────────┤
│  Price: 0.0012 BTC  │  Supply: 50/1000                  │
│  [Price Chart]                                           │
├─────────────────────────────────────────────────────────┤
│  What You Get:                                           │
│  ✓ 1 hour Rust consulting per month                     │
│  ✓ Access to private GitHub repos                       │
│  ✓ Priority support on Discord                          │
├─────────────────────────────────────────────────────────┤
│  Commitments:                                            │
│  ✅ Release v2.0 by Dec 31 (Completed)                  │
│  ⏳ Write 5 tutorials by Jan 15 (In Progress)           │
├─────────────────────────────────────────────────────────┤
│  Holders: 12  │  Total Trades: 45                       │
└─────────────────────────────────────────────────────────┘

```

### 3. **Admin Dashboard**

```
┌─────────────────────────────────────────────────────────┐
│  Admin Dashboard                                         │
├─────────────────────────────────────────────────────────┤
│  Pending KYC: (5)                                        │
│  ├─ user_123 | john@example.com | [Review]              │
│  ├─ user_456 | alice@example.com | [Review]             │
│                                                          │
│  Pending Commitments: (3)                                │
│  ├─ $KITASAN | "Release v2.0" | [Verify] [Reject]       │
│  ├─ $ALICE | "Design 10 logos" | [Verify] [Reject]      │
│                                                          │
│  Recent Actions:                                         │
│  ├─ Approved KYC for user_789                            │
│  ├─ Verified commitment for $BOB (+10 rep)              │
└─────────────────────────────────────────────────────────┘

```

---

## 🔒 MVP Security Checklist

```yaml
Essential Security (Must-Have):
  - [x] HTTPS only (Let's Encrypt)
  - [x] Password hashing (Argon2)
  - [x] JWT with short expiry (1 hour)
  - [x] SQL injection prevention (SQLx parameterized queries)
  - [x] XSS prevention (React auto-escaping)
  - [x] CSRF token (Next.js built-in)
  - [x] Rate limiting (basic: 100 req/min per IP)
  - [x] Input validation (backend + frontend)
  - [x] Bitcoin address validation
  - [x] Sensitive data encryption at rest

Nice-to-Have (Phase 2):
  - [ ] 2FA (TOTP)
  - [ ] DDoS protection (CloudFlare)
  - [ ] Advanced rate limiting (by user)
  - [ ] Audit logging (all sensitive actions)
  - [ ] Penetration testing (external firm)

```

---

## 📊 Post-MVP: Phase 2 Roadmap Preview

### After Successful MVP (Month 4-12)

```yaml
Q2 2026 (Month 4-6):
  Priority 1:
    - Lightning Network integration
    - GitHub OAuth + automatic commit verification
    - AI quality evaluation (basic)
    - Mobile-responsive optimization

  Priority 2:
    - KYC Level 2-3
    - Advanced bonding curves (Adaptive)
    - Token burn mechanism
    - Staking for $KACCY

Q3 2026 (Month 7-9):
  Priority 1:
    - $KACCY platform token launch
    - DAO governance (basic voting)
    - Mobile app (React Native)
    - Fiat on-ramp (Stripe/MoonPay)

  Priority 2:
    - Analytics dashboard
    - API for third-party integrations
    - Affiliate/referral program
    - Multi-language support

Q4 2026 (Month 10-12):
  Priority 1:
    - L2 integration (Stacks or RSK)
    - Advanced AI agent access control
    - SBT (Soulbound Token) for reputation
    - Full DAO transition preparation

  Priority 2:
    - NFT integration
    - Cross-chain bridges
    - Enterprise features (for Rally-like clients)

```

---

## 🎯 MVP Success Criteria (Go/No-Go Decision)

### After 3 Months, Evaluate:

```rust
pub struct GoNoGoDecision {
    criteria: Vec<Criterion>,
}

pub struct Criterion {
    metric: String,
    threshold_go: String,
    threshold_no_go: String,
    weight: f64,
}

impl GoNoGoDecision {
    pub fn evaluate() -> Decision {
        let criteria = vec![
            Criterion {
                metric: "Active users (30 days)".to_string(),
                threshold_go: ">= 30",
                threshold_no_go: "< 15",
                weight: 0.20,
            },
            Criterion {
                metric: "Tokens issued".to_string(),
                threshold_go: ">= 10",
                threshold_no_go: "< 5",
                weight: 0.15,
            },
            Criterion {
                metric: "Total trades".to_string(),
                threshold_go: ">= 100",
                threshold_no_go: "< 30",
                weight: 0.20,
            },
            Criterion {
                metric: "Commitment fulfillment rate".to_string(),
                threshold_go: ">= 70%",
                threshold_no_go: "< 50%",
                weight: 0.15,
            },
            Criterion {
                metric: "User NPS score".to_string(),
                threshold_go: ">= 30",
                threshold_no_go: "< 0",
                weight: 0.15,
            },
            Criterion {
                metric: "Technical stability (uptime)".to_string(),
                threshold_go: ">= 95%",
                threshold_no_go: "< 90%",
                weight: 0.10,
            },
            Criterion {
                metric: "Fundraising interest".to_string(),
                threshold_go: "1+ term sheet OR sustainable without funding",
                threshold_no_go: "No investor interest & burning cash",
                weight: 0.05,
            },
        ];

        // Weighted scoring logic
        Decision::Go  // or Decision::Pivot or Decision::NoGo
    }
}

pub enum Decision {
    Go,        // Proceed to Phase 2, raise funding, scale
    Pivot,     // Core idea validated but needs changes
    NoGo,      // Shut down, lessons learned
}

```

---

## 📝 MVP Summary

### What We're Building

> Kaccy MVP: A Bitcoin-based personal token platform where professionals can tokenize their expertise, and buyers can invest in individual creators' future output. The MVP validates the core hypothesis: "Will people pay for access to professionals' services via tokens, and will reputation tracking create a sustainable economy?"
>

### What's Included (MVP)

✅ User registration & basic KYC
✅ Token creation (linear bonding curve)
✅ Token browsing & purchase (BTC on-chain)
✅ Portfolio management
✅ Commitment creation & manual verification
✅ Reputation scoring (manual admin review)
✅ Basic admin dashboard

### What's Excluded (Phase 2+)

❌ Lightning Network (too complex for MVP)
❌ AI-powered verification (manual is faster to build)
❌ Advanced bonding curves (linear is sufficient)
❌ Mobile app (web-first)
❌ $KACCY platform token (premature)
❌ DAO governance (centralized for MVP)

### Timeline & Budget

- **Duration**: 12 weeks (3 months)
- **Team**: 1-3 people (You + contractor(s))
- **Budget**: $20K-60K
- **Target Users**: 50-100 beta users
- **Target Tokens**: 10-20 issued
- **Target Volume**: $5K-20K

### Success Metric

> If 50+ beta users, 10+ tokens, and 70%+ commitment fulfillment rate → Proceed to Phase 2
>

---

## 🚀 Next Steps

### Immediate Actions (This Week)

```yaml
Day 1:
  - [ ] Decide on team structure (solo vs hire)
  - [ ] If hiring: Post job on Upwork/Twitter
  - [ ] Set up GitHub repo (private)
  - [ ] Create project board (GitHub Projects or Linear)

Day 2-3:
  - [ ] Finalize MVP scope (review this document)
  - [ ] Create wireframes (Figma or paper)
  - [ ] Set up development environment
  - [ ] Initialize Rust + Next.js projects

Day 4-5:
  - [ ] Start Estonian VASP application (parallel track)
  - [ ] Draft beta user invitation email
  - [ ] Create landing page (coming soon)
  - [ ] First 10 personal outreach messages

Weekend:
  - [ ] Database schema review
  - [ ] Code first API endpoint (user registration)
  - [ ] Begin beta user interviews

```

---

## 🎤 Final Recommendation

Dear User, the MVP definition is complete!

### My Recommended Actions:

1. **Proceed with this MVP scope**
    - 3 months is achievable (given your technical skills)
    - Minimally validates the value proposition
2. **Hire at least 1 frontend developer**
    - Prevents burnout
    - You focus on Rust + Bitcoin + Product
3. **Start moving Week 1**
    - VASP application (takes 3-6 months)
    - Beta user acquisition (relationship building takes time)
4. **Strict 3-month cutoff**
    - Don't extend (technical debt accumulates)
    - Make Go/Pivot/NoGo decision after 3 months
