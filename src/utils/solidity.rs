use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::tempdir;

use semver::Version;

use crate::error::SoflError;

/// Get native path to solc binary
/// This method uses svm-rs internally to download solc binary
/// The binary is downloaded to [dirs::data_dir().join("svm")]
pub fn get_solc(version: &Version) -> Result<PathBuf, SoflError> {
    let installed = svm_lib::installed_versions().map_err(SoflError::SolcVM)?;
    if !installed.contains(version) {
        svm_lib::blocking_install(version).map_err(SoflError::SolcVM)
    } else {
        let path = svm_lib::version_path(version.to_string().as_str());
        Ok(path)
    }
}

/// Generate a temporary file with the given `code`
/// The file will be automatically deleted when the return value is dropped
pub fn gen_tmp_code_file(code: &str) -> PathBuf {
    let dir = tempdir().unwrap();
    let path = dir.path().join("code.sol");
    let mut file = File::create(&path).unwrap();
    file.write_all(code.as_bytes()).unwrap();
    path
}
