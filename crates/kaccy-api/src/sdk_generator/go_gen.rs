//! Go SDK generator

use std::collections::HashMap;

use super::types::{GeneratedSdk, SdkConfig, SdkLanguage, SdkMetadata};

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
