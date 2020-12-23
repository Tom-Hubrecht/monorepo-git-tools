use gumdrop::Options;

use die::die;
use super::check::run_check;
use super::split_out::run_split_out;
use super::split_out::run_split_out_as;
use super::split_in::run_split_in;
use super::split_in::run_split_in_as;
use super::topbase::run_topbase;

#[derive(Debug, Options)]
pub struct MgtCommandCheck {
    // flags
    pub all: bool,
    pub local: bool,
    pub recursive: bool,
    pub remote: bool,

    // options
    pub local_branch: Option<String>,
    #[options(short = "b")]
    pub remote_branch: Option<String>,

    // positional arg: repo_file
    // (its a vec to appease gumdrop cli parser
    // but really itll be one string)
    #[options(free)]
    pub repo_file: Vec<String>,
}

#[derive(Debug, Options)]
pub struct MgtCommandTopbase {
    #[options(free)]
    pub base_or_top: Vec<String>,

    pub dry_run: bool,
    pub verbose: bool,
}

#[derive(Debug, Options)]
pub struct MgtCommandSplit {
    #[options(short = "g", long = "gen-repo-file")]
    pub generate_repo_file: bool,
    pub verbose: bool,
    pub dry_run: bool,


    pub input_branch: Option<String>,
    pub num_commits: Option<u32>,
    #[options(short = "o")]
    pub output_branch: Option<String>,

    #[options(no_long, short = "r")]
    pub rebase_flag: bool,
    pub rebase: Option<String>,

    #[options(no_long, short = "t")]
    pub topbase_flag: bool,
    pub topbase: Option<String>,

    #[options(long = "as")]
    pub as_subdir: Option<String>,

    // for program use, not by user
    #[options(skip)]
    pub direction: Option<Direction>,

    // positional arg: repo_file
    // (its a vec to appease gumdrop cli parser
    // but really itll be one string)
    #[options(free)]
    pub repo_file: Vec<String>,
}

#[derive(Debug)]
pub enum Direction { Out, In }

#[derive(Debug, Options)]
pub struct MgtCommandHelp {}

#[derive(Debug, Options)]
pub enum MgtSubcommands {
    Help(MgtCommandHelp),

    #[options(help = "check if there are changes ready to be pushed or pulled")]
    Check(MgtCommandCheck),

    #[options(help = "rebase top branch onto bottom branch but stop the rebase after the first shared commit")]
    Topbase(MgtCommandTopbase),

    #[options(help = "fetch and rewrite a remote repository's history onto a new branch according to the repo file rules")]
    SplitIn(MgtCommandSplit),

    #[options(help = "fetch and rewrite a remote repository's history onto a new branch and into the --as <subdirectory>")]
    SplitInAs(MgtCommandSplit),

    #[options(help = "create a new branch with this repository's history rewritten according to the repo file rules")]
    SplitOut(MgtCommandSplit),

    #[options(help = "create a new branch with this repository's history rewritten according to the --as <subdirectory>")]
    SplitOutAs(MgtCommandSplit),
}


pub fn print_usage<A: AsRef<impl Options>>(mgt_opts: A) {
    let version_str = format!(
        "{} {}",
        env!("CARGO_PKG_VERSION"),
        env!("LATEST_COMMIT"),
    );
    let author = env!("CARGO_PKG_AUTHORS");
    let about = env!("CARGO_PKG_DESCRIPTION");
    let app_name = env!("CARGO_PKG_NAME");
    let space = "  ";

    let mut command = mgt_opts.as_ref() as &dyn Options;
    let mut command_str = String::new();

    loop {
        if let Some(new_command) = command.command() {
            command = new_command;

            if let Some(name) = new_command.command_name() {
                command_str.push(' ');
                command_str.push_str(name);
            }
        } else {
            break;
        }
    }

    println!("{} {}\n{}\n{}\n\nUSAGE:\n{}{} [SUBCOMMAND] [OPTIONS]\n",
        app_name, version_str,
        author,
        about,
        space,
        app_name
    );
    println!("{}", mgt_opts.as_ref().self_usage());

    if let Some(cmds) = mgt_opts.as_ref().self_command_list() {
        println!();
        println!("Available commands:");
        println!("{}", cmds);
    }
}

