# Detailed Usage Guide

This guide explains how to use `g2h` to expose your gRPC services as HTTP endpoints.

## 1. Define your gRPC service in a `.proto` file

Create a standard protobuf definition for your service:

```protobuf
syntax = "proto3";
package user.v1;

service UserService {
    rpc CreateUser(CreateUserRequest) returns (CreateUserResponse);
    rpc GetUser(GetUserRequest) returns (GetUserResponse);
}

message CreateUserRequest {
    string name = 1;
    string email = 2;
}

message CreateUserResponse {
    string id = 1;
    bool success = 2;
}

message GetUserRequest {
    string id = 1;
}

message GetUserResponse {
    string id = 1;
    string name = 2;
    string email = 3;
}
```

## 2. Set up your build script (`build.rs`)

```rust
use std::env;
use std::path::PathBuf;
use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Simple approach with default settings
    BridgeGenerator::with_tonic_build()
        .build_prost_config()
        .compile_protos(&["proto/user_service.proto"], &["proto"])?;
    
    // Alternative approach with more control
    let tonic_generator = tonic_build::configure().service_generator();
    let bridge_generator = BridgeGenerator::new(tonic_generator);
    
    prost_build::Config::new()
        .service_generator(Box::new(bridge_generator))
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .file_descriptor_set_path(out_dir.join("user_service_descriptor.bin"))
        .compile_protos(&["proto/user_service.proto"], &["proto"])?;
    
    Ok(())
}
```

## 3. Include the generated code in your project

```rust
// In your lib.rs or main.rs
include!(concat!(env!("OUT_DIR"), "/user.v1.rs"));
```

## 4. Implement your gRPC service

```rust
use tonic::{Request, Response, Status};
use user_service::user_service_server::{UserService, UserServiceServer};
use user_service::{CreateUserRequest, CreateUserResponse, GetUserRequest, GetUserResponse};

// Include the generated code
pub mod user_service {
    include!(concat!(env!("OUT_DIR"), "/user.v1.rs"));
}

// Your service implementation
#[derive(Default, Clone)]
pub struct MyUserService {}

#[tonic::async_trait]
impl UserService for MyUserService {
    async fn create_user(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        let req = request.into_inner();
        
        // Your implementation here
        let response = CreateUserResponse {
            id: "user-123".to_string(),
            success: true,
        };
        
        Ok(Response::new(response))
    }
    
    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<GetUserResponse>, Status> {
        let req = request.into_inner();
        
        // Your implementation here
        let response = GetUserResponse {
            id: req.id,
            name: "John Doe".to_string(),
            email: "john.doe@example.com".to_string(),
        };
        
        Ok(Response::new(response))
    }
}
```

## 5. Set up your Axum application using the generated router

```rust
use axum::{Router, Server};
use std::net::SocketAddr;
use user_service::user_service_server::{UserServiceServer};
use user_service::user_service_handler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create your service implementation
    let user_service = MyUserService::default();
    
    // Create the HTTP router using g2h's generated handler
    let http_router = user_service_handler(user_service);
    
    // Set up your Axum application
    let app = Router::new()
        .nest("/api", http_router);
        
    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Listening on {}", addr);
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
        
    Ok(())
}
```

## 6. Access your API

Your service is now accessible through both gRPC and HTTP:

**gRPC (using a gRPC client)**
```
service: user.v1.UserService
method: CreateUser
payload: { "name": "Jane Doe", "email": "jane.doe@example.com" }
```

**HTTP**
```http
POST /api/user.v1.UserService/CreateUser
Content-Type: application/json

{
  "name": "Jane Doe",
  "email": "jane.doe@example.com"
}
```

## Advanced Configurations

### Custom Path Prefixes

If you want to customize the route paths, you can use Axum's routing mechanisms:

```rust
let http_router = user_service_handler(user_service);

// Add a version prefix
let app = Router::new()
    .nest("/api/v1", http_router);
```

### Combining Multiple Services

You can combine multiple service handlers into a single Axum router:

```rust
let user_service = MyUserService::default();
let auth_service = MyAuthService::default();

let user_router = user_service_handler(user_service);
let auth_router = auth_service_handler(auth_service);

let app = Router::new()
    .nest("/api/users", user_router)
    .nest("/api/auth", auth_router);
```

### Working with Metadata

The generated handlers preserve metadata between HTTP headers and gRPC metadata:

```rust
async fn create_user(
    &self,
    request: Request<CreateUserRequest>,
) -> Result<Response<CreateUserResponse>, Status> {
    // Access HTTP headers as metadata
    let auth_token = request.metadata().get("authorization")
        .ok_or_else(|| Status::unauthenticated("Missing authorization"))?;
        
    // Your implementation...
    
    // Add metadata to the response
    let mut response = Response::new(create_user_response);
    response.metadata_mut().insert("x-request-id", "12345".parse().unwrap());
    
    Ok(response)
}
```
