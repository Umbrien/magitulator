use clap::Parser;
use gix::{
    ObjectId, Repository,
    bstr::BString,
    objs::Commit,
    refs::transaction::{Change, LogChange, RefEdit},
};
use std::collections::{HashMap, HashSet};

type Anyhow = Box<dyn std::error::Error>;
type AnyResult<T> = Result<T, Anyhow>;

const BRANCH_POSTFIX: &'static str = "-magitied";

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Starting object for the rewrite.
    /// If same as `target`, will start from the bottom, rewriting each commit
    base: String,

    /// Target object (branch name / commit hash) to rewrite
    target: String,
}

fn main() -> AnyResult<()> {
    let args = Args::parse();
    let repo = gix::open(".")?;

    let base_commit_id = resolve_commit_id(&repo, &args.base)?;
    let target_commit_id = resolve_commit_id(&repo, &args.target)?;

    let commits_to_rewrite = get_commits_to_rewrite(&repo, base_commit_id, target_commit_id)?;
    if commits_to_rewrite.is_empty() {
        return Ok(());
    }

    let last_new_oid = rewrite_commits(&repo, commits_to_rewrite)?;

    match last_new_oid {
        Some(final_oid) => {
            create_branch(&repo, &args.target, final_oid)?;
        }
        None => {
            return Err("No commits were processed".into());
        }
    }

    Ok(())
}

fn resolve_commit_id(repo: &Repository, object_ref: &str) -> AnyResult<ObjectId> {
    Ok(repo
        .rev_parse_single(object_ref)?
        .object()?
        .peel_to_commit()?
        .id)
}

fn get_commits_to_rewrite(
    repo: &Repository,
    base_id: ObjectId,
    target_id: ObjectId,
) -> AnyResult<Vec<ObjectId>> {
    // --- 1. Get all commits in the base history to exclude them ---
    let mut base_commits = HashSet::new();
    if base_id != target_id {
        let base_topo = gix::traverse::commit::topo::Builder::from_iters(
            &repo.objects,
            [base_id],
            None::<Vec<ObjectId>>,
        )
        .build()?;

        for info in base_topo {
            let info = info?;
            base_commits.insert(info.id);
        }
    }

    // --- 2. Get all commits in the target's history ---
    let branch_topo = gix::traverse::commit::topo::Builder::from_iters(
        &repo.objects,
        [target_id],
        None::<Vec<ObjectId>>,
    )
    .build()?;

    // --- 3. Filter to get only commits to be rewritten ---
    let mut commits_to_rewrite = Vec::new();
    for info in branch_topo {
        let info = info?;
        if !base_commits.contains(&info.id) {
            commits_to_rewrite.push(info.id);
        }
    }
    // Add the base commit itself if it wasn't excluded and needs rewriting
    if !base_commits.contains(&base_id) && !commits_to_rewrite.contains(&base_id) {
        let base_parents = repo
            .find_object(base_id)?
            .try_into_commit()?
            .parent_ids()
            .count();
        // The first commit has no parents. If --from start is used, we need to rewrite it.
        if base_parents == 0 {
            commits_to_rewrite.push(base_id);
        }
    }

    // Reverse to process from oldest to newest
    commits_to_rewrite.reverse();

    Ok(commits_to_rewrite)
}

fn rewrite_commits(
    repo: &Repository,
    commits_to_rewrite: Vec<ObjectId>,
) -> AnyResult<Option<ObjectId>> {
    let mut parent_map = HashMap::new();
    let mut last_new_oid = None;

    for old_id in commits_to_rewrite.iter() {
        let old_commit = repo.find_object(*old_id)?.try_into_commit()?;
        let old_commit_ref = old_commit.decode()?;
        let tree_id = old_commit.tree_id()?;

        // Map parent IDs to their rewritten counterparts
        let new_parent_ids: Vec<ObjectId> = old_commit
            .parent_ids()
            .map(|parent_id| {
                let parent_detached = parent_id.detach();
                // If this parent was rewritten, use the new ID
                // Otherwise, it must be in the base history, so we keep the original ID
                *parent_map.get(&parent_detached).unwrap_or(&parent_detached)
            })
            .collect();

        // --- Modify commit data here ---
        let mut author = old_commit.author()?;
        author.name = "Dr. Magitulator".into();

        let mut committer = old_commit.committer()?;
        committer.name = "Dr. Magitulator".into();

        // Create a new commit object with the modified data
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
        parent_map.insert(*old_id, new_oid);
        last_new_oid = Some(new_oid);
    }

    Ok(last_new_oid)
}

fn create_branch(repo: &Repository, target_name: &str, final_oid: ObjectId) -> AnyResult<()> {
    let new_branch_name = format!("{}{BRANCH_POSTFIX}", target_name);
    let full_ref_name = format!("refs/heads/{}", new_branch_name);

    repo.edit_reference(RefEdit {
        change: Change::Update {
            log: LogChange::default(),
            expected: gix::refs::transaction::PreviousValue::Any,
            new: gix::refs::Target::Object(final_oid),
        },
        name: full_ref_name.clone().try_into()?,
        deref: false,
    })?;

    Ok(())
}
