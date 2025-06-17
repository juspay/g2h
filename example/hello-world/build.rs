use g2h::BridgeGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple approach with default settings
    BridgeGenerator::with_tonic_build()
        .build_prost_config()
        .compile_protos(&["protos/hello-world.proto"], &["protos"])?;
    
    // Example with string enum support (when string-enums feature is enabled)
    #[cfg(feature = "string-enums")]
    {
        use prost::Message;
        use prost_types::FileDescriptorSet;
        use std::env;
        use std::path::PathBuf;
        
        // First compile to get descriptors
        let out_dir = PathBuf::from(env::var("OUT_DIR")?);
        let mut temp_config = prost_build::Config::new();
        temp_config.file_descriptor_set_path(out_dir.join("temp_descriptors.bin"));
        temp_config.compile_protos(&["protos/hello-world.proto"], &["protos"])?;
        
        // Read the descriptors
        let descriptor_bytes = std::fs::read(out_dir.join("temp_descriptors.bin"))?;
        let file_descriptor_set = FileDescriptorSet::decode(&*descriptor_bytes)?;
        
        // Generate enum deserializer code
        let enum_config = BridgeGenerator::with_tonic_build()
            .with_string_enums()
            .build_enum_config();
            
        let deserializer_code = enum_config.generate_enum_deserializer_code(&file_descriptor_set);
        std::fs::write(out_dir.join("enum_deserializer.rs"), deserializer_code)?;
        
        // Build final config with enum detection
        enum_config
            .build_prost_config_with_descriptors(&file_descriptor_set)
            .compile_protos(&["protos/hello-world.proto"], &["protos"])?;
    }
    
    Ok(())
}
