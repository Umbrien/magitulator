use crate::{AnyResult, BRANCH_POSTFIX};
use colored::Colorize;
use gix::{
    ObjectId, Repository,
    actor::Signature,
    bstr::BString,
    date::time,
    refs::transaction::{Change, LogChange, RefEdit},
};
use std::collections::{HashMap, HashSet};

struct CommitDescriptor {
    original_id: ObjectId,
    original_parent_ids: Vec<ObjectId>,
    tree: ObjectId,
    author: Signature,
    committer: Signature,
    encoding: Option<BString>,
    message: BString,
    extra_headers: Vec<(BString, BString)>,
}

pub fn mirror(base: &str, target: &str, dry_run: bool) -> AnyResult<()> {
    let repo = gix::open(".")?;

    let base_commit_id = resolve_commit_id(&repo, &base)?;
    let target_commit_id = resolve_commit_id(&repo, &target)?;

    let commits_to_rewrite = get_commits_to_rewrite(&repo, base_commit_id, target_commit_id)?;
    if commits_to_rewrite.is_empty() {
        return Ok(());
    }

    let descriptors = generate_descriptors(&repo, &commits_to_rewrite)?;

    if dry_run {
        println!("--- Commits that would be rewritten (dry run) ---");
        for descriptor in descriptors.iter().rev() {
            print_commit_descriptor_oneline(descriptor)?;
        }
    } else {
        let last_new_oid = execute_mirror(&repo, &descriptors)?;

        match last_new_oid {
            Some(final_oid) => {
                create_branch(&repo, &target, final_oid)?;
            }
            None => {
                return Err("No commits were processed".into());
            }
        }
    }

    Ok(())
}

fn generate_descriptors(
    repo: &Repository,
    commits_to_rewrite: &[ObjectId],
) -> AnyResult<Vec<CommitDescriptor>> {
    let mut descriptors = Vec::new();
    for old_id in commits_to_rewrite {
        let old_commit = repo.find_object(*old_id)?.try_into_commit()?;
        let old_commit_ref = old_commit.decode()?;

        let mut author = old_commit.author()?;
        author.name = "Dr. Magitulator".into();

        let mut committer = old_commit.committer()?;
        committer.name = "Dr. Magitulator".into();

        let descriptor = CommitDescriptor {
            original_id: *old_id,
            original_parent_ids: old_commit.parent_ids().map(|oid| oid.detach()).collect(),
            tree: old_commit.tree_id()?.detach(),
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
        descriptors.push(descriptor);
    }
    Ok(descriptors)
}

fn execute_mirror(
    repo: &Repository,
    descriptors: &[CommitDescriptor],
) -> AnyResult<Option<ObjectId>> {
    let mut parent_map = HashMap::new();
    let mut last_new_oid = None;

    for descriptor in descriptors {
        // Map original parent IDs to their newly created counterparts.
        // If a parent wasn't rewritten, it's in the base history, so we use its original ID.
        let new_parent_ids: Vec<ObjectId> = descriptor
            .original_parent_ids
            .iter()
            .map(|parent_id| *parent_map.get(parent_id).unwrap_or(parent_id))
            .collect();

        let new_commit = gix::objs::Commit {
            tree: descriptor.tree,
            parents: new_parent_ids.into(),
            author: descriptor.author.clone(),
            committer: descriptor.committer.clone(),
            encoding: descriptor.encoding.clone(),
            message: descriptor.message.clone(),
            extra_headers: descriptor.extra_headers.clone(),
        };

        let new_oid = repo.write_object(&new_commit)?.into();

        parent_map.insert(descriptor.original_id, new_oid);
        last_new_oid = Some(new_oid);
    }

    Ok(last_new_oid)
}

fn print_commit_descriptor_oneline(descriptor: &CommitDescriptor) -> AnyResult<()> {
    let t = gix::date::Time::from(descriptor.author.time).format(time::format::DEFAULT);
    let message: String = descriptor
        .message
        .to_string()
        .lines()
        .next()
        .unwrap_or("")
        .trim_end()
        .chars()
        .take(15)
        .collect();

    println!(
        "{} ({}) {} {}",
        &descriptor.original_id.to_string()[0..7].dimmed(),
        t.blue(),
        // descriptor.author.name.to_string().green(),
        descriptor.author.email.to_string().green(),
        message,
    );

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
    let mut base_commits = HashSet::new();
    if base_id != target_id {
        for info in gix::traverse::commit::topo::Builder::from_iters(
            &repo.objects,
            [base_id],
            None::<Vec<ObjectId>>,
        )
        .build()?
        {
            base_commits.insert(info?.id);
        }
    }

    let branch_topo = gix::traverse::commit::topo::Builder::from_iters(
        &repo.objects,
        [target_id],
        None::<Vec<ObjectId>>,
    )
    .build()?;

    let mut commits_to_rewrite = Vec::new();
    for info in branch_topo {
        let info = info?;
        if !base_commits.contains(&info.id) {
            commits_to_rewrite.push(info.id);
        }
    }

    if !base_commits.contains(&base_id) && !commits_to_rewrite.contains(&base_id) {
        if repo
            .find_object(base_id)?
            .try_into_commit()?
            .parent_ids()
            .count()
            == 0
        {
            commits_to_rewrite.push(base_id);
        }
    }

    commits_to_rewrite.reverse();
    Ok(commits_to_rewrite)
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
