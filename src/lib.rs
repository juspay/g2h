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
//!     // Simple approach with string enum support
//!     BridgeGenerator::with_tonic_build()
//!         .with_string_enums()
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

use prost::Message;
use prost_types::{
    field_descriptor_proto::{Label, Type},
    DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
};

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

    /// Whether to enable automatic string enum deserialization
    enable_string_enums: bool,

    /// File descriptor set for enum processing (only set when string enums are enabled)
    file_descriptor_set: Option<FileDescriptorSet>,

    /// Path where file descriptor set should be written (for tonic_reflection support)
    descriptor_set_path: Option<std::path::PathBuf>,
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

        Self {
            inner,
            enable_string_enums: false,
            file_descriptor_set: None,
            descriptor_set_path: None,
        }
    }

    ///
    /// Creates a new `prost_build::Config` instance with the service generator set to this
    /// `BridgeGenerator`.
    ///
    /// Note: For string enum support, use `compile_protos()` instead, which handles
    /// the configuration automatically.
    ///
    /// # Example
    ///
    /// ```rust
    /// use g2h::BridgeGenerator;
    ///
    /// BridgeGenerator::with_tonic_build()
    ///    .build_prost_config()
    ///    .compile_protos(&["path/to/your.proto"], &["path/to/your/include"]);
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
    /// Compile protobuf files with automatic configuration based on enabled features.
    /// This is a convenience method that handles string enum support automatically.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use g2h::BridgeGenerator;
    ///
    /// BridgeGenerator::with_tonic_build()
    ///     .with_string_enums()
    ///     .compile_protos(&["proto/service.proto"], &["proto"])?;
    /// ```
    ///
    pub fn compile_protos(
        self,
        protos: &[impl AsRef<std::path::Path>],
        includes: &[impl AsRef<std::path::Path>],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file_descriptor_set = if self.enable_string_enums || self.descriptor_set_path.is_some()
        {
            Some(prost_build::Config::new().load_fds(protos, includes)?)
        } else {
            None
        };

        // Write descriptor set if path is provided
        if let (Some(ref path), Some(ref fds)) = (&self.descriptor_set_path, &file_descriptor_set) {
            let bytes = fds.encode_to_vec();
            std::fs::write(path, bytes)?;
        }

        if !self.enable_string_enums {
            let descriptor_path = self.descriptor_set_path.clone();
            let mut config = self.build_prost_config();
            // Add descriptor set path to config if provided
            if let Some(path) = descriptor_path {
                config.file_descriptor_set_path(path);
            }
            return Ok(config.compile_protos(protos, includes)?);
        }

        // Build with automatic string enum support and compile
        let file_descriptor_set = file_descriptor_set.unwrap(); // Safe because enable_string_enums is true
        let mut generator = self;
        generator.file_descriptor_set = Some(file_descriptor_set.clone());
        let mut final_config = generator
            .build_enum_config()
            .build_prost_config_with_descriptors(&file_descriptor_set);

        final_config.compile_protos(protos, includes)?;

        Ok(())
    }

    ///
    /// Creates an EnumConfig instance for advanced enum configuration.
    ///
    /// This method returns an `EnumConfig` that can build a `prost_build::Config`
    /// with automatic enum field detection when string enums are enabled.
    ///
    /// Note: Most users should use the simpler `compile_protos()` method instead,
    /// which handles enum configuration automatically.
    ///
    fn build_enum_config(self) -> EnumConfig {
        EnumConfig::new(self)
    }

    ///
    /// Creates a new `BridgeGenerator` instance with the default Tonic service generator.
    ///
    /// It's a shorthand for `BridgeGenerator::new(tonic_build::configure().service_generator())`.
    ///
    pub fn with_tonic_build() -> Self {
        Self::new(tonic_build::configure().service_generator())
    }

    ///
    /// Enable automatic string enum deserialization for HTTP endpoints.
    ///
    /// When enabled, enum fields in protobuf messages can accept both string values
    /// (e.g., "USD", "EUR") and integer values (e.g., 1, 2) in JSON requests.
    /// The generator will automatically detect enum fields and add appropriate
    /// serde deserializers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use g2h::BridgeGenerator;
    ///
    /// BridgeGenerator::with_tonic_build()
    ///     .with_string_enums()
    ///     .compile_protos(&["proto/service.proto"], &["proto"])?;
    /// ```
    ///
    /// This allows HTTP clients to send requests like:
    /// ```json
    /// {
    ///   "currency": "USD",        // String format
    ///   "payment_method": "CARD"  // String format
    /// }
    /// ```
    ///
    /// Instead of requiring integer enum values:
    /// ```json
    /// {
    ///   "currency": 1,           // Integer format
    ///   "payment_method": 0      // Integer format
    /// }
    /// ```
    ///
    pub fn with_string_enums(mut self) -> Self {
        self.enable_string_enums = true;
        self
    }

    ///
    /// Set the path where the file descriptor set should be written.
    /// This is useful for tonic_reflection support which requires access to the
    /// file descriptor set at runtime.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use g2h::BridgeGenerator;
    /// use std::env;
    /// use std::path::PathBuf;
    ///
    /// let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    /// BridgeGenerator::with_tonic_build()
    ///     .with_string_enums()
    ///     .file_descriptor_set_path(out_dir.join("service_descriptor.bin"))
    ///     .compile_protos(&["proto/service.proto"], &["proto"])?;
    /// ```
    ///
    pub fn file_descriptor_set_path(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.descriptor_set_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Generate enum deserializer code for a specific package only
    fn generate_package_specific_enum_deserializer_code(
        file_descriptor_set: &FileDescriptorSet,
        target_package: &str,
    ) -> String {
        let package_enum_types =
            Self::extract_package_enum_types_static(file_descriptor_set, target_package);

        if package_enum_types.is_empty() {
            return String::new();
        }

        let enum_list_macro = EnumConfig::generate_enum_list_macro_static(&package_enum_types);
        let single_deserializer = EnumConfig::generate_single_enum_deserializer_static();
        let option_deserializer = EnumConfig::generate_option_enum_deserializer_static();
        let repeated_deserializer = EnumConfig::generate_repeated_enum_deserializer_static();

        // Parse the generated strings as token streams for quote
        let enum_list_tokens: proc_macro2::TokenStream = enum_list_macro
            .parse()
            .expect("Generated enum list macro should be valid Rust syntax");
        let single_tokens: proc_macro2::TokenStream = single_deserializer
            .parse()
            .expect("Generated single enum deserializer should be valid Rust syntax");
        let option_tokens: proc_macro2::TokenStream = option_deserializer
            .parse()
            .expect("Generated option enum deserializer should be valid Rust syntax");
        let repeated_tokens: proc_macro2::TokenStream = repeated_deserializer
            .parse()
            .expect("Generated repeated enum deserializer should be valid Rust syntax");

        quote! {
            // Auto-generated enum deserializer module for package: #target_package
            // This file contains utilities for deserializing protobuf enums from string values in JSON

            pub mod enum_deserializer {
                use super::*;
                #enum_list_tokens

                #single_tokens

                #option_tokens

                #repeated_tokens
            }
        }
        .to_string()
    }

    /// Extract enum types only from a specific package
    fn extract_package_enum_types_static(
        file_descriptor_set: &FileDescriptorSet,
        target_package: &str,
    ) -> Vec<String> {
        let mut enum_types = Vec::new();

        for file in &file_descriptor_set.file {
            let package = file.package();

            // Only process files that match the target package
            if package != target_package {
                continue;
            }

            // Top-level enums
            for enum_desc in &file.enum_type {
                let enum_name = enum_desc.name();
                enum_types.push(enum_name.to_string());
            }

            // Enums in messages (recursive)
            for message in &file.message_type {
                enum_types.extend(EnumConfig::extract_nested_enums_static(message, ""));
            }
        }

        enum_types
    }
}

