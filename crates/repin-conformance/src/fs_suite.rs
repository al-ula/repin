use repin_fs::{CapabilityFs, FsError};
use std::path::Path;

pub fn test_fs_containment_conformance(root: &Path) -> Result<(), FsError> {
    let fs = CapabilityFs::open("root", root)?;
    assert_eq!(fs.root_id(), "root");
    Ok(())
}
