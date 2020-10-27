use std::path::Path;
use std::fs::File;
use std::io::{BufRead, BufReader};
use toml::Value;
use super::die;

#[derive(Debug, PartialEq, Default)]
pub struct RepoFile {
    pub repo_name: Option<String>,
    pub remote_repo: Option<String>,
    pub remote_branch: Option<String>,
    pub include_as: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

impl RepoFile {
    pub fn new() -> RepoFile {
        RepoFile::default()
    }
}

pub fn read_file_into_lines(filename: &str) -> Vec<String> {
    let repo_file_path = Path::new(filename);
    if !repo_file_path.exists() {
        die!("Failed to find repo_file: {}", filename);
    }

    let file = File::open(repo_file_path);
    if let Err(file_error) = file {
        die!("Failed to open file: {}, {}", filename, file_error);
    }

    let file_contents = file.unwrap();
    let reader = BufReader::new(file_contents);
    reader.lines().map(|x| x.unwrap()).collect()
}

pub fn line_is_break(line: &String) -> bool {
    for c in line.chars() {
        if c != ' ' {
            return false;
        }
    }
    true
}

pub fn parse_repo_file_from_toml(filename: &str) -> RepoFile {
    let lines = read_file_into_lines(filename);
    parse_repo_file_from_toml_lines(lines)
}

pub fn parse_repo_file_from_toml_lines(lines: Vec<String>) -> RepoFile {
    // even though this is a toml file, and we have a toml parser
    // we still want to split by lines, and then parse specific sections
    // this is because if a user has:
    // [include]
    // this = that
    //
    // 
    // exclude = [ something ]
    // then toml will parse the exclude array as if its part of the include table
    // so we split the file into sections separated by 2 'break' lines
    // a break line is a line that only contains whitespace or a comment
    let mut last_line_was_break = false;
    let mut segment_indices = vec![];
    for (line_ind, line) in lines.iter().enumerate() {
        if line_is_break(line) {
            if last_line_was_break {
                segment_indices.push(line_ind);
                last_line_was_break = false;
            } else {
                last_line_was_break = true;
            }
        }
    }
    // always add a segment break to the end of the file
    segment_indices.push(lines.len());

    // group back the lines that are part of a contiguous segment
    let mut current_index = 0;
    let mut toml_segments = vec![];
    for i in segment_indices {
        let string_vec: Vec<&String> = lines.iter().skip(current_index).take(i - current_index).collect();
        if string_vec.len() == 0 { continue; }

        // we can calculate exactly how big the toml_segment is. its the sum
        // of all the lengths of each string in string_vec plus a few
        // newlines in between each string.
        let mut string_size = string_vec.iter().map(|s| s.len()).sum();
        // here we add the number of newlines there will be
        string_size += string_vec.len() - 1;

        let mut toml_segment = String::with_capacity(string_size);
        toml_segment.push_str(string_vec[0]);
        for j in 1..string_vec.len() {
            toml_segment.push('\n');
            toml_segment.push_str(string_vec[j]);
        }

        toml_segments.push(toml_segment);
        current_index = i;
    }

    parse_repo_file_from_toml_segments(toml_segments)
}

pub fn toml_value_to_string_opt(toml_value: &Value) -> Option<String> {
    match toml_value.as_str() {
        Some(s) => Some(s.to_owned()),
        None => None,
    }
}

pub fn parse_repo_section(toml_value: &Value, repofile: &mut RepoFile) {
    if let Value::Table(ref t) = toml_value {
        for (k, v) in t {
            match k.as_str() {
                "remote" => repofile.remote_repo = toml_value_to_string_opt(v),
                "name" => repofile.repo_name = toml_value_to_string_opt(v),
                "branch" => repofile.remote_branch = toml_value_to_string_opt(v),
                _ => (),
            }
        }
    }
}

pub fn parse_include_as_section(toml_value: &Value, repofile: &mut RepoFile) {
    if let Value::Table(ref t) = toml_value {
        let mut include_as = vec![];
        for (k, v) in t {
            if let Some(s) = v.as_str() {
                include_as.push(k.to_owned());
                include_as.push(s.to_string());
            }
        }
        repofile.include_as = Some(include_as);
    }
}

pub fn toml_value_to_vec(toml_value: &Value) -> Vec<String> {
    let mut toml_vec = vec![];
    if let Value::Array(ref a) = toml_value {
        for v in a {
            if let Some(s) = v.as_str() {
                toml_vec.push(s.to_owned());
            }
        }
    } else if let Value::String(s) = toml_value {
        toml_vec.push(s.to_owned());
    }
    toml_vec
}

pub fn parse_include_section(toml_value: &Value, repofile: &mut RepoFile) {
    let toml_vec = toml_value_to_vec(toml_value);
    if toml_vec.len() > 0 {
        repofile.include = Some(toml_vec);
    }
}

pub fn parse_exclude_section(toml_value: &Value, repofile: &mut RepoFile) {
    let toml_vec = toml_value_to_vec(toml_value);
    if toml_vec.len() > 0 {
        repofile.exclude = Some(toml_vec);
    }
}


pub fn parse_repo_file_from_toml_segments(
    toml_segments: Vec<String>
) -> RepoFile {
    let mut repo_file = RepoFile::default();
    // now we have toml_segments where each segment can be its own toml file
    // we parse each into a toml::Value, and then apply the result into a repo file object
    for s in toml_segments {
        let tomlvalue = s.parse::<Value>();
        if tomlvalue.is_err() { continue; } // TODO: report parse error to user
        let tomlvalue = tomlvalue.unwrap();

        if let Value::Table(ref t) = tomlvalue {
            for (k, v) in t {
                match k.as_str() {
                    "repo" => parse_repo_section(v, &mut repo_file),
                    "include_as" => parse_include_as_section(v, &mut repo_file),
                    "include" => parse_include_section(v, &mut repo_file),
                    "exclude" => parse_exclude_section(v, &mut repo_file),
                    _ => (),
                }
            }
        }
    }

    repo_file
}

#[cfg(test)]
mod test {
    use super::RepoFile;
    use super::parse_repo_file_from_toml_lines;