/// Configuration helper for building prost config with automatic enum detection
pub struct EnumConfig {
    generator: BridgeGenerator,
}
impl EnumConfig {
    /// Create a new EnumConfig from a BridgeGenerator
    pub fn new(generator: BridgeGenerator) -> Self {
        Self { generator }
    }

    /// Build prost config with automatic enum field detection and deserializers
    pub fn build_prost_config_with_descriptors(
        self,
        file_descriptor_set: &FileDescriptorSet,
    ) -> prost_build::Config {
        let enable_string_enums = self.generator.enable_string_enums;
        let mut config = self.generator.build_prost_config();

        if enable_string_enums {
            config = Self::add_enum_string_support_static(config, file_descriptor_set);
        }

        config
    }

    /// Add enum string support by detecting enum fields automatically (static version)
    fn add_enum_string_support_static(
        mut config: prost_build::Config,
        file_descriptor_set: &FileDescriptorSet,
    ) -> prost_build::Config {
        for file in &file_descriptor_set.file {
            config = Self::process_file_descriptor_static(config, file);
        }
        config
    }

    fn process_file_descriptor_static(
        mut config: prost_build::Config,
        file: &FileDescriptorProto,
    ) -> prost_build::Config {
        // Process all message types in the file
        for message in &file.message_type {
            let package = file.package();
            config = Self::process_message_descriptor_static(config, message, package);
        }
        config
    }

