use std::process::Command;

fn main() {
    let commit_hash = match std::env::var("COMMIT_HASH") {
        Ok(commit_hash_string) => commit_hash_string,
        Err(_) => {
            match Command::new("git")
                .args(vec!["rev-parse", "--short", "HEAD"])
                .output()
            {
                Ok(x) => String::from_utf8_lossy(x.stdout.trim_ascii_end()).into_owned(),
                Err(_) => "no hash :(".to_string(),
            }
        }
    };
    println!("cargo::rustc-env=COMMIT_HASH=\"{}\"", commit_hash);
}
