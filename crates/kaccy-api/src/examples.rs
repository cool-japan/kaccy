//! Interactive API examples and code snippets in multiple languages
//!
//! This module provides ready-to-use code examples for common API operations
//! in various programming languages.

use serde::{Deserialize, Serialize};

/// Code example in a specific language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExample {
    /// Programming language
    pub language: String,
    /// Code snippet
    pub code: String,
    /// Description of what the code does
    pub description: String,
}

/// Collection of examples for an API operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiExample {
    /// Operation name
    pub operation: String,
    /// Endpoint path
    pub endpoint: String,
    /// HTTP method
    pub method: String,
    /// Description
    pub description: String,
    /// Code examples in different languages
    pub examples: Vec<CodeExample>,
}

/// Get all API examples
pub fn get_all_examples() -> Vec<ApiExample> {
    vec![
        register_example(),
        login_example(),
        create_token_example(),
        buy_token_example(),
        sell_token_example(),
        get_portfolio_example(),
        create_commitment_example(),
        websocket_example(),
    ]
}

/// User registration examples
fn register_example() -> ApiExample {
    ApiExample {
        operation: "register".to_string(),
        endpoint: "/api/auth/register".to_string(),
        method: "POST".to_string(),
        description: "Register a new user account".to_string(),
        examples: vec![
            CodeExample {
                language: "curl".to_string(),
                description: "Register using cURL".to_string(),
                code: r#"curl -X POST http://localhost:3000/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "password": "SecurePass123!",
    "username": "creator_alice"
  }'"#
                .to_string(),
            },
            CodeExample {
                language: "javascript".to_string(),
                description: "Register using JavaScript (fetch)".to_string(),
                code: r#"const response = await fetch('http://localhost:3000/api/auth/register', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    email: 'user@example.com',
    password: 'SecurePass123!',
    username: 'creator_alice'
  })
});

const data = await response.json();
console.log('Token:', data.token);
console.log('User:', data.user);"#
                    .to_string(),
            },
            CodeExample {
                language: "python".to_string(),
                description: "Register using Python (requests)".to_string(),
                code: r#"import requests

response = requests.post(
    'http://localhost:3000/api/auth/register',
    json={
        'email': 'user@example.com',
        'password': 'SecurePass123!',
        'username': 'creator_alice'
    }
)

data = response.json()
print(f"Token: {data['token']}")
print(f"User: {data['user']}")"#
                    .to_string(),
            },
            CodeExample {
                language: "rust".to_string(),
                description: "Register using Rust (reqwest)".to_string(),
                code: r#"use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct RegisterRequest {
    email: String,
    password: String,
    username: String,
}

#[derive(Deserialize)]
struct AuthResponse {
    token: String,
    user: serde_json::Value,
}

let client = reqwest::Client::new();
let response = client
    .post("http://localhost:3000/api/auth/register")
    .json(&RegisterRequest {
        email: "user@example.com".to_string(),
        password: "SecurePass123!".to_string(),
        username: "creator_alice".to_string(),
    })
    .send()
    .await?
    .json::<AuthResponse>()
    .await?;

println!("Token: {}", response.token);"#
                    .to_string(),
            },
        ],
    }
}

/// User login examples
fn login_example() -> ApiExample {
    ApiExample {
        operation: "login".to_string(),
        endpoint: "/api/auth/login".to_string(),
        method: "POST".to_string(),
        description: "Authenticate and get JWT token".to_string(),
        examples: vec![
            CodeExample {
                language: "curl".to_string(),
                description: "Login using cURL".to_string(),
                code: r#"curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "password": "SecurePass123!"
  }'"#
                .to_string(),
            },
            CodeExample {
                language: "javascript".to_string(),
                description: "Login using JavaScript".to_string(),
                code: r#"const response = await fetch('http://localhost:3000/api/auth/login', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    email: 'user@example.com',
    password: 'SecurePass123!'
  })
});

const { token, user } = await response.json();
// Store token for future requests
localStorage.setItem('authToken', token);"#
                    .to_string(),
            },
            CodeExample {
                language: "python".to_string(),
                description: "Login using Python".to_string(),
                code: r#"import requests

response = requests.post(
    'http://localhost:3000/api/auth/login',
    json={
        'email': 'user@example.com',
        'password': 'SecurePass123!'
    }
)

data = response.json()
token = data['token']
# Use token in future requests"#
                    .to_string(),
            },
        ],
    }
}

