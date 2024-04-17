fn main() {
    tonic_build::compile_protos("proto/belanda.proto").unwrap();
}
