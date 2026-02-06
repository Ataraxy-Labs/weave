mod server;
mod tools;

use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Log to stderr so it doesn't interfere with MCP stdio transport
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("weave_mcp=info".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    // Server starts immediately — repo discovery is deferred to first tool call.
    // This way the MCP server works even when launched from outside a git repo.
    let server = server::WeaveServer::new();

    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