/// Create token examples
fn create_token_example() -> ApiExample {
    ApiExample {
        operation: "create_token".to_string(),
        endpoint: "/api/tokens".to_string(),
        method: "POST".to_string(),
        description: "Create a new creator token".to_string(),
        examples: vec![
            CodeExample {
                language: "curl".to_string(),
                description: "Create token using cURL".to_string(),
                code: r#"curl -X POST http://localhost:3000/api/tokens \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -d '{
    "symbol": "ALICE",
    "name": "Alice Token",
    "description": "Support Alice'\''s creative work",
    "base_price_btc": 0.000001,
    "price_increment": 0.0000001
  }'"#
                .to_string(),
            },
            CodeExample {
                language: "javascript".to_string(),
                description: "Create token using JavaScript".to_string(),
                code: r#"const token = localStorage.getItem('authToken');

const response = await fetch('http://localhost:3000/api/tokens', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${token}`
  },
  body: JSON.stringify({
    symbol: 'ALICE',
    name: 'Alice Token',
    description: 'Support Alice\'s creative work',
    base_price_btc: 0.000001,
    price_increment: 0.0000001
  })
});

const tokenData = await response.json();
console.log('Created token:', tokenData);"#
                    .to_string(),
            },
            CodeExample {
                language: "python".to_string(),
                description: "Create token using Python".to_string(),
                code: r#"import requests

headers = {
    'Authorization': f'Bearer {token}'
}

response = requests.post(
    'http://localhost:3000/api/tokens',
    headers=headers,
    json={
        'symbol': 'ALICE',
        'name': 'Alice Token',
        'description': 'Support Alice\'s creative work',
        'base_price_btc': 0.000001,
        'price_increment': 0.0000001
    }
)

token_data = response.json()
print(f"Created token: {token_data}")"#
                    .to_string(),
            },
        ],
    }
}

/// Buy token examples
fn buy_token_example() -> ApiExample {
    ApiExample {
        operation: "buy_token".to_string(),
        endpoint: "/api/orders/buy".to_string(),
        method: "POST".to_string(),
        description: "Place a buy order for tokens".to_string(),
        examples: vec![
            CodeExample {
                language: "curl".to_string(),
                description: "Buy tokens using cURL".to_string(),
                code: r#"curl -X POST http://localhost:3000/api/orders/buy \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -d '{
    "token_id": "550e8400-e29b-41d4-a716-446655440001",
    "amount": 100.0,
    "max_price_btc": 0.001
  }'"#
                .to_string(),
            },
            CodeExample {
                language: "javascript".to_string(),
                description: "Buy tokens using JavaScript".to_string(),
                code: r#"const response = await fetch('http://localhost:3000/api/orders/buy', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${authToken}`
  },
  body: JSON.stringify({
    token_id: '550e8400-e29b-41d4-a716-446655440001',
    amount: 100.0,
    max_price_btc: 0.001
  })
});

const order = await response.json();
console.log('BTC Address:', order.btc_address);
console.log('Amount to pay:', order.price_btc);"#
                    .to_string(),
            },
            CodeExample {
                language: "python".to_string(),
                description: "Buy tokens using Python".to_string(),
                code: r#"response = requests.post(
    'http://localhost:3000/api/orders/buy',
    headers={'Authorization': f'Bearer {token}'},
    json={
        'token_id': '550e8400-e29b-41d4-a716-446655440001',
        'amount': 100.0,
        'max_price_btc': 0.001
    }
)

order = response.json()
print(f"Send {order['price_btc']} BTC to {order['btc_address']}")"#
                    .to_string(),
            },
        ],
    }
}

/// Sell token examples
fn sell_token_example() -> ApiExample {
    ApiExample {
        operation: "sell_token".to_string(),
        endpoint: "/api/orders/sell".to_string(),
        method: "POST".to_string(),
        description: "Place a sell order for tokens".to_string(),
        examples: vec![
            CodeExample {
                language: "curl".to_string(),
                description: "Sell tokens using cURL".to_string(),
                code: r#"curl -X POST http://localhost:3000/api/orders/sell \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -d '{
    "token_id": "550e8400-e29b-41d4-a716-446655440001",
    "amount": 50.0,
    "min_price_btc": 0.0005
  }'"#
                .to_string(),
            },
            CodeExample {
                language: "javascript".to_string(),
                description: "Sell tokens using JavaScript".to_string(),
                code: r#"const response = await fetch('http://localhost:3000/api/orders/sell', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${authToken}`
  },
  body: JSON.stringify({
    token_id: '550e8400-e29b-41d4-a716-446655440001',
    amount: 50.0,
    min_price_btc: 0.0005
  })
});

const order = await response.json();
console.log('Sell order created:', order);"#
                    .to_string(),
            },
        ],
    }
}

