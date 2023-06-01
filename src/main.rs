use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use clap::Parser;
use num_cpus;
use regex::Regex;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(author = "jason xing", version, about, long_about = None)]
pub struct StatisticArgs {
    #[arg(long, short)]
    start: String,

    #[arg(long, short)]
    end: String,

    #[arg(long, short)]
    input: String,
}

fn main() {
    let start = Instant::now();
    let args = StatisticArgs::parse();
    let dir = Arc::new(args.input.clone());
    let commit_start_date_unix = unix_timestamp(&args.start);
    let commit_end_date_unix = unix_timestamp(&args.end);
    if commit_end_date_unix < commit_start_date_unix {
        panic!("end date must be greater than start date");
    }
    let all_branch_commits = git_all_branch_commits(&dir.clone());
    let commit_tree = CommitTree::new(&all_branch_commits);
    let commit_vec = commit_tree.commit_vec_by_unix(commit_start_date_unix, commit_end_date_unix);
    let statistic = Arc::new(Mutex::new(Statistic::new()));
    let count = Arc::new(Mutex::new(0));
    let total = commit_vec.len();
    let num_cores = num_cpus::get();
    let num_cores = if num_cores > 1 { num_cores - 1 } else { 1 };
    let commit_vec_chunk = commit_vec.chunks(if total > 100 { total / num_cores } else { 100 });
    let mut thread_vec = vec![];
    commit_vec_chunk.for_each(|chunk| {
        let chunk: Vec<Commit> = chunk.to_owned().into_iter().map(|v| v.clone()).collect();
        let statistic = Arc::clone(&statistic);
        let count = Arc::clone(&count);
        let dir_arc = dir.clone();
        let handle = thread::spawn(move || {
            chunk.into_iter().for_each(|v| {
                {
                    let mut num = count.lock().unwrap();
                    *num += 1;
                    // println!("process {}/{}", num, total);
                }
                if let Some(_) = &v.parent_commit_id {
                    let diff_res = get_commit_diff_by_git(&dir_arc, v);
                    statistic.lock().unwrap().add(diff_res);
                }
            });
        });
        thread_vec.push(handle);
    });
    for handle in thread_vec {
        handle.join().unwrap();
    }
    statistic.lock().unwrap().print();
    let duration = start.elapsed();
    println!("exec time: {:?}ms", duration.as_millis());
}

#[derive(Debug, Clone)]
struct Statistic {
    statistic_item_vec: Vec<StatisticItem>,
}

impl Statistic {
    fn new() -> Self {
        Self {
            statistic_item_vec: vec![],
        }
    }

    fn add(&mut self, item: StatisticItem) {
        self.statistic_item_vec.push(item);
    }

    fn print(&self) {
        let mut files = 0;
        let mut insertion = 0;
        let mut deletion = 0;
        self.statistic_item_vec.iter().for_each(|v| {
            files += v.files;
            insertion += v.insertion;
            deletion += v.deletion;
        });
        println!(
            "commits: {}, files: {}, insertion: {}, deletion: {}",
            self.statistic_item_vec.len(),
            files,
            insertion,
            deletion
        );
    }
}

#[derive(Debug, Clone)]
struct StatisticItem {
    commit: Commit,
    files: usize,
    insertion: usize,
    deletion: usize,
}

/// 使用git diff 命令 对比两个commit之间的代码行数
fn get_commit_diff_by_git(dir: &str, commit: Commit) -> StatisticItem {
    let mut git_command = std::process::Command::new("git");
    git_command.current_dir(dir);
    let output = git_command
        .args(&[
            "diff",
            "--shortstat",
            &commit.id,
            commit.parent_commit_id.as_ref().unwrap(),
        ])
        .output()
        .expect("failed to execute process");
    let output = String::from_utf8(output.stdout).unwrap();
    let mut lines = output.lines();
    let mut insertion = 0;
    let mut deletion = 0;
    let mut files = 0;
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.contains("deletion") {
            let insertion_res = Regex::new(r"(\d+) insertion").unwrap();
            let deletion_res = Regex::new(r"(\d+) deletion").unwrap();
            let files_res = Regex::new(r"(\d+) file").unwrap();
            if let Some(files_res) = files_res.captures(line) {
                files = files_res.get(1).unwrap().as_str().parse::<usize>().unwrap();
            }
            if let Some(insertion_res) = insertion_res.captures(line) {
                insertion = insertion_res
                    .get(1)
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap();
            }
            if let Some(deletion_res) = deletion_res.captures(line) {
                deletion = deletion_res
                    .get(1)
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap();
            }
        }
    }
    StatisticItem {
        commit,
        files,
        deletion,
        insertion,
    }
}

