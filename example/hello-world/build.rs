use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple approach with default settings
    BridgeGenerator::with_tonic_build()
        .build_prost_config()
        .compile_protos(&["protos/hello-world.proto"], &["protos"])?;
    
    Ok(())
}
