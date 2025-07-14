use tower::ServiceExt;

mod hello_world {
    tonic::include_proto!("hello_world");
}

struct Server;

#[tonic::async_trait]
impl hello_world::greeter_server::Greeter for Server {
    async fn say_hello(
        &self,
        request: tonic::Request<hello_world::HelloRequest>,
    ) -> Result<tonic::Response<hello_world::HelloReply>, tonic::Status> {
        let req = request.into_inner();
        let greeting_type = req.greeting_type;

        let greeting = match greeting_type {
            0 => "Good day", // FORMAL
            1 => "Hey",      // CASUAL
            2 => "Hi there", // FRIENDLY
            _ => "Hello",    // default
        };

        let reply = hello_world::HelloReply {
            message: format!("{} {}!", greeting, req.name),
            status: 0, // SUCCESS
        };
        Ok(tonic::Response::new(reply))
    }
}

#[tonic::async_trait]
impl hello_world::payment_connector_server::PaymentConnector for Server {
    async fn process_payment(
        &self,
        request: tonic::Request<hello_world::PaymentRequest>,
    ) -> Result<tonic::Response<hello_world::PaymentResponse>, tonic::Status> {
        let req = request.into_inner();

        // Simulate payment processing with error case
        let response = if req.receipt == "duplicate_receipt" {
            hello_world::PaymentResponse {
                transaction_id: "".to_string(),
                status: hello_world::PaymentStatus::BadRequestError.into(),
                error_code: "BAD_REQUEST_ERROR".to_string(),
                error_message: "Order receipt should be unique.".to_string(),
                redirection_data: "".to_string(),
                network_txn_id: "".to_string(),
                response_ref_id: "".to_string(),
                incremental_authorization_allowed: false,
                raw_connector_response: r#"{"error":{"code":"BAD_REQUEST_ERROR","description":"Order receipt should be unique."}}"#.to_string(),
                error_detail: Some(hello_world::ErrorDetail {
                    code: "BAD_REQUEST_ERROR".to_string(),
                    description: "Order receipt should be unique.".to_string(),
                    step: "payment_initiation".to_string(),
                    reason: "input_validation_failed".to_string(),
                    source: "business".to_string(),
                    metadata: [("order_id".to_string(), req.order_id.clone())].into(),
                }),
            }
        } else {
            hello_world::PaymentResponse {
                transaction_id: "txn_123456".to_string(),
                status: hello_world::PaymentStatus::Success.into(),
                error_code: "".to_string(),
                error_message: "".to_string(),
                redirection_data: "https://payment.example.com/redirect".to_string(),
                network_txn_id: "net_789".to_string(),
                response_ref_id: "ref_456".to_string(),
                incremental_authorization_allowed: true,
                raw_connector_response: r#"{"status":"success","transaction_id":"txn_123456"}"#
                    .to_string(),
                error_detail: None,
            }
        };

        Ok(tonic::Response::new(response))
    }

    async fn get_payment_status(
        &self,
        request: tonic::Request<hello_world::StatusRequest>,
    ) -> Result<tonic::Response<hello_world::PaymentResponse>, tonic::Status> {
        let _req = request.into_inner();

        let response = hello_world::PaymentResponse {
            transaction_id: "txn_123456".to_string(),
            status: hello_world::PaymentStatus::Pending.into(),
            error_code: "".to_string(),
            error_message: "".to_string(),
            redirection_data: "".to_string(),
            network_txn_id: "net_789".to_string(),
            response_ref_id: "ref_456".to_string(),
            incremental_authorization_allowed: false,
            raw_connector_response: r#"{"status":"pending"}"#.to_string(),
            error_detail: None,
        };

        Ok(tonic::Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Combine both service handlers
    let greeter_router = hello_world::greeter_handler(Server);
    let payment_router = hello_world::payment_connector_handler(Server);

    // Merge the routers
    let combined_router = greeter_router.merge(payment_router);

    println!("=== G2H Example: String Enums + Null Skipping ===\n");

    // Test 1: Greeting Service with String Enums
    println!("🔸 Testing Greeting Service with String Enum Support:");
    let greeting_request = serde_json::json!({
        "name": "World",
        "greeting_type": "CASUAL"  // String enum input
    });

    println!("Request: {}", greeting_request);

    let request = http::Request::builder()
        .method("POST")
        .uri("/hello_world.Greeter/SayHello")
        .header("Content-Type", "application/json")
        .body(greeting_request.to_string())?;

    let response = combined_router.clone().oneshot(request).await?;
    let body: axum::body::Body = response.into_body();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await?;
    let json_body = serde_json::from_slice::<serde_json::Value>(&body_bytes)?;

    println!("Response: {}", serde_json::to_string_pretty(&json_body)?);
    println!("✅ String enum 'CASUAL' accepted, got casual greeting\n");

    // Test 2: Payment Service - Error Case (demonstrates null skipping)
    println!("🔸 Testing Payment Service - Error Case:");
    let error_payment_request = serde_json::json!({
        "order_id": "order_123",
        "receipt": "duplicate_receipt",  // Triggers error
        "amount": 100.50,
        "currency": "USD",
        "customer_id": "cust_123",
        "payment_method": "card"
    });

    println!("Request: {}", error_payment_request);

    let request2 = http::Request::builder()
        .method("POST")
        .uri("/hello_world.PaymentConnector/ProcessPayment")
        .header("Content-Type", "application/json")
        .body(error_payment_request.to_string())?;

    let response2 = combined_router.clone().oneshot(request2).await?;
    let body2: axum::body::Body = response2.into_body();
    let body_bytes2 = axum::body::to_bytes(body2, usize::MAX).await?;
    let json_body2 = serde_json::from_slice::<serde_json::Value>(&body_bytes2)?;

    println!("Response: {}", serde_json::to_string_pretty(&json_body2)?);
    println!("✅ Status shows as 'BAD_REQUEST_ERROR' (string), empty fields omitted\n");

    // Test 3: Payment Service - Success Case
    println!("🔸 Testing Payment Service - Success Case:");
    let success_payment_request = serde_json::json!({
        "order_id": "order_456",
        "receipt": "unique_receipt",
        "amount": 250.75,
        "currency": "USD",
        "customer_id": "cust_456",
        "payment_method": "upi"
    });

    println!("Request: {}", success_payment_request);

    let request3 = http::Request::builder()
        .method("POST")
        .uri("/hello_world.PaymentConnector/ProcessPayment")
        .header("Content-Type", "application/json")
        .body(success_payment_request.to_string())?;

    let response3 = combined_router.oneshot(request3).await?;
    let body3: axum::body::Body = response3.into_body();
    let body_bytes3 = axum::body::to_bytes(body3, usize::MAX).await?;
    let json_body3 = serde_json::from_slice::<serde_json::Value>(&body_bytes3)?;

    println!("Response: {}", serde_json::to_string_pretty(&json_body3)?);
    println!("✅ Status shows as 'SUCCESS' (string), empty fields omitted\n");

    println!("🎯 Key Features Demonstrated:");
    println!("• String enum serialization: 'BAD_REQUEST_ERROR' instead of 21");
    println!("• String enum deserialization: 'CASUAL' accepted as input");
    println!("• Null field skipping: error_detail omitted when None");
    println!("• Empty string skipping: empty fields like error_code omitted");
    println!("• Clean JSON output: only meaningful data included");

    Ok(())
}
