use gix::{
    ObjectId,
    bstr::BString,
    objs::Commit,
    refs::transaction::{Change, LogChange, RefEdit},
};
use std::{collections::HashMap, env};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- 1. Setup: Read args and open the repository ---
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <branch-name>", args[0]);
        std::process::exit(1);
    }
    let branch_name = &args[1];
    let repo = gix::open(".")?;

    // --- 2. Topological traversal ---
    let head_id = repo.head_id()?;

    let topo = gix::traverse::commit::topo::Builder::from_iters(
        &repo.objects,
        // [head_commit_id],
        [head_id],
        None::<Vec<gix::ObjectId>>,
    )
    .build()?;

    let mut ids_to_process: Vec<ObjectId> = Vec::new();
    for info in topo {
        let info = info?;
        ids_to_process.push(info.id);
    }
    ids_to_process.reverse();

    // --- 3. Rewrite commits one by one ---
    let mut parent_map: HashMap<ObjectId, ObjectId> = HashMap::new();
    let mut last_new_oid: Option<ObjectId> = None;

    for old_id in ids_to_process {
        let old_commit = repo.find_object(old_id)?.try_into_commit()?;
        let old_commit_ref = old_commit.decode()?;

        // Since we're in the same repo, we don't need to copy the tree.
        // We just point our new commit to the *exact same tree*.
        let tree_id = old_commit.tree_id()?;

        // Map old parent IDs to their new, rewritten counterparts.
        let new_parent_ids: Vec<ObjectId> = old_commit
            .parent_ids()
            .map(|parent_id| {
                *parent_map
                    .get(&parent_id.detach())
                    .expect("Parent must have been processed and mapped already")
            })
            .collect();

        // --- !! This is where you'd implement your modification logic !! ---
        let mut author = old_commit.author()?;
        author.name = "Dr. Magitulator".into();

        let committer = old_commit.committer()?;
        // Example: Add one day to the commit time
        // committer.time.seconds += 86400;

        // Create a new commit object with the modified data.
        let new_commit = Commit {
            tree: tree_id.detach(),
            parents: new_parent_ids.into(),
            author: author.into(),
            committer: committer.into(),
            encoding: old_commit_ref.encoding.map(|s| s.into()),
            message: old_commit_ref.message.into(),
            extra_headers: old_commit_ref
                .extra_headers
                .into_iter()
                .map(|(k, v)| (k.into(), BString::from(v.as_ref())))
                .collect(),
        };

        let new_oid = repo.write_object(&new_commit)?.into();

        parent_map.insert(old_id, new_oid);
        last_new_oid = Some(new_oid);
    }

    // --- 4. Create the new branch pointing to the final rewritten commit ---
    let new_branch_name = format!("{}-magitied", branch_name);
    let full_ref_name = format!("refs/heads/{}", new_branch_name);

    if let Some(final_oid) = last_new_oid {
        repo.edit_reference(RefEdit {
            change: Change::Update {
                log: LogChange::default(),
                expected: gix::refs::transaction::PreviousValue::Any,
                new: gix::refs::Target::Object(final_oid),
            },
            name: full_ref_name.clone().try_into().map_err(|e| {
                format!(
                    "Failed to create valid reference name '{}': {}",
                    full_ref_name, e
                )
            })?,
            deref: false,
        })
        .map_err(|e| format!("Failed to edit reference '{}': {}", full_ref_name, e))?;
    }

    Ok(())
}
