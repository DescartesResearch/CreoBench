use std::future::Future;
use std::path::{Path, PathBuf};

use crate::config::FromBytes;

use super::{ConfigError, OutputError};

/// A type that can be loaded asynchronously from a file.
pub trait FromFile: Sized {
    fn from_file(
        path: impl AsRef<Path> + Send,
    ) -> impl Future<Output = Result<Self, ConfigError>> + Send;
}

impl<T: FromBytes> FromFile for T {
    async fn from_file(path: impl AsRef<Path> + Send) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|source| ConfigError::FileOpen {
                path: path.to_path_buf(),
                source,
            })?;
        T::from_bytes(&bytes).map_err(|source| ConfigError::ParseError {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// A validated output directory for result files.
///
/// New files can be created using [`OutputDir::create_file`].
#[derive(Debug)]
pub struct OutputDir {
    path: PathBuf,
}

impl OutputDir {
    /// Creates a new [`OutputDir`] for the current load test.
    ///
    /// * If the directory at `path` does not exist, it is created.
    /// * If the directory already exists, it must be empty.
    /// * If `overwrite` is `true`, the empty-directory check is bypassed,
    ///   allowing existing contents to be overwritten.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory exists and is not empty (when `overwrite`
    /// is `false`), or if the directory cannot be created.
    pub async fn new(path: PathBuf, overwrite: bool) -> Result<Self, OutputError> {
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::create_dir_all(&path).await.map_err(|source| {
                OutputError::CreateOutputDir {
                    path: path.clone(),
                    source,
                }
            })?;
            return Ok(Self { path });
        }

        let is_dir = tokio::fs::metadata(&path)
            .await
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false);
        if !is_dir {
            return Err(OutputError::NotADirectory(path));
        }

        if !overwrite {
            let mut entries =
                tokio::fs::read_dir(&path)
                    .await
                    .map_err(|source| OutputError::DirectoryRead {
                        path: path.clone(),
                        source,
                    })?;
            if entries
                .next_entry()
                .await
                .map_err(|source| OutputError::DirectoryRead {
                    path: path.clone(),
                    source,
                })?
                .is_some()
            {
                return Err(OutputError::NotEmpty(path));
            }
        }

        Ok(Self { path })
    }

    /// Creates and opens a file with the given `name` relative to this directory.
    ///
    /// The file is opened in write-only mode. If a file with the specified name already
    /// exists, its contents will be truncated.
    ///
    /// # Errors
    ///
    /// Returns a [`std::io::Error`] if the file cannot be created (for example, due to
    /// insufficient permissions or invalid path components).
    pub fn create_file(&self, name: &'static str) -> std::io::Result<std::fs::File> {
        let path = self.path.join(name);
        std::fs::File::create(&path)
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn new_creates_directory_when_missing() {
        let temp_dir = tempdir().unwrap();
        let target_path = temp_dir.path().join("nested_missing");

        OutputDir::new(target_path.clone(), false).await.unwrap();

        assert!(target_path.is_dir());
    }

    #[tokio::test]
    async fn new_creates_nested_missing_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested_path = temp_dir.path().join("level1").join("level2");

        let result = OutputDir::new(nested_path.clone(), false).await;

        assert!(result.is_ok());
        assert!(nested_path.is_dir());
    }

    #[tokio::test]
    async fn new_succeeds_when_directory_is_empty() {
        let temp_dir = tempdir().unwrap();

        let result = OutputDir::new(temp_dir.path().to_path_buf(), false).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn new_fails_on_non_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();
        std::fs::write(path.join("stale.txt"), "stale").unwrap();

        let result = OutputDir::new(path.clone(), false).await;

        assert_matches!(result, Err(OutputError::NotEmpty(ref p)) if p == &path);
    }

    #[tokio::test]
    async fn new_succeeds_on_non_empty_directory_with_overwrite() {
        let temp_dir = tempdir().unwrap();
        std::fs::write(temp_dir.path().join("stale.txt"), "stale").unwrap();

        let result = OutputDir::new(temp_dir.path().to_path_buf(), true).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn new_fails_when_path_is_not_a_directory() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_path_buf();

        let result = OutputDir::new(file_path.clone(), false).await;

        assert_matches!(result, Err(OutputError::NotADirectory(p)) if p == file_path);
    }

    #[tokio::test]
    async fn new_fails_when_path_is_not_a_directory_with_overwrite() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_path_buf();

        let result = OutputDir::new(file_path.clone(), true).await;

        assert_matches!(result, Err(OutputError::NotADirectory(p)) if p == file_path);
    }

    #[tokio::test]
    async fn new_fails_on_invalid_path() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let invalid_dir_path = temp_file.path().join("sub_directory");

        let result = OutputDir::new(invalid_dir_path.clone(), false).await;

        assert_matches!(
            result,
            Err(OutputError::CreateOutputDir { path, .. }) if path == invalid_dir_path
        );
    }

    #[tokio::test]
    async fn new_fails_on_invalid_path_with_overwrite() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let invalid_dir_path = temp_file.path().join("sub_directory");

        let result = OutputDir::new(invalid_dir_path.clone(), true).await;

        assert_matches!(
            result,
            Err(OutputError::CreateOutputDir { path, .. }) if path == invalid_dir_path
        );
    }

    #[tokio::test]
    async fn create_file_creates_file_in_output_directory() {
        let temp_dir = tempdir().unwrap();
        let output_dir = OutputDir::new(temp_dir.path().to_path_buf(), false)
            .await
            .unwrap();

        let file = output_dir.create_file("results.csv").unwrap();

        assert!(file.metadata().unwrap().is_file());
        assert!(temp_dir.path().join("results.csv").is_file());
    }
}
