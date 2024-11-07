use std::{
    cell::RefCell,
    cmp::min,
    env::current_dir,
    fs::{read_dir, DirEntry},
    io::Stdout,
    os::linux::fs::MetadataExt,
    path::Path,
};

use anyhow::{ensure, Context, Result};
use histogram::Histogram;
use terminal::{stdout, Action, Clear, Retrieved, Terminal, Value};

fn visit_entries<F, D>(path: &Path, mut file_visitor: F, mut dir_visitor: D) -> Result<u64>
where
    F: FnMut(&DirEntry),
    D: FnMut(&DirEntry),
{
    return visit_entries_2(path, &mut file_visitor, &mut dir_visitor);
}

fn visit_entries_2<F, D>(path: &Path, file_visitor: &mut F, dir_visitor: &mut D) -> Result<u64>
where
    F: FnMut(&DirEntry),
    D: FnMut(&DirEntry),
{
    ensure!(path.is_dir());
    let mut err_count = 0;
    for entry in read_dir(path).context("Failed to read dir")? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                println!("ERROR: {:?}", err);
                err_count += 1;
                continue;
            }
        };

        if entry
            .file_type()
            .with_context(|| format!("Could not get filetype for {:?}", entry.path()))?
            .is_dir()
        {
            dir_visitor(&entry);
            match visit_entries_2(&entry.path(), file_visitor, dir_visitor)
                .with_context(|| format!("visit {}", entry.file_name().to_string_lossy()))
            {
                Ok(inner_err_count) => {
                    err_count *= inner_err_count;
                }
                Err(err) => {
                    println!("ERROR: {:?}", err);
                    err_count += 1;
                    continue;
                }
            }
        } else {
            file_visitor(&entry);
        }
    }

    Ok(err_count)
}

struct DictStats {
    /// The total number of dirs visited
    count: u64,
    dirs: Histogram,
    files: Histogram,
    entries: Histogram,
    access_errors: u64,
}

struct FileStats {
    count: u64,
    block_count: Histogram,
    block_size: Histogram,
    size: Histogram,
    access_errors: u64,
}

fn clear_info(terminal: &Terminal<Stdout>) {
    let Retrieved::CursorPosition(_, y) = terminal.get(Value::CursorPosition).unwrap() else {
        panic!();
    };
    terminal.batch(Action::MoveCursorTo(0, y - 3)).unwrap();
    terminal
        .batch(Action::ClearTerminal(Clear::FromCursorDown))
        .unwrap();
    terminal.flush_batch().unwrap();
}

fn print_info(terminal: &Terminal<Stdout>, path: &Path, files: &FileStats, dicts: &DictStats) {
    clear_info(terminal);
    let Retrieved::TerminalSize(width, _) = terminal.get(Value::TerminalSize).unwrap() else {
        panic!();
    };

    let path = path.to_string_lossy();
    let path = &path[0..min(path.len(), (width - 13) as usize)];
    println!("visit file: {}", path);
    println!("Total Files: {}", dicts.count + files.count);
    println!("Files: {}, Dicts: {}", files.count, dicts.count);
}

fn main() {
    let terminal = stdout();
    let dirs = RefCell::new(DictStats {
        count: 0,
        dirs: Histogram::new(4, 32).unwrap(),
        files: Histogram::new(4, 32).unwrap(),
        entries: Histogram::new(4, 32).unwrap(),
        access_errors: 0,
    });

    let files = RefCell::new(FileStats {
        count: 0,
        block_count: Histogram::new(4, 44).unwrap(),
        block_size: Histogram::new(4, 32).unwrap(),
        size: Histogram::new(4, 44).unwrap(),
        access_errors: 0,
    });

    let pwd = current_dir().unwrap();
    println!("");

    let err_count = visit_entries(
        &pwd,
        |entry: &DirEntry| {
            print_info(&terminal, &entry.path(), &files.borrow(), &dirs.borrow());

            let mut files = files.borrow_mut();
            files.count += 1;
            if let Ok(stats) = std::fs::metadata(&entry.path()) {
                files.size.add(stats.len(), 1).unwrap();
                files.block_size.add(stats.st_blksize(), 1).unwrap();
                files.block_count.add(stats.st_blocks(), 1).unwrap();
            } else {
                files.access_errors += 1;
            }
        },
        |entry: &DirEntry| {
            print_info(&terminal, &entry.path(), &files.borrow(), &dirs.borrow());

            let mut dicts = dirs.borrow_mut();
            dicts.count += 1;

            if let Ok(dir_iter) = read_dir(&entry.path()) {
                let mut dir_count = 0;
                let mut file_count = 0;
                for entry in dir_iter {
                    if let Ok(entry) = entry {
                        if entry.file_type().unwrap().is_dir() {
                            dir_count += 1;
                        }
                        if entry.file_type().unwrap().is_file() {
                            file_count += 1;
                        }
                    }
                }
                dicts.files.add(file_count, 1).unwrap();
                dicts.dirs.add(dir_count, 1).unwrap();
                dicts.entries.add(file_count + dir_count, 1).unwrap();
            } else {
                dicts.access_errors += 1;
            }
        },
    )
    .unwrap();

    clear_info(&terminal);

    // for bucket in &self.alloc_sizes {

    let files = files.borrow();
    let dirs = dirs.borrow();

    println!("Done");
    println!("Total Files: {}", dirs.count + files.count);
    println!("Files: {}, Dirs: {}", files.count, dirs.count);
    println!(
        "Errors: Files: {}, Dirs: {}",
        files.access_errors, dirs.access_errors
    );

    println!();

    println!("File sizes(min bytes - max bytes: count):");
    print_historgram(&files.size);
    println!("File block counts: (min - max: count):");
    print_historgram(&files.block_count);
    println!("File block sizes: (min - max: count):");

    println!();

    println!("Entries in dir(min - max: count):");
    print_historgram(&dirs.entries);
    println!("Dir count in dir(min - max: count):");
    print_historgram(&dirs.dirs);
    println!("Files in dir(min - max: count):");
    print_historgram(&dirs.files);

    if err_count > 0 {
        println!("{} errors encountered", err_count);
    }
}

fn print_historgram(histogram: &Histogram) {
    for bucket in histogram {
        if bucket.count() == 0 {
            continue;
        }
        print!(
            "\t{} - {}: {}",
            bucket.start(),
            bucket.end(),
            bucket.count()
        )
    }
    println!();
}

/*
fn main_test() {
    println!("bar");
    for _ in 0..100 {
        test();
    }
    println!("foo");
}
fn test() {
    let terminal = stdout();

    if let Ok(Retrieved::CursorPosition(_, y)) = terminal.get(Value::CursorPosition) {
        terminal.batch(Action::MoveCursorTo(0, y - 1)).unwrap();
        terminal
            .batch(Action::ClearTerminal(Clear::FromCursorDown))
            .unwrap();
        terminal.batch(Action::MoveCursorTo(0, y - 1)).unwrap();
        terminal.flush_batch().unwrap();
    }

    println!("test this should be delted");
}
*/
