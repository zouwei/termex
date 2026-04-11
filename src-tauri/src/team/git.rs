use git2::{
    Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository,
    Signature,
};
use std::path::{Path, PathBuf};

use super::types::GitAuthConfig;

/// Result of a pull operation.
#[derive(Debug)]
pub enum PullResult {
    /// Already up to date.
    UpToDate,
    /// Fast-forwarded to remote HEAD.
    FastForward,
    /// Remote has diverged — cannot fast-forward.
    Conflict,
}

/// Repository status summary.
pub struct RepoStatus {
    pub has_changes: bool,
    pub changed_files: usize,
}

/// Git operation errors.
#[derive(Debug, thiserror::Error)]
pub enum TeamGitError {
    #[error("clone failed: {0}")]
    Clone(String),
    #[error("open failed: {0}")]
    Open(String),
    #[error("init failed: {0}")]
    Init(String),
    #[error("fetch failed: {0}")]
    Fetch(String),
    #[error("merge failed: {0}")]
    Merge(String),
    #[error("commit failed: {0}")]
    Commit(String),
    #[error("push failed: {0}")]
    Push(String),
}

/// Wrapper for team Git repository operations.
pub struct TeamRepo {
    path: PathBuf,
}

impl TeamRepo {
    /// Clones a remote repository to the local path.
    pub fn clone_repo(
        url: &str,
        path: &Path,
        auth: &GitAuthConfig,
    ) -> Result<Self, TeamGitError> {
        let mut callbacks = RemoteCallbacks::new();
        setup_credentials(&mut callbacks, auth);

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        builder
            .clone(url, path)
            .map_err(|e| TeamGitError::Clone(e.to_string()))?;

        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Opens an existing local repository.
    pub fn open(path: &Path) -> Result<Self, TeamGitError> {
        Repository::open(path).map_err(|e| TeamGitError::Open(e.to_string()))?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Initializes a new repository, adds remote, and pushes.
    pub fn init_and_push(
        path: &Path,
        url: &str,
        username: &str,
        auth: &GitAuthConfig,
    ) -> Result<Self, TeamGitError> {
        let repo = Repository::init(path).map_err(|e| TeamGitError::Init(e.to_string()))?;

        // Add remote
        repo.remote("origin", url)
            .map_err(|e| TeamGitError::Init(e.to_string()))?;

        // Stage all files
        let mut index = repo.index().map_err(|e| TeamGitError::Commit(e.to_string()))?;
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;
        index
            .write()
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;

        let oid = index
            .write_tree()
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;
        let tree = repo
            .find_tree(oid)
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;

        let sig = Signature::now(username, &format!("{username}@termex.local"))
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;

        repo.commit(Some("HEAD"), &sig, &sig, "init team", &tree, &[])
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;

        // Push
        let me = Self {
            path: path.to_path_buf(),
        };
        me.push(auth)?;
        Ok(me)
    }

    /// Returns the local repository path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Fetches from origin/main and fast-forwards if possible.
    pub fn pull(&self, auth: &GitAuthConfig) -> Result<PullResult, TeamGitError> {
        let repo = Repository::open(&self.path).map_err(|e| TeamGitError::Open(e.to_string()))?;

        // Fetch
        let mut callbacks = RemoteCallbacks::new();
        setup_credentials(&mut callbacks, auth);
        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| TeamGitError::Fetch(e.to_string()))?;
        remote
            .fetch(&["main"], Some(&mut fetch_opts), None)
            .map_err(|e| TeamGitError::Fetch(e.to_string()))?;

        // Check merge analysis
        let fetch_head = match repo.find_reference("FETCH_HEAD") {
            Ok(r) => r,
            Err(_) => return Ok(PullResult::UpToDate),
        };
        let fetch_commit = repo
            .reference_to_annotated_commit(&fetch_head)
            .map_err(|e| TeamGitError::Merge(e.to_string()))?;

        let (analysis, _) = repo
            .merge_analysis(&[&fetch_commit])
            .map_err(|e| TeamGitError::Merge(e.to_string()))?;

        if analysis.is_up_to_date() {
            return Ok(PullResult::UpToDate);
        }

        if analysis.is_fast_forward() {
            let refname = "refs/heads/main";
            let mut reference = repo
                .find_reference(refname)
                .map_err(|e| TeamGitError::Merge(e.to_string()))?;
            reference
                .set_target(fetch_commit.id(), "fast-forward")
                .map_err(|e| TeamGitError::Merge(e.to_string()))?;
            repo.set_head(refname)
                .map_err(|e| TeamGitError::Merge(e.to_string()))?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                .map_err(|e| TeamGitError::Merge(e.to_string()))?;
            return Ok(PullResult::FastForward);
        }

        Ok(PullResult::Conflict)
    }

    /// Stages all changes, commits, and pushes to origin/main.
    pub fn commit_and_push(
        &self,
        message: &str,
        username: &str,
        auth: &GitAuthConfig,
    ) -> Result<(), TeamGitError> {
        let repo = Repository::open(&self.path).map_err(|e| TeamGitError::Open(e.to_string()))?;

        // Stage all
        let mut index = repo.index().map_err(|e| TeamGitError::Commit(e.to_string()))?;
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;
        // Also stage deletions
        index
            .update_all(["*"].iter(), None)
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;
        index
            .write()
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;

        let oid = index
            .write_tree()
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;
        let tree = repo
            .find_tree(oid)
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;

        let sig = Signature::now(username, &format!("{username}@termex.local"))
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;

        let parent = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .map_err(|e| TeamGitError::Commit(e.to_string()))?;

        self.push(auth)
    }

    /// Pushes refs/heads/main to origin.
    fn push(&self, auth: &GitAuthConfig) -> Result<(), TeamGitError> {
        let repo = Repository::open(&self.path).map_err(|e| TeamGitError::Open(e.to_string()))?;

        let mut callbacks = RemoteCallbacks::new();
        setup_credentials(&mut callbacks, auth);
        let mut push_opts = PushOptions::new();
        push_opts.remote_callbacks(callbacks);

        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| TeamGitError::Push(e.to_string()))?;
        remote
            .push(&["refs/heads/main"], Some(&mut push_opts))
            .map_err(|e| TeamGitError::Push(e.to_string()))?;

        Ok(())
    }

