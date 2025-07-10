use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Building connector service with string enum support...");
    
    // Build both services with string enum support (serde is built-in)
    BridgeGenerator::with_tonic_build()
        .with_string_enums()  // This enables string serialization for enums!
        .compile_protos(&[
            "protos/hello-world.proto"
        ], &["protos"])?;

    println!("✅ Build completed - enums will serialize as strings!");
    Ok(())
}
