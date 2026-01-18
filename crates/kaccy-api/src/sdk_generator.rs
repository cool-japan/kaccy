//! SDK generator for multiple programming languages
//!
//! Generates client libraries for TypeScript, Python, and Rust from OpenAPI spec

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported SDK languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SdkLanguage {
    TypeScript,
    Python,
    Rust,
    Go,
}

impl std::fmt::Display for SdkLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SdkLanguage::TypeScript => write!(f, "typescript"),
            SdkLanguage::Python => write!(f, "python"),
            SdkLanguage::Rust => write!(f, "rust"),
            SdkLanguage::Go => write!(f, "go"),
        }
    }
}

/// SDK generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkConfig {
    /// Target language
    pub language: SdkLanguage,
    /// Package name
    pub package_name: String,
    /// Package version
    pub version: String,
    /// API base URL
    pub base_url: String,
    /// Additional configuration
    pub options: HashMap<String, String>,
}

/// Generated SDK files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSdk {
    /// Language
    pub language: SdkLanguage,
    /// Generated files (filename -> content)
    pub files: HashMap<String, String>,
    /// Package metadata
    pub metadata: SdkMetadata,
}

/// SDK package metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMetadata {
    /// Package name
    pub name: String,
    /// Version
    pub version: String,
    /// Description
    pub description: String,
    /// Dependencies
    pub dependencies: Vec<String>,
}

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

/// Python SDK generator
pub struct PythonGenerator;

impl PythonGenerator {
    /// Generate Python SDK
    pub fn generate(config: &SdkConfig) -> GeneratedSdk {
        let mut files = HashMap::new();

        // Generate main client
        files.insert("client.py".to_string(), Self::generate_client(config));

        // Generate models
        files.insert("models.py".to_string(), Self::generate_models());

        // Generate exceptions
        files.insert("exceptions.py".to_string(), Self::generate_exceptions());

        // Generate __init__.py
        files.insert("__init__.py".to_string(), Self::generate_init());

        // Generate setup.py
        files.insert("setup.py".to_string(), Self::generate_setup(config));

        // Generate README
        files.insert("README.md".to_string(), Self::generate_readme(config));

        GeneratedSdk {
            language: SdkLanguage::Python,
            files,
            metadata: SdkMetadata {
                name: config.package_name.clone(),
                version: config.version.clone(),
                description: "Python SDK for Kaccy Protocol API".to_string(),
                dependencies: vec![
                    "requests>=2.31.0".to_string(),
                    "pydantic>=2.0.0".to_string(),
                ],
            },
        }
    }

    fn generate_client(config: &SdkConfig) -> String {
        format!(
            r#"""
Kaccy Protocol API Client

Python SDK for interacting with Kaccy Protocol
"""

import requests
from typing import Optional, Dict, Any, List
from .models import (
    User, Token, Order, Trade, Commitment,
    RegisterRequest, LoginRequest, AuthResponse,
    CreateTokenRequest, BuyOrderRequest, SellOrderRequest
)
from .exceptions import ApiError


class KaccyClient:
    """Main client for Kaccy Protocol API"""

    def __init__(self, base_url: str = "{}", token: Optional[str] = None):
        """
        Initialize the Kaccy client

        Args:
            base_url: API base URL
            token: Optional authentication token
        """
        self.base_url = base_url.rstrip('/')
        self.token = token
        self.session = requests.Session()
        self.session.headers.update({{'Content-Type': 'application/json'}})

    def set_token(self, token: str) -> None:
        """Set authentication token"""
        self.token = token

    def _request(self, method: str, path: str, **kwargs) -> Any:
        """Make HTTP request"""
        url = f"{{self.base_url}}{{path}}"

        if self.token:
            kwargs.setdefault('headers', {{}})
            kwargs['headers']['Authorization'] = f'Bearer {{self.token}}'

        response = self.session.request(method, url, **kwargs)

        if not response.ok:
            raise ApiError(
                message=response.json().get('message', 'API request failed'),
                status_code=response.status_code,
                details=response.json()
            )

        return response.json()

    def register(self, email: str, password: str, username: str) -> AuthResponse:
        """Register a new user"""
        data = self._request('POST', '/api/auth/register', json={{
            'email': email,
            'password': password,
            'username': username
        }})
        self.token = data['token']
        return AuthResponse(**data)

    def login(self, email: str, password: str) -> AuthResponse:
        """Login with email and password"""
        data = self._request('POST', '/api/auth/login', json={{
            'email': email,
            'password': password
        }})
        self.token = data['token']
        return AuthResponse(**data)

    def get_profile(self) -> User:
        """Get current user profile"""
        data = self._request('GET', '/api/users/me')
        return User(**data)

    def update_profile(self, **updates) -> User:
        """Update user profile"""
        data = self._request('PUT', '/api/users/me', json=updates)
        return User(**data)

    def list_tokens(self, page: int = 1, per_page: int = 20) -> Dict[str, Any]:
        """List tokens"""
        params = {{'page': page, 'per_page': per_page}}
        return self._request('GET', '/api/tokens', params=params)

    def get_token(self, token_id: str) -> Token:
        """Get token details"""
        data = self._request('GET', f'/api/tokens/{{token_id}}')
        return Token(**data)

    def create_token(
        self,
        symbol: str,
        name: str,
        base_price_btc: float,
        price_increment: float,
        description: Optional[str] = None
    ) -> Token:
        """Create a new token"""
        data = self._request('POST', '/api/tokens', json={{
            'symbol': symbol,
            'name': name,
            'description': description,
            'base_price_btc': base_price_btc,
            'price_increment': price_increment
        }})
        return Token(**data)

    def buy_tokens(
        self,
        token_id: str,
        amount: float,
        max_price_btc: Optional[float] = None
    ) -> Order:
        """Place a buy order"""
        data = self._request('POST', '/api/orders/buy', json={{
            'token_id': token_id,
            'amount': amount,
            'max_price_btc': max_price_btc
        }})
        return Order(**data)

    def sell_tokens(
        self,
        token_id: str,
        amount: float,
        min_price_btc: Optional[float] = None
    ) -> Order:
        """Place a sell order"""
        data = self._request('POST', '/api/orders/sell', json={{
            'token_id': token_id,
            'amount': amount,
            'min_price_btc': min_price_btc
        }})
        return Order(**data)

    def get_orders(self) -> List[Order]:
        """Get user's orders"""
        data = self._request('GET', '/api/orders')
        return [Order(**order) for order in data]

    def get_portfolio(self) -> Dict[str, Any]:
        """Get user's portfolio"""
        return self._request('GET', '/api/portfolio')

    def create_commitment(
        self,
        token_id: str,
        title: str,
        deadline: str,
        description: Optional[str] = None
    ) -> Commitment:
        """Create a new commitment"""
        data = self._request('POST', '/api/commitments', json={{
            'token_id': token_id,
            'title': title,
            'description': description,
            'deadline': deadline
        }})
        return Commitment(**data)
"#,
            config.base_url
        )
    }