    fn process_message_descriptor_static(
        mut config: prost_build::Config,
        message: &DescriptorProto,
        package: &str,
    ) -> prost_build::Config {
        let message_name = message.name();

        // Process all fields in the message
        for field in &message.field {
            if Self::is_enum_field_static(field) {
                config = Self::add_enum_deserializer_static(config, message_name, field, package);
            }
        }

        // Recursively process nested message types
        for nested_message in &message.nested_type {
            config = Self::process_message_descriptor_static(config, nested_message, package);
        }

        config
    }

    fn is_enum_field_static(field: &FieldDescriptorProto) -> bool {
        // Check if the field type is an enum
        field.r#type() == Type::Enum
    }

    fn add_enum_deserializer_static(
        mut config: prost_build::Config,
        message_name: &str,
        field: &FieldDescriptorProto,
        _package: &str,
    ) -> prost_build::Config {
        let field_path = format!("{}.{}", message_name, field.name());

        let serde_attribute = match Self::get_field_label_static(field) {
            FieldLabel::Optional => {
                // For optional fields, check if prost would generate Option<T> or just T with default
                if field.proto3_optional() {
                    "#[serde(deserialize_with = \"enum_deserializer::deserialize_option_enum_from_string\", default)]".to_string()
                } else {
                    // In proto3, scalar types have implicit defaults, so use regular deserializer
                    "#[serde(deserialize_with = \"enum_deserializer::deserialize_enum_from_string\", default)]".to_string()
                }
            },
            FieldLabel::Required => "#[serde(deserialize_with = \"enum_deserializer::deserialize_enum_from_string\")]".to_string(),
            FieldLabel::Repeated => "#[serde(deserialize_with = \"enum_deserializer::deserialize_repeated_enum_from_string\", default)]".to_string(),
        };

        config.field_attribute(&field_path, &serde_attribute);
        config
    }

    fn get_field_label_static(field: &FieldDescriptorProto) -> FieldLabel {
        match field.label() {
            Label::Optional => FieldLabel::Optional,
            Label::Required => FieldLabel::Required,
            Label::Repeated => FieldLabel::Repeated,
        }
    }

