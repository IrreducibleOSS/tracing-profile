use chrono::Local;
use git2::Repository;

pub fn get_formatted_time() -> String {
    Local::now().format("%Y_%m_%d_%H_%M").to_string()
}
pub fn get_current_branch_revision() -> Option<String> {
    let repo = Repository::discover(".").ok()?;
    let head = repo.head().ok()?;
    let branch = head.shorthand()?;
    let commit = head.peel_to_commit().ok()?;
    let short_hash = &commit.id().to_string()[..7];

    Some(format!("{}_{}", sanitize_filename(branch), short_hash))
}

pub fn sanitize_filename(branch: &str) -> String {
    branch
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | ' ' | '.' => '-',
            _ => c,
        })
        .collect()
}