#[derive(Debug, Options)]
pub struct Mgt {
    #[options(help = "Dont run anything. Just print output of what a run would do.")]
    pub dry_run: bool,
    #[options(help = "More detailed output")]
    pub verbose: bool,
    #[options(short = "h", help = "Prints help information")]
    pub help: bool,
    #[options(short = "V", help = "Prints version information")]
    pub version: bool,
    // thing: Option<String>,

    #[options(command)]
    pub command: Option<MgtSubcommands>,
}

impl AsRef<Mgt> for Mgt {
    fn as_ref(&self) -> &Mgt {
        self
    }
}

impl Mgt {
    pub fn new() -> Mgt {
        Mgt {
            dry_run: false,
            verbose: false,
            help: false,
            version: false,
            command: None,
        }
    }
}


pub fn get_cli_input() -> Mgt {
    let args = ::std::env::args().collect::<Vec<_>>();
    match <Mgt as Options>::parse_args_default(&args[1..]) {
        Err(e) => {
            println!("Failed to parse cli input: {}\n", e);
            let dummy_mgt = Mgt::new();
            print_usage(dummy_mgt);
            std::process::exit(2);
        }
        Ok(m) => m,
    }
}

/// validate the input options, and adjust as needed
/// print an error message and exit if invalid.
/// otherwise, call each commands run function
pub fn validate_input_and_run(mgt_opts: Mgt) {
    let mut mgt_opts = mgt_opts;
    match mgt_opts.command.take() {
        None => (),
        Some(mut command) => match command {
            MgtSubcommands::Help(_) => {
                // TODO: print help for the specific command
                print_usage(&mgt_opts);
                std::process::exit(0);
            },
            MgtSubcommands::Check(mut cmd) => {
                if cmd.remote && cmd.local {
                    die!("--remote cannot be used with --local");
                }
                run_check(&mut cmd);
            },
            MgtSubcommands::Topbase(mut cmd) => {
                cmd.verbose = cmd.verbose || mgt_opts.verbose;
                cmd.dry_run = cmd.dry_run || mgt_opts.dry_run;
                run_topbase(&mut cmd);
            },

            MgtSubcommands::SplitIn(ref mut cmd) => {
                cmd.verbose = mgt_opts.verbose || cmd.verbose;
                cmd.dry_run = mgt_opts.dry_run || cmd.dry_run;
                cmd.direction = Some(Direction::In);

                if cmd.topbase_flag && cmd.topbase.is_none() {
                    cmd.topbase = Some("".into());
                }
                if cmd.rebase_flag && cmd.rebase.is_none() {
                    cmd.rebase = Some("".into())
                }
                if cmd.rebase.is_some() && cmd.topbase.is_some() {
                    die!("Cannot use both --topbase and --rebase");
                }

                run_split_in(cmd);
            },
            MgtSubcommands::SplitInAs(ref mut cmd) => {
                cmd.verbose = mgt_opts.verbose || cmd.verbose;
                cmd.dry_run = mgt_opts.dry_run || cmd.dry_run;
                cmd.direction = Some(Direction::In);

                if cmd.topbase_flag && cmd.topbase.is_none() {
                    cmd.topbase = Some("".into());
                }
                if cmd.rebase_flag && cmd.rebase.is_none() {
                    cmd.rebase = Some("".into())
                }
                if cmd.rebase.is_some() && cmd.topbase.is_some() {
                    die!("Cannot use both --topbase and --rebase");
                }

                run_split_in_as(cmd);
            },

            MgtSubcommands::SplitOut(ref mut cmd) => {
                cmd.verbose = mgt_opts.verbose || cmd.verbose;
                cmd.dry_run = mgt_opts.dry_run || cmd.dry_run;
                cmd.direction = Some(Direction::Out);

                if cmd.topbase_flag && cmd.topbase.is_none() {
                    cmd.topbase = Some("".into());
                }
                if cmd.rebase_flag && cmd.rebase.is_none() {
                    cmd.rebase = Some("".into())
                }
                if cmd.rebase.is_some() && cmd.topbase.is_some() {
                    die!("Cannot use both --topbase and --rebase");
                }
                run_split_out(cmd);
            },
            MgtSubcommands::SplitOutAs(ref mut cmd) => {
                cmd.verbose = mgt_opts.verbose || cmd.verbose;
                cmd.dry_run = mgt_opts.dry_run || cmd.dry_run;
                cmd.direction = Some(Direction::Out);
                run_split_out_as(cmd);
            }
        },
    }
}
