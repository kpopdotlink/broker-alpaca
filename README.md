# broker-alpaca

Alpaca Markets broker plugin for KL Investment platform.

## Features

- **Commission-free trading** - Stocks and ETFs
- **Paper trading** - Built-in sandbox environment
- **Simple authentication** - API Key + Secret
- **Real-time data** - Account, positions, orders

## Setup

### 1. Get API Keys

1. Create account at [Alpaca Markets](https://alpaca.markets)
2. Navigate to Paper Trading or Live Trading
3. Generate API Key and Secret

### 2. Configure Plugin

```json
{
    "api_key": "YOUR_API_KEY",
    "api_secret": "YOUR_API_SECRET",
    "is_paper": true
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `api_key` | Yes | Alpaca API Key ID |
| `api_secret` | Yes | Alpaca API Secret Key |
| `is_paper` | No | Use paper trading (default: true) |

## API Endpoints Used

| Endpoint | Description |
|----------|-------------|
| `GET /v2/account` | Account information |
| `GET /v2/positions` | List all positions |
| `POST /v2/orders` | Submit new order |
| `DELETE /v2/orders/{id}` | Cancel order |
| `GET /v2/orders/{id}` | Get order status |

## Persona Integration

This plugin supports KL Investment's Persona feature for virtual sub-accounts:

```rust
// In persona configuration
Persona {
    id: "my-strategy",
    broker_id: "broker-alpaca",  // Use this plugin
    broker_account_id: "ACCOUNT_NUMBER",
    // ...
}
```

## Order Types

| Type | Alpaca Value | Description |
|------|--------------|-------------|
| Market | `market` | Execute at current price |
| Limit | `limit` | Execute at specified price or better |
| Stop | `stop` | Trigger market order at stop price |
| Stop Limit | `stop_limit` | Trigger limit order at stop price |

## Data Mapping

### Account → AccountSummary

| Alpaca Field | KL Field |
|--------------|----------|
| `account_number` | `id` |
| `equity` | `balance.total_equity` |
| `cash` | `balance.available_cash` |
| `buying_power` | `balance.buying_power` |
| `currency` | `balance.currency` |

### Position → Position

| Alpaca Field | KL Field |
|--------------|----------|
| `symbol` | `symbol_id` |
| `qty` | `quantity` |
| `avg_entry_price` | `average_price` |
| `current_price` | `current_price` |
| `unrealized_pl` | `unrealized_pnl` |
| `unrealized_plpc` | `unrealized_pnl_percent` |

## Build

```bash
# Development build
cargo build

# WASM build for plugin runtime
cargo build --target wasm32-wasip1 --release
```

## Environment URLs

| Environment | Base URL |
|-------------|----------|
| Paper | `https://paper-api.alpaca.markets` |
| Live | `https://api.alpaca.markets` |
| Data | `https://data.alpaca.markets` |

## Resources

- [Alpaca Documentation](https://docs.alpaca.markets)
- [API Reference](https://docs.alpaca.markets/reference)
- [Paper Trading Guide](https://alpaca.markets/docs/trading/paper-trading/)

## License

MIT
