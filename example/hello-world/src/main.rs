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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = hello_world::greeter_handler(Server);

    let sample_request = serde_json::json!({
        "name": "World",
        "greeting_type": "CASUAL"  // Test string enum support
    });

    println!("request: {}", sample_request);

    let request = http::Request::builder()
        .method("POST")
        .uri("/hello_world.Greeter/SayHello")
        .header("Content-Type", "application/json")
        .body(sample_request.to_string())?;

    let response = router.oneshot(request).await?;
    let status = response.status();

    assert_eq!(status, http::StatusCode::OK);

    let body: axum::body::Body = response.into_body();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await?;
    let json_body = serde_json::from_slice::<serde_json::Value>(&body_bytes)?;

    // With string enum support, should get "Hey World!"
    println!("Expected: Hey World!");
    println!("Actual: {}", json_body["message"]);

    println!("response: {}", json_body);

    Ok(())
}
