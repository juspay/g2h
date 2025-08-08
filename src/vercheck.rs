use cargo_metadata::semver::Version;

/// Dependency version checker for g2h build-time validation.
///
/// The `Deps` struct provides compile-time dependency version validation to ensure
/// compatibility between g2h and the dependencies it generates code for. Since g2h
/// is a build-dependency that generates code using specific versions of runtime
/// dependencies, version mismatches can cause compilation errors or runtime issues.
///
/// ## Checked Dependencies
///
/// This module validates versions for:
/// - **`axum`** - Web framework used for HTTP endpoint generation
/// - **`tonic`** - gRPC framework used for service definitions and metadata handling  
/// - **`http`** - HTTP types used for request/response conversion
///
/// ## How It Works
///
/// 1. **Version Definition**: g2h defines expected versions for each dependency
/// 2. **Runtime Check**: During build, cargo metadata is queried to find actual versions
/// 3. **Compatibility Validation**: Versions are compared using semantic versioning precedence
/// 4. **Error Reporting**: Mismatches are reported with clear error messages
///
/// ## When Validation Occurs
///
/// Version checking happens automatically when:
/// - `BridgeGenerator::new()` is called with the `validate` feature enabled
/// - The validation runs during the build process (in `build.rs`)
/// - Errors are printed to stderr but don't halt compilation (warnings only)
///
/// ## Compatibility Strategy
///
/// The validation uses `Version::cmp_precedence()` which compares:
/// - Major version (must match for compatibility)
/// - Minor version (must match for API stability)
/// - Patch version (must match to ensure bug fix consistency)
///
/// This ensures that the generated code is compatible with the exact versions
/// of dependencies that g2h was designed to work with.
///
/// ## Feature Flag
///
/// This functionality is only available when the `validate` feature is enabled:
///
/// ```toml
/// [dependencies]
/// g2h = { version = "0.4.0", features = ["validate"] }
/// ```
/// Dependency version container for validation.
///
/// Holds the expected versions of runtime dependencies that g2h generates code for.
/// These versions must match exactly with what's found in the project's Cargo.toml
/// to ensure compatibility between generated code and runtime dependencies.
pub struct Deps {
    /// Expected version of the axum web framework
    axum_version: Version,
    /// Expected version of the tonic gRPC framework  
    tonic_version: Version,
    /// Expected version of the http types crate
    http_version: Version,
}

/// Errors that can occur during dependency version validation.
///
/// These errors indicate compatibility issues between g2h and the project's
/// runtime dependencies that could cause compilation or runtime problems.
#[derive(Debug, thiserror::Error)]
pub enum DepError {
    /// A required dependency is completely missing from the project.
    ///
    /// This typically means the dependency hasn't been added to Cargo.toml,
    /// or the dependency name doesn't match what g2h expects.
    #[error("Dependency `{name}` is absent")]
    DependencyAbsent { name: String },

    /// A dependency exists but has an incompatible version.
    ///
    /// This indicates a version mismatch that could cause the generated code
    /// to be incompatible with the runtime dependency. The expected version
    /// is what g2h was designed to work with.
    #[error("Incompatible dependency `{name}`: expected `{expected}`, found `{actual}`")]
    DependencyVersionMismatch {
        name: String,
        expected: String,
        actual: String,
    },

    /// Failed to read project metadata using cargo_metadata.
    ///
    /// This usually indicates an issue with the Cargo.toml file or that
    /// the command isn't being run in a valid Cargo project.
    #[error("Failed to read Cargo metadata: {0}")]
    MetadataError(#[from] cargo_metadata::Error),

    /// Failed to parse a version string into a semantic version.
    ///
    /// This indicates an internal error where g2h's expected version
    /// strings are malformed.
    #[error("Failed to parse version: {0}")]
    VersionParseError(#[from] cargo_metadata::semver::Error),
}

impl Deps {
    /// Create a new dependency checker with expected versions.
    ///
    /// Parses the provided version strings into semantic versions for later
    /// comparison with the project's actual dependency versions.
    ///
    /// # Arguments
    ///
    /// * `axum_version` - Expected version string for axum (e.g., "0.8.3")
    /// * `tonic_version` - Expected version string for tonic (e.g., "0.13.0")
    /// * `http_version` - Expected version string for http (e.g., "1.3.1")
    ///
    /// # Returns
    ///
    /// Returns a `Deps` instance ready for validation, or a `DepError` if any
    /// of the version strings are malformed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use g2h::vercheck::Deps;
    ///
    /// let deps = Deps::new("0.8.3", "0.13.0", "1.3.1")?;
    /// deps.validate()?;
    /// ```
    pub fn new(
        axum_version: &str,
        tonic_version: &str,
        http_version: &str,
    ) -> Result<Self, DepError> {
        Ok(Self {
            axum_version: Version::parse(axum_version)?,
            tonic_version: Version::parse(tonic_version)?,
            http_version: Version::parse(http_version)?,
        })
    }

    /// Validate that the project's dependencies match the expected versions.
    ///
    /// This method queries the current Cargo project's metadata to find the actual
    /// versions of runtime dependencies and compares them with the expected versions
    /// that g2h was designed to work with.
    ///
    /// ## Validation Process
    ///
    /// 1. **Metadata Query**: Uses `cargo_metadata` to read the project's dependency graph
    /// 2. **Package Lookup**: Finds each required dependency in the dependency list
    /// 3. **Version Comparison**: Compares actual vs expected using semantic versioning precedence
    /// 4. **Error Reporting**: Returns detailed errors for any mismatches or missing dependencies
    ///
    /// ## Version Matching Strategy
    ///
    /// The validation uses strict precedence matching (`cmp_precedence`) which requires:
    /// - Exact major version match (API compatibility)
    /// - Exact minor version match (feature compatibility)
    /// - Exact patch version match (bug fix compatibility)
    ///
    /// This ensures the generated code works correctly with the exact dependency
    /// versions that g2h was tested with.
    ///
    /// ## Error Handling
    ///
    /// Validation can fail if:
    /// - A required dependency is not found in Cargo.toml
    /// - A dependency has a different version than expected
    /// - The project metadata cannot be read (invalid Cargo.toml)
    ///
    /// ## Usage in Build Scripts
    ///
    /// This is typically called automatically during build:
    ///
    /// ```rust,ignore
    /// // This happens automatically in BridgeGenerator::new()
    /// let deps = Deps::new("0.8.3", "0.13.0", "1.3.1")?;
    /// if let Err(e) = deps.validate() {
    ///     eprintln!("g2h: {}", e);  // Warning, but doesn't stop build
    /// }
    /// ```
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all dependencies match expected versions, or a `DepError`
    /// describing the first compatibility issue found.
    pub fn validate(self) -> Result<(), DepError> {
        let deps = [
            ("axum", self.axum_version),
            ("tonic", self.tonic_version),
            ("http", self.http_version),
        ];

        let metadata = cargo_metadata::MetadataCommand::new().exec()?;

        for (name, expected_version) in deps {
            let actual_version = &metadata
                .packages
                .iter()
                .find(|pkg| pkg.name == name)
                .ok_or_else(|| DepError::DependencyAbsent {
                    name: name.to_string(),
                })?
                .version;

            if expected_version.cmp_precedence(actual_version) != std::cmp::Ordering::Equal {
                return Err(DepError::DependencyVersionMismatch {
                    name: name.to_string(),
                    expected: expected_version.to_string(),
                    actual: actual_version.to_string(),
                });
            }
        }

        Ok(())
    }
}