    /// Generate enum deserializer code that can be included in the generated crate
    pub fn generate_enum_deserializer_code(
        &self,
        file_descriptor_set: &FileDescriptorSet,
    ) -> String {
        Self::generate_enum_deserializer_code_static(file_descriptor_set)
    }

    /// Static version for generating enum deserializer code
    fn generate_enum_deserializer_code_static(file_descriptor_set: &FileDescriptorSet) -> String {
        let enum_types = Self::extract_all_enum_types_static(file_descriptor_set);

        let enum_list_macro = Self::generate_enum_list_macro_static(&enum_types);
        let single_deserializer = Self::generate_single_enum_deserializer_static();
        let option_deserializer = Self::generate_option_enum_deserializer_static();
        let repeated_deserializer = Self::generate_repeated_enum_deserializer_static();

        // Parse the generated strings as token streams for quote
        let enum_list_tokens: proc_macro2::TokenStream = enum_list_macro.parse().unwrap();
        let single_tokens: proc_macro2::TokenStream = single_deserializer.parse().unwrap();
        let option_tokens: proc_macro2::TokenStream = option_deserializer.parse().unwrap();
        let repeated_tokens: proc_macro2::TokenStream = repeated_deserializer.parse().unwrap();

        quote! {
            // Auto-generated enum deserializer module
            // This file contains utilities for deserializing protobuf enums from string values in JSON

            pub mod enum_deserializer {
                use super::*;

                #enum_list_tokens

                #single_tokens

                #option_tokens

                #repeated_tokens
            }
        }
        .to_string()
    }

    fn extract_all_enum_types_static(file_descriptor_set: &FileDescriptorSet) -> Vec<String> {
        let mut enum_types = Vec::new();

        for file in &file_descriptor_set.file {
            // Top-level enums
            for enum_desc in &file.enum_type {
                let enum_name = enum_desc.name();
                enum_types.push(enum_name.to_string());
            }

            // Enums in messages (recursive)
            for message in &file.message_type {
                enum_types.extend(Self::extract_nested_enums_static(message, ""));
            }
        }

        enum_types
    }

    fn extract_nested_enums_static(message: &DescriptorProto, module_path: &str) -> Vec<String> {
        let mut enum_types = Vec::new();
        let message_name = message.name();

        // Convert message name to snake_case for module path (prost convention)
        let message_module = Self::to_snake_case(message_name);

        // Enums directly in this message
        for enum_desc in &message.enum_type {
            let enum_name = enum_desc.name();
            enum_types.push(format!("{}{}::{}", module_path, message_module, enum_name));
        }

        // Recursively check nested messages
        for nested_message in &message.nested_type {
            let nested_path = format!("{}{}::", module_path, message_module);
            enum_types.extend(Self::extract_nested_enums_static(
                nested_message,
                &nested_path,
            ));
        }

        enum_types
    }

    fn to_snake_case(input: &str) -> String {
        let mut result = String::new();

        for c in input.chars() {
            if c.is_uppercase() {
                if !result.is_empty() {
                    result.push('_');
                }
                result.push(c.to_lowercase().next().unwrap());
            } else {
                result.push(c);
            }
        }

        result
    }

