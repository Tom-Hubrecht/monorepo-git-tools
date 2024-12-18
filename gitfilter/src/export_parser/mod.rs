use super::*;
pub mod structured_parse;
pub use structured_parse::*;

pub mod unstructured_parse;
pub use unstructured_parse::*;

use num_cpus;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::{
    fmt::Display,
    io,
    path::{Path, PathBuf},
};
// use std::time::Instant;
// use std::time::Duration;

pub struct WaitObj {
    pub index: usize,
    pub obj: StructuredExportObject,
}

impl PartialEq for WaitObj {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl Eq for WaitObj {}

impl Ord for WaitObj {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
    }
}

impl PartialOrd for WaitObj {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.index.cmp(&other.index))
    }
}

pub fn parse_git_filter_export<O, E: Display + From<io::Error>, P: AsRef<Path>>(
    export_branch: Option<String>,
    with_blobs: bool,
    location: Option<P>,
    cb: impl FnMut(StructuredExportObject) -> Result<O, E>,
) -> io::Result<()> {
    let mut cb = cb;
    parse_git_filter_export_with_callback(export_branch, with_blobs, location, |unparsed| {
        match parse_into_structured_object(unparsed) {
            Ok(parsed) => cb(parsed),
            Err(e) => Err(E::from(e)),
        }
    })
}

/// uses mpsc channel to parse a bit faster. the rationale
/// is that the thread that spawns the git fast-export command
/// only needs to:
/// 1. read from the stdout of that command
/// 2. parse the data section
/// then it can pass that parsed data to the main thread
/// which can finish the more intensive parsing/transformations
/// Pass `n_parsing_threads = None` if you want to find a number
/// of CPUs available on the system, otherwise, pass Some(n)
/// to limit to a specific number of CPUs.
/// passing 1 here is still faster than using the above
/// `parse_git_filter_export` because it will still read via
/// a channel, rather than be blocking
pub fn parse_git_filter_export_via_channel<O, E: Display, P: AsRef<Path>>(
    export_branch: Option<String>,
    with_blobs: bool,
    n_parsing_threads: Option<usize>,
    location: Option<P>,
    cb: impl FnMut(StructuredExportObject) -> Result<O, E>,
) -> io::Result<()> {
    let n_parsing_threads = match n_parsing_threads {
        Some(n) => n,
        None => {
            // if user didnt provide n parsing threads,
            // we try to get number of cpus
            // and if we fail to do that, or theres not enough,
            // we will just use 1.
            let cpu_count = num_cpus::get() as isize;
            // minus 2 because we are already using 2 threads:
            // one for outbound processing,
            // one for starting the fast-export stream
            let spawn_parser_threads = cpu_count - 2;
            if spawn_parser_threads <= 0 {
                1
            } else {
                spawn_parser_threads as usize
            }
        }
    };

    let mut cb = cb;
    let mut spawned_threads = vec![];
    let (tx, rx) = mpsc::channel();
    for _ in 0..n_parsing_threads {
        let (parse_tx, parse_rx) = mpsc::channel();
        let parse_consumer_tx_clone = tx.clone();
        let parse_thread = thread::spawn(move || {
            let mut err = Ok(());
            for (counter, received) in parse_rx {
                let parsed = export_parser::parse_into_structured_object(received);
                if let Err(e) = parse_consumer_tx_clone.send((counter, parsed)) {
                    err = Err(e);
                    break;
                }
            }
            err
        });
        spawned_threads.push((parse_tx, parse_thread));
    }

    // this transmitter is not doing anything, only the cloned
    // versions of it are in use, so we HAVE to drop it here
    // otherwise our program will hang.
    drop(tx);

    let location: Option<PathBuf> = match location {
        Some(p) => Some(p.as_ref().to_path_buf()),
        None => None,
    };

    // on the thread that is running the git fast-export,
    // it will alternate passing these UNPARSED messages to one of our
    // parsing threads. the parsing threads (created above)
    // will then pass the PARSED message back to our main thread
    let thread_handle = thread::spawn(move || {
        let mut counter = 0;
        parse_git_filter_export_with_callback(export_branch, with_blobs, location, |x| {
            let thread_index = counter % n_parsing_threads as usize;
            let (parse_tx, _) = &spawned_threads[thread_index];
            let res = parse_tx.send((counter, x));
            counter += 1;
            res
        })
        // TODO: do we need to join all of the spawned
        // parsing threads?
    });

    // we want our vec of parsed objects
    // to be in the same order as they were received. so
    // we check the index of the object, and ensure that we are only
    // adding to the out_vec if the entry is consecutive.
    // otherwise we put it into a temporary reverse binary heap
    // which we then keep checking to remove elements from the heap
    // and put them into the out_vec in the correct order
    let mut first_received = false;
    let mut expected = 0;
    // let mut out_vec = vec![];
    let mut wait_heap = BinaryHeap::new();
    for received in rx {
        let received_obj = received.1?;
        if received.0 == expected {
            // out_vec.push(received.1);
            cb(received_obj).map_err(|e| ioerr!("{}", e))?;
            expected += 1;
        } else {
            let wait_obj = WaitObj {
                index: received.0,
                obj: received_obj,
            };
            wait_heap.push(Reverse(wait_obj));
        }

        while let Some(wait_obj) = wait_heap.pop() {
            let wait_obj = wait_obj.0;
            if wait_obj.index == expected {
                // out_vec.push(wait_obj.obj);
                cb(wait_obj.obj).map_err(|e| ioerr!("{}", e))?;
                expected += 1;
            } else {
                wait_heap.push(Reverse(wait_obj));
                break;
            }
        }

        if !first_received {
            first_received = true;
            // eprintln!("Received first PARSED thing at {:?}", std::time::Instant::now());
        }
    }

    match thread_handle.join() {
        Err(_) => ioerre!("Failed to join thread!"),
        Ok(filter_res) => filter_res,
    }
}

