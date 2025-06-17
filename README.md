# g2h: gRPC to HTTP Bridge Generator

[![Crates.io](https://img.shields.io/crates/v/g2h.svg)](https://crates.io/crates/g2h)
[![Docs.rs](https://docs.rs/g2h/badge.svg)](https://docs.rs/g2h)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> Seamlessly expose your gRPC services as HTTP/JSON endpoints using Axum

`g2h` (gRPC-to-HTTP) automatically generates Axum HTTP handlers for your gRPC services, allowing them to be consumed by both gRPC clients and traditional web clients using HTTP/JSON.

## Features

- 🔄 Automatic conversion between gRPC and HTTP/JSON
- 🛣️ Creates Axum routes that match gRPC service methods
- 🔌 Works with existing Tonic services with zero modification
- 🧠 Preserves metadata and headers between protocols
- 🚦 Proper error status conversion from gRPC to HTTP
- ✨ **NEW**: Automatic string enum support for HTTP endpoints

## Quick Start

```toml
# Cargo.toml
[dependencies]
tonic = "0.13.0"
prost = "0.13.5"
axum = "0.8.3"
http = "1.3.1"
serde = { version = "1.0", features = ["derive"] }

[build-dependencies]
g2h = "0.1.0"
tonic-build = "0.13.0"
prost-build = "0.13.5"
```

```rust
// In your build.rs
use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple approach with default settings
    BridgeGenerator::with_tonic_build()
        .build_prost_config()
        .compile_protos(&["proto/service.proto"], &["proto"])?;
    
    Ok(())
}
```

```rust
// In your main.rs - Create an Axum app with your gRPC service
let my_service = MyServiceImpl::default();
let http_router = my_service_handler(my_service);

let app = Router::new().nest("/api", http_router);
```

Now your service is accessible through both gRPC and HTTP:

```http
POST /api/package.ServiceName/MethodName
Content-Type: application/json

{
  "field": "value"
}
```

## Documentation

For complete usage examples and API documentation:

- [Detailed Usage Guide](docs/usage.md)
- [Documentation](https://docs.rs/g2h)
- [Example Project](docs/example.md)

## How It Works

`g2h` extends the standard gRPC code generation pipeline to create additional Axum router functions. These routers map HTTP POST requests to their corresponding gRPC methods, handling serialization/deserialization and status code conversion automatically.

## String Enum Support

Enable automatic string enum deserialization for more user-friendly HTTP APIs:

```rust
// build.rs with string enum support
use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    BridgeGenerator::with_tonic_build()
        .build_prost_config()
        .with_string_enums()  // Enable automatic enum string support
        .compile_protos(&["proto/service.proto"], &["proto"])?;
    
    Ok(())
}
```

Now your HTTP endpoints accept both string and integer enum values:

```json
// ✅ Both formats work
{
  "status": "ACTIVE",        // String format (user-friendly)
  "priority": 1              // Integer format (still supported)
}
```

### Key Benefits

- **Drop-in replacement**: Just add `.with_string_enums()` to your existing build chain
- **No feature flags**: Works out of the box
- **No manual includes**: Enum deserializers are automatically included
- **Dynamic**: Works with any protobuf package structure
- **Chainable API**: Follows the same pattern as standard prost_build configuration

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.


---

<div align="center">

Built by Human, Documented by LLM.

</div>