    fn generate_enum_list_macro_static(enum_types: &[String]) -> String {
        // Convert enum type strings to identifiers for quote
        let enum_idents: Vec<proc_macro2::TokenStream> = enum_types
            .iter()
            .map(|enum_type| {
                // Parse the enum type path as tokens (e.g., "MyEnum" or "module::MyEnum")
                enum_type
                    .parse()
                    .unwrap_or_else(|e| panic!("Invalid enum type path '{}': {}", enum_type, e))
            })
            .collect();

        quote! {
            macro_rules! try_parse_all_enums {
                ($s:expr) => {
                    {
                        // Try each enum type
                        #(
                            if let Some(val) = #enum_idents::from_str_name($s) {
                                return Some(val as i32);
                            }
                        )*

                        None
                    }
                };
            }
        }
        .to_string()
    }

    fn generate_single_enum_deserializer_static() -> String {
        quote! {
            #[allow(dead_code)]
            pub fn deserialize_enum_from_string<'de, D>(deserializer: D) -> Result<i32, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::Deserialize;

                #[derive(Deserialize)]
                #[serde(untagged)]
                #[allow(dead_code)]
                enum EnumOrString {
                    String(String),
                    Int(i32),
                }

                match EnumOrString::deserialize(deserializer)? {
                    EnumOrString::String(s) => {
                        fn try_parse_enum(s: &str) -> Option<i32> {
                            try_parse_all_enums!(s)
                        }
                        try_parse_enum(&s).ok_or_else(|| {
                            serde::de::Error::custom(format!("Unknown enum value: {}", s))
                        })
                    }
                    EnumOrString::Int(i) => Ok(i),
                }
            }
        }
        .to_string()
    }

    fn generate_option_enum_deserializer_static() -> String {
        quote! {
            #[allow(dead_code)]
            pub fn deserialize_option_enum_from_string<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::Deserialize;
                #[derive(Deserialize)]
                #[serde(untagged)]
                #[allow(dead_code)]
                enum OptionalEnumOrString {
                    String(String),
                    Int(i32),
                    None,
                }
                match Option::<OptionalEnumOrString>::deserialize(deserializer)? {
                    Some(OptionalEnumOrString::String(s)) => {
                        fn try_parse_enum(s: &str) -> Option<i32> {
                            try_parse_all_enums!(s)
                        }
                        try_parse_enum(&s)
                            .map(Some)
                            .ok_or_else(|| serde::de::Error::custom(format!("Unknown enum value: {}", s)))
                    }
                    Some(OptionalEnumOrString::Int(i)) => Ok(Some(i)),
                    Some(OptionalEnumOrString::None) | None => Ok(None),
                }
            }
        }.to_string()
    }

    fn generate_repeated_enum_deserializer_static() -> String {
        quote! {
            #[allow(dead_code)]
            pub fn deserialize_repeated_enum_from_string<'de, D>(deserializer: D) -> Result<Vec<i32>, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::Deserialize;
                #[derive(Deserialize)]
                #[serde(untagged)]
                #[allow(dead_code)]
                enum EnumOrStringItem {
                    String(String),
                    Int(i32),
                }
                let items: Vec<EnumOrStringItem> = Vec::deserialize(deserializer)?;
                let mut result = Vec::with_capacity(items.len());

                for item in items {
                    match item {
                        EnumOrStringItem::String(s) => {
                            fn try_parse_enum(s: &str) -> Option<i32> {
                                try_parse_all_enums!(s)
                            }
                            if let Some(enum_val) = try_parse_enum(&s) {
                                result.push(enum_val);
                            } else {
                                return Err(serde::de::Error::custom(format!("Unknown enum value: {}", s)));
                            }
                        }
                        EnumOrStringItem::Int(i) => {
                            result.push(i);
                        }
                    }
                }

                Ok(result)
            }
        }.to_string()
    }
}

#[derive(Debug)]
enum FieldLabel {
    Optional,
    Required,
    Repeated,
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
    fn finalize(&mut self, buf: &mut String) {
        self.inner.finalize(buf);
    }

    fn finalize_package(&mut self, package: &str, buf: &mut String) {
        self.inner.finalize_package(package, buf);

        // If string enums are enabled, add the enum deserializer module at the end of each package
        if self.enable_string_enums {
            if let Some(ref file_descriptor_set) = self.file_descriptor_set {
                // Generate enum deserializer code only for enums in this specific package
                let enum_deserializer_code = Self::generate_package_specific_enum_deserializer_code(
                    file_descriptor_set,
                    package,
                );
                if !enum_deserializer_code.trim().is_empty() {
                    buf.push('\n');
                    buf.push_str(&enum_deserializer_code);
                }
            }
        }
    }
}
