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

    /// Compile protobuf files with a custom prost_build::Config while applying all BridgeGenerator functionality.
    ///
    /// This method accepts a pre-configured `prost_build::Config` and enhances it with all the
    /// BridgeGenerator features including HTTP bridge generation, string enum support (if enabled),
    /// and skip nulls support. This provides maximum flexibility for users who need custom
    /// protobuf compilation configuration while still getting all the g2h functionality.
    ///
    /// ## Features Applied
    ///
    /// - **HTTP Bridge Generation**: Creates Axum HTTP endpoints for all gRPC service methods
    /// - **Service Generator**: Sets the `BridgeGenerator` as the service generator on the provided config
    /// - **String Enum Support**: When enabled via `with_string_enums()`, automatically detects enum fields
    ///   and adds appropriate serde serialization/deserialization functions
    /// - **Skip Nulls Support**: Adds `skip_serializing_if` attributes for cleaner JSON output
    /// - **File Descriptor Set**: Handles loading and optionally writing file descriptor sets (if configured)
    /// - **Custom Configuration**: Preserves all existing configuration on the provided config
    ///
    /// ## Configuration Preservation
    ///
    /// The method preserves and enhances your existing `prost_build::Config` settings:
    /// - Type attributes (e.g., `#[derive(Clone, PartialEq)]`)
    /// - Field attributes (e.g., custom serde annotations)
    /// - Message attributes
    /// - Enum attributes
    /// - Custom type mappings
    /// - Include paths and protoc settings
    ///
    /// ## String Enum Integration
    ///
    /// When string enums are enabled, the method will:
    /// 1. Automatically detect all enum fields in your protobuf messages
    /// 2. Generate field-specific serialization/deserialization functions
    /// 3. Add appropriate serde attributes to the config
    /// 4. Ensure enum fields accept both string and numeric values in JSON
    ///
    /// ## Skip Nulls Integration
    ///
    /// The method automatically adds `skip_serializing_if` attributes for:
    /// - Optional fields: `skip_serializing_if = "Option::is_none"`
    /// - String fields: `skip_serializing_if = "String::is_empty"`
    /// - This results in cleaner JSON output without null/empty values
    ///
    /// # Arguments
    ///
    /// * `config` - A pre-configured `prost_build::Config` that will be enhanced with BridgeGenerator functionality
    /// * `protos` - Paths to the protobuf files to compile
    /// * `includes` - Include directories for protobuf compilation
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful compilation, or an error if:
    /// - Proto files cannot be found or parsed
    /// - Configuration conflicts arise
    /// - Code generation fails
    /// - File I/O operations fail
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use g2h::BridgeGenerator;
    /// use prost_build::Config;
    ///
    /// // Create a custom config with your specific requirements
    /// let mut custom_config = Config::new();
    /// custom_config.type_attribute(".", "#[derive(Clone, PartialEq)]");
    /// custom_config.field_attribute("MyMessage.timestamp", "#[serde(with = \"custom_time_format\")]");
    /// custom_config.message_attribute("ErrorResponse", "#[derive(thiserror::Error)]");
    ///
    /// // Apply BridgeGenerator functionality to your custom config
    /// BridgeGenerator::with_tonic_build()
    ///     .with_string_enums()
    ///     .file_descriptor_set_path("target/descriptors.bin")
    ///     .compile_protos_with_config(
    ///         custom_config,
    ///         &["proto/service.proto", "proto/types.proto"],
    ///         &["proto", "third_party/googleapis"]
    ///     )?;
    /// ```
    ///
    /// ## Advanced Usage
    ///
    /// You can combine this with other prost features:
    ///
    /// ```rust,ignore
    /// let mut config = Config::new();
    ///
    /// // Custom derive attributes
    /// config.type_attribute(".", "#[derive(Clone, PartialEq, Eq, Hash)]");
    ///
    /// // Custom field transformations
    /// config.field_attribute("*.created_at", "#[serde(with = \"timestamp_format\")]");
    ///
    /// // Custom message attributes
    /// config.message_attribute("User", "#[derive(sqlx::FromRow)]");
    ///
    /// // Custom enum handling
    /// config.enum_attribute("Status", "#[derive(strum::EnumString)]");
    ///
    /// BridgeGenerator::with_tonic_build()
    ///     .with_string_enums()
    ///     .compile_protos_with_config(config, &["proto/api.proto"], &["proto"])?;
    /// ```
    ///
    /// This approach gives you complete control over protobuf code generation while automatically
    /// getting HTTP bridge functionality, string enum support, and clean JSON serialization.
    ///
    pub fn compile_protos_with_config(
        mut self,
        mut config: prost_build::Config,
        protos: &[impl AsRef<std::path::Path>],
        includes: &[impl AsRef<std::path::Path>],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Load file descriptor set if needed for string enums or descriptor set writing
        let file_descriptor_set = if self.enable_string_enums || self.descriptor_set_path.is_some()
        {
            Some(prost_build::Config::new().load_fds(protos, includes)?)
        } else {
            None
        };

        // Write descriptor set to file if path is configured
        if let (Some(ref path), Some(ref fds)) = (&self.descriptor_set_path, &file_descriptor_set) {
            let bytes = fds.encode_to_vec();
            std::fs::write(path, bytes)?;
        }

        // Add default serde derives if not already present
        config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");

        // Add descriptor set path to config if provided
        if let Some(ref path) = self.descriptor_set_path {
            config.file_descriptor_set_path(path);
        }

        // If string enums are not enabled, set the service generator and compile directly
        if !self.enable_string_enums {
            config.service_generator(Box::new(self));
            return Ok(config.compile_protos(protos, includes)?);
        }

        // Apply string enum support and skip nulls support when string enums are enabled
        let file_descriptor_set = file_descriptor_set.unwrap(); // Safe because enable_string_enums is true

        // Store the file descriptor set for the service generator
        self.file_descriptor_set = Some(file_descriptor_set.clone());

        // Apply enum string support by detecting enum fields automatically
        config = EnumConfig::add_enum_string_support_static(config, &file_descriptor_set);

        // Add skip nulls support by default
        config = EnumConfig::add_skip_nulls_support_static(config, &file_descriptor_set);

        // Set the service generator with the file descriptor set at the end
        config.service_generator(Box::new(self));

        // Compile with the fully enhanced config
        config.compile_protos(protos, includes)?;

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

    /// Generate enum deserializer code for a specific package with field-specific serializers
    ///
    /// This method creates type-safe enum serialization functions that prevent conflicts
    /// between different enums that might have the same integer values. Each enum field
    /// gets its own dedicated serializer/deserializer functions.
    ///
    /// # Arguments
    /// * `file_descriptor_set` - The protobuf file descriptor set containing enum definitions
    /// * `target_package` - The specific package to generate serializers for
    ///
    /// # Returns
    /// A string containing the generated Rust code with field-specific enum functions
    fn generate_package_specific_enum_deserializer_code(
        file_descriptor_set: &FileDescriptorSet,
        target_package: &str,
    ) -> String {
        let package_enum_fields =
            Self::extract_package_enum_fields_static(file_descriptor_set, target_package);

        if package_enum_fields.is_empty() {
            return String::new();
        }

        let field_specific_functions =
            Self::generate_field_specific_enum_functions_static(&package_enum_fields);

        // Parse the generated string as token stream for quote
        let field_functions_tokens: proc_macro2::TokenStream = field_specific_functions
            .parse()
            .expect("Generated field-specific enum functions should be valid Rust syntax");

        quote! {
            // Auto-generated enum deserializer module for package: #target_package
            // This file contains field-specific utilities for serializing and deserializing protobuf enums from string values in JSON

            pub mod enum_deserializer {
                use super::*;

                #field_functions_tokens
            }
        }
        .to_string()
    }

    /// Extract enum fields with their types from a specific package
    fn extract_package_enum_fields_static(
        file_descriptor_set: &FileDescriptorSet,
        target_package: &str,
    ) -> Vec<(String, String, String)> {
        // (field_id, enum_type, field_label)
        let mut enum_fields = Vec::new();

        for file in &file_descriptor_set.file {
            let package = file.package();

            // Only process files that match the target package
            if package != target_package {
                continue;
            }

            // Process all message types in the file
            for message in &file.message_type {
                Self::extract_enum_fields_from_message_static(message, &mut enum_fields);
            }
        }

        enum_fields
    }

    /// Recursively extract enum fields from a message
    fn extract_enum_fields_from_message_static(
        message: &DescriptorProto,
        enum_fields: &mut Vec<(String, String, String)>,
    ) {
        Self::extract_enum_fields_from_message_with_path_static(message, enum_fields, "");
    }

    /// Helper function to extract enum fields with full message path tracking
    fn extract_enum_fields_from_message_with_path_static(
        message: &DescriptorProto,
        enum_fields: &mut Vec<(String, String, String)>,
        message_path: &str,
    ) {
        let message_name = message.name();
        let current_path = if message_path.is_empty() {
            message_name.to_snake_case()
        } else {
            format!("{}_{}", message_path, message_name.to_snake_case())
        };

        // Process all fields in the message
        for field in &message.field {
            if field.r#type() == Type::Enum {
                let field_id = format!("{}_{}", current_path, field.name().to_snake_case());
                let enum_type = field.type_name().trim_start_matches('.');

                let enum_path = Self::resolve_enum_path(enum_type);

                let field_label = match field.label() {
                    Label::Optional => {
                        if field.proto3_optional() {
                            "Option"
                        } else {
                            "Single"
                        }
                    }
                    Label::Required => "Single",
                    Label::Repeated => "Repeated",
                };

                enum_fields.push((field_id, enum_path, field_label.to_string()));
            }
        }

        // Recursively process nested message types
        for nested_message in &message.nested_type {
            Self::extract_enum_fields_from_message_with_path_static(
                nested_message,
                enum_fields,
                &current_path,
            );
        }
    }

    /// Resolve the correct Rust path for an enum type from its protobuf type name
    fn resolve_enum_path(enum_type: &str) -> String {
        if !enum_type.contains('.') {
            return enum_type.to_string();
        }

        let parts: Vec<&str> = enum_type.split('.').collect();

        match parts.len() {
            0 | 1 => parts.last().unwrap_or(&"UnknownEnum").to_string(),
            2 => {
                // Package-level enum like "package.EnumName"
                // Use just the enum name since it's in the same crate
                parts[1].to_string()
            }
            _ => {
                // Three or more parts - need to determine the structure
                let enum_name = parts[parts.len() - 1];

                // Look for message parts (PascalCase) vs package parts (lowercase/version)
                let mut message_parts = Vec::new();
                let start_idx = 1; // Skip the package name

                for &part in &parts[start_idx..parts.len() - 1] {
                    if Self::is_message_name(part) {
                        message_parts.push(part.to_snake_case());
                    }
                }

                if message_parts.is_empty() {
                    // No message parts found, treat as package-level enum
                    enum_name.to_string()
                } else {
                    // Build the nested module path
                    format!("{}::{}", message_parts.join("::"), enum_name)
                }
            }
        }
    }

    /// Check if a name looks like a protobuf message name (PascalCase)
    fn is_message_name(name: &str) -> bool {
        name.chars().next().is_some_and(|c| c.is_uppercase())
    }

    /// Generate field-specific enum serialization/deserialization functions
    fn generate_field_specific_enum_functions_static(
        enum_fields: &[(String, String, String)],
    ) -> String {
        let mut functions = String::new();

        for (field_id, enum_name, field_label) in enum_fields {
            let enum_ident: proc_macro2::TokenStream = enum_name
                .parse()
                .unwrap_or_else(|e| panic!("Invalid enum type path '{enum_name}': {e}"));

            let function_code = match field_label.as_str() {
                "Single" => Self::generate_single_enum_functions(field_id, &enum_ident),
                "Option" => Self::generate_option_enum_functions(field_id, &enum_ident),
                "Repeated" => Self::generate_repeated_enum_functions(field_id, &enum_ident),
                _ => String::new(),
            };

            functions.push_str(&function_code);
        }

        functions
    }

    /// Generate serializer/deserializer functions for a single enum field
    fn generate_single_enum_functions(
        field_id: &str,
        enum_ident: &proc_macro2::TokenStream,
    ) -> String {
        let serialize_fn = quote::format_ident!("serialize_{}_as_string", field_id);
        let deserialize_fn = quote::format_ident!("deserialize_{}_from_string", field_id);

        quote! {
            #[allow(dead_code)]
            pub fn #serialize_fn<S>(value: &i32, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::Serialize;
                if let Ok(enum_val) = #enum_ident::try_from(*value) {
                    enum_val.as_str_name().serialize(serializer)
                } else {
                    value.serialize(serializer)
                }
            }

            #[allow(dead_code)]
            pub fn #deserialize_fn<'de, D>(deserializer: D) -> Result<i32, D::Error>
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
                        if let Some(enum_val) = #enum_ident::from_str_name(&s) {
                            Ok(enum_val as i32)
                        } else {
                            Err(serde::de::Error::custom(format!("Unknown enum value for {}: {}", stringify!(#enum_ident), s)))
                        }
                    }
                    EnumOrString::Int(i) => Ok(i),
                }
            }
        }.to_string()
    }

    /// Generate serializer/deserializer functions for an optional enum field
    fn generate_option_enum_functions(
        field_id: &str,
        enum_ident: &proc_macro2::TokenStream,
    ) -> String {
        let serialize_fn = quote::format_ident!("serialize_option_{}_as_string", field_id);
        let deserialize_fn = quote::format_ident!("deserialize_option_{}_from_string", field_id);

        quote! {
            #[allow(dead_code)]
            pub fn #serialize_fn<S>(value: &Option<i32>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::Serialize;
                match value {
                    Some(val) => {
                        if let Ok(enum_val) = #enum_ident::try_from(*val) {
                            Some(enum_val.as_str_name()).serialize(serializer)
                        } else {
                            Some(*val).serialize(serializer)
                        }
                    }
                    None => None::<&str>.serialize(serializer),
                }
            }

            #[allow(dead_code)]
            pub fn #deserialize_fn<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
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
                        if let Some(enum_val) = #enum_ident::from_str_name(&s) {
                            Ok(Some(enum_val as i32))
                        } else {
                            Err(serde::de::Error::custom(format!("Unknown enum value for {}: {}", stringify!(#enum_ident), s)))
                        }
                    }
                    Some(OptionalEnumOrString::Int(i)) => Ok(Some(i)),
                    Some(OptionalEnumOrString::None) | None => Ok(None),
                }
            }
        }.to_string()
    }

    /// Generate serializer/deserializer functions for a repeated enum field
    fn generate_repeated_enum_functions(
        field_id: &str,
        enum_ident: &proc_macro2::TokenStream,
    ) -> String {
        let serialize_fn = quote::format_ident!("serialize_repeated_{}_as_string", field_id);
        let deserialize_fn = quote::format_ident!("deserialize_repeated_{}_from_string", field_id);

        quote! {
            #[allow(dead_code)]
            pub fn #serialize_fn<S>(values: &[i32], serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::Serialize;
                let string_values: Vec<_> = values.iter().map(|val| {
                    if let Ok(enum_val) = #enum_ident::try_from(*val) {
                        enum_val.as_str_name().to_string()
                    } else {
                        val.to_string()
                    }
                }).collect();
                string_values.serialize(serializer)
            }

            #[allow(dead_code)]
            pub fn #deserialize_fn<'de, D>(deserializer: D) -> Result<Vec<i32>, D::Error>
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
                            if let Some(enum_val) = #enum_ident::from_str_name(&s) {
                                result.push(enum_val as i32);
                            } else {
                                return Err(serde::de::Error::custom(format!("Unknown enum value for {}: {}", stringify!(#enum_ident), s)));
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

        // Add skip nulls support by default
        config = Self::add_skip_nulls_support_static(config, file_descriptor_set);

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
        config: prost_build::Config,
        message: &DescriptorProto,
        package: &str,
    ) -> prost_build::Config {
        Self::process_message_descriptor_with_path_static(config, message, package, "")
    }

    fn process_message_descriptor_with_path_static(
        mut config: prost_build::Config,
        message: &DescriptorProto,
        package: &str,
        message_path: &str,
    ) -> prost_build::Config {
        let message_name = message.name();
        let current_path = if message_path.is_empty() {
            message_name.to_snake_case()
        } else {
            format!("{}_{}", message_path, message_name.to_snake_case())
        };

        // Determine if this is a nested message (has a parent message path)
        let is_nested = !message_path.is_empty();

        // Process all fields in the message
        for field in &message.field {
            if Self::is_enum_field_static(field) {
                config = Self::add_enum_deserializer_with_path_static(
                    config,
                    &current_path,
                    message_name,
                    field,
                    package,
                    is_nested,
                );
            }
        }

        // Recursively process nested message types
        for nested_message in &message.nested_type {
            config = Self::process_message_descriptor_with_path_static(
                config,
                nested_message,
                package,
                &current_path,
            );
        }

        config
    }

    fn is_enum_field_static(field: &FieldDescriptorProto) -> bool {
        // Check if the field type is an enum
        field.r#type() == Type::Enum
    }

    fn add_enum_deserializer_with_path_static(
        mut config: prost_build::Config,
        message_path: &str,
        message_name: &str,
        field: &FieldDescriptorProto,
        _package: &str,
        is_nested: bool,
    ) -> prost_build::Config {
        // Use the actual message name for the field_path (what prost expects)
        let field_path = format!("{}.{}", message_name, field.name());

        // Create field-specific serializer function names using the full path
        let field_id = format!("{}_{}", message_path, field.name().to_snake_case());

        // Use the correct module path based on whether this message is nested
        let enum_deserializer_path = if is_nested {
            "super::enum_deserializer"
        } else {
            "enum_deserializer"
        };

        let serde_attribute = match Self::get_field_label_static(field) {
            FieldLabel::Optional => {
                // For optional fields, check if prost would generate Option<T> or just T with default
                if field.proto3_optional() {
                    format!("#[serde(serialize_with = \"{enum_deserializer_path}::serialize_option_{field_id}_as_string\", deserialize_with = \"{enum_deserializer_path}::deserialize_option_{field_id}_from_string\", default)]")
                } else {
                    // In proto3, scalar types have implicit defaults, so use regular deserializer
                    format!("#[serde(serialize_with = \"{enum_deserializer_path}::serialize_{field_id}_as_string\", deserialize_with = \"{enum_deserializer_path}::deserialize_{field_id}_from_string\", default)]")
                }
            },
            FieldLabel::Required => format!("#[serde(serialize_with = \"{enum_deserializer_path}::serialize_{field_id}_as_string\", deserialize_with = \"{enum_deserializer_path}::deserialize_{field_id}_from_string\")]"),
            FieldLabel::Repeated => format!("#[serde(serialize_with = \"{enum_deserializer_path}::serialize_repeated_{field_id}_as_string\", deserialize_with = \"{enum_deserializer_path}::deserialize_repeated_{field_id}_from_string\", default)]"),
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

    /// Add skip nulls support by detecting field types and adding appropriate skip_serializing_if attributes
    fn add_skip_nulls_support_static(
        mut config: prost_build::Config,
        file_descriptor_set: &FileDescriptorSet,
    ) -> prost_build::Config {
        for file in &file_descriptor_set.file {
            for message in &file.message_type {
                config = Self::process_message_skip_nulls_recursive(config, message);
            }
        }
        config
    }

    fn process_message_skip_nulls_recursive(
        mut config: prost_build::Config,
        message: &DescriptorProto,
    ) -> prost_build::Config {
        let message_name = message.name();

        // Process all fields in the message
        for field in &message.field {
            config = Self::add_skip_null_attribute_static(config, message_name, field);
        }

        // Recursively process nested message types
        for nested_message in &message.nested_type {
            config = Self::process_message_skip_nulls_recursive(config, nested_message);
        }

        config
    }

    fn add_skip_null_attribute_static(
        mut config: prost_build::Config,
        message_name: &str,
        field: &FieldDescriptorProto,
    ) -> prost_build::Config {
        const SKIP_NONE: &str = "#[serde(skip_serializing_if = \"Option::is_none\")]";
        const SKIP_EMPTY: &str = "#[serde(skip_serializing_if = \"String::is_empty\")]";
        let field_path = format!("{}.{}", message_name, field.name());
        let skip_attribute = if field.proto3_optional()
            || (field.label() == Label::Optional && field.r#type() == Type::Message)
        {
            Some(SKIP_NONE)
        } else if field.r#type() == Type::String && field.label() != Label::Repeated {
            Some(SKIP_EMPTY)
        } else {
            None
        };

        if let Some(attribute) = skip_attribute {
            config.field_attribute(&field_path, attribute);
        }

        config
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
        let enum_serializer_macro = Self::generate_enum_serializer_macro_static(&enum_types);
        let single_deserializer = Self::generate_single_enum_deserializer_static();
        let option_deserializer = Self::generate_option_enum_deserializer_static();
        let repeated_deserializer = Self::generate_repeated_enum_deserializer_static();
        let single_serializer = Self::generate_single_enum_serializer_static();
        let option_serializer = Self::generate_option_enum_serializer_static();
        let repeated_serializer = Self::generate_repeated_enum_serializer_static();

        // Parse the generated strings as token streams for quote
        let enum_list_tokens: proc_macro2::TokenStream = enum_list_macro.parse().unwrap();
        let enum_serializer_tokens: proc_macro2::TokenStream =
            enum_serializer_macro.parse().unwrap();
        let single_deserializer_tokens: proc_macro2::TokenStream =
            single_deserializer.parse().unwrap();
        let option_deserializer_tokens: proc_macro2::TokenStream =
            option_deserializer.parse().unwrap();
        let repeated_deserializer_tokens: proc_macro2::TokenStream =
            repeated_deserializer.parse().unwrap();
        let single_serializer_tokens: proc_macro2::TokenStream = single_serializer.parse().unwrap();
        let option_serializer_tokens: proc_macro2::TokenStream = option_serializer.parse().unwrap();
        let repeated_serializer_tokens: proc_macro2::TokenStream =
            repeated_serializer.parse().unwrap();

        quote! {
            // Auto-generated enum deserializer module
            // This file contains utilities for serializing and deserializing protobuf enums from string values in JSON

            pub mod enum_deserializer {
                use super::*;

                #enum_list_tokens

                #enum_serializer_tokens

                #single_deserializer_tokens

                #option_deserializer_tokens

                #repeated_deserializer_tokens

                #single_serializer_tokens

                #option_serializer_tokens

                #repeated_serializer_tokens
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
        let message_module = message_name.to_snake_case();

        // Enums directly in this message
        for enum_desc in &message.enum_type {
            let enum_name = enum_desc.name();
            enum_types.push(format!("{module_path}{message_module}::{enum_name}"));
        }

        // Recursively check nested messages
        for nested_message in &message.nested_type {
            let nested_path = format!("{module_path}{message_module}::");
            enum_types.extend(Self::extract_nested_enums_static(
                nested_message,
                &nested_path,
            ));
        }

        enum_types
    }

    fn generate_enum_list_macro_static(enum_types: &[String]) -> String {
        // Convert enum type strings to identifiers for quote
        let enum_idents: Vec<proc_macro2::TokenStream> = enum_types
            .iter()
            .map(|enum_type| {
                // Parse the enum type path as tokens (e.g., "MyEnum" or "module::MyEnum")
                enum_type
                    .parse()
                    .unwrap_or_else(|e| panic!("Invalid enum type path '{enum_type}': {e}"))
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

    fn generate_enum_serializer_macro_static(enum_types: &[String]) -> String {
        // Convert enum type strings to identifiers for quote
        let enum_idents: Vec<proc_macro2::TokenStream> = enum_types
            .iter()
            .map(|enum_type| {
                // Parse the enum type path as tokens (e.g., "MyEnum" or "module::MyEnum")
                enum_type
                    .parse()
                    .unwrap_or_else(|e| panic!("Invalid enum type path '{enum_type}': {e}"))
            })
            .collect();

        quote! {
            macro_rules! try_serialize_all_enums {
                ($value:expr) => {
                    {
                        // Try each enum type
                        #(
                            if let Ok(enum_val) = #enum_idents::try_from($value) {
                                return Some(enum_val.as_str_name());
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

    fn generate_single_enum_serializer_static() -> String {
        quote! {
            #[allow(dead_code)]
            pub fn serialize_enum_as_string<S>(value: &i32, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::Serialize;
                fn try_enum_to_string(value: i32) -> Option<&'static str> {
                    try_serialize_all_enums!(value)
                }
                if let Some(enum_str) = try_enum_to_string(*value) {
                    enum_str.serialize(serializer)
                } else {
                    value.serialize(serializer)
                }
            }
        }.to_string()
    }

    fn generate_option_enum_serializer_static() -> String {
        quote! {
            #[allow(dead_code)]
            pub fn serialize_option_enum_as_string<S>(value: &Option<i32>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::Serialize;
                fn try_enum_to_string(value: i32) -> Option<&'static str> {
                    try_serialize_all_enums!(value)
                }
                match value {
                    Some(val) => {
                        if let Some(enum_str) = try_enum_to_string(*val) {
                            Some(enum_str).serialize(serializer)
                        } else {
                            Some(*val).serialize(serializer)
                        }
                    }
                    None => None::<&str>.serialize(serializer),
                }
            }
        }.to_string()
    }

    fn generate_repeated_enum_serializer_static() -> String {
        quote! {
            #[allow(dead_code)]
            pub fn serialize_repeated_enum_as_string<S>(values: &[i32], serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::Serialize;
                fn try_enum_to_string(value: i32) -> Option<&'static str> {
                    try_serialize_all_enums!(value)
                }
                let string_values: Vec<_> = values.iter().map(|val| {
                    if let Some(enum_str) = try_enum_to_string(*val) {
                        enum_str.to_string()
                    } else {
                        val.to_string()
                    }
                }).collect();
                string_values.serialize(serializer)
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
                                let code = match status.code() {
                                    ::tonic::Code::Ok => ::http::StatusCode::OK,
                                    ::tonic::Code::InvalidArgument => ::http::StatusCode::BAD_REQUEST,
                                    ::tonic::Code::NotFound => ::http::StatusCode::NOT_FOUND,
                                    ::tonic::Code::AlreadyExists | ::tonic::Code::Aborted => ::http::StatusCode::CONFLICT,
                                    ::tonic::Code::PermissionDenied => ::http::StatusCode::FORBIDDEN,
                                    ::tonic::Code::Unauthenticated => ::http::StatusCode::UNAUTHORIZED,
                                    ::tonic::Code::ResourceExhausted => ::http::StatusCode::TOO_MANY_REQUESTS,
                                    ::tonic::Code::FailedPrecondition => ::http::StatusCode::PRECONDITION_FAILED,
                                    ::tonic::Code::Unimplemented => ::http::StatusCode::NOT_IMPLEMENTED,
                                    ::tonic::Code::Unavailable => ::http::StatusCode::SERVICE_UNAVAILABLE,
                                    ::tonic::Code::DeadlineExceeded | ::tonic::Code::Cancelled => ::http::StatusCode::REQUEST_TIMEOUT,
                                    ::tonic::Code::OutOfRange => ::http::StatusCode::RANGE_NOT_SATISFIABLE,
                                    _ => ::http::StatusCode::INTERNAL_SERVER_ERROR,
                                };

                                // Create JSON error response
                                let error_body = ErrorResponse {
                                    error: ErrorDetails {
                                        code: status.code().to_string(),
                                        message: status.message().to_string(),
                                    }
                                };

                                let body = ::axum::Json(error_body);

                                (code, body).into_response()
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

        // Add error response structures once per package
        let error_structs = quote! {
            // Error response structures for HTTP endpoints
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub struct ErrorResponse {
                pub error: ErrorDetails,
            }

            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            pub struct ErrorDetails {
                pub code: String,
                pub message: String,
            }
        };

        buf.push('\n');
        buf.push_str(&error_structs.to_string());

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
