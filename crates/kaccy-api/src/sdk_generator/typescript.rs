//! TypeScript SDK generator

use std::collections::HashMap;

use super::types::{GeneratedSdk, SdkConfig, SdkLanguage, SdkMetadata};

/// TypeScript SDK generator
pub struct TypeScriptGenerator;

impl TypeScriptGenerator {
    /// Generate TypeScript SDK
    pub fn generate(config: &SdkConfig) -> GeneratedSdk {
        let mut files = HashMap::new();

        // Generate main client file
        files.insert("client.ts".to_string(), Self::generate_client(config));

        // Generate type definitions
        files.insert("types.ts".to_string(), Self::generate_types());

        // Generate API methods
        files.insert("api.ts".to_string(), Self::generate_api_methods());

        // Generate package.json
        files.insert(
            "package.json".to_string(),
            Self::generate_package_json(config),
        );

        // Generate README
        files.insert("README.md".to_string(), Self::generate_readme(config));

        GeneratedSdk {
            language: SdkLanguage::TypeScript,
            files,
            metadata: SdkMetadata {
                name: config.package_name.clone(),
                version: config.version.clone(),
                description: "TypeScript SDK for Kaccy Protocol API".to_string(),
                dependencies: vec!["axios".to_string()],
            },
        }
    }

    fn generate_client(config: &SdkConfig) -> String {
        format!(
            r#"/**
 * Kaccy Protocol API Client
 *
 * TypeScript/JavaScript SDK for interacting with Kaccy Protocol
 */

import axios, {{ AxiosInstance, AxiosRequestConfig }} from 'axios';
import {{
    User, Token, Order, Trade, Commitment,
    RegisterRequest, LoginRequest, AuthResponse,
    CreateTokenRequest, BuyOrderRequest, SellOrderRequest
}} from './types';

export interface KaccyClientConfig {{
    baseUrl?: string;
    apiKey?: string;
    token?: string;
}}

export class KaccyClient {{
    private client: AxiosInstance;
    private token?: string;

    constructor(config: KaccyClientConfig = {{}}) {{
        this.token = config.token;
        this.client = axios.create({{
            baseURL: config.baseUrl || '{}',
            headers: {{
                'Content-Type': 'application/json',
            }},
        }});

        // Add auth interceptor
        this.client.interceptors.request.use((config) => {{
            if (this.token) {{
                config.headers.Authorization = `Bearer ${{this.token}}`;
            }}
            return config;
        }});
    }}

    /**
     * Set authentication token
     */
    setToken(token: string): void {{
        this.token = token;
    }}

    /**
     * Register a new user
     */
    async register(request: RegisterRequest): Promise<AuthResponse> {{
        const {{ data }} = await this.client.post<AuthResponse>('/api/auth/register', request);
        this.token = data.token;
        return data;
    }}

    /**
     * Login with email and password
     */
    async login(request: LoginRequest): Promise<AuthResponse> {{
        const {{ data }} = await this.client.post<AuthResponse>('/api/auth/login', request);
        this.token = data.token;
        return data;
    }}

    /**
     * Get current user profile
     */
    async getProfile(): Promise<User> {{
        const {{ data }} = await this.client.get<User>('/api/users/me');
        return data;
    }}

    /**
     * Update user profile
     */
    async updateProfile(updates: Partial<User>): Promise<User> {{
        const {{ data }} = await this.client.put<User>('/api/users/me', updates);
        return data;
    }}

    /**
     * List tokens
     */
    async listTokens(params?: {{ page?: number; per_page?: number }}): Promise<{{ tokens: Token[]; total: number }}> {{
        const {{ data }} = await this.client.get('/api/tokens', {{ params }});
        return data;
    }}

    /**
     * Get token details
     */
    async getToken(tokenId: string): Promise<Token> {{
        const {{ data }} = await this.client.get<Token>(`/api/tokens/${{tokenId}}`);
        return data;
    }}

    /**
     * Create a new token
     */
    async createToken(request: CreateTokenRequest): Promise<Token> {{
        const {{ data }} = await this.client.post<Token>('/api/tokens', request);
        return data;
    }}

    /**
     * Place a buy order
     */
    async buyTokens(request: BuyOrderRequest): Promise<Order> {{
        const {{ data }} = await this.client.post<Order>('/api/orders/buy', request);
        return data;
    }}

    /**
     * Place a sell order
     */
    async sellTokens(request: SellOrderRequest): Promise<Order> {{
        const {{ data }} = await this.client.post<Order>('/api/orders/sell', request);
        return data;
    }}

    /**
     * Get user's orders
     */
    async getOrders(): Promise<Order[]> {{
        const {{ data }} = await this.client.get<Order[]>('/api/orders');
        return data;
    }}

    /**
     * Get portfolio
     */
    async getPortfolio(): Promise<any> {{
        const {{ data }} = await this.client.get('/api/portfolio');
        return data;
    }}

    /**
     * Create a commitment
     */
    async createCommitment(request: any): Promise<Commitment> {{
        const {{ data }} = await this.client.post<Commitment>('/api/commitments', request);
        return data;
    }}
}}

export default KaccyClient;
"#,
            config.base_url
        )
    }

