use super::*;

/// Initialize a new ngdar repository in the current directory.
pub fn init() -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    if cwd.join(crate::config::NGDAR_DIR).exists() {
        return Err(NgdarError::Config("Repository already exists".into()));
    }
    let repo = Repository::init(&cwd)?;
    println!(
        "Initialized empty ngdar repository in {}",
        cwd.join(crate::config::NGDAR_DIR).display()
    );
    println!("Repository ID: {}", repo.repo_id);
    Ok(())
}