    /// Returns the current repository status.
    pub fn status(&self) -> Result<RepoStatus, TeamGitError> {
        let repo = Repository::open(&self.path).map_err(|e| TeamGitError::Open(e.to_string()))?;
        let statuses = repo
            .statuses(None)
            .map_err(|e| TeamGitError::Open(e.to_string()))?;

        Ok(RepoStatus {
            has_changes: !statuses.is_empty(),
            changed_files: statuses.len(),
        })
    }
}

/// Sets up git2 authentication callbacks from GitAuthConfig.
fn setup_credentials(callbacks: &mut RemoteCallbacks, auth: &GitAuthConfig) {
    let auth = auth.clone();
    callbacks.credentials(move |_url, username_from_url, _allowed| match auth.auth_type.as_str() {
        "ssh_key" => {
            let key_path = auth
                .ssh_key_path
                .as_deref()
                .unwrap_or("~/.ssh/id_ed25519");
            Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                Path::new(key_path),
                auth.ssh_passphrase.as_deref(),
            )
        }
        "https_token" => {
            let token = auth.token.as_deref().unwrap_or("");
            Cred::userpass_plaintext(token, "")
        }
        "https_userpass" => {
            let user = auth.username.as_deref().unwrap_or("");
            let pass = auth.password.as_deref().unwrap_or("");
            Cred::userpass_plaintext(user, pass)
        }
        _ => Cred::default(),
    });
}
