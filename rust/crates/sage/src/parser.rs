use crate::{ProcPidStat, ProcStat};

pub fn parse_proc_stat(input: &str) -> ProcStat {
    let mut parser = Parser::new(input);

    let mut out = ProcStat {
        user: 0,
        nice: 0,
        system: 0,
        idle: 0,
        iowait: 0,
        irq: 0,
        softirq: 0,
        btime: 0,
    };

    loop {
        let id = parser.read_word();
        match id {
            "cpu" => {
                parser.skip_spaces();

                out.user = parser.read_word().parse().unwrap();
                parser.skip_spaces();

                out.nice = parser.read_word().parse().unwrap();
                parser.skip_spaces();

                out.system = parser.read_word().parse().unwrap();
                parser.skip_spaces();

                out.idle = parser.read_word().parse().unwrap();
                parser.skip_spaces();

                out.iowait = parser.read_word().parse().unwrap();
                parser.skip_spaces();

                out.irq = parser.read_word().parse().unwrap();
                parser.skip_spaces();

                out.softirq = parser.read_word().parse().unwrap();
                parser.skip_spaces();
            }
            "btime" => {
                parser.skip_spaces();

                out.btime = parser.read_word().parse().unwrap();
            }
            _ => (),
        }
        if !parser.next_line() {
            break;
        }
    }

    out
}

pub fn parse_proc_pid_stat(input: &str) -> ProcPidStat {
    let mut parser = Parser::new(input);

    let mut out = ProcPidStat {
        pid: 0,
        comm: String::new(),
        state: '\0',
        utime: 0,
        stime: 0,
    };

    out.pid = parser.read_word().parse().unwrap();
    parser.skip_spaces();

    out.comm = parser.read_word().to_string();
    out.comm
        .remove(out.comm.char_indices().map(|(x, _)| x).last().unwrap());
    out.comm.remove(0);
    parser.skip_spaces();

    out.state = parser.read_word().chars().next().unwrap();
    parser.skip_spaces();

    // ppid
    parser.skip_field();

    // pgrp
    parser.skip_field();

    // session
    parser.skip_field();

    // tty_nr
    parser.skip_field();

    // tpgid
    parser.skip_field();

    // flags
    parser.skip_field();

    // minflt
    parser.skip_field();

    // cminflt
    parser.skip_field();

    // majflt
    parser.skip_field();

    // cmajflt
    parser.skip_field();

    out.utime = parser.read_word().parse().unwrap();
    parser.skip_spaces();

    out.stime = parser.read_word().parse().unwrap();
    parser.skip_spaces();

    out
}

#[derive(Debug)]
struct Parser<'a> {
    input: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, position: 0 }
    }

    fn read_word<'b>(&mut self) -> &'b str
    where
        'a: 'b,
    {
        let mut chars = self.input[self.position..].char_indices();
        let mut offset = 0;
        loop {
            match chars.next() {
                Some((_, ' ')) | Some((_, '\n')) => {
                    break;
                }
                Some((i, c)) => {
                    offset = i + c.len_utf8();
                }
                None => break,
            }
        }
        let start = self.position;
        self.position += offset;
        let word = &self.input[start..self.position];
        word
    }

    fn skip_spaces(&mut self) {
        let mut chars = self.input[self.position..].char_indices();
        loop {
            match chars.next() {
                Some((_, ' ')) => (),
                Some((i, _)) => {
                    self.position += i;
                    break;
                }
                None => break,
            }
        }
    }

    fn skip_field(&mut self) {
        self.read_word();
        self.skip_spaces();
    }

    fn next_line(&mut self) -> bool {
        let mut chars = self.input[self.position..].char_indices();
        loop {
            match chars.next() {
                Some((i, '\n')) => {
                    self.position += i + '\n'.len_utf8();
                    break true;
                }
                Some(_) => (),
                None => {
                    self.position = self.input.len();
                    break false;
                }
            }
        }
    }
}
