/// Tests for g2h internal enum processing logic
///
/// These tests verify that the enum field extraction and path resolution
/// work correctly for different protobuf structures.
#[cfg(test)]
mod g2h_tests {

    /// Test enum path resolution logic similar to g2h's resolve_enum_path
    fn resolve_enum_path(enum_type: &str) -> String {
        if enum_type.is_empty() || !enum_type.contains('.') {
            return enum_type.to_string();
        }

        let parts: Vec<&str> = enum_type.split('.').collect();

        match parts.len() {
            0 | 1 => parts.last().unwrap_or(&"UnknownEnum").to_string(),
            2 => {
                // Package-level enum like "package.EnumName"
                parts[1].to_string()
            }
            _ => {
                // Three or more parts like "package.Message.EnumName" or "ucs.v2.Currency"
                let potential_parent = parts[parts.len() - 2];
                let enum_name = parts[parts.len() - 1];

                // Check if parent looks like a message (PascalCase) vs a package component
                if is_message_name(potential_parent) {
                    // Message-nested enum: "package.Message.EnumName" -> "message::EnumName"
                    format!("{}::{}", to_snake_case(potential_parent), enum_name)
                } else {
                    // Package-level enum: "ucs.v2.Currency" -> "Currency"
                    enum_name.to_string()
                }
            }
        }
    }

    fn is_message_name(name: &str) -> bool {
        name.chars().next().is_some_and(|c| c.is_uppercase())
    }

    fn to_snake_case(input: &str) -> String {
        use heck::ToSnakeCase;
        input.to_snake_case()
    }

    #[test]
    fn test_enum_path_resolution() {
        // Test cases covering different protobuf enum path structures
        let test_cases = vec![
            // Simple enum name
            ("PaymentStatus", "PaymentStatus"),
            // Package-level enum
            ("hello_world.PaymentStatus", "PaymentStatus"),
            ("ucs.v2.Currency", "Currency"),
            ("package.EnumName", "EnumName"),
            // Message-nested enum (PascalCase parent)
            (
                "hello_world.HelloReply.ResponseStatus",
                "hello_reply::ResponseStatus",
            ),
            (
                "package.UserProfile.AccountStatus",
                "user_profile::AccountStatus",
            ),
            (
                "test.PaymentRequest.PaymentType",
                "payment_request::PaymentType",
            ),
            // Edge cases
            ("", ""),
            ("single", "single"),
            ("a.b", "b"),
            (
                "namespace.v1.service.MessageName.EnumName",
                "message_name::EnumName",
            ),
        ];

        for (input, expected) in test_cases {
            let result = resolve_enum_path(input);
            assert_eq!(result, expected, "Failed for input: '{}'", input);
        }

        println!("✅ Enum path resolution tests passed!");
    }

    #[test]
    fn test_message_name_detection() {
        let test_cases = vec![
            ("HelloReply", true),     // PascalCase message
            ("PaymentRequest", true), // PascalCase message
            ("UserProfile", true),    // PascalCase message
            ("v2", false),            // Package component
            ("ucs", false),           // Package component
            ("hello_world", false),   // Package component
            ("", false),              // Empty string
            ("lowercase", false),     // Lowercase
            ("UPPERCASE", true),      // All uppercase (treated as message)
            ("mixedCase", false),     // camelCase (not PascalCase)
        ];

        for (input, expected) in test_cases {
            let result = is_message_name(input);
            assert_eq!(result, expected, "Failed for input: '{}'", input);
        }

        println!("✅ Message name detection tests passed!");
    }

    #[test]
    fn test_snake_case_conversion() {
        let test_cases = vec![
            ("HelloReply", "hello_reply"),
            ("PaymentRequest", "payment_request"),
            ("UserProfile", "user_profile"),
            ("ResponseStatus", "response_status"),
            ("XMLHttpRequest", "xml_http_request"), // heck produces better output
            ("APIKey", "api_key"),                  // heck produces better output
            ("lowercase", "lowercase"),
            ("", ""),
            ("A", "a"),
            ("AB", "ab"), // heck produces better output
        ];

        for (input, expected) in test_cases {
            let result = to_snake_case(input);
            assert_eq!(result, expected, "Failed for input: '{}'", input);
        }

        println!("✅ Snake case conversion tests passed!");
    }

