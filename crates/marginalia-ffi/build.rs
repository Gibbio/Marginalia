fn main() {
    uniffi::generate_scaffolding("src/marginalia.udl").expect("uniffi scaffolding");
}
