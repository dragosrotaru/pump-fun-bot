# Database Setup and Usage

This document describes how to set up and use the PostgreSQL database for the Pump Bot.

## Prerequisites

- PostgreSQL 12 or higher
- SQLx CLI (for running migrations)

## Setup

1. Install PostgreSQL if you haven't already:

   ```bash
   # macOS (using Homebrew)
   brew install postgresql@14
   brew services start postgresql@14

   # Ubuntu/Debian
   sudo apt update
   sudo apt install postgresql postgresql-contrib
   ```

2. Create the database:

   ```bash
   createdb pump_bot
   ```

3. Install SQLx CLI:

   ```bash
   cargo install sqlx-cli
   ```

4. Set up the environment:

   ```bash
   # Copy the example .env file
   cp .env.example .env

   # Edit the .env file with your database credentials
   # Default is: DATABASE_URL=postgres://postgres:postgres@localhost:5432/pump_bot
   ```

5. Run the migrations:

   ```bash
   sqlx migrate run
   ```

## Database Schema

The database consists of the following tables:

- `tokens`: Stores token information
- `prices`: Records token price history
- `trades`: Tracks buy/sell trades
- `timings`: Records operation timing metrics
- `errors`: Logs errors and their context
- `strategy`: Stores trading strategy configurations
- `outcomes`: Records trade outcomes
- `skipped`: Tracks skipped trading opportunities

## Usage

The database operations are handled through the `Database` struct in `src/db.rs`. Here's an example of how to use it:

```rust
use solana_pump_bot::db::{Database, Token};
use chrono::Utc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenv::dotenv()?;
    
    // Create database connection
    let database_url = std::env::var("DATABASE_URL")?;
    let db = Database::new(&database_url).await?;
    
    // Insert a token
    let token = Token {
        token_addr: "token123".to_string(),
        symbol: "TKN".to_string(),
        name: "Test Token".to_string(),
        user_addr: "user123".to_string(),
        bond_curve_addr: "curve123".to_string(),
        assoc_bond_curve_addr: "assoc123".to_string(),
        timestamp: Utc::now(),
    };
    
    db.insert_token(&token).await?;
    
    Ok(())
}
```

## Migrations

To create a new migration:

```bash
sqlx migrate add <migration_name>
```

To run migrations:

```bash
sqlx migrate run
```

To revert the last migration:

```bash
sqlx migrate revert
```
