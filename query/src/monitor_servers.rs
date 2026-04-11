use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    time::Duration,
};

use xash3d_observer::{GetServerInfoResponse, Handler};

use crate::{
    cli::Cli,
    server_info::ServerInfo,
    server_result::{ServerResult, ServerResultKind},
    QueryError,
};

struct Monitor<'a> {
    cli: &'a Cli,
    custom_servers: Vec<SocketAddr>,
    servers: HashMap<SocketAddr, ServerInfo>,
}

impl<'a> Monitor<'a> {
    fn new(cli: &'a Cli, custom_servers: Vec<SocketAddr>) -> Self {
        Self {
            cli,
            custom_servers,
            servers: Default::default(),
        }
    }
}

impl Handler for Monitor<'_> {
    fn extra_servers(&mut self) -> &[SocketAddr] {
        &self.custom_servers
    }

    fn server_update(
        &mut self,
        addr: SocketAddr,
        info: &GetServerInfoResponse,
        _: bool,
        ping: Duration,
    ) {
        let info = ServerInfo::from(info);
        if self.cli.json {
            let result = ServerResult::ok(addr, ping, info);
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        } else {
            match self.servers.entry(addr) {
                Entry::Occupied(mut e) => {
                    let p = e.get().printer(self.cli);
                    println!("{:24?} --- {:>7.1} {}", addr, ' ', p,);
                    let p = info.printer(self.cli);
                    println!("{addr:24?} +++ {ping:>7.1?} {p}");
                    e.insert(info);
                }
                Entry::Vacant(e) => {
                    let p = info.printer(self.cli);
                    println!("{addr:24?} +++ {ping:>7.1?} {p}");
                    e.insert(info);
                }
            }
        }
    }

    fn server_update_ping(&mut self, addr: SocketAddr, ping: Duration) {
        if self.cli.json {
            let result = ServerResult::ping(addr, ping);
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
    }

    fn server_timeout(&mut self, addr: SocketAddr) {
        if self.cli.json {
            let result = ServerResult::timeout(addr);
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
    }

    fn server_remove(&mut self, addr: SocketAddr) {
        if self.cli.json {
            let result = ServerResult::new(addr, None, ServerResultKind::Remove);
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        } else {
            self.servers.remove(&addr);
        }
    }
}

pub(crate) fn run(cli: &Cli, servers: Vec<SocketAddr>) -> Result<(), QueryError> {
    let is_custom_servers = !servers.is_empty();
    let handler = Monitor::new(cli, servers);
    let mut observer = if is_custom_servers {
        crate::create_observer_no_masters(cli, handler)?
    } else {
        crate::create_observer(cli, handler)?
    };
    observer.run()?;
    Ok(())
}