    fn parse_from_lines(toml_str: &str) -> RepoFile {
        let lines: Vec<String> = toml_str.split('\n').map(|s| s.to_string()).collect();
        parse_repo_file_from_toml_lines(lines)
    }

    #[test]
    fn toml_parse_repo_quotes_dont_matter() {
        let toml_str1 = r#"
            [repo]
            "name" = "hello"
            "remote" = "https://githost.com/repo"
        "#;
        let toml_str2 = r#"
            [repo]
            name = "hello"
            remote = "https://githost.com/repo"
        "#;
        let repofile1 = parse_from_lines(toml_str1);
        let repofile2 = parse_from_lines(toml_str2);
        assert_eq!(repofile1, repofile2);
        println!("{:#?}", repofile1);
    }

    #[test]
    fn toml_parse_remote_branch() {
        let toml_str = r#"
            [repo]
            branch="something"
        "#;
        let repofile = parse_from_lines(toml_str);
        assert_eq!(repofile.remote_branch.unwrap(), "something");
    }

    #[test]
    fn toml_should_return_repo_file_obj() {
        let toml_str = r#"
            [repo]
            remote = "something"
            [include_as]
            "one/x/y/" = "two/x/y/"
            "three/a/b" = "four/a/b"


            exclude = ["abc"]
            include = ["xyz", "qqq", "www"]
        "#;
        let mut expectedrepofileobj = RepoFile::new();
        expectedrepofileobj.remote_repo = Some("something".into());
        expectedrepofileobj.include_as = Some(vec![
            "one/x/y/".into(), "two/x/y/".into(),
            "three/a/b".into(), "four/a/b".into(),
        ]);
        expectedrepofileobj.exclude = Some(vec!["abc".into()]);
        expectedrepofileobj.include = Some(vec![
            "xyz".into(), "qqq".into(), "www".into(),
        ]);
        let repofileobj = parse_from_lines(toml_str);
        assert_eq!(expectedrepofileobj, repofileobj);
    }

    #[test]
    fn toml_space_parse_workd() {
        let toml_str = r#"
            [include_as]
            " " = "some path/lib/"
            "something/else" = " "
        "#;
        let repofile = parse_from_lines(toml_str);
        let include_as = repofile.include_as.unwrap();
        assert_eq!(include_as.len(), 4);
        assert_eq!(include_as[0], " ");
        assert_eq!(include_as[1], "some path/lib/");
        assert_eq!(include_as[2], "something/else");
        assert_eq!(include_as[3], " ");
    }

    #[test]
    fn toml_comments_not_included() {
        let toml_str = r#"
            [repo]
            name = "somename"
            # comment1
            # comment2
            # comment3
            branch = "somebranch"
        "#;
        let repofile = parse_from_lines(toml_str);
        assert_eq!(repofile.repo_name.unwrap(), "somename");
        assert_eq!(repofile.remote_branch.unwrap(), "somebranch");
    }
}
