use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output();

    let git_hash = match output {
        Ok(o) if o.status.success() => {
            let hash = String::from_utf8(o.stdout)
                .unwrap_or_default()
                .trim()
                .to_owned();
            if hash.is_empty() { "unknown".to_owned() } else { hash }
        }
        _ => "unknown".to_owned(),
    };

    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");
}
