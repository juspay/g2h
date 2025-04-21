use cargo_metadata::semver::Version;

///
/// [`Deps`] this is used to check the dependencies of the project. `g2h` is a build-dependency.
///
/// We can preemptively recognize the version of the following dependencies:
/// - `axum`
/// - `tonic`
/// - `http`
///
pub struct Deps {
    axum_version: Version,
    tonic_version: Version,
    http_version: Version,
}

#[derive(Debug, thiserror::Error)]
pub enum DepError {
    #[error("Dependency `{name}` is absent")]
    DependencyAbsent { name: String },
    #[error("Incompatible dependency `{name}`: expected `{expected}`, found `{actual}`")]
    DependencyVersionMismatch {
        name: String,
        expected: String,
        actual: String,
    },
    #[error("Failed to read Cargo metadata: {0}")]
    MetadataError(#[from] cargo_metadata::Error),
    #[error("Failed to parse version: {0}")]
    VersionParseError(#[from] cargo_metadata::semver::Error),
}

impl Deps {
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