/// 获取git项目的分支列表
fn git_branches(dir: &str) -> Vec<String> {
    let mut git_command = std::process::Command::new("git");
    git_command.current_dir(dir);
    let output = git_command
        .args(&["branch", "-a"])
        .output()
        .expect("failed to execute process");

    // 解析git branch输出
    let output = String::from_utf8(output.stdout).unwrap();
    let mut lines = output.lines();
    let mut branches = Vec::new();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.starts_with("remotes/origin/") {
            let re = Regex::new(r".*origin/").unwrap();
            let branch = re.replace_all(line, "origin/");
            branches.push(branch.to_string());
        }
    }
    branches
}

#[derive(Debug, Clone, Hash)]
struct Commit {
    author: String,
    unix_date: String,
    id: String,
    email: String,
    msg: String,
    parent_commit_id: Option<String>,
}

impl PartialEq for Commit {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Commit {}

/// 根据分支名获取commit列表
fn git_commits(dir: &str, branch: &str) -> Vec<Commit> {
    let mut git_command = std::process::Command::new("git");
    git_command.current_dir(dir);
    // 输出格式: author-name;author-email;commit-id;commit-date;commit-msg
    let output = git_command
        .args(&[
            "log",
            "--no-merges",
            "--pretty=format:%an;;;;%ae;;;;%H;;;;%at;;;;%s;;;;%P",
            branch,
        ])
        .output()
        .expect("failed to execute process");

    // 解析git log输出
    let output = String::from_utf8(output.stdout).unwrap();
    let mut lines = output.lines();
    let mut commits = Vec::new();
    while let Some(line) = lines.next() {
        let line = line.trim();
        let items: Vec<&str> = line.split(";;;;").collect();
        if items.len() == 6 {
            let commit_id = items[2].to_string();
            let commit = Commit {
                author: items[0].to_string(),
                unix_date: items[3].to_string(),
                id: commit_id.clone(),
                email: items[1].to_string(),
                msg: items[4].to_string(),
                parent_commit_id: items.get(5).and_then(|x| Some(x.to_string())),
            };
            commits.push(commit);
        }
    }
    commits.reverse();

    commits
}

/// 获取所有分支的commit列表
fn git_all_branch_commits(dir: &str) -> Vec<Vec<Commit>> {
    let branches = git_branches(dir);
    let mut commits = Vec::new();
    for branch in branches {
        commits.push(git_commits(dir, &branch));
    }
    commits
}

#[derive(Debug, Clone)]
struct CommitTreeNode<'a> {
    commit: &'a Commit,
    children: Vec<CommitTreeNode<'a>>,
}

impl<'a> PartialEq for CommitTreeNode<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.commit == other.commit
    }
}

impl<'a> Eq for CommitTreeNode<'a> {}

impl<'a> CommitTreeNode<'a> {
    fn new(commit: &'a Commit) -> Self {
        CommitTreeNode {
            commit,
            children: vec![],
        }
    }

    fn add_child(&mut self, commit: &'a Commit) {
        self.children.push(CommitTreeNode::new(commit));
    }

    /// 深度优先遍历
    ///
    /// 查找unix_date大于指定日期的子树列表
    fn sub_tree_vec_by_unix_date(
        &self,
        start_unix_date: u64,
        end_unix_date: u64,
    ) -> Vec<&CommitTreeNode<'a>> {
        let mut result = Vec::new();
        let commit_date = self.commit.unix_date.parse::<u64>().unwrap();
        if commit_date >= start_unix_date && commit_date <= end_unix_date {
            result.push(self);
        } else {
            for child in &self.children {
                let mut child_result =
                    child.sub_tree_vec_by_unix_date(start_unix_date, end_unix_date);
                result.append(&mut child_result);
            }
        }
        result
    }
}