    fn generate_types() -> String {
        r#"/**
 * Type definitions for Kaccy Protocol API
 */

export interface User {
    user_id: string;
    username: string;
    email?: string;
    display_name?: string;
    bio?: string;
    avatar_url?: string;
    reputation_score: number;
    role: string;
    created_at: string;
}

export interface Token {
    token_id: string;
    issuer_id: string;
    symbol: string;
    name: string;
    description?: string;
    current_supply: number;
    current_price_btc: number;
    status: string;
    created_at: string;
}

export interface Order {
    order_id: string;
    user_id: string;
    token_id: string;
    order_type: 'buy' | 'sell';
    amount: number;
    price_btc: number;
    status: 'pending' | 'completed' | 'cancelled' | 'expired';
    btc_address?: string;
    created_at: string;
}

export interface Trade {
    trade_id: string;
    token_id: string;
    buyer_id: string;
    seller_id?: string;
    amount: number;
    price_btc: number;
    total_btc: number;
    platform_fee_btc: number;
    issuer_royalty_btc: number;
    executed_at: string;
}

export interface Commitment {
    commitment_id: string;
    user_id: string;
    token_id: string;
    title: string;
    description?: string;
    deadline: string;
    status: string;
    evidence_url?: string;
    verified_at?: string;
    created_at: string;
}

export interface RegisterRequest {
    email: string;
    password: string;
    username: string;
}

export interface LoginRequest {
    email: string;
    password: string;
}

export interface AuthResponse {
    token: string;
    user: User;
}

export interface CreateTokenRequest {
    symbol: string;
    name: string;
    description?: string;
    base_price_btc: number;
    price_increment: number;
}

export interface BuyOrderRequest {
    token_id: string;
    amount: number;
    max_price_btc?: number;
}

export interface SellOrderRequest {
    token_id: string;
    amount: number;
    min_price_btc?: number;
}
"#
        .to_string()
    }

    fn generate_api_methods() -> String {
        r#"/**
 * API method helpers
 */

export class ApiError extends Error {
    constructor(
        message: string,
        public statusCode: number,
        public details?: any
    ) {
        super(message);
        this.name = 'ApiError';
    }
}

export function handleApiError(error: any): never {
    if (error.response) {
        throw new ApiError(
            error.response.data.message || 'API request failed',
            error.response.status,
            error.response.data
        );
    } else if (error.request) {
        throw new ApiError('No response from server', 0);
    } else {
        throw new ApiError(error.message, 0);
    }
}
"#
        .to_string()
    }

    fn generate_package_json(config: &SdkConfig) -> String {
        format!(
            r#"{{
  "name": "{}",
  "version": "{}",
  "description": "TypeScript SDK for Kaccy Protocol API",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "scripts": {{
    "build": "tsc",
    "test": "jest",
    "prepublishOnly": "npm run build"
  }},
  "keywords": [
    "kaccy",
    "bitcoin",
    "tokens",
    "api",
    "sdk"
  ],
  "author": "COOLJAPAN OU (Team KitaSan)",
  "license": "MIT",
  "dependencies": {{
    "axios": "^1.6.0"
  }},
  "devDependencies": {{
    "@types/node": "^20.0.0",
    "typescript": "^5.0.0",
    "jest": "^29.0.0"
  }}
}}
"#,
            config.package_name, config.version
        )
    }

    fn generate_readme(config: &SdkConfig) -> String {
        format!(
            r#"# {} - Kaccy Protocol TypeScript SDK

Official TypeScript/JavaScript SDK for the Kaccy Protocol API.

## Installation

```bash
npm install {}
```

## Quick Start

```typescript
import KaccyClient from '{}';

// Initialize client
const client = new KaccyClient({{
  baseUrl: '{}'
}});

// Register a new user
const {{ token, user }} = await client.register({{
  email: 'user@example.com',
  password: 'SecurePass123!',
  username: 'alice'
}});

// Client is automatically authenticated after registration/login

// Create a token
const token = await client.createToken({{
  symbol: 'ALICE',
  name: 'Alice Token',
  description: 'Support Alice\'s creative work',
  base_price_btc: 0.000001,
  price_increment: 0.0000001
}});

// Buy tokens
const order = await client.buyTokens({{
  token_id: token.token_id,
  amount: 100,
  max_price_btc: 0.001
}});

// Get portfolio
const portfolio = await client.getPortfolio();
```

## API Reference

### Authentication

- `register(request)` - Register a new user
- `login(request)` - Login with email/password
- `setToken(token)` - Set authentication token

### Users

- `getProfile()` - Get current user profile
- `updateProfile(updates)` - Update user profile

### Tokens

- `listTokens(params)` - List all tokens
- `getToken(tokenId)` - Get token details
- `createToken(request)` - Create a new token

### Orders

- `buyTokens(request)` - Place a buy order
- `sellTokens(request)` - Place a sell order
- `getOrders()` - Get user's orders

### Portfolio

- `getPortfolio()` - Get user's portfolio

### Commitments

- `createCommitment(request)` - Create a new commitment

## Error Handling

```typescript
try {{
  await client.createToken(request);
}} catch (error) {{
  if (error instanceof ApiError) {{
    console.error('API Error:', error.message);
    console.error('Status:', error.statusCode);
    console.error('Details:', error.details);
  }}
}}
```

## License

MIT
"#,
            config.package_name, config.package_name, config.package_name, config.base_url
        )
    }
}
