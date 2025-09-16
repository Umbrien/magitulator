use gix::{
    ObjectId,
    bstr::BString,
    diff::object::commit::MessageRef,
    objs::Commit,
    refs::transaction::{Change, LogChange, RefEdit},
};
use std::collections::HashMap;
use std::path::Path;

mod copy_object_recursive;
use crate::copy_object_recursive::copy_object_recursive;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let src_repo_path = "repo-a";
    let dst_repo_path = "repo-b";

    let src_repo = gix::open(src_repo_path)?;

    let dst_repo = if Path::new(dst_repo_path).exists() {
        gix::open(dst_repo_path)?
    } else {
        gix::init(dst_repo_path)?
    };

    let head_id = src_repo.head()?.id().expect("Unborn head");

    let revwalk = src_repo.rev_walk([head_id]).all()?;

    let mut ids_to_process: Vec<ObjectId> = revwalk
        .map(|res| res.map(|info| info.id))
        .collect::<Result<_, _>>()?;

    ids_to_process.reverse();

    let mut parent_map: HashMap<ObjectId, ObjectId> = HashMap::new();
    let mut last_new_oid: Option<ObjectId> = None;

    for id in ids_to_process {
        println!("Processing commit: {id}");

        let commit = src_repo.find_commit(id)?;

        let commit_ref = commit.decode()?;

        let message_ref = commit.message().unwrap_or(MessageRef {
            title: "no message".into(),
            body: None,
        });

        let message = match message_ref.body {
            Some(body) => format!("{}\n\n{}", message_ref.title, body),
            None => message_ref.title.to_string(),
        };

        let src_tree_id = commit.tree_id()?;
        let new_tree_id = copy_object_recursive(&src_repo, &dst_repo, &src_tree_id)?;

        let author = commit.author()?;
        let committer = commit.committer()?;

        let new_parent_ids: Vec<ObjectId> = commit
            .parent_ids()
            .map(|parent_id| {
                *parent_map
                    .get(&parent_id.detach())
                    .expect("Parent must have been processed")
            })
            .collect();

        let new_commit = Commit {
            tree: new_tree_id,
            parents: new_parent_ids.into(),
            author: author.into(),
            committer: committer.into(),
            encoding: commit_ref.encoding.map(|s| s.into()),
            message: message.into(),
            extra_headers: commit_ref
                .extra_headers
                .into_iter()
                .map(|(k, v)| (k.into(), BString::from(v.as_ref())))
                .collect(),
        };

        let new_oid = dst_repo.write_object(&new_commit)?.into();

        parent_map.insert(id, new_oid);
        last_new_oid = Some(new_oid);
    }

    if let Some(final_oid) = last_new_oid {
        let head_ref_name = "refs/heads/main";
        dst_repo.edit_reference(RefEdit {
            change: Change::Update {
                log: LogChange::default(),
                expected: gix::refs::transaction::PreviousValue::Any,
                new: gix::refs::Target::Object(final_oid),
            },
            name: head_ref_name.try_into()?,
            deref: false,
        })?;
        println!("✅ Set {} to point to {}", head_ref_name, final_oid);
    }

    println!("✅ New repo created at {}", dst_repo_path);
    Ok(())
}