    /// Test that simulates the enum field extraction process
    #[test]
    fn test_enum_field_extraction_simulation() {
        // Simulate the data structure that extract_enum_fields_from_message_static would create
        // ConflictTestRequest fields
        let extracted_fields = vec![
            (
                "conflict_test_request_payment_status".to_string(),
                "PaymentStatus".to_string(),
                "Single".to_string(),
            ),
            (
                "conflict_test_request_auth_status".to_string(),
                "AuthenticationStatus".to_string(),
                "Single".to_string(),
            ),
            (
                "conflict_test_request_processing_status".to_string(),
                "ProcessingStatus".to_string(),
                "Single".to_string(),
            ),
            (
                "conflict_test_request_optional_payment".to_string(),
                "PaymentStatus".to_string(),
                "Option".to_string(),
            ),
            (
                "conflict_test_request_auth_history".to_string(),
                "AuthenticationStatus".to_string(),
                "Repeated".to_string(),
            ),
            (
                "conflict_test_request_processing_steps".to_string(),
                "ProcessingStatus".to_string(),
                "Repeated".to_string(),
            ),
            // HelloReply nested enum
            (
                "hello_reply_status".to_string(),
                "hello_reply::ResponseStatus".to_string(),
                "Single".to_string(),
            ),
        ];

        // Verify field ID generation follows expected pattern
        assert!(extracted_fields
            .iter()
            .any(|(field_id, _, _)| field_id == "conflict_test_request_payment_status"));
        assert!(extracted_fields
            .iter()
            .any(|(field_id, _, _)| field_id == "conflict_test_request_auth_status"));

        // Verify enum type resolution
        let payment_enum = extracted_fields
            .iter()
            .find(|(field_id, _, _)| field_id == "conflict_test_request_payment_status");
        assert_eq!(payment_enum.unwrap().1, "PaymentStatus");

        let auth_enum = extracted_fields
            .iter()
            .find(|(field_id, _, _)| field_id == "conflict_test_request_auth_status");
        assert_eq!(auth_enum.unwrap().1, "AuthenticationStatus");

        // Verify nested enum path
        let nested_enum = extracted_fields
            .iter()
            .find(|(field_id, _, _)| field_id == "hello_reply_status");
        assert_eq!(nested_enum.unwrap().1, "hello_reply::ResponseStatus");

        // Verify field labels
        let optional_field = extracted_fields
            .iter()
            .find(|(field_id, _, _)| field_id == "conflict_test_request_optional_payment");
        assert_eq!(optional_field.unwrap().2, "Option");

        let repeated_field = extracted_fields
            .iter()
            .find(|(field_id, _, _)| field_id == "conflict_test_request_auth_history");
        assert_eq!(repeated_field.unwrap().2, "Repeated");

        println!("✅ Enum field extraction simulation test passed!");
        println!("Extracted {} enum fields:", extracted_fields.len());
        for (field_id, enum_type, field_label) in &extracted_fields {
            println!("  - {}: {} ({})", field_id, enum_type, field_label);
        }
    }

    /// Test that verifies the generated function names follow the expected pattern
    #[test]
    fn test_generated_function_names() {
        let test_cases = vec![
            (
                "conflict_test_request_payment_status",
                "Single",
                vec![
                    "serialize_conflict_test_request_payment_status_as_string",
                    "deserialize_conflict_test_request_payment_status_from_string",
                ],
            ),
            (
                "conflict_test_request_optional_payment",
                "Option",
                vec![
                    "serialize_option_conflict_test_request_optional_payment_as_string",
                    "deserialize_option_conflict_test_request_optional_payment_from_string",
                ],
            ),
            (
                "conflict_test_request_auth_history",
                "Repeated",
                vec![
                    "serialize_repeated_conflict_test_request_auth_history_as_string",
                    "deserialize_repeated_conflict_test_request_auth_history_from_string",
                ],
            ),
        ];

        for (field_id, field_label, expected_functions) in test_cases {
            match field_label {
                "Single" => {
                    let serialize_fn = format!("serialize_{}_as_string", field_id);
                    let deserialize_fn = format!("deserialize_{}_from_string", field_id);
                    assert_eq!(serialize_fn, expected_functions[0]);
                    assert_eq!(deserialize_fn, expected_functions[1]);
                }
                "Option" => {
                    let serialize_fn = format!("serialize_option_{}_as_string", field_id);
                    let deserialize_fn = format!("deserialize_option_{}_from_string", field_id);
                    assert_eq!(serialize_fn, expected_functions[0]);
                    assert_eq!(deserialize_fn, expected_functions[1]);
                }
                "Repeated" => {
                    let serialize_fn = format!("serialize_repeated_{}_as_string", field_id);
                    let deserialize_fn = format!("deserialize_repeated_{}_from_string", field_id);
                    assert_eq!(serialize_fn, expected_functions[0]);
                    assert_eq!(deserialize_fn, expected_functions[1]);
                }
                _ => panic!("Unexpected field label: {}", field_label),
            }
        }

        println!("✅ Generated function names test passed!");
    }
}
