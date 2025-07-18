/// Test that verifies the actual generated code contains field-specific enum serializers
/// 
/// This test reads the generated protobuf code to ensure that g2h actually
/// created the field-specific serialization functions we expect.

use std::fs;
use std::path::Path;

#[test]
fn test_generated_code_contains_field_specific_serializers() {
    // Find the generated protobuf file
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let generated_file = Path::new(&out_dir).join("hello_world.rs");
    
    assert!(generated_file.exists(), "Generated file should exist: {:?}", generated_file);
    
    let generated_code = fs::read_to_string(&generated_file)
        .expect("Should be able to read generated file");
    
    // Verify that field-specific serializers are generated
    let expected_functions = vec![
        // ConflictTestRequest field serializers
        "serialize_conflict_test_request_payment_status_as_string",
        "deserialize_conflict_test_request_payment_status_from_string",
        "serialize_conflict_test_request_auth_status_as_string", 
        "deserialize_conflict_test_request_auth_status_from_string",
        "serialize_conflict_test_request_processing_status_as_string",
        "deserialize_conflict_test_request_processing_status_from_string",
        
        // Optional field serializers
        "serialize_option_conflict_test_request_optional_payment_as_string",
        "deserialize_option_conflict_test_request_optional_payment_from_string",
        
        // Repeated field serializers
        "serialize_repeated_conflict_test_request_auth_history_as_string",
        "deserialize_repeated_conflict_test_request_auth_history_from_string",
        "serialize_repeated_conflict_test_request_processing_steps_as_string",
        "deserialize_repeated_conflict_test_request_processing_steps_from_string",
        
        // Other field serializers
        "serialize_hello_request_greeting_type_as_string",
        "serialize_hello_reply_status_as_string",
        "serialize_payment_response_status_as_string",
    ];
    
    for expected_function in expected_functions {
        assert!(generated_code.contains(expected_function), 
            "Generated code should contain function: {}", expected_function);
        println!("✅ Found expected function: {}", expected_function);
    }
    
    // Verify that the functions are type-specific (use the correct enum types)
    let type_specific_checks = vec![
        ("serialize_conflict_test_request_payment_status_as_string", "PaymentStatus::try_from"),
        ("serialize_conflict_test_request_auth_status_as_string", "AuthenticationStatus::try_from"),
        ("serialize_conflict_test_request_processing_status_as_string", "ProcessingStatus::try_from"),
        ("deserialize_conflict_test_request_payment_status_from_string", "PaymentStatus::from_str_name"),
        ("deserialize_conflict_test_request_auth_status_from_string", "AuthenticationStatus::from_str_name"),
        ("deserialize_conflict_test_request_processing_status_from_string", "ProcessingStatus::from_str_name"),
    ];
    
    for (function_name, expected_enum_usage) in type_specific_checks {
        // Find the function definition (not just the serde attribute)
        let function_def_pattern = format!("pub fn {}", function_name);
        if let Some(function_start) = generated_code.find(&function_def_pattern) {
            // Look for the next function or end of module
            let function_end = generated_code[function_start..].find("\n    #[allow(dead_code)]\n    pub fn ")
                .unwrap_or_else(|| generated_code[function_start..].find("\n}").unwrap_or(2000));
            let function_code = &generated_code[function_start..function_start + function_end];
            
            assert!(function_code.contains(expected_enum_usage),
                "Function {} should use {}, but doesn't in its implementation", 
                function_name, expected_enum_usage);
            
            println!("✅ Function {} correctly uses {}", function_name, expected_enum_usage);
        } else {
            println!("⚠️  Could not find function definition for {}", function_name);
        }
    }
    
    // Verify the enum_deserializer module exists
    assert!(generated_code.contains("pub mod enum_deserializer"), 
        "Generated code should contain enum_deserializer module");
    
    // Verify that the serde attributes reference the correct functions
    let serde_attribute_checks = vec![
        ("payment_status", "serialize_conflict_test_request_payment_status_as_string"),
        ("auth_status", "serialize_conflict_test_request_auth_status_as_string"),
        ("processing_status", "serialize_conflict_test_request_processing_status_as_string"),
    ];
    
    for (field_name, expected_serializer) in serde_attribute_checks {
        // Look for the serde attribute in the struct definition
        let field_pattern = format!("pub {}: i32", field_name);
        if let Some(field_pos) = generated_code.find(&field_pattern) {
            // Look backwards for the serde attribute
            let preceding_code = &generated_code[field_pos.saturating_sub(500)..field_pos];
            assert!(preceding_code.contains(expected_serializer),
                "Field {} should have serde attribute referencing {}", field_name, expected_serializer);
            
            println!("✅ Field {} has correct serde attribute for {}", field_name, expected_serializer);
        }
    }
    
    println!("✅ Generated code verification passed!");
    println!("Generated file: {:?}", generated_file);
    println!("Total generated code size: {} bytes", generated_code.len());
}

#[test]
fn test_no_generic_enum_serializers() {
    // Verify that the old generic serializers are NOT present
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let generated_file = Path::new(&out_dir).join("hello_world.rs");
    
    let generated_code = fs::read_to_string(&generated_file)
        .expect("Should be able to read generated file");
    
    // These are the old generic functions that caused the enum conflicts
    let deprecated_functions = vec![
        "serialize_repeated_enum_as_string",  // The function that was originally highlighted
        "deserialize_repeated_enum_from_string",
        "serialize_enum_as_string",
        "deserialize_enum_from_string", 
        "serialize_option_enum_as_string",
        "deserialize_option_enum_from_string",
        "try_serialize_all_enums!",
        "try_parse_all_enums!",
    ];
    
    for deprecated_function in deprecated_functions {
        assert!(!generated_code.contains(deprecated_function),
            "Generated code should NOT contain deprecated generic function: {}", deprecated_function);
        println!("✅ Correctly does not contain deprecated function: {}", deprecated_function);
    }
    
    println!("✅ No generic enum serializers test passed!");
}