mod pb;

use pb::{hello_service_server::HelloServiceServer, HelloReply};
use tonic::{transport::Server, Request, Response, Status};

pub async fn run() {
    let hello_service = HelloService {};
    let addr = "".parse().unwrap();
    Server::builder()
        .add_service(HelloServiceServer::new(hello_service))
        .serve(addr)
        .await
        .unwrap();
}

struct HelloService {}

#[tonic::async_trait]
impl pb::hello_service_server::HelloService for HelloService {
    async fn hello(
        &self,
        request: Request<pb::HelloMessage>,
    ) -> Result<Response<HelloReply>, Status> {
        let msg = request.into_inner();
        let name = msg.name;
        let greeting = format!("hello {}", name);
        let reply = HelloReply { greeting };
        let response = Response::new(reply);
        Ok(response)
    }
}