    fn generate_models() -> String {
        r#"""
Data models for Kaccy Protocol API
"""

from typing import Optional
from pydantic import BaseModel
from datetime import datetime


class User(BaseModel):
    user_id: str
    username: str
    email: Optional[str] = None
    display_name: Optional[str] = None
    bio: Optional[str] = None
    avatar_url: Optional[str] = None
    reputation_score: float
    role: str
    created_at: str


class Token(BaseModel):
    token_id: str
    issuer_id: str
    symbol: str
    name: str
    description: Optional[str] = None
    current_supply: float
    current_price_btc: float
    status: str
    created_at: str


class Order(BaseModel):
    order_id: str
    user_id: str
    token_id: str
    order_type: str
    amount: float
    price_btc: float
    status: str
    btc_address: Optional[str] = None
    created_at: str


class Trade(BaseModel):
    trade_id: str
    token_id: str
    buyer_id: str
    seller_id: Optional[str] = None
    amount: float
    price_btc: float
    total_btc: float
    platform_fee_btc: float
    issuer_royalty_btc: float
    executed_at: str


class Commitment(BaseModel):
    commitment_id: str
    user_id: str
    token_id: str
    title: str
    description: Optional[str] = None
    deadline: str
    status: str
    evidence_url: Optional[str] = None
    verified_at: Optional[str] = None
    created_at: str


class RegisterRequest(BaseModel):
    email: str
    password: str
    username: str


class LoginRequest(BaseModel):
    email: str
    password: str


class AuthResponse(BaseModel):
    token: str
    user: User


class CreateTokenRequest(BaseModel):
    symbol: str
    name: str
    description: Optional[str] = None
    base_price_btc: float
    price_increment: float


class BuyOrderRequest(BaseModel):
    token_id: str
    amount: float
    max_price_btc: Optional[float] = None


class SellOrderRequest(BaseModel):
    token_id: str
    amount: float
    min_price_btc: Optional[float] = None
"#
        .to_string()
    }

    fn generate_exceptions() -> String {
        r#"""
Exception classes for Kaccy SDK
"""

from typing import Optional, Dict, Any


class ApiError(Exception):
    """API request error"""

    def __init__(self, message: str, status_code: int = 0, details: Optional[Dict[str, Any]] = None):
        super().__init__(message)
        self.message = message
        self.status_code = status_code
        self.details = details

    def __str__(self):
        return f"ApiError({self.status_code}): {self.message}"
"#.to_string()
    }

    fn generate_init() -> String {
        r#"""
Kaccy Protocol Python SDK
"""

from .client import KaccyClient
from .models import (
    User, Token, Order, Trade, Commitment,
    RegisterRequest, LoginRequest, AuthResponse
)
from .exceptions import ApiError

__version__ = "1.0.0"
__all__ = [
    "KaccyClient",
    "User",
    "Token",
    "Order",
    "Trade",
    "Commitment",
    "RegisterRequest",
    "LoginRequest",
    "AuthResponse",
    "ApiError",
]
"#
        .to_string()
    }

    fn generate_setup(config: &SdkConfig) -> String {
        format!(
            r#"""
Setup configuration for Kaccy Python SDK
"""

from setuptools import setup, find_packages

setup(
    name='{}',
    version='{}',
    description='Python SDK for Kaccy Protocol API',
    long_description=open('README.md').read(),
    long_description_content_type='text/markdown',
    author='COOLJAPAN OU (Team KitaSan)',
    author_email='support@kaccy.io',
    url='https://github.com/kaccy/kaccy-python',
    packages=find_packages(),
    install_requires=[
        'requests>=2.31.0',
        'pydantic>=2.0.0',
    ],
    python_requires='>=3.8',
    classifiers=[
        'Development Status :: 4 - Beta',
        'Intended Audience :: Developers',
        'License :: OSI Approved :: MIT License',
        'Programming Language :: Python :: 3',
        'Programming Language :: Python :: 3.8',
        'Programming Language :: Python :: 3.9',
        'Programming Language :: Python :: 3.10',
        'Programming Language :: Python :: 3.11',
    ],
    keywords='kaccy bitcoin tokens api sdk',
)
"#,
            config.package_name, config.version
        )
    }

    fn generate_readme(config: &SdkConfig) -> String {
        format!(
            r#"# {} - Kaccy Protocol Python SDK

Official Python SDK for the Kaccy Protocol API.

## Installation

```bash
pip install {}
```

## Quick Start

```python
from kaccy import KaccyClient

# Initialize client
client = KaccyClient(base_url='{}')

# Register a new user
auth = client.register(
    email='user@example.com',
    password='SecurePass123!',
    username='alice'
)

# Client is automatically authenticated

# Create a token
token = client.create_token(
    symbol='ALICE',
    name='Alice Token',
    description="Support Alice's creative work",
    base_price_btc=0.000001,
    price_increment=0.0000001
)

# Buy tokens
order = client.buy_tokens(
    token_id=token.token_id,
    amount=100,
    max_price_btc=0.001
)

# Get portfolio
portfolio = client.get_portfolio()
```

## API Reference

### Authentication

- `register(email, password, username)` - Register a new user
- `login(email, password)` - Login with credentials
- `set_token(token)` - Set authentication token

### Users

- `get_profile()` - Get current user profile
- `update_profile(**updates)` - Update user profile

### Tokens

- `list_tokens(page, per_page)` - List all tokens
- `get_token(token_id)` - Get token details
- `create_token(...)` - Create a new token

### Orders

- `buy_tokens(token_id, amount, max_price_btc)` - Place buy order
- `sell_tokens(token_id, amount, min_price_btc)` - Place sell order
- `get_orders()` - Get user's orders

### Portfolio

- `get_portfolio()` - Get user's portfolio

### Commitments

- `create_commitment(token_id, title, deadline, description)` - Create commitment

## Error Handling

```python
from kaccy import ApiError

try:
    client.create_token(...)
except ApiError as e:
    print(f"API Error: {{e.message}}")
    print(f"Status: {{e.status_code}}")
    print(f"Details: {{e.details}}")
```

## License

MIT
"#,
            config.package_name, config.package_name, config.base_url
        )
    }
}

/// Rust SDK generator
pub struct RustGenerator;

impl RustGenerator {
    /// Generate Rust SDK
    pub fn generate(config: &SdkConfig) -> GeneratedSdk {
        let mut files = HashMap::new();

        // Generate lib.rs
        files.insert("lib.rs".to_string(), Self::generate_lib());

        // Generate client.rs
        files.insert("client.rs".to_string(), Self::generate_client(config));

        // Generate types.rs
        files.insert("types.rs".to_string(), Self::generate_types());

        // Generate error.rs
        files.insert("error.rs".to_string(), Self::generate_error());

        // Generate Cargo.toml
        files.insert("Cargo.toml".to_string(), Self::generate_cargo_toml(config));

        // Generate README
        files.insert("README.md".to_string(), Self::generate_readme(config));

        GeneratedSdk {
            language: SdkLanguage::Rust,
            files,
            metadata: SdkMetadata {
                name: config.package_name.clone(),
                version: config.version.clone(),
                description: "Rust SDK for Kaccy Protocol API".to_string(),
                dependencies: vec![
                    "reqwest".to_string(),
                    "serde".to_string(),
                    "serde_json".to_string(),
                    "tokio".to_string(),
                ],
            },
        }
    }

    fn generate_lib() -> String {
        r#"//! Kaccy Protocol Rust SDK
//!
//! Official Rust client library for the Kaccy Protocol API

pub mod client;
pub mod error;
pub mod types;

pub use client::KaccyClient;
pub use error::{Error, Result};
pub use types::*;
"#
        .to_string()
    }

    fn generate_client(config: &SdkConfig) -> String {
        let base_url = &config.base_url;
        format!(
            r#"//! Kaccy API Client

use crate::error::{{Error, Result}};
use crate::types::*;
use reqwest::Client;
use serde::{{Deserialize, Serialize}};

/// Kaccy API client
pub struct KaccyClient {{
    client: Client,
    base_url: String,
    token: Option<String>,
}}

impl KaccyClient {{
    /// Create a new client
    pub fn new(base_url: impl Into<String>) -> Self {{
        Self {{
            client: Client::new(),
            base_url: base_url.into(),
            token: None,
        }}
    }}

    /// Create a new client with default base URL
    pub fn default_client() -> Self {{
        Self::new("{base_url}")
    }}

    /// Set authentication token
    pub fn set_token(&mut self, token: String) {{
        self.token = Some(token);
    }}

    /// Register a new user
    pub async fn register(
        &mut self,
        email: impl Into<String>,
        password: impl Into<String>,
        username: impl Into<String>,
    ) -> Result<AuthResponse> {{
        let request = RegisterRequest {{
            email: email.into(),
            password: password.into(),
            username: username.into(),
        }};

        let response: AuthResponse = self
            .client
            .post(format!("{{}}/api/auth/register", self.base_url))
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        self.token = Some(response.token.clone());
        Ok(response)
    }}

    /// Login with email and password
    pub async fn login(
        &mut self,
        email: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<AuthResponse> {{
        let request = LoginRequest {{
            email: email.into(),
            password: password.into(),
        }};

        let response: AuthResponse = self
            .client
            .post(format!("{{}}/api/auth/login", self.base_url))
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        self.token = Some(response.token.clone());
        Ok(response)
    }}

    /// Get current user profile
    pub async fn get_profile(&self) -> Result<User> {{
        self.authenticated_request("GET", "/api/users/me", None::<&()>)
            .await
    }}

    /// Update user profile
    pub async fn update_profile(&self, updates: UpdateProfileRequest) -> Result<User> {{
        self.authenticated_request("PUT", "/api/users/me", Some(&updates))
            .await
    }}

    /// List tokens
    pub async fn list_tokens(&self, page: i32, per_page: i32) -> Result<TokenListResponse> {{
        let url = format!(
            "{{}}/api/tokens?page={{}}&per_page={{}}",
            self.base_url, page, per_page
        );

        self.client.get(&url).send().await?.json().await.map_err(Into::into)
    }}

    /// Get token details
    pub async fn get_token(&self, token_id: &str) -> Result<Token> {{
        let url = format!("{{}}/api/tokens/{{}}", self.base_url, token_id);
        self.client.get(&url).send().await?.json().await.map_err(Into::into)
    }}

    /// Create a new token
    pub async fn create_token(&self, request: CreateTokenRequest) -> Result<Token> {{
        self.authenticated_request("POST", "/api/tokens", Some(&request))
            .await
    }}

    /// Place a buy order
    pub async fn buy_tokens(&self, request: BuyOrderRequest) -> Result<Order> {{
        self.authenticated_request("POST", "/api/orders/buy", Some(&request))
            .await
    }}

    /// Place a sell order
    pub async fn sell_tokens(&self, request: SellOrderRequest) -> Result<Order> {{
        self.authenticated_request("POST", "/api/orders/sell", Some(&request))
            .await
    }}

    /// Get user's portfolio
    pub async fn get_portfolio(&self) -> Result<serde_json::Value> {{
        self.authenticated_request("GET", "/api/portfolio", None::<&()>)
            .await
    }}

    /// Make an authenticated request
    async fn authenticated_request<T, B>(
        &self,
        method: &str,
        path: &str,
        body: Option<&B>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
        B: Serialize,
    {{
        let token = self.token.as_ref().ok_or(Error::NotAuthenticated)?;
        let url = format!("{{}}{{}}", self.base_url, path);

        let mut request = match method {{
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            _ => return Err(Error::InvalidMethod),
        }};

        request = request.header("Authorization", format!("Bearer {{}}", token));

        if let Some(body) = body {{
            request = request.json(body);
        }}

        let response = request.send().await?;

        if !response.status().is_success() {{
            return Err(Error::ApiError(response.status().as_u16()));
        }}

        response.json().await.map_err(Into::into)
    }}
}}
"#,
            base_url = base_url
        )
    }

    fn generate_types() -> String {
        r#"//! Type definitions

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub reputation_score: f64,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub token_id: String,
    pub issuer_id: String,
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub current_supply: f64,
    pub current_price_btc: f64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub user_id: String,
    pub token_id: String,
    pub order_type: String,
    pub amount: f64,
    pub price_btc: f64,
    pub status: String,
    pub btc_address: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTokenRequest {
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub base_price_btc: f64,
    pub price_increment: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyOrderRequest {
    pub token_id: String,
    pub amount: f64,
    pub max_price_btc: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellOrderRequest {
    pub token_id: String,
    pub amount: f64,
    pub min_price_btc: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenListResponse {
    pub tokens: Vec<Token>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}
"#
        .to_string()
    }

    fn generate_error() -> String {
        r#"//! Error types

use thiserror::Error;

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// SDK error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("Not authenticated")]
    NotAuthenticated,

    #[error("Invalid HTTP method")]
    InvalidMethod,

    #[error("API error: {0}")]
    ApiError(u16),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
"#
        .to_string()
    }

    fn generate_cargo_toml(config: &SdkConfig) -> String {
        format!(
            r#"[package]
name = "{}"
version = "{}"
edition = "2021"
description = "Rust SDK for Kaccy Protocol API"
license = "MIT"
repository = "https://github.com/kaccy/kaccy-rust"
keywords = ["kaccy", "bitcoin", "tokens", "api", "sdk"]

[dependencies]
reqwest = {{ version = "0.12", features = ["json"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
tokio = {{ version = "1.0", features = ["full"] }}
thiserror = "2.0"

[dev-dependencies]
tokio-test = "0.4"
"#,
            config.package_name, config.version
        )
    }

    fn generate_readme(config: &SdkConfig) -> String {
        format!(
            r#"# {} - Kaccy Protocol Rust SDK

Official Rust client library for the Kaccy Protocol API.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
{} = "{}"
```

## Quick Start

```rust
use kaccy::KaccyClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    // Initialize client
    let mut client = KaccyClient::default();

    // Register a new user
    let auth = client.register(
        "user@example.com",
        "SecurePass123!",
        "alice"
    ).await?;

    println!("Authenticated as: {{}}", auth.user.username);

    // Create a token
    let token = client.create_token(CreateTokenRequest {{
        symbol: "ALICE".to_string(),
        name: "Alice Token".to_string(),
        description: Some("Support Alice's creative work".to_string()),
        base_price_btc: 0.000001,
        price_increment: 0.0000001,
    }}).await?;

    // Buy tokens
    let order = client.buy_tokens(BuyOrderRequest {{
        token_id: token.token_id.clone(),
        amount: 100.0,
        max_price_btc: Some(0.001),
    }}).await?;

    // Get portfolio
    let portfolio = client.get_portfolio().await?;

    Ok(())
}}
```

## API Reference

### Authentication

- `register()` - Register a new user
- `login()` - Login with email/password
- `set_token()` - Set authentication token

### Users

- `get_profile()` - Get current user profile
- `update_profile()` - Update user profile

### Tokens

- `list_tokens()` - List all tokens
- `get_token()` - Get token details
- `create_token()` - Create a new token

### Orders

- `buy_tokens()` - Place buy order
- `sell_tokens()` - Place sell order

### Portfolio

- `get_portfolio()` - Get user's portfolio

## Error Handling

```rust
match client.create_token(request).await {{
    Ok(token) => println!("Created: {{}}", token.symbol),
    Err(e) => eprintln!("Error: {{}}", e),
}}
```

## License

MIT
"#,
            config.package_name, config.package_name, config.version
        )
    }
}

/// Go SDK generator
pub struct GoGenerator;

impl GoGenerator {
    /// Generate Go SDK
    pub fn generate(config: &SdkConfig) -> GeneratedSdk {
        let mut files = HashMap::new();

        // Generate client.go
        files.insert("client.go".to_string(), Self::generate_client(config));

        // Generate types.go
        files.insert("types.go".to_string(), Self::generate_types());

        // Generate errors.go
        files.insert("errors.go".to_string(), Self::generate_errors());

        // Generate go.mod
        files.insert("go.mod".to_string(), Self::generate_go_mod(config));

        // Generate README
        files.insert("README.md".to_string(), Self::generate_readme(config));

        GeneratedSdk {
            language: SdkLanguage::Go,
            files,
            metadata: SdkMetadata {
                name: config.package_name.clone(),
                version: config.version.clone(),
                description: "Go SDK for Kaccy Protocol API".to_string(),
                dependencies: vec![
                    "encoding/json".to_string(),
                    "net/http".to_string(),
                    "bytes".to_string(),
                ],
            },
        }
    }

    fn generate_client(config: &SdkConfig) -> String {
        format!(
            r#"// Package kaccy provides a Go client for the Kaccy Protocol API
package kaccy

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Client is the Kaccy API client
type Client struct {{
	baseURL    string
	token      string
	httpClient *http.Client
}}

// NewClient creates a new Kaccy API client
func NewClient(baseURL string) *Client {{
	if baseURL == "" {{
		baseURL = "{}"
	}}
	return &Client{{
		baseURL: baseURL,
		httpClient: &http.Client{{
			Timeout: 30 * time.Second,
		}},
	}}
}}

// SetToken sets the authentication token
func (c *Client) SetToken(token string) {{
	c.token = token
}}

// Register creates a new user account
func (c *Client) Register(req *RegisterRequest) (*AuthResponse, error) {{
	var resp AuthResponse
	if err := c.request("POST", "/api/auth/register", req, &resp); err != nil {{
		return nil, err
	}}
	c.token = resp.Token
	return &resp, nil
}}

// Login authenticates a user
func (c *Client) Login(req *LoginRequest) (*AuthResponse, error) {{
	var resp AuthResponse
	if err := c.request("POST", "/api/auth/login", req, &resp); err != nil {{
		return nil, err
	}}
	c.token = resp.Token
	return &resp, nil
}}

// GetProfile retrieves the current user's profile
func (c *Client) GetProfile() (*User, error) {{
	var user User
	if err := c.authenticatedRequest("GET", "/api/users/me", nil, &user); err != nil {{
		return nil, err
	}}
	return &user, nil
}}

// UpdateProfile updates the current user's profile
func (c *Client) UpdateProfile(req *UpdateProfileRequest) (*User, error) {{
	var user User
	if err := c.authenticatedRequest("PUT", "/api/users/me", req, &user); err != nil {{
		return nil, err
	}}
	return &user, nil
}}

// ListTokens retrieves a list of tokens
func (c *Client) ListTokens(page, perPage int) (*TokenListResponse, error) {{
	path := fmt.Sprintf("/api/tokens?page=%d&per_page=%d", page, perPage)
	var resp TokenListResponse
	if err := c.request("GET", path, nil, &resp); err != nil {{
		return nil, err
	}}
	return &resp, nil
}}

// GetToken retrieves a specific token by ID
func (c *Client) GetToken(tokenID string) (*Token, error) {{
	path := fmt.Sprintf("/api/tokens/%s", tokenID)
	var token Token
	if err := c.request("GET", path, nil, &token); err != nil {{
		return nil, err
	}}
	return &token, nil
}}

// CreateToken creates a new token
func (c *Client) CreateToken(req *CreateTokenRequest) (*Token, error) {{
	var token Token
	if err := c.authenticatedRequest("POST", "/api/tokens", req, &token); err != nil {{
		return nil, err
	}}
	return &token, nil
}}

// BuyTokens places a buy order
func (c *Client) BuyTokens(req *BuyOrderRequest) (*Order, error) {{
	var order Order
	if err := c.authenticatedRequest("POST", "/api/orders/buy", req, &order); err != nil {{
		return nil, err
	}}
	return &order, nil
}}

// SellTokens places a sell order
func (c *Client) SellTokens(req *SellOrderRequest) (*Order, error) {{
	var order Order
	if err := c.authenticatedRequest("POST", "/api/orders/sell", req, &order); err != nil {{
		return nil, err
	}}
	return &order, nil
}}

// GetOrders retrieves the user's orders
func (c *Client) GetOrders() ([]*Order, error) {{
	var orders []*Order
	if err := c.authenticatedRequest("GET", "/api/orders", nil, &orders); err != nil {{
		return nil, err
	}}
	return orders, nil
}}

// GetPortfolio retrieves the user's portfolio
func (c *Client) GetPortfolio() (map[string]interface{{}}, error) {{
	var portfolio map[string]interface{{}}
	if err := c.authenticatedRequest("GET", "/api/portfolio", nil, &portfolio); err != nil {{
		return nil, err
	}}
	return portfolio, nil
}}

// CreateCommitment creates a new commitment
func (c *Client) CreateCommitment(req *CreateCommitmentRequest) (*Commitment, error) {{
	var commitment Commitment
	if err := c.authenticatedRequest("POST", "/api/commitments", req, &commitment); err != nil {{
		return nil, err
	}}
	return &commitment, nil
}}

// request makes an HTTP request
func (c *Client) request(method, path string, body, result interface{{}}) error {{
	url := c.baseURL + path

	var reqBody io.Reader
	if body != nil {{
		jsonData, err := json.Marshal(body)
		if err != nil {{
			return fmt.Errorf("failed to marshal request: %w", err)
		}}
		reqBody = bytes.NewBuffer(jsonData)
	}}

	req, err := http.NewRequest(method, url, reqBody)
	if err != nil {{
		return fmt.Errorf("failed to create request: %w", err)
	}}

	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {{
		return fmt.Errorf("request failed: %w", err)
	}}
	defer resp.Body.Close()

	if resp.StatusCode >= 400 {{
		var apiErr APIError
		if err := json.NewDecoder(resp.Body).Decode(&apiErr); err != nil {{
			return fmt.Errorf("API error (status %d)", resp.StatusCode)
		}}
		apiErr.StatusCode = resp.StatusCode
		return &apiErr
	}}

	if result != nil {{
		if err := json.NewDecoder(resp.Body).Decode(result); err != nil {{
			return fmt.Errorf("failed to decode response: %w", err)
		}}
	}}

	return nil
}}

// authenticatedRequest makes an authenticated HTTP request
func (c *Client) authenticatedRequest(method, path string, body, result interface{{}}) error {{
	if c.token == "" {{
		return ErrNotAuthenticated
	}}

	url := c.baseURL + path

	var reqBody io.Reader
	if body != nil {{
		jsonData, err := json.Marshal(body)
		if err != nil {{
			return fmt.Errorf("failed to marshal request: %w", err)
		}}
		reqBody = bytes.NewBuffer(jsonData)
	}}

	req, err := http.NewRequest(method, url, reqBody)
	if err != nil {{
		return fmt.Errorf("failed to create request: %w", err)
	}}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+c.token)

	resp, err := c.httpClient.Do(req)
	if err != nil {{
		return fmt.Errorf("request failed: %w", err)
	}}
	defer resp.Body.Close()

	if resp.StatusCode >= 400 {{
		var apiErr APIError
		if err := json.NewDecoder(resp.Body).Decode(&apiErr); err != nil {{
			return fmt.Errorf("API error (status %d)", resp.StatusCode)
		}}
		apiErr.StatusCode = resp.StatusCode
		return &apiErr
	}}

	if result != nil {{
		if err := json.NewDecoder(resp.Body).Decode(result); err != nil {{
			return fmt.Errorf("failed to decode response: %w", err)
		}}
	}}

	return nil
}}
"#,
            config.base_url
        )
    }

    fn generate_types() -> String {
        r#"package kaccy

// User represents a user account
type User struct {
	UserID          string  `json:"user_id"`
	Username        string  `json:"username"`
	Email           *string `json:"email,omitempty"`
	DisplayName     *string `json:"display_name,omitempty"`
	Bio             *string `json:"bio,omitempty"`
	AvatarURL       *string `json:"avatar_url,omitempty"`
	ReputationScore float64 `json:"reputation_score"`
	Role            string  `json:"role"`
	CreatedAt       string  `json:"created_at"`
}

// Token represents a token
type Token struct {
	TokenID         string  `json:"token_id"`
	IssuerID        string  `json:"issuer_id"`
	Symbol          string  `json:"symbol"`
	Name            string  `json:"name"`
	Description     *string `json:"description,omitempty"`
	CurrentSupply   float64 `json:"current_supply"`
	CurrentPriceBTC float64 `json:"current_price_btc"`
	Status          string  `json:"status"`
	CreatedAt       string  `json:"created_at"`
}

// Order represents an order
type Order struct {
	OrderID    string   `json:"order_id"`
	UserID     string   `json:"user_id"`
	TokenID    string   `json:"token_id"`
	OrderType  string   `json:"order_type"`
	Amount     float64  `json:"amount"`
	PriceBTC   float64  `json:"price_btc"`
	Status     string   `json:"status"`
	BTCAddress *string  `json:"btc_address,omitempty"`
	CreatedAt  string   `json:"created_at"`
}

// Trade represents a trade
type Trade struct {
	TradeID          string  `json:"trade_id"`
	TokenID          string  `json:"token_id"`
	BuyerID          string  `json:"buyer_id"`
	SellerID         *string `json:"seller_id,omitempty"`
	Amount           float64 `json:"amount"`
	PriceBTC         float64 `json:"price_btc"`
	TotalBTC         float64 `json:"total_btc"`
	PlatformFeeBTC   float64 `json:"platform_fee_btc"`
	IssuerRoyaltyBTC float64 `json:"issuer_royalty_btc"`
	ExecutedAt       string  `json:"executed_at"`
}

// Commitment represents a commitment
type Commitment struct {
	CommitmentID string  `json:"commitment_id"`
	UserID       string  `json:"user_id"`
	TokenID      string  `json:"token_id"`
	Title        string  `json:"title"`
	Description  *string `json:"description,omitempty"`
	Deadline     string  `json:"deadline"`
	Status       string  `json:"status"`
	EvidenceURL  *string `json:"evidence_url,omitempty"`
	VerifiedAt   *string `json:"verified_at,omitempty"`
	CreatedAt    string  `json:"created_at"`
}

// RegisterRequest is the request for user registration
type RegisterRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
	Username string `json:"username"`
}

// LoginRequest is the request for user login
type LoginRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

// AuthResponse is the response after authentication
type AuthResponse struct {
	Token string `json:"token"`
	User  User   `json:"user"`
}

// CreateTokenRequest is the request for creating a token
type CreateTokenRequest struct {
	Symbol         string  `json:"symbol"`
	Name           string  `json:"name"`
	Description    *string `json:"description,omitempty"`
	BasePriceBTC   float64 `json:"base_price_btc"`
	PriceIncrement float64 `json:"price_increment"`
}

// BuyOrderRequest is the request for buying tokens
type BuyOrderRequest struct {
	TokenID     string   `json:"token_id"`
	Amount      float64  `json:"amount"`
	MaxPriceBTC *float64 `json:"max_price_btc,omitempty"`
}

// SellOrderRequest is the request for selling tokens
type SellOrderRequest struct {
	TokenID     string   `json:"token_id"`
	Amount      float64  `json:"amount"`
	MinPriceBTC *float64 `json:"min_price_btc,omitempty"`
}

// UpdateProfileRequest is the request for updating user profile
type UpdateProfileRequest struct {
	DisplayName *string `json:"display_name,omitempty"`
	Bio         *string `json:"bio,omitempty"`
	AvatarURL   *string `json:"avatar_url,omitempty"`
}

// TokenListResponse is the response for listing tokens
type TokenListResponse struct {
	Tokens  []Token `json:"tokens"`
	Total   int64   `json:"total"`
	Page    int     `json:"page"`
	PerPage int     `json:"per_page"`
}

// CreateCommitmentRequest is the request for creating a commitment
type CreateCommitmentRequest struct {
	TokenID     string  `json:"token_id"`
	Title       string  `json:"title"`
	Description *string `json:"description,omitempty"`
	Deadline    string  `json:"deadline"`
}
"#
        .to_string()
    }

    fn generate_errors() -> String {
        r#"package kaccy

import (
	"errors"
	"fmt"
)

var (
	// ErrNotAuthenticated is returned when authentication is required but not provided
	ErrNotAuthenticated = errors.New("not authenticated")
)

// APIError represents an API error response
type APIError struct {
	Message    string `json:"message"`
	StatusCode int    `json:"-"`
	Details    interface{} `json:"details,omitempty"`
}

// Error implements the error interface
func (e *APIError) Error() string {
	if e.Details != nil {
		return fmt.Sprintf("API error (%d): %s - %v", e.StatusCode, e.Message, e.Details)
	}
	return fmt.Sprintf("API error (%d): %s", e.StatusCode, e.Message)
}
"#
        .to_string()
    }

    fn generate_go_mod(config: &SdkConfig) -> String {
        format!(
            r#"module {}

go 1.21

// No external dependencies required - uses only Go standard library
"#,
            config.package_name
        )
    }

    fn generate_readme(config: &SdkConfig) -> String {
        format!(
            r#"# {} - Kaccy Protocol Go SDK

Official Go client library for the Kaccy Protocol API.

## Installation

```bash
go get {}
```

## Quick Start

```go
package main

import (
	"fmt"
	"log"

	kaccy "{}"
)

func main() {{
	// Initialize client
	client := kaccy.NewClient("{}")

	// Register a new user
	auth, err := client.Register(&kaccy.RegisterRequest{{
		Email:    "user@example.com",
		Password: "SecurePass123!",
		Username: "alice",
	}})
	if err != nil {{
		log.Fatal(err)
	}}

	fmt.Printf("Authenticated as: %s\n", auth.User.Username)

	// Create a token
	token, err := client.CreateToken(&kaccy.CreateTokenRequest{{
		Symbol:         "ALICE",
		Name:           "Alice Token",
		Description:    ptr("Support Alice's creative work"),
		BasePriceBTC:   0.000001,
		PriceIncrement: 0.0000001,
	}})
	if err != nil {{
		log.Fatal(err)
	}}

	fmt.Printf("Created token: %s\n", token.Symbol)

	// Buy tokens
	order, err := client.BuyTokens(&kaccy.BuyOrderRequest{{
		TokenID:     token.TokenID,
		Amount:      100,
		MaxPriceBTC: ptr(0.001),
	}})
	if err != nil {{
		log.Fatal(err)
	}}

	fmt.Printf("Order placed: %s\n", order.OrderID)

	// Get portfolio
	portfolio, err := client.GetPortfolio()
	if err != nil {{
		log.Fatal(err)
	}}

	fmt.Printf("Portfolio: %+v\n", portfolio)
}}

// Helper function to create string pointers
func ptr[T any](v T) *T {{
	return &v
}}
```

## API Reference

### Authentication

- `Register(req *RegisterRequest) (*AuthResponse, error)` - Register a new user
- `Login(req *LoginRequest) (*AuthResponse, error)` - Login with email/password
- `SetToken(token string)` - Set authentication token

### Users

- `GetProfile() (*User, error)` - Get current user profile
- `UpdateProfile(req *UpdateProfileRequest) (*User, error)` - Update user profile

### Tokens

- `ListTokens(page, perPage int) (*TokenListResponse, error)` - List all tokens
- `GetToken(tokenID string) (*Token, error)` - Get token details
- `CreateToken(req *CreateTokenRequest) (*Token, error)` - Create a new token

### Orders

- `BuyTokens(req *BuyOrderRequest) (*Order, error)` - Place buy order
- `SellTokens(req *SellOrderRequest) (*Order, error)` - Place sell order
- `GetOrders() ([]*Order, error)` - Get user's orders

### Portfolio

- `GetPortfolio() (map[string]interface{{}}, error)` - Get user's portfolio

### Commitments

- `CreateCommitment(req *CreateCommitmentRequest) (*Commitment, error)` - Create a new commitment

## Error Handling

```go
token, err := client.CreateToken(req)
if err != nil {{
	if apiErr, ok := err.(*kaccy.APIError); ok {{
		fmt.Printf("API Error: %s (status %d)\n", apiErr.Message, apiErr.StatusCode)
	}} else {{
		fmt.Printf("Error: %v\n", err)
	}}
	return
}}
```

## License

MIT
"#,
            config.package_name, config.package_name, config.package_name, config.base_url
        )
    }
}

/// Generate SDK for a given language
pub fn generate_sdk(config: SdkConfig) -> GeneratedSdk {
    match config.language {
        SdkLanguage::TypeScript => TypeScriptGenerator::generate(&config),
        SdkLanguage::Python => PythonGenerator::generate(&config),
        SdkLanguage::Rust => RustGenerator::generate(&config),
        SdkLanguage::Go => GoGenerator::generate(&config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(language: SdkLanguage) -> SdkConfig {
        SdkConfig {
            language,
            package_name: "kaccy-sdk".to_string(),
            version: "1.0.0".to_string(),
            base_url: "http://localhost:3000".to_string(),
            options: HashMap::new(),
        }
    }

    #[test]
    fn test_typescript_generation() {
        let config = test_config(SdkLanguage::TypeScript);
        let sdk = TypeScriptGenerator::generate(&config);

        assert_eq!(sdk.language, SdkLanguage::TypeScript);
        assert!(sdk.files.contains_key("client.ts"));
        assert!(sdk.files.contains_key("types.ts"));
        assert!(sdk.files.contains_key("package.json"));
        assert!(sdk.files.contains_key("README.md"));
    }

    #[test]
    fn test_python_generation() {
        let config = test_config(SdkLanguage::Python);
        let sdk = PythonGenerator::generate(&config);

        assert_eq!(sdk.language, SdkLanguage::Python);
        assert!(sdk.files.contains_key("client.py"));
        assert!(sdk.files.contains_key("models.py"));
        assert!(sdk.files.contains_key("setup.py"));
        assert!(sdk.files.contains_key("README.md"));
    }

    #[test]
    fn test_rust_generation() {
        let config = test_config(SdkLanguage::Rust);
        let sdk = RustGenerator::generate(&config);

        assert_eq!(sdk.language, SdkLanguage::Rust);
        assert!(sdk.files.contains_key("lib.rs"));
        assert!(sdk.files.contains_key("client.rs"));
        assert!(sdk.files.contains_key("types.rs"));
        assert!(sdk.files.contains_key("Cargo.toml"));
        assert!(sdk.files.contains_key("README.md"));
    }

    #[test]
    fn test_go_generation() {
        let config = test_config(SdkLanguage::Go);
        let sdk = GoGenerator::generate(&config);

        assert_eq!(sdk.language, SdkLanguage::Go);
        assert!(sdk.files.contains_key("client.go"));
        assert!(sdk.files.contains_key("types.go"));
        assert!(sdk.files.contains_key("errors.go"));
        assert!(sdk.files.contains_key("go.mod"));
        assert!(sdk.files.contains_key("README.md"));
    }

    #[test]
    fn test_sdk_language_display() {
        assert_eq!(SdkLanguage::TypeScript.to_string(), "typescript");
        assert_eq!(SdkLanguage::Python.to_string(), "python");
        assert_eq!(SdkLanguage::Rust.to_string(), "rust");
        assert_eq!(SdkLanguage::Go.to_string(), "go");
    }
}
