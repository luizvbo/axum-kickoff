// Replaces the inert `.github/workflows/ci.yml` stub (which exists only for
// the template repository, where the checkout cannot compile) with the real
// generated-project CI shipped verbatim at `.github/ci.yml`, so rendered
// projects get a clean workflow with no template-detection machinery.
file::delete(".github/workflows/ci.yml");
file::rename(".github/ci.yml", ".github/workflows/ci.yml");

// The hook file itself is not part of a generated project.
file::delete("post.rhai");
