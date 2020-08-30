use clap::{Arg, App, SubCommand, ArgMatches};

use super::split_out::run_split_out;
use super::split_in::run_split_in;
use super::split_in::run_split_in_as;

pub const INPUT_BRANCH_ARG: &'static str = "input-branch";
pub const INPUT_BRANCH_NAME: &'static str = "branch-name";
pub const OUTPUT_BRANCH_ARG: [&'static str; 2] = ["output-branch", "o"];
pub const OUTPUT_BRANCH_NAME: &'static str = "branch-name";
pub const REPO_FILE_ARG: &'static str = "repo-file";
pub const REPO_URI_ARG: &'static str = "git-repo-uri";
pub const AS_SUBDIR_ARG: &'static str = "as";
pub const AS_SUBDIR_ARG_NAME: &'static str = "subdirectory";
pub const DRY_RUN_ARG: [&'static str; 2] = ["dry-run", "d"];
pub const VERBOSE_ARG: [&'static str; 2] = ["verbose", "v"];
pub const REBASE_ARG: [&'static str; 2] = ["rebase", "r"];
pub const TOPBASE_ARG: [&'static str; 2] = ["topbase", "t"];

const SPLIT_IN_STR: &'static str = "split-in";
const SPLIT_IN_AS_STR: &'static str = "split-in-as";
const SPLIT_OUT_STR: &'static str = "split-out";
const SPLIT_OUT_DESCRIPTION: &'static str = "rewrite this repository history onto a new branch such that it only contains certain paths according to a repo-file";
const SPLIT_IN_DESCRIPTION: &'static str = "fetch and rewrite a remote repository's history onto a new branch such that it only contains certain paths according to a repo-file";
const SPLIT_IN_AS_DESCRIPTION: &'static str = "fetch the entirety of a remote repository and place it in a subdirectory of this repository";
const REPO_FILE_DESCRIPTION: &'static str = "path to file that contains instructions of how to split a repository";
const REPO_URI_DESCRIPTION: &'static str = "a valid git url of the repository to split in";
const AS_SUBDIR_DESCRIPTION: &'static str = "path relative to root of the local repository that will contain the entire repository being split in";
const REBASE_DESCRIPTION: &'static str = "after generating a branch with rewritten history, rebase that branch such that it can be fast forwarded back into the comparison branch. For split-in, the comparison branch is the branch you started on. For split-out, the comparison branch is the remote branch";
const TOPBASE_DESCRIPTION: &'static str = "like rebase, but it finds a fork point to only take the top commits from the created branch that dont exist in your starting branch";

#[derive(Clone)]
pub enum CommandName {
    SplitInAs,
    SplitIn,
    SplitOut,
    UnknownCommand,
}

use self::CommandName::*;

impl From<CommandName> for &'static str {
    fn from(original: CommandName) -> &'static str {
        match original {
            SplitInAs => SPLIT_IN_AS_STR,
            SplitIn => SPLIT_IN_STR,
            SplitOut => SPLIT_OUT_STR,
            UnknownCommand => "",
        }
    }
}

impl From<&str> for CommandName {
    fn from(value: &str) -> CommandName {
        match value {
            SPLIT_IN_AS_STR => SplitInAs,
            SPLIT_IN_STR => SplitIn,
            SPLIT_OUT_STR => SplitOut,
            _ => UnknownCommand,
        }
    }
}

impl CommandName {
    pub fn description(&self) -> &'static str {
        match self {
            SplitInAs => SPLIT_IN_AS_DESCRIPTION,
            SplitIn => SPLIT_IN_DESCRIPTION,
            SplitOut => SPLIT_OUT_DESCRIPTION,
            _ => "",
        }
    }
}

fn base_command<'a, 'b>(cmd: CommandName) -> App<'a, 'b> {
    let name = cmd.clone().into();
    return SubCommand::with_name(name)
        .about(cmd.description())
        .arg(
            Arg::with_name(REPO_FILE_ARG)
                .required(true)
                .help(REPO_FILE_DESCRIPTION)
        )
        .arg(
            Arg::with_name(DRY_RUN_ARG[0])
                .long(DRY_RUN_ARG[0])
                .short(DRY_RUN_ARG[1])
                .help("Print out the steps taken, but don't actually run or change anything.")
        )
        .arg(
            Arg::with_name(VERBOSE_ARG[0])
                .long(VERBOSE_ARG[0])
                .short(VERBOSE_ARG[1])
                .help("show more detailed logs")
        )
        .arg(
            Arg::with_name(REBASE_ARG[0])
                .long(REBASE_ARG[0])
                .short(REBASE_ARG[1])
                .help(REBASE_DESCRIPTION)
                .conflicts_with(TOPBASE_ARG[0])
        )
        .arg(
            Arg::with_name(TOPBASE_ARG[0])
                .long(TOPBASE_ARG[0])
                .short(TOPBASE_ARG[1])
                .help(TOPBASE_DESCRIPTION)
                .conflicts_with(REBASE_ARG[0])
        )
        .arg(
            Arg::with_name(OUTPUT_BRANCH_ARG[0])
                .long(OUTPUT_BRANCH_ARG[0])
                .short(OUTPUT_BRANCH_ARG[1])
                .takes_value(true)
                .value_name(OUTPUT_BRANCH_NAME)
                .help("name of branch that will be created with new split history")
        );
}

pub fn split_in<'a, 'b>() -> App<'a, 'b> {
    // split in has specific arguments in addition to base
    base_command(SplitIn)
        .arg(
            Arg::with_name(INPUT_BRANCH_ARG)
                .long(INPUT_BRANCH_ARG)
                .takes_value(true)
                .value_name(INPUT_BRANCH_NAME)
                .help("split in from a local branch in this repository")
        )
}

pub fn split_in_as<'a, 'b>() -> App<'a, 'b> {
    // split in as has specific arguments in addition to base
    let cmd = SplitInAs;
    let name = cmd.clone().into();
    return SubCommand::with_name(name)
        .about(cmd.description())
        .arg(
            Arg::with_name(REPO_URI_ARG)
                .required(true)
                .help(REPO_URI_DESCRIPTION)
        )
        .arg(
            Arg::with_name(REBASE_ARG[0])
                .long(REBASE_ARG[0])
                .short(REBASE_ARG[1])
                .help(REBASE_DESCRIPTION)
                .conflicts_with(TOPBASE_ARG[0])
        )
        .arg(
            Arg::with_name(TOPBASE_ARG[0])
                .long(TOPBASE_ARG[0])
                .short(TOPBASE_ARG[1])
                .help(TOPBASE_DESCRIPTION)
                .conflicts_with(REBASE_ARG[0])
        )
        .arg(
            Arg::with_name(AS_SUBDIR_ARG)
                .long(AS_SUBDIR_ARG)
                .help(AS_SUBDIR_DESCRIPTION)
                .value_name(AS_SUBDIR_ARG_NAME)
                .required(true)
                .takes_value(true)
        )
        .arg(
            Arg::with_name(DRY_RUN_ARG[0])
                .long(DRY_RUN_ARG[0])
                .short(DRY_RUN_ARG[1])
                .help("Print out the steps taken, but don't actually run or change anything.")
        )
        .arg(
            Arg::with_name(VERBOSE_ARG[0])
                .long(VERBOSE_ARG[0])
                .short(VERBOSE_ARG[1])
                .help("show more detailed logs")
        )
        .arg(
            Arg::with_name(OUTPUT_BRANCH_ARG[0])
                .long(OUTPUT_BRANCH_ARG[0])
                .short(OUTPUT_BRANCH_ARG[1])
                .takes_value(true)
                .value_name(OUTPUT_BRANCH_NAME)
                .help("name of branch that will be created with new split history")
        );
}

pub fn split_out<'a, 'b>() -> App<'a, 'b> {
    base_command(SplitOut)
}

pub fn run_command(name: &str, matches: &ArgMatches) {
    let command: CommandName = name.into();
    match command {
        UnknownCommand => (),
        // it is safe to unwrap here because this function is called
        // if we know that the name subcommand exists
        SplitIn => run_split_in(matches.subcommand_matches(name).unwrap()),
        SplitInAs => run_split_in_as(matches.subcommand_matches(name).unwrap()),
        SplitOut => run_split_out(matches.subcommand_matches(name).unwrap()),
    }
}
