//! # g2h: gRPC to HTTP Bridge Generator
//!
//! `g2h` automatically generates Axum HTTP/JSON endpoints from your gRPC service definitions,
//! allowing a single service implementation to be exposed through both gRPC and REST interfaces.
//!
//! ## Overview
//!
//! Modern APIs often need to support multiple protocols to accommodate different clients:
//! - **gRPC** provides excellent performance and type safety for service-to-service communication
//! - **HTTP/JSON** remains the standard for web browsers and many client applications
//!
//! Rather than maintaining separate implementations, `g2h` lets you:
//! - Define your API once using Protocol Buffers
//! - Implement your service logic once using Tonic
//! - Automatically expose both gRPC and HTTP/JSON endpoints
//!
//! ## Features
//!
//! - **Zero-boilerplate API exposure** - Automatically generate HTTP endpoints from gRPC services
//! - **Protocol conversion** - Transparent conversion between gRPC and HTTP/JSON formats
//! - **Metadata preservation** - Headers and metadata are properly mapped between protocols
//! - **Error handling** - gRPC status codes are correctly translated to HTTP status codes
//! - **Axum integration** - Generated code uses Axum, a modern Rust web framework
//! - **Type safety** - Leverages Rust's type system for safe request/response handling
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! // build.rs
//! use g2h::BridgeGenerator;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Simple approach with default settings
//!     BridgeGenerator::with_tonic_build()
//!         .build_prost_config()
//!         .compile_protos(&["proto/service.proto"], &["proto"])?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! In your service code:
//!
//! ```rust,ignore
//! use axum::Router;
//!
//! // Import the generated code
//! pub mod service {
//!     include!(concat!(env!("OUT_DIR"), "/my_package.service.rs"));
//! }
//!
//! // Get the HTTP router function that was generated for your service
//! use service::my_service_handler;
//!
//! // Create your service instance
//! let my_service = MyServiceImpl::default();
//!
//! // Create your Axum router
//! let http_router = my_service_handler(my_service);
//!
//! // Use it in your Axum application
//! let app = Router::new().nest("/api", http_router);
//! ```
//!
//! Each gRPC method is now accessible via an HTTP endpoint with the pattern:
//! `POST /{package}.{ServiceName}/{MethodName}`
//!
//! ## How It Works
//!
//! `g2h` extends the standard gRPC code generation pipeline by implementing
//! `prost_build::ServiceGenerator`. For each gRPC service:
//!
//! 1. It generates an Axum router function that creates a POST route for each service method
//! 2. Requests are automatically converted between JSON and Protocol Buffers
//! 3. HTTP headers are mapped to gRPC metadata and vice versa
//! 4. Error status codes are properly translated between protocols
//!
//! This allows your service implementation to be called seamlessly through
//! either protocol without any additional code.

#[cfg(feature = "validate")]
mod ver {
    pub const AXUM_VERSION: &str = "0.8.3";
    pub const TONIC_VERSION: &str = "0.13.0";
    pub const HTTP_VERSION: &str = "1.3.1";
}

use heck::ToSnakeCase;
use prost_build::ServiceGenerator;
use quote::quote;

#[cfg(feature = "validate")]
pub(crate) mod vercheck;

/// A service generator that creates web endpoints for gRPC services using Axum.
///
/// The `WebGenerator` wraps another service generator and extends its functionality
/// by generating additional code for web-based access to gRPC services. It creates
/// Axum routes that correspond to the gRPC service methods, allowing the service
/// to be accessed via HTTP POST requests.
///
/// # Implementation Details
///
/// The generator creates:
/// - An Axum router function for each gRPC service
/// - POST endpoints for each service method
/// - Automatic conversion between HTTP and gRPC requests/responses
/// - Proper handling of metadata and headers
/// - Error status conversion from gRPC to HTTP
///
/// # Example
///
/// Given this proto file:
/// ```protobuf
/// syntax = "proto3";
/// package user.v1;
///
/// service UserService {
///     rpc CreateUser(CreateUserRequest) returns (CreateUserResponse);
///     rpc GetUser(GetUserRequest) returns (GetUserResponse);
/// }
/// ```
///
/// The generator creates corresponding HTTP endpoints:
/// ```http
/// POST /user.v1.UserService/CreateUser
/// Content-Type: application/json
///
/// {
///   // CreateUserRequest fields in JSON format
/// }
///
/// POST /user.v1.UserService/GetUser
/// Content-Type: application/json
///
/// {
///   // GetUserRequest fields in JSON format
/// }
/// ```
///
/// # Type Parameters
///
/// The generated router function accepts any type that implements the service trait.
///
pub struct BridgeGenerator {
    /// The inner generator that handles the base gRPC code generation.
    /// This is typically the default Tonic generator.
    inner: Box<dyn ServiceGenerator>,
}