pub fn write_person_info(write_data: &mut Vec<u8>, person: &CommitPersonOwned, is_author: bool) {
    if is_author {
        write_data.extend(b"author ");
    } else {
        write_data.extend(b"committer ");
    }
    if let Some(name) = &person.name {
        write_data.extend(name.as_bytes());
        write_data.push(b' ');
    }
    write_data.push(b'<');
    write_data.extend(person.email.as_bytes());
    write_data.extend(b"> ");
    write_data.extend(person.timestr.as_bytes());
    write_data.push(b'\n');
}

/// convenience function to write the structured export object
/// out to some writable stream. This function ensures that your
/// structured export object is formatted properly for git-fast-import
/// to read it in.
pub fn write_to_stream<W: Write>(stream: W, obj: StructuredExportObject) -> io::Result<()> {
    // eprintln!("Using: {:#?}", obj);
    let mut stream = stream;
    let mut write_data: Vec<u8> = vec![];
    if obj.has_feature_done {
        write_data.extend(b"feature done\n");
    }
    if let Some(reset_ref) = &obj.has_reset {
        write_data.extend(b"reset ");
        write_data.extend(reset_ref.as_bytes());
        write_data.push(b'\n');
    }
    if let Some(reset_from) = obj.has_reset_from {
        write_data.extend(b"from ");
        write_data.extend(reset_from.as_bytes());
        write_data.push(b'\n');
    }
    if obj.has_reset.is_some() {
        write_data.push(b'\n');
    }

    if let StructuredObjectType::Commit(commit_obj) = obj.object_type {
        write_data.extend(b"commit ");
        write_data.extend(commit_obj.commit_ref.as_bytes());
        write_data.push(b'\n');
        write_data.extend(b"mark :");
        write_data.extend(commit_obj.mark.to_string().as_bytes());
        write_data.push(b'\n');
        write_data.extend(b"original-oid ");
        write_data.extend(commit_obj.original_oid.as_bytes());
        write_data.push(b'\n');
        if let Some(author) = commit_obj.get_author() {
            write_person_info(&mut write_data, author, true);
        }
        write_person_info(&mut write_data, &commit_obj.committer, false);
        write_data.extend(b"data ");
        let msg_as_bytes = commit_obj.commit_message.as_bytes();
        let data_len = msg_as_bytes.len();
        // since we used string lossy to get the commit message,
        // its possible its not the same length as it was when we
        // ran git fast-export. the correct solution
        // would be to keep the commit message as a byte vec...
        // but for now, we will just ensure that we output the data length
        // to be the exact byte count of the commit_message.as_bytes();
        write_data.extend(data_len.to_string().as_bytes());
        write_data.push(b'\n');
        write_data.extend(msg_as_bytes);
        write_data.push(b'\n');

        let mut first_merge = true;
        for merge_info in commit_obj.merges {
            if first_merge {
                // first merge should be a 'from'
                first_merge = false;
                write_data.extend(b"from :");
                write_data.extend(merge_info.to_string().as_bytes());
                write_data.push(b'\n');
            } else {
                write_data.extend(b"merge :");
                write_data.extend(merge_info.to_string().as_bytes());
                write_data.push(b'\n');
            }
        }
        for fileop in commit_obj.fileops {
            match fileop {
                FileOpsOwned::FileModify(mode, dataref, path) => {
                    write_data.extend(b"M ");
                    write_data.extend(mode.as_bytes());
                    write_data.push(b' ');
                    write_data.extend(dataref.as_bytes());
                    write_data.push(b' ');
                    write_data.extend(path.as_bytes());
                }
                FileOpsOwned::FileDelete(path) => {
                    write_data.extend(b"D ");
                    write_data.extend(path.as_bytes());
                }
                FileOpsOwned::FileCopy(a, b) => {
                    write_data.extend(b"C ");
                    write_data.extend(a.as_bytes());
                    write_data.push(b' ');
                    write_data.extend(b.as_bytes());
                }
                FileOpsOwned::FileRename(a, b) => {
                    write_data.extend(b"R ");
                    write_data.extend(a.as_bytes());
                    write_data.push(b' ');
                    write_data.extend(b.as_bytes());
                }
                FileOpsOwned::FileDeleteAll => {
                    write_data.extend(b"deleteall");
                }
                FileOpsOwned::NoteModify(dataref, commitish) => {
                    write_data.extend(b"N ");
                    write_data.extend(dataref.as_bytes());
                    write_data.push(b' ');
                    write_data.extend(commitish.as_bytes());
                }
            }
            write_data.push(b'\n');
        }
        write_data.push(b'\n');
    } else if let StructuredObjectType::Blob(blob_obj) = obj.object_type {
        write_data.extend(b"blob\n");
        write_data.extend(b"mark :");
        write_data.extend(blob_obj.mark.to_string().as_bytes());
        write_data.push(b'\n');
        write_data.extend(b"original-oid ");
        write_data.extend(blob_obj.original_oid.as_bytes());
        write_data.push(b'\n');
        write_data.extend(b"data ");
        write_data.extend(obj.data_size.as_bytes());
        write_data.push(b'\n');
        write_data.extend(blob_obj.data);
        write_data.push(b'\n');
    }

    stream.write_all(&write_data)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // use std::io::prelude::*;

    // TODO: whats a unit test? ;)
    pub const NO_LOCATION: Option<PathBuf> = None;

    #[test]
    fn using_multiple_parsing_threads_keeps_order_the_same() {
        let mut expected_count = 1;
        parse_git_filter_export_via_channel(None, false, Some(4), NO_LOCATION, |obj| {
            if let StructuredObjectType::Commit(commit_obj) = obj.object_type {
                let mark_str = format!(":{}", commit_obj.mark);
                let expected_mark_str = format!(":{}", expected_count);
                assert_eq!(expected_mark_str, mark_str);
            } else {
                panic!("Expected commit object");
            }
            expected_count += 1;
            if 1 == 2 {
                return Err("a");
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn using_blobs_and_multiple_parsing_threads_keeps_order_the_same() {
        let mut expected_count = 1;
        parse_git_filter_export_via_channel(None, true, Some(4), NO_LOCATION, |obj| {
            if let StructuredObjectType::Commit(commit_obj) = obj.object_type {
                let mark_str = format!(":{}", commit_obj.mark);
                let expected_mark_str = format!(":{}", expected_count);
                assert_eq!(expected_mark_str, mark_str);
            } else if let StructuredObjectType::Blob(blob_obj) = obj.object_type {
                let mark_str = format!(":{}", blob_obj.mark);
                let expected_mark_str = format!(":{}", expected_count);
                assert_eq!(expected_mark_str, mark_str);
            }
            expected_count += 1;
            if 1 == 2 {
                return Err("a");
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test1() {
        let now = std::time::Instant::now();
        parse_git_filter_export_via_channel(None, false, Some(1), NO_LOCATION, |_| {
            if 1 == 1 {
                Ok(())
            } else {
                Err("a")
            }
        })
        .unwrap();
        eprintln!("total time {:?}", now.elapsed());
    }

    #[test]
    fn works_with_blobs() {
        let now = std::time::Instant::now();
        parse_git_filter_export_via_channel(None, true, Some(1), NO_LOCATION, |_| {
            if 1 == 1 {
                Ok(())
            } else {
                Err("a")
            }
        })
        .unwrap();
        eprintln!("total time {:?}", now.elapsed());
    }
}
