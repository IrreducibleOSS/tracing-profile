use git2::Repository;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

pub fn get_current_git_branch() -> Option<String> {
    let repo = Repository::discover(".").ok()?;
    let head = repo.head().ok()?;
    let branch = head.shorthand()?;
    Some(sanitize_filename(branch).to_string())
}

fn sanitize_filename(branch: &str) -> String {
    branch
        .chars()
        .map(|c| {
            match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | ' ' | '.' => '-',
                _ => c,
            }
        })
        .collect()
}
