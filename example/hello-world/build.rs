use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Building service with string enum support and validation...");

    // Use g2h's new compile_protos_with_validation method that supports both string enums and prost-validate
    BridgeGenerator::with_tonic_build()
        .with_string_enums() // This enables string serialization for enums!
        .compile_protos_with_validation(
            &["protos/hello-world.proto"],
            &["protos", "../prost-validate-types/proto"],
        )?;

    println!("✅ Build completed - enums will serialize as strings and validation is enabled!");
    Ok(())
}
