use gix::ObjectId;
use gix::prelude::Write;
use gix::{Repository, object::Kind, oid};

use gix::object::find;
use gix::object::write;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CopyError {
    #[error("failed to find object: {0}")]
    FindObject(#[from] find::existing::Error),

    #[error("failed to write blob: {0}")]
    WriteBlob(#[from] write::Error),

    #[error("failed to find tree: {0}")]
    FindTree(#[from] find::existing::with_conversion::Error),

    #[error("failed to decode object: {0}")]
    Decode(#[from] gix::diff::object::decode::Error),

    #[error("failed with write buf error: {0}")]
    WriteBuf(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub fn copy_object_recursive<'src, 'dst>(
    src_repo: &'src Repository,
    dst_repo: &'dst Repository,
    oid: &oid,
) -> Result<ObjectId, CopyError>
where
    'src: 'dst,
{
    if dst_repo.has_object(oid) {
        let existing = dst_repo.find_object(oid)?;
        return Ok(existing.id().into());
    }

    let obj = src_repo.find_object(oid)?;

    match obj.kind {
        Kind::Blob => {
            let data = obj.data.clone();
            let new_id = dst_repo.write_blob(data)?;
            Ok(new_id.into())
        }
        Kind::Tree => {
            let tree = src_repo.find_tree(oid)?;

            for entry_res in tree.iter() {
                let entry = entry_res?;
                let child_oid = entry.id();
                copy_object_recursive(src_repo, dst_repo, &child_oid)?;
            }

            let tree_data = obj.data.clone();
            let new_id = dst_repo.write_buf(Kind::Tree, &tree_data)?;
            Ok(new_id.into())
        }
        Kind::Commit | Kind::Tag => {
            // commits/tags handled at higher level
            Ok(obj.id().into())
        }
    }
}
