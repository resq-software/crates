// No copyright header here either.

/// Configuration loader with some planted fake secrets.
/// Run `resq secrets` from this directory to detect them.

// FAKE credentials — these are intentionally planted for the demo.
// They are not real keys and cannot be used for anything.
const AWS_ACCESS_KEY: &str = "AKIAIOSFODNN7EXAMPLE";
const DATABASE_URL: &str = "postgres://admin:SuperSecret123@db.example.com:5432/myapp";
const GITHUB_TOKEN: &str = "ghp_FAKE_EXAMPLE_TOKEN_00000000000000000";
const SLACK_WEBHOOK: &str = "https://hooks.example.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX";

pub fn get_database_url() -> &'static str {
    DATABASE_URL
}

pub fn get_aws_key() -> &'static str {
    AWS_ACCESS_KEY
}
