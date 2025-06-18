use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple one-liner with automatic string enum support
    BridgeGenerator::with_tonic_build()
        .with_string_enums()
        .compile_protos_with_string_enums(&["protos/hello-world.proto"], &["protos"])?;
    
    Ok(())
}