/// Get portfolio examples
fn get_portfolio_example() -> ApiExample {
    ApiExample {
        operation: "get_portfolio".to_string(),
        endpoint: "/api/portfolio".to_string(),
        method: "GET".to_string(),
        description: "Get user's token portfolio".to_string(),
        examples: vec![
            CodeExample {
                language: "curl".to_string(),
                description: "Get portfolio using cURL".to_string(),
                code: r#"curl -X GET http://localhost:3000/api/portfolio \
  -H "Authorization: Bearer YOUR_JWT_TOKEN""#
                    .to_string(),
            },
            CodeExample {
                language: "javascript".to_string(),
                description: "Get portfolio using JavaScript".to_string(),
                code: r#"const response = await fetch('http://localhost:3000/api/portfolio', {
  headers: {
    'Authorization': `Bearer ${authToken}`
  }
});

const portfolio = await response.json();
portfolio.holdings.forEach(holding => {
  console.log(`${holding.symbol}: ${holding.balance} tokens`);
  console.log(`Current value: ${holding.current_value_btc} BTC`);
});"#
                    .to_string(),
            },
            CodeExample {
                language: "python".to_string(),
                description: "Get portfolio using Python".to_string(),
                code: r#"response = requests.get(
    'http://localhost:3000/api/portfolio',
    headers={'Authorization': f'Bearer {token}'}
)

portfolio = response.json()
for holding in portfolio['holdings']:
    print(f"{holding['symbol']}: {holding['balance']} tokens")
    print(f"Value: {holding['current_value_btc']} BTC")"#
                    .to_string(),
            },
        ],
    }
}

/// Create commitment examples
fn create_commitment_example() -> ApiExample {
    ApiExample {
        operation: "create_commitment".to_string(),
        endpoint: "/api/commitments".to_string(),
        method: "POST".to_string(),
        description: "Create an output commitment".to_string(),
        examples: vec![
            CodeExample {
                language: "curl".to_string(),
                description: "Create commitment using cURL".to_string(),
                code: r#"curl -X POST http://localhost:3000/api/commitments \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -d '{
    "token_id": "550e8400-e29b-41d4-a716-446655440001",
    "title": "Weekly Newsletter",
    "description": "Publish a weekly newsletter for token holders",
    "deadline": "2024-02-01T00:00:00Z"
  }'"#
                .to_string(),
            },
            CodeExample {
                language: "javascript".to_string(),
                description: "Create commitment using JavaScript".to_string(),
                code: r#"const response = await fetch('http://localhost:3000/api/commitments', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${authToken}`
  },
  body: JSON.stringify({
    token_id: '550e8400-e29b-41d4-a716-446655440001',
    title: 'Weekly Newsletter',
    description: 'Publish a weekly newsletter for token holders',
    deadline: '2024-02-01T00:00:00Z'
  })
});

const commitment = await response.json();
console.log('Commitment created:', commitment);"#
                    .to_string(),
            },
        ],
    }
}

/// WebSocket examples
fn websocket_example() -> ApiExample {
    ApiExample {
        operation: "websocket".to_string(),
        endpoint: "/ws/tokens/:token_id".to_string(),
        method: "WebSocket".to_string(),
        description: "Subscribe to real-time token updates".to_string(),
        examples: vec![
            CodeExample {
                language: "javascript".to_string(),
                description: "WebSocket connection using JavaScript".to_string(),
                code: r#"const ws = new WebSocket('ws://localhost:3000/ws/tokens/550e8400-e29b-41d4-a716-446655440001');

ws.onopen = () => {
  console.log('Connected to token updates');
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);

  if (data.type === 'price_update') {
    console.log('Price:', data.price_btc);
  } else if (data.type === 'trade') {
    console.log('New trade:', data.amount, 'tokens at', data.price_btc, 'BTC');
  }
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};

ws.onclose = () => {
  console.log('Disconnected from token updates');
};"#.to_string(),
            },
            CodeExample {
                language: "python".to_string(),
                description: "WebSocket connection using Python".to_string(),
                code: r#"import asyncio
import websockets
import json

async def subscribe_to_token():
    uri = "ws://localhost:3000/ws/tokens/550e8400-e29b-41d4-a716-446655440001"
    async with websockets.connect(uri) as websocket:
        print("Connected to token updates")

        async for message in websocket:
            data = json.loads(message)

            if data['type'] == 'price_update':
                print(f"Price: {data['price_btc']}")
            elif data['type'] == 'trade':
                print(f"Trade: {data['amount']} tokens at {data['price_btc']} BTC")

asyncio.run(subscribe_to_token())"#.to_string(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_examples() {
        let examples = get_all_examples();
        assert!(!examples.is_empty());
        assert!(examples.len() >= 8);
    }

    #[test]
    fn test_register_example() {
        let example = register_example();
        assert_eq!(example.operation, "register");
        assert_eq!(example.method, "POST");
        assert!(example.examples.len() >= 3);

        // Check that we have curl, javascript, and python examples
        let languages: Vec<_> = example
            .examples
            .iter()
            .map(|e| e.language.as_str())
            .collect();
        assert!(languages.contains(&"curl"));
        assert!(languages.contains(&"javascript"));
        assert!(languages.contains(&"python"));
    }

    #[test]
    fn test_websocket_example() {
        let example = websocket_example();
        assert_eq!(example.method, "WebSocket");
        assert!(!example.examples.is_empty());
    }
}