#[derive(Debug)]
struct CommitTree<'a> {
    children: Vec<CommitTreeNode<'a>>,
}

impl<'a> CommitTree<'a> {
    fn new(commits: &'a Vec<Vec<Commit>>) -> Self {
        let commit_tree_vec = commits
            .iter()
            .map(|x| {
                let mut root = CommitTreeNode::new(&x[0]);
                let mut parent = &mut root;
                for i in 1..x.len() {
                    parent.add_child(&x[i]);
                    parent = parent.children.last_mut().unwrap();
                }
                root
            })
            .collect();
        let mut tree = CommitTree {
            children: commit_tree_vec,
        };
        tree.merge();
        tree
    }

    /// 将节点链表合并成一个链表
    fn merge(&mut self) {
        let mut merge_in_vec = Vec::new();
        let mut merge_wait_vec = self.children.clone();
        // 设置
        merge_in_vec.push(merge_wait_vec.remove(0));
        for i in 0..merge_wait_vec.len() {
            let merge_wait_node = merge_wait_vec.get_mut(i).unwrap();
            let mut merged = false;
            for j in 0..merge_in_vec.len() {
                let merge_in_node = merge_in_vec.get_mut(j).unwrap();
                merged = merge_in(merge_in_node, merge_wait_node);
                if merged {
                    break;
                }
            }
            if !merged {
                merge_in_vec.push(merge_wait_node.clone());
            }
        }
        self.children = merge_in_vec
    }

    /// 根据日期获取commit列表
    fn sub_tree_vec_by_unix_date(
        &self,
        start_unix_date: u64,
        end_unix_date: u64,
    ) -> Vec<&CommitTreeNode<'a>> {
        let mut sub_tree_vec = vec![];
        for tree in self.children.iter() {
            sub_tree_vec
                .append(&mut tree.sub_tree_vec_by_unix_date(start_unix_date, end_unix_date));
        }
        sub_tree_vec
    }

    /// 根据日期获取commit列表
    fn commit_vec_by_unix(&self, start_unix_date: u64, end_unix_date: u64) -> Vec<&Commit> {
        let tree_vec = self.sub_tree_vec_by_unix_date(start_unix_date, end_unix_date);
        let mut commits = vec![];
        tree_vec.iter().for_each(|v| {
            commits.append(&mut flatten_commit_tree_node(v));
        });
        let commits: Vec<_> = commits
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        commits
    }
}

/// 抹平CommitTreeNode
///
/// 生成 Commit 列表
fn flatten_commit_tree_node<'a>(commit_tree: &'a CommitTreeNode<'a>) -> Vec<&Commit> {
    let mut commits = vec![commit_tree.commit];
    for tree in commit_tree.children.iter() {
        commits.append(&mut flatten_commit_tree_node(tree));
    }
    commits
}

fn merge_in<'a, 'b>(base: &'a mut CommitTreeNode<'b>, commit: &'a mut CommitTreeNode<'b>) -> bool {
    if base.commit != commit.commit {
        return false;
    }
    let mut base_node = base;
    let mut merged_in_node: &mut CommitTreeNode = commit;
    while !merged_in_node.children.is_empty() {
        merged_in_node = &mut merged_in_node.children[0];
        let position = base_node.children.iter().position(|x| x == merged_in_node);
        if position.is_none() {
            base_node.children.push(merged_in_node.clone());
            return true;
        } else if let Some(positon) = position {
            base_node = base_node.children.get_mut(positon).unwrap();
        }
    }
    true
}

fn unix_timestamp(time: &str) -> u64 {
    let date = NaiveDate::parse_from_str(time, "%Y-%m-%d").expect("parse date error");
    let date = NaiveDateTime::new(
        date,
        NaiveTime::parse_from_str("00:00:00", "%H:%M:%S").unwrap(),
    );
    let timestamp_millis = date.timestamp_millis() / 1000;
    if timestamp_millis > 0 {
        timestamp_millis as u64
    } else {
        0
    }
}