impl BridgeGenerator {
    ///
    /// Creates a new `BridgeGenerator` instance.
    ///
    /// # Arguments
    /// inner: A boxed service generator that will be used to generate the base gRPC code.
    ///
    /// # Example
    ///
    /// ```rust
    /// use g2h::BridgeGenerator;
    /// let service_generator = BridgeGenerator::new(tonic_build::configure().service_generator());
    /// ```
    ///
    pub fn new(inner: Box<dyn ServiceGenerator>) -> Self {
        #[cfg(feature = "validate")]
        {
            let output =
                vercheck::Deps::new(ver::AXUM_VERSION, ver::TONIC_VERSION, ver::HTTP_VERSION)
                    .and_then(vercheck::Deps::validate);
            if let Err(err) = output {
                eprintln!("g2h: {err}");
            }
        }

        Self { inner }
    }

    ///
    /// Creates a new `prost_build::Config` instance with the service generator set to this
    /// `BridgeGenerator`.
    ///
    /// # Example
    ///
    /// ```rust
    ///
    /// use g2h::BridgeGenerator;
    /// use prost_build::Config;
    ///
    /// BridgeGenerator::with_tonic_build()                         // create the service generator
    ///    .build_prost_config()                                    // convert to `prost_build::Config`
    ///    .compile_protos(&["path/to/your.proto"], &["path/to/your/include"]); // compile the proto files
    ///
    /// ```
    ///
    pub fn build_prost_config(self) -> prost_build::Config {
        let mut config = prost_build::Config::new();
        config
            .service_generator(Box::new(self))
            .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
        config
    }

    ///
    /// Creates a new `BridgeGenerator` instance with the default Tonic service generator.
    ///
    /// It's a shorthand for `BridgeGenerator::new(tonic_build::configure().service_generator())`.
    ///
    pub fn with_tonic_build() -> Self {
        Self::new(tonic_build::configure().service_generator())
    }
}

impl prost_build::ServiceGenerator for BridgeGenerator {
    fn generate(&mut self, service: prost_build::Service, buf: &mut String) {
        self.inner.generate(service.clone(), buf);

        let package = &service.package;
        let name = &service.proto_name;
        let func_name = service.name.to_string();
        let ident_func_name = quote::format_ident!("{}", func_name);
        let branch_names = service
            .methods
            .iter()
            .map(|method| format!("/{package}.{name}/{}", method.proto_name))
            .collect::<Vec<_>>();

        let func_names = service
            .methods
            .iter()
            .map(|method| quote::format_ident!("{}", method.name))
            .collect::<Vec<_>>();

        let branch_request = service
            .methods
            .iter()
            .map(|method| quote::format_ident!("{}", method.input_type.trim_matches('"')))
            .collect::<Vec<_>>();

        #[cfg(feature = "doc")]
        let branch_response = service
            .methods
            .iter()
            .map(|method| quote::format_ident!("{}", method.output_type.trim_matches('"')))
            .collect::<Vec<_>>();

        let snake_case_name = func_name.to_snake_case();
        let service_name = quote::format_ident!("{}_handler", snake_case_name);
        let server_module = quote::format_ident!("{}_server", snake_case_name);

        #[cfg(feature = "doc")]
        let docs = quote! {
            #[doc = "Axum Router for handling the gRPC service. This router is generated with the [`prost-build`] crate. This builds a web router on top of the gRPC service."]
            #[doc = ""]
            #[doc = ::std::concat!("Package: `", stringify!(#package), "`")]
            #[doc = ""]
            #[doc = ::std::concat!("Name: `", stringify!(#name), "`")]
            #[doc = ""]
            #[doc = "Routes:"]
            #(
                #[doc = ::std::concat!("- `", stringify!(#func_names), "` `::` [`", stringify!(#branch_request), "`]` -> `[`", stringify!(#branch_response), "`]")]
            )*
        };
        #[cfg(not(feature = "doc"))]
        let docs = quote! {};

        let output = quote! {
            #[allow(dead_code)]
            #docs
            pub fn #service_name<T: #server_module::#ident_func_name>(server: T) -> ::axum::Router {
                use ::axum::extract::State;
                use ::axum::response::IntoResponse;
                use std::sync::Arc;
                let router = ::axum::Router::new();

                #(
                    let router = router.route(#branch_names, ::axum::routing::post(|State(state): State<Arc<T>>, extension: ::http::Extensions, headers: ::http::header::HeaderMap, ::axum::Json(body): ::axum::Json<#branch_request>| async move {

                        let metadata_map = ::tonic::metadata::MetadataMap::from_headers(headers);
                        let request = ::tonic::Request::from_parts(metadata_map, extension, body);

                        let output = <T as #server_module::#ident_func_name>::#func_names(&state, request).await;

                        match output {
                            Ok(response) => {
                                let (metadata_map, body, extension) = response.into_parts();
                                let headers = metadata_map.into_headers();
                                let body = ::axum::Json(body);

                                (headers, extension, body).into_response()
                            },
                            Err(status) => {
                                let (parts, body) = status.into_http::<::axum::body::Body>().into_parts();

                                ::http::response::Response::from_parts(parts, ::axum::body::Body::new(body))
                            }
                        }

                    }));
                )*

                router.with_state(Arc::new(server))
            }
        };

        buf.push_str(&output.to_string());
    }
    fn finalize(&mut self, _buf: &mut String) {
        self.inner.finalize(_buf);
    }

    fn finalize_package(&mut self, _package: &str, _buf: &mut String) {
        self.inner.finalize_package(_package, _buf);
    }
}
