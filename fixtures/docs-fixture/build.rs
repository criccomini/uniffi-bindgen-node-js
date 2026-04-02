fn main() {
    uniffi::generate_scaffolding("src/docs_fixture.udl")
        .expect("UDL scaffolding should generate");
}
