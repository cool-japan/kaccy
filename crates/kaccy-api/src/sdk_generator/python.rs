//! Python SDK generator

use std::collections::HashMap;

use super::types::{GeneratedSdk, SdkConfig, SdkLanguage, SdkMetadata};

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
