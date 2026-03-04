use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output();

    let git_hash = match output {
        Ok(o) => String::from_utf8(o.stdout)
            .unwrap_or_else(|_| "unknown".to_owned())
            .trim()
            .to_owned(),
        Err(_) => "unknown".to_owned(),
    };

    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");
}
