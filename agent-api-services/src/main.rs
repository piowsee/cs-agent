//! Binary entry point. All logic lives in the library crate
//! (`agent_api_services`) so it is exercised by integration tests.

/// Starts the API service.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agent_api_services::run().await
}
