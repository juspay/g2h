# Example Project

This document walks through a complete example project using `g2h`.

## Project Structure

```
grpc-web-example/
├── build.rs
├── Cargo.toml
├── proto/
│   └── greeter.proto
└── src/
    └── main.rs
```

## Proto Definition

```protobuf
// proto/greeter.proto
syntax = "proto3";
package greeter.v1;

service GreeterService {
    rpc SayHello(HelloRequest) returns (HelloResponse);
    rpc SayGoodbye(GoodbyeRequest) returns (GoodbyeResponse);
}

message HelloRequest {
    string name = 1;
}

message HelloResponse {
    string message = 1;
    string timestamp = 2;
}

message GoodbyeRequest {
    string name = 1;
}

message GoodbyeResponse {
    string message = 1;
    string timestamp = 2;
}
```

## Cargo.toml

```toml
[package]
name = "grpc-web-example"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7.0"
g2h = "0.1.0"
prost = "0.13.5"
tokio = { version = "1.35.0", features = ["full"] }
tonic = "0.13.0"
serde = { version = "1.0", features = ["derive"] }
chrono = "0.4.31"

[build-dependencies]
g2h = "0.1.0"
prost-build = "0.13.5"
tonic-build = "0.13.0"
```

## Build Script

```rust
// build.rs
use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    BridgeGenerator::with_tonic_build()
        .build_prost_config()
        .compile_protos(&["proto/greeter.proto"], &["proto"])?;
    
    Ok(())
}
```

## Service Implementation & Server

```rust
// src/main.rs
use axum::{Router, Server};
use std::net::SocketAddr;
use tonic::{Request, Response, Status};
use chrono::Utc;

// Include the generated code
pub mod greeter {
    include!(concat!(env!("OUT_DIR"), "/greeter.v1.rs"));
}

use greeter::greeter_service_server::{GreeterService, GreeterServiceServer};
use greeter::{HelloRequest, HelloResponse, GoodbyeRequest, GoodbyeResponse};
use greeter::greeter_service_handler;

// Service implementation
#[derive(Default, Clone)]
struct MyGreeterService {}

#[tonic::async_trait]
impl GreeterService for MyGreeterService {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloResponse>, Status> {
        let name = request.into_inner().name;
        let timestamp = Utc::now().to_rfc3339();
        
        let response = HelloResponse {
            message: format!("Hello, {}!", name),
            timestamp,
        };
        
        Ok(Response::new(response))
    }
    
    async fn say_goodbye(
        &self,
        request: Request<GoodbyeRequest>,
    ) -> Result<Response<GoodbyeResponse>, Status> {
        let name = request.into_inner().name;
        let timestamp = Utc::now().to_rfc3339();
        
        let response = GoodbyeResponse {
            message: format!("Goodbye, {}!", name),
            timestamp,
        };
        
        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the service implementation
    let greeter_service = MyGreeterService::default();
    
    // Set up the gRPC server (optional)
    let grpc_service = GreeterServiceServer::new(greeter_service.clone());
    
    // Create the HTTP router with our service
    let http_router = greeter_service_handler(greeter_service);
    
    // Configure our Axum application
    let app = Router::new()
        .nest("/api", http_router);
    
    // Start the HTTP server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("HTTP server listening on {}", addr);
    
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}
```

## Test it with curl

Once your server is running, you can test it with curl:

```bash
curl -X POST http://localhost:3000/api/greeter.v1.GreeterService/SayHello \
  -H "Content-Type: application/json" \
  -d '{"name": "World"}'
```

Expected response:
```json
{
  "message": "Hello, World!",
  "timestamp": "2025-04-07T12:34:56.789Z"
}
```

## Test it with a gRPC client

For gRPC clients, the service is accessible at the same address using the standard gRPC protocol.
