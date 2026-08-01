fn main() {
    // Embed the requireAdministrator manifest into the production CLI binary.
    embed_resource::compile("resources/sysdll-cli.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed manifest");
}
